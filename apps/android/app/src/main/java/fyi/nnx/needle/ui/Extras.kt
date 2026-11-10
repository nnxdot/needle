package fyi.nnx.needle.ui

import android.content.Intent
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.Button
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import kotlinx.coroutines.withContext
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.unit.dp
import androidx.core.content.FileProvider
import fyi.nnx.needle.NeedleApp
import fyi.nnx.needle.core.LyricLine
import fyi.nnx.needle.core.Update
import kotlinx.coroutines.launch
import java.io.File

// ---------- What's new

@Composable
fun WhatsNewScreen() = WhatsNewPage()

// ---------- The sync file

@Composable
fun SyncScreen() {
    val context = LocalContext.current
    var passphrase by remember { mutableStateOf("") }
    var busy by remember { mutableStateOf(false) }
    fun run(work: suspend () -> String) {
        busy = true
        NeedleApp.instance.scope.launch {
            val message = runCatching { work() }.fold({ it }, { it.message ?: "It did not work" })
            busy = false
            showMessage(message)
        }
    }
    val save = rememberLauncherForActivityResult(ActivityResultContracts.CreateDocument("application/octet-stream")) { uri ->
        uri ?: return@rememberLauncherForActivityResult
        run {
            val file = File(context.cacheDir, "needle-sync.bin")
            core.exportSync(file.absolutePath, passphrase)
            context.contentResolver.openOutputStream(uri)!!.use { out -> file.inputStream().use { it.copyTo(out) } }
            file.delete()
            "Saved your history, ratings, and playlists"
        }
    }
    val read = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument()) { uri ->
        uri ?: return@rememberLauncherForActivityResult
        run {
            val report = withPickedFile(context, uri) { core.importSync(it, passphrase) }
            NeedleApp.instance.libraryChanged()
            "Matched ${count(report.songsMatched.toInt(), "song")}, added ${count(report.listensAdded.toInt(), "listen")} and ${count(report.playlistsAdded.toInt(), "playlist")}" +
                if (report.songsNotFound > 0u) ". ${count(report.songsNotFound.toInt(), "song")} not on this phone." else "."
        }
    }
    LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(bottom = 32.dp + LocalBottomInset.current)) {
        item { LargeTitle("Sync") }
        item {
            Column(Modifier.padding(horizontal = Edge), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                Text("Move your play history, ratings, and playlists between Needle on this phone and on a computer, as one locked file. No music and no passwords are in it.", style = MaterialTheme.typography.bodyLarge)
                Text("On the computer: Settings › Library, under Move to another computer. Use the same passphrase on both.", style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                OutlinedTextField(
                    passphrase, { passphrase = it },
                    label = { Text("Passphrase") },
                    singleLine = true,
                    visualTransformation = PasswordVisualTransformation(),
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Password),
                    supportingText = { Text("At least 12 characters. Needle cannot open the file without it.") },
                    modifier = Modifier.remoteEscape().fillMaxWidth(),
                )
                Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                    Button(enabled = !busy && passphrase.length >= 12, onClick = { tryOpen("file picker") { save.launch("library.needle") } }) { Text("Save a sync file") }
                    FilledTonalButton(enabled = !busy && passphrase.length >= 12, onClick = { tryOpen("file picker") { read.launch(arrayOf("*/*")) } }) { Text("Read one") }
                }
            }
        }
    }
}

// ---------- Updates

