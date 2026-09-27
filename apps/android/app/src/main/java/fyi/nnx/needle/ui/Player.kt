package fyi.nnx.needle.ui

import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.animateDpAsState
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.QueueMusic
import androidx.compose.material.icons.rounded.KeyboardArrowDown
import androidx.compose.material.icons.rounded.Lyrics
import androidx.compose.material.icons.rounded.Pause
import androidx.compose.material.icons.rounded.PlayArrow
import androidx.compose.material.icons.rounded.Repeat
import androidx.compose.material.icons.rounded.RepeatOne
import androidx.compose.material.icons.rounded.Shuffle
import androidx.compose.material.icons.rounded.SkipNext
import androidx.compose.material.icons.rounded.SkipPrevious
import androidx.compose.material3.FilledIconButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.IconButtonDefaults
import androidx.compose.material3.IconToggleButton
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Slider
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.palette.graphics.Palette
import coil3.imageLoader
import coil3.request.ImageRequest
import coil3.request.SuccessResult
import coil3.request.allowHardware
import coil3.toBitmap
import fyi.nnx.needle.NeedleApp
import fyi.nnx.needle.core.RepeatMode
import fyi.nnx.needle.core.Song
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

/** The bar above the tabs: the song playing, play or pause, and next. Tap it for the player. */
@Composable
fun MiniPlayer(onOpen: () -> Unit) {
    val playback by NeedleApp.instance.playback.collectAsState()
    val p = playback ?: return
    val song = p.current ?: return
    Surface(
        color = MaterialTheme.colorScheme.surfaceContainerHigh,
        shape = RoundedCornerShape(24.dp),
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = 10.dp, vertical = 6.dp)
            .clickable(onClick = onOpen),
    ) {
        Column {
            Row(
                Modifier.padding(start = 8.dp, end = 4.dp, top = 8.dp, bottom = 6.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                Cover(song.artwork, Modifier.size(46.dp), RoundedCornerShape(14.dp))
                Column(Modifier.weight(1f)) {
                    Text(song.title, maxLines = 1, overflow = TextOverflow.Ellipsis, fontWeight = FontWeight.SemiBold)
                    Text(
                        song.artist,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
                IconButton(onClick = { core.toggle() }) {
                    Icon(if (p.playing) Icons.Rounded.Pause else Icons.Rounded.PlayArrow, contentDescription = if (p.playing) "Pause" else "Play")
                }
                IconButton(onClick = { core.next() }) {
                    Icon(Icons.Rounded.SkipNext, contentDescription = "Next")
                }
            }
            LinearProgressIndicator(
                progress = { if (song.duration > 0) (p.position / song.duration).toFloat().coerceIn(0f, 1f) else 0f },
                modifier = Modifier.fillMaxWidth().padding(horizontal = 18.dp).padding(bottom = 6.dp).height(3.dp),
                drawStopIndicator = {},
            )
        }
    }
}

/** The strongest colour of the cover, for the player's glow. */
@Composable
private fun coverColor(path: String?): Color {
    val context = LocalContext.current
    var color by remember { mutableStateOf(Color(0xFFE2B46C)) }
    LaunchedEffect(path) {
        if (path == null) return@LaunchedEffect
        val found = withContext(Dispatchers.IO) {
            val request = ImageRequest.Builder(context).data(fileUri(path)).size(96).allowHardware(false).build()
            val bitmap = (context.imageLoader.execute(request) as? SuccessResult)?.image?.toBitmap()
                ?: return@withContext null
            val palette = Palette.from(bitmap).generate()
            (palette.vibrantSwatch ?: palette.dominantSwatch)?.rgb
        }
        if (found != null) color = Color(found)
    }
    return animateColorAsState(color, label = "cover colour").value
}

private enum class Panel { Cover, Lyrics, UpNext }

@Composable
fun FullPlayer(onClose: () -> Unit) {
    val playback by NeedleApp.instance.playback.collectAsState()
    val p = playback ?: return
    val song = p.current ?: return
    val glow = coverColor(song.artwork)
    var panel by rememberSaveable { mutableStateOf(Panel.Cover) }
    // While the seek bar is held, it shows where the finger is, not the song.
    var dragging by remember { mutableStateOf<Float?>(null) }

    Surface(color = MaterialTheme.colorScheme.background, modifier = Modifier.fillMaxSize()) {
    Box(
        Modifier
            .fillMaxSize()
            .background(Brush.verticalGradient(listOf(glow.copy(alpha = 0.55f), Color.Transparent), endY = 1600f))
            .clickable(enabled = false) {},
    ) {
        Column(Modifier.fillMaxSize().statusBarsPadding().navigationBarsPadding().padding(horizontal = 24.dp)) {
            Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
                IconButton(onClick = onClose) {
                    Icon(Icons.Rounded.KeyboardArrowDown, contentDescription = "Close the player")
                }
                Text(
                    song.album,
                    style = MaterialTheme.typography.labelLarge,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                    modifier = Modifier.weight(1f).padding(horizontal = 8.dp),
                )
                Spacer(Modifier.size(48.dp))
            }
            Box(Modifier.weight(1f).fillMaxWidth(), contentAlignment = Alignment.Center) {
                when (panel) {
                    Panel.Cover -> {
                        val corner by animateDpAsState(if (p.playing) 28.dp else 44.dp, label = "cover corner")
                        val scale by androidx.compose.animation.core.animateFloatAsState(if (p.playing) 1f else 0.9f, label = "cover size")
                        Cover(
                            song.artwork,
                            Modifier
                                .fillMaxWidth(scale)
                                .aspectRatio(1f),
                            RoundedCornerShape(corner),
                        )
                    }
                    Panel.Lyrics -> LyricsPanel(song, p.position)
                    Panel.UpNext -> UpNextPanel(p.queueVersion)
                }
            }
            Text(song.title, style = MaterialTheme.typography.headlineSmall, fontWeight = FontWeight.Bold, maxLines = 1, overflow = TextOverflow.Ellipsis)
            Text(song.artist, style = MaterialTheme.typography.titleMedium, color = MaterialTheme.colorScheme.onSurfaceVariant, maxLines = 1, overflow = TextOverflow.Ellipsis)
            val duration = song.duration.toFloat().coerceAtLeast(1f)
            Slider(
                value = dragging ?: p.position.toFloat().coerceIn(0f, duration),
                onValueChange = { dragging = it },
                onValueChangeFinished = {
                    dragging?.let { core.seek(it.toDouble()) }
                    dragging = null
                },
                valueRange = 0f..duration,
                modifier = Modifier.padding(top = 12.dp),
            )
            Row(Modifier.fillMaxWidth()) {
                Text(time((dragging ?: p.position.toFloat()).toDouble()), style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                Spacer(Modifier.weight(1f))
                Text(time(song.duration), style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
            Row(
                Modifier.fillMaxWidth().padding(vertical = 12.dp),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically,
            ) {
                IconButton(onClick = { core.shuffle() }) { Icon(Icons.Rounded.Shuffle, contentDescription = "Shuffle what is up next") }
                IconButton(onClick = { core.previous() }, modifier = Modifier.size(56.dp)) {
                    Icon(Icons.Rounded.SkipPrevious, contentDescription = "Previous", modifier = Modifier.size(36.dp))
                }
                // Expressive: the button is rounder while paused, squarer while playing.
                val corner by animateDpAsState(if (p.playing) 28.dp else 44.dp, label = "play corner")
                FilledIconButton(
                    onClick = { core.toggle() },
                    shape = RoundedCornerShape(corner),
                    colors = IconButtonDefaults.filledIconButtonColors(containerColor = MaterialTheme.colorScheme.primary),
                    modifier = Modifier.size(88.dp),
                ) {
                    Icon(
                        if (p.playing) Icons.Rounded.Pause else Icons.Rounded.PlayArrow,
                        contentDescription = if (p.playing) "Pause" else "Play",
                        modifier = Modifier.size(44.dp),
                    )
                }
                IconButton(onClick = { core.next() }, modifier = Modifier.size(56.dp)) {
                    Icon(Icons.Rounded.SkipNext, contentDescription = "Next", modifier = Modifier.size(36.dp))
                }
                IconButton(onClick = {
                    core.setRepeat(
                        when (p.repeat) {
                            RepeatMode.OFF -> RepeatMode.ALL
                            RepeatMode.ALL -> RepeatMode.ONE
                            RepeatMode.ONE -> RepeatMode.OFF
                        },
                    )
                }) {
                    Icon(
                        if (p.repeat == RepeatMode.ONE) Icons.Rounded.RepeatOne else Icons.Rounded.Repeat,
                        contentDescription = "Repeat",
                        tint = if (p.repeat == RepeatMode.OFF) MaterialTheme.colorScheme.onSurfaceVariant else MaterialTheme.colorScheme.primary,
                    )
                }
            }
            Row(
                Modifier.fillMaxWidth().padding(bottom = 12.dp),
                horizontalArrangement = Arrangement.SpaceEvenly,
            ) {
                IconToggleButton(checked = panel == Panel.Lyrics, onCheckedChange = { panel = if (it) Panel.Lyrics else Panel.Cover }) {
                    Icon(Icons.Rounded.Lyrics, contentDescription = "Lyrics")
                }
                IconToggleButton(checked = panel == Panel.UpNext, onCheckedChange = { panel = if (it) Panel.UpNext else Panel.Cover }) {
                    Icon(Icons.AutoMirrored.Rounded.QueueMusic, contentDescription = "Up next")
                }
            }
        }
    }
    }
}

@Composable
private fun LyricsPanel(song: Song, position: Double) {
    val lyrics by rememberLoaded(song.id) { lyrics(song.id) }
    val lines = lyrics?.lines.orEmpty()
    val state = rememberLazyListState()
    val now = lines.indexOfLast { it.time <= position + 0.2 }
    LaunchedEffect(now) {
        if (now >= 0) state.animateScrollToItem((now - 2).coerceAtLeast(0))
    }
    when {
        lyrics == null || (lines.isEmpty() && lyrics?.plain.isNullOrBlank()) -> Text(
            if (lyrics?.instrumental == true) "Instrumental" else "No lyrics for this song",
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        lines.isEmpty() -> LazyColumn(Modifier.fillMaxSize()) {
            item { Text(lyrics!!.plain, style = MaterialTheme.typography.titleLarge) }
        }
        else -> LazyColumn(Modifier.fillMaxSize(), state = state) {
            itemsIndexed(lines) { i, line ->
                Text(
                    line.text.ifBlank { "♪" },
                    style = MaterialTheme.typography.headlineSmall,
                    fontWeight = FontWeight.Bold,
                    color = when {
                        i == now -> MaterialTheme.colorScheme.onSurface
                        i < now -> MaterialTheme.colorScheme.onSurface.copy(alpha = 0.3f)
                        else -> MaterialTheme.colorScheme.onSurface.copy(alpha = 0.45f)
                    },
                    modifier = Modifier
                        .fillMaxWidth()
                        .clickable { core.seek(line.time) }
                        .padding(vertical = 10.dp),
                )
            }
        }
    }
}

@Composable
private fun UpNextPanel(version: ULong) {
    val songs by rememberLoaded(version) { upNext() }
    val list = songs.orEmpty()
    if (songs != null && list.isEmpty()) {
        Text("Nothing up next", color = MaterialTheme.colorScheme.onSurfaceVariant)
        return
    }
    LazyColumn(Modifier.fillMaxSize()) {
        itemsIndexed(list) { i, song ->
            SongRow(song) { core.jump(i.toUInt()) }
        }
    }
}
