package fyi.nnx.needle.ui

import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInHorizontally
import androidx.compose.animation.slideOutHorizontally
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Computer
import androidx.compose.material.icons.rounded.Dns
import androidx.compose.material.icons.rounded.Folder
import androidx.compose.material.icons.rounded.LibraryMusic
import androidx.compose.material3.Button
import androidx.compose.material3.FilterChip
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import fyi.nnx.needle.NeedleApp
import fyi.nnx.needle.R
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import java.io.File

/**
 * The welcome guide, on first start (and again from Settings › About): what Needle is, where
 * the music comes from, and a look to start with.
 */
@Composable
fun WelcomeGuide(open: (Route) -> Unit) {
    val app by NeedleApp.instance.app.collectAsState()
    var step by rememberSaveable { mutableIntStateOf(0) }
    fun finish() {
        Ui.welcome.value = false
        NeedleApp.instance.updateApp { it.copy(welcomed = true) }
    }
    val pick = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocumentTree()) { uri ->
        val path = uri?.let(::folderPath) ?: return@rememberLauncherForActivityResult
        NeedleApp.instance.scope.launch(Dispatchers.IO) { runCatching { core.addFolder(path) } }
        step = 2
    }
    androidx.activity.compose.BackHandler { if (step > 0) step-- else finish() }
    Box(
        Modifier.fillMaxSize().background(MaterialTheme.colorScheme.background).clickable(enabled = false) {}
            .statusBarsPadding().navigationBarsPadding().padding(24.dp),
    ) {
        AnimatedContent(step, transitionSpec = {
            (slideInHorizontally { it / 4 } + fadeIn()) togetherWith (slideOutHorizontally { -it / 4 } + fadeOut())
        }, label = "welcome") { page ->
            Column(Modifier.fillMaxSize(), verticalArrangement = Arrangement.Center) {
                when (page) {
                    0 -> {
                        Image(painterResource(R.drawable.needle_logo), contentDescription = null, modifier = Modifier.size(96.dp).clip(RoundedCornerShape(24.dp)))
                        Spacer(Modifier.height(28.dp))
                        Text("Welcome to Needle", style = MaterialTheme.typography.displaySmall)
                        Text(
                            "Your music, on your phone: lyrics that follow the song, radio from your own library, and your year told back. No account, no ads, no tracking.",
                            style = MaterialTheme.typography.bodyLarge,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                            modifier = Modifier.padding(top = 12.dp),
                        )
                        Spacer(Modifier.height(32.dp))
                        Button(onClick = { step = 1 }, modifier = Modifier.fillMaxWidth()) { Text("Start") }
                        TextButton(onClick = ::finish, modifier = Modifier.fillMaxWidth()) { Text("Skip the guide") }
                    }
                    1 -> {
                        Text("Where is your music?", style = MaterialTheme.typography.headlineMedium)
                        Text("You can add more places later, in Settings.", style = MaterialTheme.typography.bodyLarge, color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.padding(top = 8.dp, bottom = 20.dp))
                        val music = File(android.os.Environment.getExternalStorageDirectory(), android.os.Environment.DIRECTORY_MUSIC).path
                        Choice(Icons.Rounded.LibraryMusic, "In this phone's Music folder", "The usual place for music files") {
                            NeedleApp.instance.scope.launch(Dispatchers.IO) { runCatching { core.addFolder(music) } }
                            step = 2
                        }
                        Choice(Icons.Rounded.Folder, "In another folder", "On the phone or an SD card") { pick.launch(null) }
                        Choice(Icons.Rounded.Dns, "On my Navidrome or Subsonic server", "Streamed, with your plays and stars") { finish(); open(Route.Plugins) }
                        Choice(Icons.Rounded.Computer, "On my computer, in Needle", "Control it from here, or play its music here") { finish(); open(Route.Connect) }
                        TextButton(onClick = { step = 2 }, modifier = Modifier.fillMaxWidth().padding(top = 8.dp)) { Text("Skip for now") }
                    }
                    else -> {
                        Text("Make it yours", style = MaterialTheme.typography.headlineMedium)
                        Text("Everything here, and much more, is in Settings › Appearance.", style = MaterialTheme.typography.bodyLarge, color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.padding(top = 8.dp, bottom = 20.dp))
                        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                            listOf("night" to "Night", "midnight" to "Midnight", "day" to "Day").forEach { (id, label) ->
                                FilterChip(app.theme == id, { NeedleApp.instance.updateApp { it.copy(theme = id) } }, label = { Text(label) })
                            }
                        }
                        Row(Modifier.fillMaxWidth().padding(vertical = 16.dp), verticalAlignment = Alignment.CenterVertically) {
                            Column(Modifier.weight(1f)) {
                                Text("Colours from what's playing", style = MaterialTheme.typography.bodyLarge)
                                Text("The whole app takes the song's colours", style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                            }
                            Switch(app.ambientColors, { on -> NeedleApp.instance.updateApp { it.copy(ambientColors = on) } })
                        }
                        Spacer(Modifier.height(20.dp))
                        Button(onClick = ::finish, modifier = Modifier.fillMaxWidth()) { Text("Start listening") }
                    }
                }
            }
        }
        // Where you are in the guide.
        Row(Modifier.align(Alignment.BottomCenter), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            repeat(3) { i ->
                Box(Modifier.size(if (i == step) 20.dp else 8.dp, 8.dp).clip(CircleShape).background(if (i == step) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.outlineVariant))
            }
        }
    }
}

@Composable
private fun Choice(icon: ImageVector, title: String, detail: String, onClick: () -> Unit) {
    Row(
        Modifier.fillMaxWidth().padding(vertical = 6.dp).clip(RoundedCornerShape(24.dp)).background(MaterialTheme.colorScheme.surfaceContainer).clickable(onClick = onClick).padding(16.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        ShapedIcon(icon, 44)
        Column(Modifier.weight(1f)) {
            Text(title, style = MaterialTheme.typography.titleMedium)
            Text(detail, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant, textAlign = TextAlign.Start)
        }
    }
}

/** A folder chosen in Android's picker, as a path Needle can read. */
private fun folderPath(uri: android.net.Uri): String? {
    val id = runCatching { android.provider.DocumentsContract.getTreeDocumentId(uri) }.getOrNull() ?: return null
    val volume = id.substringBefore(':')
    val rest = id.substringAfter(':', "")
    val root = if (volume == "primary") android.os.Environment.getExternalStorageDirectory().path else "/storage/$volume"
    return if (rest.isEmpty()) root else "$root/$rest"
}