@Composable
fun UpdatesRow() {
    val context = LocalContext.current
    var found by remember { mutableStateOf<Update?>(null) }
    var status by remember { mutableStateOf("You have Needle ${core.version()}.") }
    var busy by remember { mutableStateOf(false) }
    // While an update downloads: a bar and what it is doing, read from the core four times a
    // second, so a slow network shows that something is happening.
    var downloading by remember { mutableStateOf<String?>(null) }
    var progress by remember { mutableStateOf<fyi.nnx.needle.core.UpdateProgress?>(null) }
    LaunchedEffect(downloading) {
        while (downloading != null) {
            // Off the main thread: the core may be busy with the download's file meanwhile.
            progress = kotlinx.coroutines.withContext(kotlinx.coroutines.Dispatchers.IO) { runCatching { core.updateProgress() }.getOrNull() }
            kotlinx.coroutines.delay(250)
        }
        progress = null
    }
    // Whether the network is metered, asked off the main thread once an update is found.
    var metered by remember { mutableStateOf(false) }
    LaunchedEffect(found) {
        metered = found?.installable == true &&
            kotlinx.coroutines.withContext(kotlinx.coroutines.Dispatchers.IO) { runCatching { core.metered() }.getOrDefault(false) }
    }
    Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
        val version = downloading
        val p = progress
        Text(if (version != null && p != null) updateWords(version, p) else status, style = MaterialTheme.typography.bodyLarge)
        if (version != null) {
            if (p != null && p.stage == "download" && p.total > 0uL) {
                androidx.compose.material3.LinearProgressIndicator(progress = { (p.done.toDouble() / p.total.toDouble()).toFloat() }, modifier = Modifier.fillMaxWidth())
            } else {
                androidx.compose.material3.LinearProgressIndicator(Modifier.fillMaxWidth())
            }
        }
        // A hotspot or mobile data: the download is big, so say so first.
        if (found?.installable == true && version == null && metered) {
            Text("You are on mobile data or a hotspot: the update downloads tens of MB.", style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.error)
        }
        Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            val update = found
            if (update == null) {
                FilledTonalButton(enabled = !busy, onClick = {
                    busy = true
                    NeedleApp.instance.scope.launch {
                        val result = runCatching { core.checkUpdate() }
                        busy = false
                        result.onSuccess {
                            found = it
                            status = if (it == null) "Needle ${core.version()} is the latest." else "Needle ${it.version} is out."
                            showMessage(if (it == null) "You're up to date" else "Needle ${it.version} is out")
                        }.onFailure {
                            status = "Could not check: ${it.message}"
                            showMessage("Could not check for updates")
                        }
                    }
                }) {
                    if (busy) {
                        androidx.compose.material3.CircularProgressIndicator(Modifier.size(18.dp), strokeWidth = 2.dp)
                        Spacer(Modifier.width(10.dp))
                        Text("Checking…")
                    } else Text("Check for updates")
                }
            } else if (update.installable) {
                Button(enabled = !busy, onClick = {
                    busy = true
                    status = "Downloading Needle ${update.version} and checking it…"
                    downloading = update.version
                    NeedleApp.instance.scope.launch {
                        val result = runCatching {
                            val folder = File(context.cacheDir, "updates").apply { mkdirs() }
                            withContext(kotlinx.coroutines.Dispatchers.IO) { core.downloadUpdate(folder.absolutePath) }
                        }
                        busy = false
                        downloading = null
                        result.onSuccess { path ->
                            status = "Checked. Android asks you to install it."
                            // Android's installer takes it from here, and asks the person.
                            val opened = runCatching {
                                val uri = FileProvider.getUriForFile(context, "${context.packageName}.files", File(path))
                                tryOpen("app installer") {
                                    context.startActivity(
                                        Intent(Intent.ACTION_VIEW).setDataAndType(uri, "application/vnd.android.package-archive")
                                            .addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_ACTIVITY_NEW_TASK),
                                    )
                                }
                            }.getOrDefault(false)
                            if (!opened) status = "This device cannot install it here. Get Needle from needle.nnx.fyi instead."
                        }.onFailure { status = "The update did not work: ${it.message}" }
                    }
                }) { Text("Update to ${update.version}") }
            } else {
                Button(onClick = {
                    tryOpen("web browser") { context.startActivity(Intent(Intent.ACTION_VIEW, android.net.Uri.parse(update.page.ifBlank { "https://needle.nnx.fyi" })).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)) }
                }) { Text("Get Needle ${update.version}") }
            }
        }
    }
}

/** What an update is doing, in words: "Downloading Needle 2.0.4 · 12.3 of 45.6 MB". */
private fun updateWords(version: String, p: fyi.nnx.needle.core.UpdateProgress): String {
    fun mb(bytes: ULong) = "%.1f".format(bytes.toDouble() / 1_000_000)
    return when (p.stage) {
        "download" -> if (p.total > 0uL) "Downloading Needle $version · ${mb(p.done)} of ${mb(p.total)} MB" else "Downloading Needle $version · ${mb(p.done)} MB"
        "check" -> "Checking Needle $version…"
        "install" -> "Installing Needle $version…"
        else -> "Getting ready to download Needle $version…"
    }
}
