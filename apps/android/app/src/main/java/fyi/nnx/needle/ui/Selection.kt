package fyi.nnx.needle.ui

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.slideOutVertically
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.PlaylistAdd
import androidx.compose.material.icons.automirrored.rounded.QueueMusic
import androidx.compose.material.icons.rounded.Close
import androidx.compose.material.icons.rounded.DownloadForOffline
import androidx.compose.material.icons.rounded.Edit
import androidx.compose.material.icons.rounded.MoreVert
import androidx.compose.material.icons.rounded.PlayArrow
import androidx.compose.material.icons.rounded.QueuePlayNext
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import fyi.nnx.needle.NeedleApp
import fyi.nnx.needle.core.Song
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.launch

/**
 * Many songs chosen at once, as on desktop: "Select" in a song's menu starts it, then a tap
 * on a row adds or takes it away, and a bar offers what to do with them all.
 */
object Selection {
    val songs = MutableStateFlow<List<Song>>(emptyList())

    val active get() = songs.value.isNotEmpty()

    fun start(song: Song) {
        songs.value = listOf(song)
    }

    fun toggle(song: Song) {
        val now = songs.value
        songs.value = if (now.any { it.id == song.id }) now.filterNot { it.id == song.id } else now + song
    }

    fun clear() {
        songs.value = emptyList()
    }
}

/** Songs from a music server have ids that start with this. */
fun Song.fromServer() = id.startsWith("src-")

/** The bar over the tabs while songs are chosen. */
@Composable
fun SelectionBar() {
    val chosen by Selection.songs.collectAsState()
    AnimatedVisibility(
        chosen.isNotEmpty(),
        enter = fadeIn() + slideInVertically { it },
        exit = fadeOut() + slideOutVertically { it },
    ) {
        val ids = chosen.map { it.id }
        var more by remember { mutableStateOf(false) }
        Surface(
            color = MaterialTheme.colorScheme.primaryContainer,
            contentColor = MaterialTheme.colorScheme.onPrimaryContainer,
            shape = RoundedCornerShape(28.dp),
            shadowElevation = 8.dp,
            modifier = Modifier.fillMaxWidth().padding(horizontal = 10.dp, vertical = 8.dp),
        ) {
            Row(Modifier.padding(horizontal = 6.dp, vertical = 4.dp), verticalAlignment = Alignment.CenterVertically) {
                IconButton(onClick = Selection::clear) { Icon(Icons.Rounded.Close, contentDescription = "Stop choosing") }
                Text(count(chosen.size, "song"), style = MaterialTheme.typography.titleMedium, modifier = Modifier.weight(1f))
                IconButton(onClick = {
                    core.play(ids, 0u)
                    NeedleApp.instance.openPlayer.tryEmit(Unit)
                    Selection.clear()
                }) { Icon(Icons.Rounded.PlayArrow, contentDescription = "Play") }
                IconButton(onClick = { core.playNext(ids); showMessage("Play next"); Selection.clear() }) {
                    Icon(Icons.Rounded.QueuePlayNext, contentDescription = "Play next")
                }
                IconButton(onClick = { Ui.sheet.value = Sheet.AddToPlaylist(ids); Selection.clear() }) {
                    Icon(Icons.AutoMirrored.Rounded.PlaylistAdd, contentDescription = "Add to a playlist")
                }
                IconButton(onClick = { more = true }) { Icon(Icons.Rounded.MoreVert, contentDescription = "More") }
                DropdownMenu(more, { more = false }) {
                    DropdownMenuItem(
                        text = { Text("Add to queue") },
                        leadingIcon = { Icon(Icons.AutoMirrored.Rounded.QueueMusic, null) },
                        onClick = { more = false; core.enqueue(ids); showMessage("Added to the queue"); Selection.clear() },
                    )
                    DropdownMenuItem(
                        text = { Text("New playlist from these") },
                        leadingIcon = { Icon(Icons.AutoMirrored.Rounded.PlaylistAdd, null) },
                        onClick = { more = false; Ui.sheet.value = Sheet.NewPlaylist(ids); Selection.clear() },
                    )
                    val own = chosen.filterNot { it.fromServer() }
                    if (own.isNotEmpty()) DropdownMenuItem(
                        text = { Text("Edit tags") },
                        leadingIcon = { Icon(Icons.Rounded.Edit, null) },
                        onClick = { more = false; Ui.sheet.value = Sheet.EditTags(own.map { it.id }); Selection.clear() },
                    )
                    val server = chosen.filter { it.fromServer() }
                    if (server.isNotEmpty()) DropdownMenuItem(
                        text = { Text("Keep on this phone") },
                        leadingIcon = { Icon(Icons.Rounded.DownloadForOffline, null) },
                        onClick = {
                            more = false
                            keepOffline(server.map { it.id }, true)
                            Selection.clear()
                        },
                    )
                }
            }
        }
    }
}

/** Downloads server songs to play without a connection (or lets them go), saying how it went. */
fun keepOffline(ids: List<String>, keep: Boolean) {
    showMessage(if (keep) "Keeping ${count(ids.size, "song")} on this phone…" else "Letting them go…")
    NeedleApp.instance.scope.launch(Dispatchers.IO) {
        runCatching { core.keepOffline(ids, keep) }
            .onSuccess { showMessage(if (keep) "${count(it.toInt(), "song")} kept for listening offline" else "They play from the server again") }
            .onFailure { showMessage(it.message ?: "Could not keep them") }
    }
}

/** Saves server songs as files in the first music folder, where they join the library. */
fun saveToMusic(ids: List<String>) {
    NeedleApp.instance.scope.launch(Dispatchers.IO) {
        val folder = runCatching { core.folders().firstOrNull() }.getOrNull()
            ?: android.os.Environment.getExternalStoragePublicDirectory(android.os.Environment.DIRECTORY_MUSIC).path
        showMessage("Saving to your music…")
        runCatching { core.saveToMusic(ids, folder) }
            .onSuccess { showMessage("Saved ${count(it.toInt(), "song")} to ${folder.substringAfterLast('/')}") }
            .onFailure { showMessage(it.message ?: "Could not save them") }
    }
}
