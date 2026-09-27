package fyi.nnx.needle.ui

import android.net.Uri
import android.os.Environment
import android.provider.DocumentsContract
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Folder
import androidx.compose.material3.Button
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.Icon
import androidx.compose.material3.LinearWavyProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.ListItem
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import fyi.nnx.needle.NeedleApp
import java.io.File

/**
 * A folder chosen in Android's picker, as a path Needle can read ("primary:Music" is the
 * phone's own Music folder; other volumes, like an SD card, have their own id).
 */
private fun treePath(uri: Uri): String? {
    val id = runCatching { DocumentsContract.getTreeDocumentId(uri) }.getOrNull() ?: return null
    val volume = id.substringBefore(':')
    val rest = id.substringAfter(':', "")
    val root = if (volume == "primary") Environment.getExternalStorageDirectory().path else "/storage/$volume"
    return if (rest.isEmpty()) root else "$root/$rest"
}

@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun SettingsScreen() {
    val folders by rememberLoaded { folders() }
    val scan by NeedleApp.instance.scan.collectAsState()
    val count by rememberLoaded(scan.running) { songCount() }
    val pick = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocumentTree()) { uri ->
        val path = uri?.let(::treePath) ?: return@rememberLauncherForActivityResult
        runCatching { core.addFolder(path) }
        NeedleApp.instance.libraryVersion.value++
    }
    val music = File(Environment.getExternalStorageDirectory(), Environment.DIRECTORY_MUSIC).path

    Column(Modifier.fillMaxSize().verticalScroll(rememberScrollState())) {
        LargeTitle("Settings")
    Column(
        Modifier.padding(horizontal = Edge),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Text("Music folders", style = MaterialTheme.typography.titleLarge, fontWeight = FontWeight.Bold, modifier = Modifier.padding(top = 12.dp))
        folders.orEmpty().forEach { folder ->
            ListItem(
                headlineContent = { Text(folder.substringAfterLast('/').ifBlank { folder }) },
                supportingContent = { Text(folder) },
                leadingContent = { Icon(Icons.Rounded.Folder, contentDescription = null) },
            )
        }
        if (folders?.isEmpty() == true) {
            Text(
                "Needle reads music from folders on this phone. Choose where your music is.",
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            Button(onClick = { pick.launch(null) }) { Text("Add a folder") }
            if (folders?.contains(music) == false) {
                FilledTonalButton(onClick = {
                    runCatching { core.addFolder(music) }
                    NeedleApp.instance.libraryVersion.value++
                }) { Text("Use the Music folder") }
            }
        }
        if (scan.running) {
            LinearWavyProgressIndicator(modifier = Modifier.fillMaxWidth().padding(top = 8.dp))
            Text(
                "Reading ${scan.scanned} files, ${scan.imported} new",
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        } else {
            Text(
                "${count ?: 0u} songs in your library",
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            scan.error?.let { Text(it, color = MaterialTheme.colorScheme.error) }
            if (folders?.isNotEmpty() == true) {
                FilledTonalButton(onClick = { core.rescan() }) { Text("Read the folders again") }
            }
        }
        Text("About", style = MaterialTheme.typography.titleLarge, fontWeight = FontWeight.Bold, modifier = Modifier.padding(top = 20.dp))
        Text("Needle for Android · preview", color = MaterialTheme.colorScheme.onSurfaceVariant)
    }
    }
}
