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
import kotlinx.coroutines.flow.collectLatest
import androidx.lifecycle.repeatOnLifecycle
import kotlinx.coroutines.flow.receiveAsFlow
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
import kotlinx.coroutines.ensureActive

/** What floats over the screens: a sheet or a dialog at a time, and short messages. */
object Ui {
    /**
     * Short messages, with when each was said. Held while no screen shows them (Needle started
     * by a car, a widget, or a voice search), and shown when one opens, if still fresh.
     */
    val messages = kotlinx.coroutines.channels.Channel<Pair<Long, String>>(16, kotlinx.coroutines.channels.BufferOverflow.DROP_OLDEST)
    val sheet = MutableStateFlow<Sheet?>(null)
    /** The welcome guide, asked for again from Settings › About. */
    val welcome = MutableStateFlow(false)
    /** A tablet's side pane, open with the lyrics or what is up next; null while shut. */
    val pane = MutableStateFlow<SideKind?>(null)
    /** The visualizer full screen (from the player's visualizer). */
    val fullVisualizer = MutableStateFlow(false)
    /** Opens the player (Home's card of the song playing). */
    val showPlayer = kotlinx.coroutines.flow.MutableSharedFlow<Unit>(extraBufferCapacity = 1)
}

/** What a tablet's side pane shows. */
enum class SideKind { Lyrics, Queue }

sealed interface Sheet {
    data class AddToPlaylist(val ids: List<String>) : Sheet
    data class NewPlaylist(val ids: List<String>, val smart: Boolean = false) : Sheet
    data class EditPlaylist(val id: String) : Sheet
    data object PlayOn : Sheet
    data class EditTags(val ids: List<String>) : Sheet
    data object SleepTimer : Sheet
    data object SpeedPitch : Sheet
}

/**
 * Opens one of Android's own screens (a browser, the file picker, a settings page). A device
 * without it, as many TVs are, says so instead of closing Needle.
 */
fun tryOpen(what: String, open: () -> Unit): Boolean = try {
    open()
    true
} catch (_: android.content.ActivityNotFoundException) {
    showMessage("This device has no $what")
    false
}

fun showMessage(text: String) {
    Ui.messages.trySend(android.os.SystemClock.uptimeMillis() to text)
}

private fun refreshLibrary() {
    NeedleApp.instance.libraryChanged()
}

/** Messages, plugin questions, and the sheets, over whatever page shows. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun Overlays(snackbar: SnackbarHostState) {
    val context = LocalContext.current
    // Questions in a line: one at a time, each answered, none lost to the next.
    val questions = remember { androidx.compose.runtime.mutableStateListOf<Notice.Question>() }
    val sheet by Ui.sheet.collectAsState()
    // The newest message at once, in place of one still showing, rather than in a long line.
    // Taken only while Needle is on screen, so one is never held in a stopped app and shown
    // on its return, long after.
    val shownLifecycle = androidx.lifecycle.compose.LocalLifecycleOwner.current.lifecycle
    LaunchedEffect(shownLifecycle) {
        shownLifecycle.repeatOnLifecycle(androidx.lifecycle.Lifecycle.State.STARTED) {
            Ui.messages.receiveAsFlow().collectLatest { (at, text) ->
                // One held from long ago (a car's, an hour back) is no longer news.
                // Shown for a moment (about two and a half seconds), not Material's four.
                if (android.os.SystemClock.uptimeMillis() - at < 30_000) {
                    kotlinx.coroutines.withTimeoutOrNull(2500) {
                        snackbar.showSnackbar(text, duration = androidx.compose.material3.SnackbarDuration.Indefinite)
                    }
                }
            }
        }
    }
    // What plugins ask of the app, read a few times a second while Needle is on screen.
    val lifecycle = androidx.lifecycle.compose.LocalLifecycleOwner.current.lifecycle
    LaunchedEffect(lifecycle) {
        lifecycle.repeatOnLifecycle(androidx.lifecycle.Lifecycle.State.STARTED) {
        while (true) {
            val notices = withContext(Dispatchers.IO) { runCatching { core.takeNotices() }.getOrDefault(emptyList()) }
            for (notice in notices) {
                when (notice) {
                    is Notice.Message -> showMessage(notice.text)
                    is Notice.LibraryChanged -> refreshLibrary()
                    is Notice.Question -> questions += notice
                }
            }
            delay(700)
        }
        }
    }
    questions.firstOrNull()?.let { q -> androidx.compose.runtime.key(q.id) { PluginQuestion(q) { questions.remove(q) } } }
    when (val s = sheet) {
        is Sheet.AddToPlaylist -> AddToPlaylistSheet(s.ids)
        is Sheet.NewPlaylist -> PlaylistDialog(null, s.ids, s.smart)
        is Sheet.EditPlaylist -> PlaylistDialog(s.id, emptyList(), false)
        Sheet.PlayOn -> SpeakersSheet(context)
        is Sheet.EditTags -> TagEditorDialog(s.ids) { Ui.sheet.value = null }
        Sheet.SleepTimer -> SleepTimerSheet { Ui.sheet.value = null }
        Sheet.SpeedPitch -> SpeedPitchSheet { Ui.sheet.value = null }
        null -> {}
    }
}

@Composable
private fun PluginQuestion(q: Notice.Question, done: () -> Unit) {
    val context = LocalContext.current
    var text by remember(q.id) { mutableStateOf(q.default) }
    val pick = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument()) { uri ->
        // A plugin reads the file's text: a copy in the app's cache is handed over.
        if (uri == null) { core.answer(q.id, null, null); done() }
        else NeedleApp.instance.scope.launch(kotlinx.coroutines.Dispatchers.Main) {
            fyi.nnx.needle.performOperation({ copyToCache(context, uri) }, { path ->
                // The plugin resumes asynchronously and reads this path after answer returns.
                core.answer(q.id, null, path)
                done()
            }, { error ->
                showMessage(error.message ?: "Could not read that file")
                // The plugin hears "no file" rather than waiting.
                core.answer(q.id, null, null)
                done()
            }, {})
        }
    }
    if (q.file) {
        LaunchedEffect(q.id) {
            // Without a file picker, the plugin hears "no file" rather than waiting for ever.
            if (!tryOpen("file picker") { pick.launch(arrayOf("*/*")) }) {
                core.answer(q.id, null, null)
                done()
            }
        }
        return
    }
    AlertDialog(
        onDismissRequest = { core.answer(q.id, null, null); done() },
        title = { Text(q.title.ifBlank { q.plugin }) },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Text("${q.plugin} asks", style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                if (q.prompt.isNotBlank()) Text(q.prompt)
                OutlinedTextField(text, { text = it }, singleLine = true, modifier = Modifier.remoteEscape().fillMaxWidth())
            }
        },
        confirmButton = { TextButton(onClick = { core.answer(q.id, text, null); done() }) { Text("OK") } },
        dismissButton = { TextButton(onClick = { core.answer(q.id, null, null); done() }) { Text("Cancel") } },
    )
}

