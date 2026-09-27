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
fun WhatsNewScreen() {
    val notes = remember { core.whatsNew() }
    LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(bottom = 32.dp)) {
        item { LargeTitle("What's new") }
        notes.take(6).forEach { note ->
            item { SectionHeader("Needle ${note.version}") }
            item {
                Column(Modifier.padding(horizontal = Edge), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    note.text.lines().filter { it.isNotBlank() }.forEach { line ->
                        // "- [icon] **Title.** words": the icon hint is for desktop.
                        val clean = line.removePrefix("! ").removePrefix("- ").replace(Regex("^\\[[^\\]]*\\]\\s*"), "").replace("**", "")
                        Text(if (line.startsWith("- ") || line.startsWith("! ")) "• $clean" else clean, style = MaterialTheme.typography.bodyLarge)
                    }
                }
            }
        }
    }
}

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
            val path = copyToCache(context, uri) ?: error("Could not read that file")
            val report = core.importSync(path, passphrase)
            File(path).delete()
            NeedleApp.instance.libraryVersion.value++
            "Matched ${count(report.songsMatched.toInt(), "song")}, added ${count(report.listensAdded.toInt(), "listen")} and ${count(report.playlistsAdded.toInt(), "playlist")}" +
                if (report.songsNotFound > 0u) ". ${count(report.songsNotFound.toInt(), "song")} not on this phone." else "."
        }
    }
    LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(bottom = 32.dp)) {
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
                    modifier = Modifier.fillMaxWidth(),
                )
                Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                    Button(enabled = !busy && passphrase.length >= 12, onClick = { save.launch("library.needle") }) { Text("Save a sync file") }
                    FilledTonalButton(enabled = !busy && passphrase.length >= 12, onClick = { read.launch(arrayOf("*/*")) }) { Text("Read one") }
                }
            }
        }
    }
}

// ---------- Timing lyrics

/**
 * Times a song's lyrics by tapping along: each tap stamps the next line at the moment it
 * is sung. Starts from the song's plain lyrics, or its timed ones to fix them.
 */
@Composable
fun TimingScreen(songId: String) {
    val playback by NeedleApp.instance.playback.collectAsState()
    val lyrics by rememberLoaded(songId) { lyrics(songId) }
    val lines = remember { mutableStateListOf<LyricLine>() }
    var next by remember { mutableStateOf(0) }
    var text by remember { mutableStateOf("") }
    LaunchedEffect(lyrics) {
        val l = lyrics ?: return@LaunchedEffect
        lines.clear()
        if (l.lines.isNotEmpty()) lines.addAll(l.lines)
        else lines.addAll(l.plain.lines().filter { it.isNotBlank() }.map { LyricLine(-1.0, it.trim(), emptyList()) })
        text = lines.joinToString("\n") { it.text }
    }
    val state = rememberLazyListState()
    LaunchedEffect(next) { if (next > 1) state.animateScrollToItem(next + 1) }
    val position = playback?.position ?: 0.0
    val thisSong = playback?.current?.id == songId
    Column(Modifier.fillMaxSize()) {
        LazyColumn(Modifier.weight(1f), state = state, contentPadding = PaddingValues(bottom = 16.dp)) {
            item { LargeTitle("Time the lyrics") }
            item {
                Text(
                    if (!thisSong) "Play this song first, then tap as each line starts."
                    else if (lines.isEmpty()) "This song has no lyrics to time. Paste them below, one line per line."
                    else "Tap the big button as each line starts. Tap a line to go back to it.",
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.padding(horizontal = Edge),
                )
            }
            if (lines.isEmpty()) {
                item {
                    OutlinedTextField(text, { text = it }, label = { Text("Lyrics") }, minLines = 6, modifier = Modifier.fillMaxWidth().padding(Edge))
                    TextButton(onClick = {
                        lines.addAll(text.lines().filter { it.isNotBlank() }.map { LyricLine(-1.0, it.trim(), emptyList()) })
                    }, modifier = Modifier.padding(horizontal = 12.dp)) { Text("Use these lines") }
                }
            }
            itemsIndexed(lines) { i, line ->
                Row(
                    Modifier
                        .fillMaxWidth()
                        .clickable { next = i }
                        .background(if (i == next) MaterialTheme.colorScheme.primaryContainer else androidx.compose.ui.graphics.Color.Transparent)
                        .padding(horizontal = Edge, vertical = 10.dp),
                ) {
                    Text(if (line.time >= 0) time(line.time) else "–", style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.primary, modifier = Modifier.width(56.dp))
                    Text(line.text, style = MaterialTheme.typography.bodyLarge, fontWeight = if (i == next) FontWeight.Bold else FontWeight.Normal)
                }
            }
        }
        Row(
            Modifier.fillMaxWidth().navigationBarsPadding().padding(Edge),
            horizontalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            Button(
                enabled = thisSong && next < lines.size,
                onClick = {
                    lines[next] = lines[next].copy(time = position)
                    next++
                },
                modifier = Modifier.weight(1f),
            ) { Text(if (next < lines.size) "Tap: line ${next + 1} starts" else "All timed") }
            FilledTonalButton(
                enabled = lines.isNotEmpty() && lines.all { it.time >= 0 },
                onClick = {
                    runCatching { core.saveTimedLyrics(songId, lines.toList()) }
                        .onSuccess { showMessage("Saved the timed lyrics") }
                        .onFailure { showMessage(it.message ?: "Could not save them") }
                },
            ) { Text("Save") }
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
    Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
        Text(status, style = MaterialTheme.typography.bodyLarge)
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
                        }.onFailure { status = "Could not check: ${it.message}" }
                    }
                }) { Text("Check for updates") }
            } else if (update.installable) {
                Button(enabled = !busy, onClick = {
                    busy = true
                    status = "Downloading Needle ${update.version} and checking it…"
                    NeedleApp.instance.scope.launch {
                        val result = runCatching {
                            val folder = File(context.cacheDir, "updates").apply { mkdirs() }
                            core.downloadUpdate(folder.absolutePath)
                        }
                        busy = false
                        result.onSuccess { path ->
                            status = "Checked. Android asks you to install it."
                            // Android's installer takes it from here, and asks the person.
                            val uri = FileProvider.getUriForFile(context, "${context.packageName}.files", File(path))
                            context.startActivity(
                                Intent(Intent.ACTION_VIEW).setDataAndType(uri, "application/vnd.android.package-archive")
                                    .addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_ACTIVITY_NEW_TASK),
                            )
                        }.onFailure { status = "The update did not work: ${it.message}" }
                    }
                }) { Text("Update to ${update.version}") }
            } else {
                Button(onClick = {
                    context.startActivity(Intent(Intent.ACTION_VIEW, android.net.Uri.parse(update.page.ifBlank { "https://needle.nnx.fyi" })).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
                }) { Text("Get Needle ${update.version}") }
            }
        }
    }
}
