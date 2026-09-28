package fyi.nnx.needle.ui

import android.content.Context
import android.net.Uri
import android.net.wifi.WifiManager
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.PlaylistAdd
import androidx.compose.material.icons.rounded.Cast
import androidx.compose.material.icons.rounded.CheckCircle
import androidx.compose.material.icons.rounded.PhoneAndroid
import androidx.compose.material.icons.rounded.Speaker
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilterChip
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import fyi.nnx.needle.NeedleApp
import fyi.nnx.needle.core.Notice
import fyi.nnx.needle.core.Output
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.io.File

/** What floats over the screens: a sheet or a dialog at a time, and short messages. */
object Ui {
    val messages = MutableSharedFlow<String>(extraBufferCapacity = 8)
    val sheet = MutableStateFlow<Sheet?>(null)
    /** The welcome guide, asked for again from Settings › About. */
    val welcome = MutableStateFlow(false)
}

sealed interface Sheet {
    data class AddToPlaylist(val ids: List<String>) : Sheet
    data class NewPlaylist(val ids: List<String>, val smart: Boolean = false) : Sheet
    data class EditPlaylist(val id: String) : Sheet
    data object PlayOn : Sheet
    data class EditTags(val ids: List<String>) : Sheet
}

fun showMessage(text: String) {
    Ui.messages.tryEmit(text)
}

private fun refreshLibrary() {
    NeedleApp.instance.libraryVersion.value++
}

/** Messages, plugin questions, and the sheets, over whatever page shows. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun Overlays(snackbar: SnackbarHostState) {
    val context = LocalContext.current
    var question by remember { mutableStateOf<Notice.Question?>(null) }
    val sheet by Ui.sheet.collectAsState()
    LaunchedEffect(Unit) { Ui.messages.collect { snackbar.showSnackbar(it) } }
    // What plugins ask of the app, read a few times a second.
    LaunchedEffect(Unit) {
        while (true) {
            val notices = withContext(Dispatchers.IO) { runCatching { core.takeNotices() }.getOrDefault(emptyList()) }
            for (notice in notices) {
                when (notice) {
                    is Notice.Message -> showMessage(notice.text)
                    is Notice.LibraryChanged -> refreshLibrary()
                    is Notice.Question -> question = notice
                }
            }
            delay(700)
        }
    }
    question?.let { q -> PluginQuestion(q) { question = null } }
    when (val s = sheet) {
        is Sheet.AddToPlaylist -> AddToPlaylistSheet(s.ids)
        is Sheet.NewPlaylist -> PlaylistDialog(null, s.ids, s.smart)
        is Sheet.EditPlaylist -> PlaylistDialog(s.id, emptyList(), false)
        Sheet.PlayOn -> SpeakersSheet(context)
        is Sheet.EditTags -> TagEditorDialog(s.ids) { Ui.sheet.value = null }
        null -> {}
    }
}

@Composable
private fun PluginQuestion(q: Notice.Question, done: () -> Unit) {
    val context = LocalContext.current
    var text by remember(q.id) { mutableStateOf(q.default) }
    val pick = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument()) { uri ->
        // A plugin reads the file's text: a copy in the app's cache is handed over.
        val path = uri?.let { copyToCache(context, it) }
        core.answer(q.id, null, path)
        done()
    }
    if (q.file) {
        LaunchedEffect(q.id) { pick.launch(arrayOf("*/*")) }
        return
    }
    AlertDialog(
        onDismissRequest = { core.answer(q.id, null, null); done() },
        title = { Text(q.title.ifBlank { q.plugin }) },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Text("${q.plugin} asks", style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                if (q.prompt.isNotBlank()) Text(q.prompt)
                OutlinedTextField(text, { text = it }, singleLine = true, modifier = Modifier.fillMaxWidth())
            }
        },
        confirmButton = { TextButton(onClick = { core.answer(q.id, text, null); done() }) { Text("OK") } },
        dismissButton = { TextButton(onClick = { core.answer(q.id, null, null); done() }) { Text("Cancel") } },
    )
}