/** A file picked in Android's picker, copied where Needle can read it by path. */
suspend fun copyToCache(context: Context, uri: Uri): String {
    val name = kotlinx.coroutines.withContext(kotlinx.coroutines.Dispatchers.IO) {
        context.contentResolver.query(uri, arrayOf(android.provider.OpenableColumns.DISPLAY_NAME), null, null, null)?.use {
            if (it.moveToFirst()) it.getString(0) else null
        } ?: "picked"
    }
    return fyi.nnx.needle.copyFileOnIo(context.cacheDir, name) {
        context.contentResolver.openInputStream(uri) ?: error("Could not read that file")
    }.absolutePath
}

suspend fun <T> withPickedFile(context: Context, uri: Uri, action: (String) -> T): T {
    val path = copyToCache(context, uri)
    return try { action(path) } finally { File(path).delete() }
}

fun <T> pickedFile(context: Context, uri: Uri?, action: (String) -> T, finished: () -> Unit = {}, done: (T) -> Unit) {
    if (uri == null) { finished(); return }
    NeedleApp.instance.scope.launch(kotlinx.coroutines.Dispatchers.Main) {
        fyi.nnx.needle.performOperation(
            { kotlinx.coroutines.withContext(kotlinx.coroutines.Dispatchers.IO) { withPickedFile(context, uri, action) } },
            done, { showMessage(it.message ?: "Could not read that file") }, finished,
        )
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun AddToPlaylistSheet(ids: List<String>) {
    val playlistsLoad = rememberLoaded { playlists() }
    val playlists by playlistsLoad
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
                            libraryWrite({ core.addToPlaylist(playlist.id, ids) }, { showMessage(it.message ?: "Could not add it") }) {
                                showMessage("Added to ${playlist.name}")
                                refreshLibrary()
                            }
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
    val detailLoad = rememberLoaded(editing) { editing?.let { playlistDetail(it) } }
    val detail by detailLoad
    // Filled in once the playlist has loaded, then left to the person: a scan that reloads the
    // library while they type does not put the old words back.
    var name by remember(detail?.id) { mutableStateOf(detail?.name ?: "") }
    var description by remember(detail?.id) { mutableStateOf(detail?.description ?: "") }
    var smart by remember(detail?.id) { mutableStateOf(detail?.rule != null || (editing == null && smartAtFirst)) }
    var rule by remember(detail?.id) { mutableStateOf(detail?.rule ?: "") }
    var saving by remember { mutableStateOf(false) }
    // Checked off the main thread as the rule is typed.
    val problem by androidx.compose.runtime.produceState<String?>(null, smart, rule) {
        value = if (smart && rule.isNotBlank()) withContext(kotlinx.coroutines.Dispatchers.IO) { runCatching { core.checkRule(rule) }.getOrNull() } else null
    }
    val close = { Ui.sheet.value = null }
    AlertDialog(
        onDismissRequest = close,
        title = { Text(if (editing == null) "New playlist" else "Edit playlist") },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
                OutlinedTextField(name, { name = it }, label = { Text("Name") }, singleLine = true, modifier = Modifier.remoteEscape().fillMaxWidth())
                OutlinedTextField(description, { description = it }, label = { Text("Description (if you like)") }, modifier = Modifier.remoteEscape(lines = true).fillMaxWidth())
                if (editing == null && ids.isEmpty()) {
                    // Onto a second line with large text, rather than squeezed.
                    androidx.compose.foundation.layout.FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
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
                        modifier = Modifier.remoteEscape(lines = true).fillMaxWidth(),
                    )
                }
            }
        },
        confirmButton = {
            TextButton(
                // Once only: a second tap while it saves would make the playlist twice.
                enabled = !saving && name.isNotBlank() && (!smart || (rule.isNotBlank() && problem == null)),
                onClick = {
                    saving = true
                    libraryWrite({
                        if (editing == null) {
                            core.createPlaylist(name, description, ids, if (smart) rule else null)
                        } else {
                            core.editPlaylist(editing, name, description, if (smart) rule else null)
                        }
                    }, { saving = false; showMessage(it.message ?: "Could not save it") }) {
                        showMessage(if (editing == null) "Made $name" else "Saved")
                        refreshLibrary()
                        close()
                    }
                },
            ) { Text(if (editing == null) "Make it" else "Save") }
        },
        dismissButton = { TextButton(onClick = close) { Text("Cancel") } },
    )
}
