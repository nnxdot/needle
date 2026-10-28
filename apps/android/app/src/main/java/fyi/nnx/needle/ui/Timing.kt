package fyi.nnx.needle.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.Undo
import androidx.compose.material.icons.rounded.Close
import androidx.compose.material.icons.rounded.Refresh
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.FilterChip
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import fyi.nnx.needle.NeedleApp
import fyi.nnx.needle.core.LyricTimer

/** A line's time as the editor shows it: 01:13.62. */
private fun stamp(seconds: Double): String {
    val cs = (seconds * 100).toLong().coerceAtLeast(0)
    return "%02d:%02d.%02d".format(cs / 6000, cs / 100 % 60, cs % 100)
}

/**
 * Time the lyrics (NEEDLE-2.0.md, "Time the lyrics"): tap as each line starts, or each word in
 * word mode; −0.2 s and +0.2 s move the line last tapped, Line taps it again, Undo takes the last
 * tap back. The steps are the core's (`LyricTimer`); saved beside Needle's own copy of the song's
 * lyrics. Starts from the song's lyrics (timed ones to fix them), or lines pasted in.
 */
@Composable
fun TimingScreen(songId: String) {
    val back = androidx.activity.compose.LocalOnBackPressedDispatcherOwner.current
    val playback by NeedleApp.instance.playback.collectAsState()
    // The song: the one playing, or else its tags (it has no cover then).
    val tagsLoad = rememberLoaded(songId) { songTags(songId) }
    val tags by tagsLoad
    val lyricsLoad = rememberLoaded(songId) { lyrics(songId) }
    val lyrics by lyricsLoad
    var pasted by remember { mutableStateOf("") }
    var timer by remember { mutableStateOf<LyricTimer?>(null) }
    // Every step changes the timer in place: this is read again after each.
    var version by remember { mutableIntStateOf(0) }
    LaunchedEffect(lyrics) {
        val l = lyrics ?: return@LaunchedEffect
        if (timer == null && (l.lines.isNotEmpty() || l.plain.isNotBlank())) timer = LyricTimer(l, "")
    }
    val t = timer
    val thisSong = playback?.current?.id == songId
    val position = playback?.position ?: 0.0
    fun step(work: LyricTimer.() -> Unit) {
        t?.work()
        version++
    }
    Column(Modifier.fillMaxSize().statusBarsPadding()) {
        // ✕, the title, and Save.
        Row(Modifier.fillMaxWidth().padding(horizontal = 4.dp), verticalAlignment = Alignment.CenterVertically) {
            IconButton(onClick = { back?.onBackPressedDispatcher?.onBackPressed() }) { Icon(Icons.Rounded.Close, contentDescription = "Close") }
            Text("Time the lyrics", style = MaterialTheme.typography.titleLarge, fontWeight = FontWeight.Bold, modifier = Modifier.weight(1f))
            val canSave = remember(version, t) { t?.changed() == true }
            TextButton(enabled = canSave, onClick = {
                val timed = t ?: return@TextButton
                libraryWrite({ core.saveLyricTimer(songId, timed) }, { showMessage(it.message ?: "Could not save them") }) { saved ->
                    showMessage(if (saved) "Saved the timed lyrics" else "Nothing was tapped yet")
                }
            }) { Text("Save") }
        }
        // The song, where it is, and how far.
        val s = playback?.current?.takeIf { it.id == songId }
        val title = s?.title ?: tags?.title
        if (title != null) {
            Row(Modifier.fillMaxWidth().padding(horizontal = Edge, vertical = 8.dp), verticalAlignment = Alignment.CenterVertically) {
                Cover(s?.artwork, Modifier.size(44.dp), SmallCoverShape, seed = (s?.album ?: tags?.album.orEmpty()) + (s?.artist ?: tags?.artist.orEmpty()))
                Column(Modifier.weight(1f).padding(horizontal = 12.dp)) {
                    Text(title, style = MaterialTheme.typography.titleSmall, fontWeight = FontWeight.SemiBold, maxLines = 1, overflow = TextOverflow.Ellipsis)
                    Text(s?.artist ?: tags?.artist.orEmpty(), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant, maxLines = 1)
                }
                if (s != null) Text("${time(position)} / ${time(s.duration)}", style = MaterialTheme.typography.labelMedium, fontFamily = FontFamily.Monospace, color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
            if (s != null && s.duration > 0) Bar((position / s.duration).toFloat(), 3.dp, MaterialTheme.colorScheme.primary, MaterialTheme.colorScheme.onSurface.copy(alpha = 0.15f), Modifier.padding(horizontal = Edge))
        }
        if (t == null) {
            // No lyrics of its own: paste them in.
            Column(Modifier.weight(1f).padding(Edge)) {
                Text(
                    if (lyricsLoad.loading) "Loading the lyrics…" else "This song has no lyrics to time. Paste them here, one line per line.",
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                if (!lyricsLoad.loading) {
                    OutlinedTextField(pasted, { pasted = it }, label = { Text("Lyrics") }, minLines = 6, modifier = Modifier.remoteEscape(lines = true).fillMaxWidth().padding(top = 12.dp))
                    TextButton(enabled = pasted.isNotBlank(), onClick = { timer = LyricTimer(null, pasted) }) { Text("Use these lines") }
                }
            }
            return@Column
        }
        val lines = remember(version, t) { t.lines() }
        val next = remember(version, t) { t.nextLine().toInt() }
        val wordMode = remember(version, t) { t.wordMode() }
        val done = remember(version, t) { t.done() }
        Row(Modifier.fillMaxWidth().padding(horizontal = Edge, vertical = 4.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            FilterChip(selected = !wordMode, onClick = { step { setWordMode(false) } }, label = { Text("Lines") }, shape = RoundedCornerShape(50))
            FilterChip(selected = wordMode, onClick = { step { setWordMode(true) } }, label = { Text("Words") }, shape = RoundedCornerShape(50))
        }
        if (!thisSong) {
            Text(
                "Play this song, then tap as each line starts.",
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(horizontal = Edge, vertical = 4.dp),
            )
        }
        val state = rememberLazyListState()
        LaunchedEffect(next) { if (next > 2) state.animateScrollToItem(next - 2) }
        LazyColumn(Modifier.weight(1f), state = state, contentPadding = PaddingValues(vertical = 8.dp)) {
            itemsIndexed(lines) { i, line ->
                val isNext = i == next && !done
                val timed = line.time != null
                Row(
                    Modifier
                        .fillMaxWidth()
                        .clickable(onClickLabel = "Time from this line") { step { select(i.toUInt()) } }
                        .padding(horizontal = Edge, vertical = 9.dp),
                    verticalAlignment = Alignment.Top,
                ) {
                    Text(
                        line.time?.let { stamp(it) } ?: "--:--.--",
                        style = MaterialTheme.typography.labelMedium,
                        fontFamily = FontFamily.Monospace,
                        color = if (isNext) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = if (timed) 0.9f else 0.5f),
                        modifier = Modifier.width(76.dp).padding(top = 4.dp),
                    )
                    Text(
                        line.text.ifBlank { "♪" },
                        style = MaterialTheme.typography.titleMedium,
                        fontWeight = if (isNext || timed) FontWeight.Bold else FontWeight.SemiBold,
                        color = MaterialTheme.colorScheme.onSurface.copy(alpha = if (isNext || timed) 1f else 0.4f),
                    )
                }
            }
        }
        // The fixes, then the one big tap.
        Column(Modifier.fillMaxWidth().padding(horizontal = Edge).padding(bottom = 16.dp + LocalBottomInset.current), verticalArrangement = Arrangement.spacedBy(12.dp)) {
            Row(horizontalArrangement = Arrangement.spacedBy(10.dp), modifier = Modifier.fillMaxWidth()) {
                FilledTonalButton(onClick = { step { earlier() } }, modifier = Modifier.weight(1f)) { Text("− 0.2 s") }
                FilledTonalButton(onClick = { step { redoLine() } }, modifier = Modifier.weight(1f)) {
                    Icon(Icons.Rounded.Refresh, contentDescription = null, modifier = Modifier.size(18.dp))
                    Spacer(Modifier.width(4.dp))
                    Text("Line")
                }
                FilledTonalButton(onClick = { step { later() } }, modifier = Modifier.weight(1f)) { Text("+ 0.2 s") }
                IconButton(onClick = { step { undo() } }) { Icon(Icons.AutoMirrored.Rounded.Undo, contentDescription = "Undo") }
            }
            Button(
                enabled = thisSong && !done,
                // The position asked now, not the last redraw's (a fifth of a second old).
                onClick = { step { tap(runCatching { core.playback().position }.getOrDefault(position)) } },
                shape = CircleShape,
                colors = ButtonDefaults.buttonColors(containerColor = MaterialTheme.colorScheme.primary, contentColor = MaterialTheme.colorScheme.onPrimary),
                modifier = Modifier.fillMaxWidth().height(64.dp),
            ) {
                Text(
                    when {
                        done -> "All timed"
                        wordMode -> "Tap as the next word starts"
                        else -> "Tap as the next line starts"
                    },
                    style = MaterialTheme.typography.titleMedium,
                    fontWeight = FontWeight.SemiBold,
                )
            }
        }
    }
}
