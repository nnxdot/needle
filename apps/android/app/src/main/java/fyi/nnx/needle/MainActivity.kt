package fyi.nnx.needle

import android.Manifest
import android.content.ComponentName
import android.content.Intent
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.provider.MediaStore
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.core.splashscreen.SplashScreen.Companion.installSplashScreen
import androidx.activity.result.contract.ActivityResultContracts
import androidx.media3.session.MediaController
import androidx.media3.session.SessionToken
import com.google.common.util.concurrent.ListenableFuture
import fyi.nnx.needle.ui.NeedleRoot
import fyi.nnx.needle.ui.NeedleTheme
import fyi.nnx.needle.ui.showMessage
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import java.io.File

class MainActivity : ComponentActivity() {
    private var controller: ListenableFuture<MediaController>? = null

    override fun onCreate(savedInstanceState: Bundle?) {
        // Needle's record while it starts; then the record turns, grows, and fades into the app.
        installSplashScreen().setOnExitAnimationListener { splash ->
            val icon = splash.iconView
            val reduce = android.provider.Settings.Global.getFloat(contentResolver, android.provider.Settings.Global.ANIMATOR_DURATION_SCALE, 1f) == 0f
            if (reduce) {
                splash.remove()
                return@setOnExitAnimationListener
            }
            icon.animate()
                .rotationBy(120f)
                .scaleX(1.25f)
                .scaleY(1.25f)
                .setDuration(420)
                .setInterpolator(android.view.animation.PathInterpolator(0.3f, 0f, 0.1f, 1f))
                .start()
            splash.view.animate()
                .alpha(0f)
                .setStartDelay(120)
                .setDuration(300)
                .withEndAction { splash.remove() }
                .start()
        }
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        askPermissions()
        setContent {
            NeedleTheme {
                NeedleRoot()
            }
        }
        if (savedInstanceState == null) openFrom(intent)
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        openFrom(intent)
    }

    override fun onStart() {
        super.onStart()
        // Connecting starts the background player, so the notification shows what plays.
        val token = SessionToken(this, ComponentName(this, PlaybackService::class.java))
        controller = MediaController.Builder(this, token).buildAsync()
    }

    override fun onStop() {
        controller?.let { MediaController.releaseFuture(it) }
        controller = null
        super.onStop()
    }

    /** A music file opened with Needle from another app: it plays, and joins the library. */
    private fun openFrom(intent: Intent?) {
        if (intent?.action != Intent.ACTION_VIEW) return
        val uri = intent.data ?: return
        val app = NeedleApp.instance
        app.scope.launch(Dispatchers.IO) {
            val path = pathOf(uri) ?: copy(uri)
            val song = path?.let { runCatching { app.core.openFile(it) }.getOrNull() }
            if (song == null) {
                showMessage("Needle could not open that. It may not be a music file Needle can play.")
            } else {
                app.libraryVersion.value++
                app.openPlayer.tryEmit(Unit)
            }
        }
    }

    /** The file's own path, when Android's media list knows it (music on the phone). */
    private fun pathOf(uri: Uri): String? {
        if (uri.scheme == "file") return uri.path
        return runCatching {
            contentResolver.query(uri, arrayOf(MediaStore.MediaColumns.DATA), null, null, null)?.use { c ->
                if (c.moveToFirst()) c.getString(0) else null
            }
        }.getOrNull()?.takeIf { File(it).canRead() }
    }

    /** A copy in the app's own storage, for a file from elsewhere (a download, a message). */
    private fun copy(uri: Uri): String? = runCatching {
        val name = contentResolver.query(uri, arrayOf(MediaStore.MediaColumns.DISPLAY_NAME), null, null, null)?.use { c ->
            if (c.moveToFirst()) c.getString(0) else null
        } ?: "opened-${System.currentTimeMillis()}"
        val folder = File(filesDir, "opened").apply { mkdirs() }
        val file = File(folder, name.replace('/', '_'))
        contentResolver.openInputStream(uri)!!.use { input -> file.outputStream().use { input.copyTo(it) } }
        file.absolutePath
    }.getOrNull()

    private fun askPermissions() {
        val wanted = buildList {
            if (Build.VERSION.SDK_INT >= 33) {
                add(Manifest.permission.READ_MEDIA_AUDIO)
                add(Manifest.permission.POST_NOTIFICATIONS)
            } else {
                add(Manifest.permission.READ_EXTERNAL_STORAGE)
            }
        }
        registerForActivityResult(ActivityResultContracts.RequestMultiplePermissions()) {}
            .launch(wanted.toTypedArray())
    }
}