/** A file picked in Android's picker, copied where Needle can read it by path. */
fun copyToCache(context: Context, uri: Uri): String? = runCatching {
    val name = uri.lastPathSegment?.substringAfterLast('/')?.substringAfterLast(':')?.ifBlank { null } ?: "picked"
    val file = File(context.cacheDir, "picked-$name")
    context.contentResolver.openInputStream(uri)!!.use { input -> file.outputStream().use { input.copyTo(it) } }
    file.absolutePath
}.getOrNull()

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun AddToPlaylistSheet(ids: List<String>) {
    val playlists by rememberLoaded { playlists() }
    ModalBottomSheet(onDismissRequest = { Ui.sheet.value = null }) {
        Column(Modifier.navigationBarsPadding().padding(bottom = 16.dp)) {
            Text("Add to a playlist", style = MaterialTheme.typography.titleLarge, fontWeight = FontWeight.Bold, modifier = Modifier.padding(horizontal = Edge, vertical = 8.dp))
            Row(
                Modifier.fillMaxWidth().clickable { Ui.sheet.value = Sheet.NewPlaylist(ids) }.padding(horizontal = Edge, vertical = 14.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(16.dp),
            ) {
                Icon(Icons.AutoMirrored.Rounded.PlaylistAdd, contentDescription = null, tint = MaterialTheme.colorScheme.primary)
                Text("New playlist", style = MaterialTheme.typography.bodyLarge, color = MaterialTheme.colorScheme.primary)
            }
            LazyColumn(Modifier.heightIn(max = 420.dp)) {
                items(playlists.orEmpty().filter { !it.smart }, key = { it.id }) { playlist ->
                    Row(
                        Modifier.fillMaxWidth().clickable {
                            runCatching { core.addToPlaylist(playlist.id, ids) }
                                .onSuccess { showMessage("Added to ${playlist.name}"); refreshLibrary() }
                                .onFailure { showMessage(it.message ?: "Could not add it") }
                            Ui.sheet.value = null
                        }.padding(horizontal = Edge, vertical = 8.dp),
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(14.dp),
                    ) {
                        PlaylistCover(playlist, Modifier.size(48.dp), SmallCoverShape)
                        Column {
                            Text(playlist.name, style = MaterialTheme.typography.bodyLarge)
                            Text(count(playlist.songs.toInt(), "song"), style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                        }
                    }
                }
            }
        }
    }
}

/** Makes a playlist (of songs, or a smart one that follows a rule), or changes one. */
@Composable
private fun PlaylistDialog(editing: String?, ids: List<String>, smartAtFirst: Boolean) {
    val detail by rememberLoaded(editing) { editing?.let { playlistDetail(it) } }
    var name by remember(detail) { mutableStateOf(detail?.name ?: "") }
    var description by remember(detail) { mutableStateOf(detail?.description ?: "") }
    var smart by remember(detail) { mutableStateOf(detail?.rule != null || (editing == null && smartAtFirst)) }
    var rule by remember(detail) { mutableStateOf(detail?.rule ?: "") }
    val problem = if (smart && rule.isNotBlank()) core.checkRule(rule) else null
    val close = { Ui.sheet.value = null }
    AlertDialog(
        onDismissRequest = close,
        title = { Text(if (editing == null) "New playlist" else "Edit playlist") },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
                OutlinedTextField(name, { name = it }, label = { Text("Name") }, singleLine = true, modifier = Modifier.fillMaxWidth())
                OutlinedTextField(description, { description = it }, label = { Text("Description (if you like)") }, modifier = Modifier.fillMaxWidth())
                if (editing == null && ids.isEmpty()) {
                    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        FilterChip(selected = !smart, onClick = { smart = false }, label = { Text("Songs I pick") })
                        FilterChip(selected = smart, onClick = { smart = true }, label = { Text("Smart") })
                    }
                }
                if (smart) {
                    OutlinedTextField(
                        rule, { rule = it },
                        label = { Text("Rule") },
                        placeholder = { Text("rating >= 4 and genre = \"Pop\"") },
                        isError = problem != null,
                        supportingText = { Text(problem ?: "Songs that match this rule, as you would type it in search.") },
                        modifier = Modifier.fillMaxWidth(),
                    )
                }
            }
        },
        confirmButton = {
            TextButton(
                enabled = name.isNotBlank() && (!smart || (rule.isNotBlank() && problem == null)),
                onClick = {
                    runCatching {
                        if (editing == null) {
                            core.createPlaylist(name, description, ids, if (smart) rule else null)
                        } else {
                            core.editPlaylist(editing, name, description, if (smart) rule else null)
                        }
                    }.onSuccess {
                        showMessage(if (editing == null) "Made $name" else "Saved")
                        refreshLibrary()
                        close()
                    }.onFailure { showMessage(it.message ?: "Could not save it") }
                },
            ) { Text(if (editing == null) "Make it" else "Save") }
        },
        dismissButton = { TextButton(onClick = close) { Text("Cancel") } },
    )
}
