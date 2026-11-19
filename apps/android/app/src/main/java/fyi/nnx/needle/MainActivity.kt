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
import androidx.activity.SystemBarStyle
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
import kotlinx.coroutines.flow.first
import androidx.lifecycle.lifecycleScope
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
                barsAgain()
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
                .withEndAction { splash.remove(); barsAgain() }
                .start()
        }
        super.onCreate(savedInstanceState)
        systemBars()
        askPermissions()
        setContent {
            NeedleTheme {
                NeedleRoot()
            }
        }
        // Not again from Recents: a task reopened there brings back the file it was first
        // opened with, which would play it (and copy it) once more.
        if (savedInstanceState == null && intent.flags and Intent.FLAG_ACTIVITY_LAUNCHED_FROM_HISTORY == 0) openFrom(intent)
    }

    /**
     * Edge to edge, with the clock and the icons in the colour Needle's page needs. Android asks
     * for it again at each change of the screen (a turn, a fold, the text size, the phone's light
     * or dark mode): the answer is Needle's page each time, never the phone's own mode.
     */
    private fun systemBars() {
        val dark: (android.content.res.Resources) -> Boolean = { !fyi.nnx.needle.ui.lightBars.value }
        enableEdgeToEdge(
            statusBarStyle = SystemBarStyle.auto(android.graphics.Color.TRANSPARENT, android.graphics.Color.TRANSPARENT, dark),
            // The library's own scrims, for a phone with three buttons at its foot.
            navigationBarStyle = SystemBarStyle.auto(android.graphics.Color.argb(0xe6, 0xFF, 0xFF, 0xFF), android.graphics.Color.argb(0x80, 0x1b, 0x1b, 0x1b), dark),
        )
    }

    /**
     * After the start picture goes: on Android 12 and newer it sets the bars from the window's
     * own (dark) theme as it leaves, so the Day theme had white icons on a light page.
     */
    private fun barsAgain() {
        window.decorView.post {
            val light = fyi.nnx.needle.ui.lightBars.value
            androidx.core.view.WindowCompat.getInsetsController(window, window.decorView).apply {
                isAppearanceLightStatusBars = light
                isAppearanceLightNavigationBars = light
            }
        }
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
        // The Listening widget: the History page, once the pages are there to take it.
        if (intent?.getStringExtra(OpenExtra) == "history") {
            intent.removeExtra(OpenExtra)
            lifecycleScope.launch {
                fyi.nnx.needle.ui.Ui.openRoute.subscriptionCount.first { it > 0 }
                fyi.nnx.needle.ui.Ui.openRoute.emit(fyi.nnx.needle.ui.Route.History)
            }
            return
        }
        // A long press on the Quick Settings tile: its setting.
        if (intent?.action == "android.service.quicksettings.action.QS_TILE_PREFERENCES") {
            fyi.nnx.needle.ui.Ui.openRoute.tryEmit(fyi.nnx.needle.ui.Route.SettingsPage(fyi.nnx.needle.ui.SettingsSection.Playback))
            return
        }
        if (intent?.action == "android.media.action.MEDIA_PLAY_FROM_SEARCH") {
            val query = intent.getStringExtra(android.app.SearchManager.QUERY).orEmpty()
            val app = NeedleApp.instance
            app.scope.launch(Dispatchers.IO) {
                runCatching {
                    val songs = app.core.searchSongsPage(query, 0u, 300u)
                    check(songs.isNotEmpty()) { "No songs match that search" }
                    app.core.play(songs.map { it.id }, 0u)
                    fyi.nnx.needle.ui.startedFrom.value = null
                }.onFailure { showMessage(it.message ?: "Could not play that search") }
            }
            return
        }
        if (intent?.action != Intent.ACTION_VIEW) return
        val uri = intent.data ?: return
        val app = NeedleApp.instance
        app.scope.launch(Dispatchers.IO) {
            val path = pathOf(uri) ?: copy(uri)
            val song = path?.let { runCatching { app.core.openFile(it) }.getOrNull() }
            if (song == null) {
                showMessage("Needle could not open that. It may not be a music file Needle can play.")
            } else {
                app.libraryChanged()
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
        val file = copyFileAtomically(folder, name, { contentResolver.openInputStream(uri) ?: error("Could not read the opened file") })
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
