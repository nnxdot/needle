package fyi.nnx.needle.ui

import android.net.Uri
import androidx.compose.foundation.background
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.KeyboardArrowRight
import androidx.compose.material.icons.automirrored.rounded.PlaylistAdd
import androidx.compose.material.icons.rounded.Album
import androidx.compose.material.icons.automirrored.rounded.QueueMusic
import androidx.compose.material.icons.rounded.Extension
import androidx.compose.material.icons.rounded.Lyrics
import androidx.compose.material.icons.rounded.Radio
import androidx.compose.material.icons.rounded.RemoveCircleOutline
import androidx.compose.material.icons.rounded.Star
import androidx.compose.material.icons.rounded.StarBorder
import kotlinx.coroutines.launch
import androidx.compose.material.icons.rounded.MoreVert
import androidx.compose.material.icons.rounded.MusicNote
import androidx.compose.material.icons.rounded.Person
import androidx.compose.material.icons.rounded.QueuePlayNext
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.State
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.foundation.layout.offset
import androidx.compose.ui.unit.sp
import androidx.compose.ui.graphics.luminance
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import coil3.compose.AsyncImage
import fyi.nnx.needle.NeedleApp
import fyi.nnx.needle.core.Album
import fyi.nnx.needle.core.Needle
import fyi.nnx.needle.core.Song
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.io.File

val core: Needle get() = NeedleApp.instance.core

/** Covers, as in Apple Music: small corners, so the picture stays the picture. */
val CoverShape = RoundedCornerShape(8.dp)
val SmallCoverShape = RoundedCornerShape(6.dp)

/** Space from the screen edge to content. */
val Edge = 20.dp

/** Loads `load` off the main thread, again whenever the library changes; `null` meanwhile. */
@Composable
fun <T> rememberLoaded(vararg keys: Any?, load: Needle.() -> T): State<T?> {
    val version by NeedleApp.instance.libraryVersion.collectAsState()
    return produceState<T?>(null, version, *keys) {
        value = withContext(Dispatchers.IO) { runCatching { core.load() }.getOrNull() }
    }
}

/** A cover's place: a file on the phone, or a link (covers from Needle on a computer). */
fun fileUri(path: String?): Uri? = path?.let {
    if (it.startsWith("http://") || it.startsWith("https://")) Uri.parse(it) else Uri.fromFile(File(it))
}

fun time(seconds: Double): String {
    val s = seconds.coerceAtLeast(0.0).toLong()
    return if (s >= 3600) "%d:%02d:%02d".format(s / 3600, s / 60 % 60, s % 60)
    else "%d:%02d".format(s / 60, s % 60)
}

fun count(n: Int, word: String) = if (n == 1) "1 $word" else "$n ${word}s"

/** "12 songs, 48 minutes", as at the foot of an Apple Music album. */
fun songsAndLength(songs: List<Song>): String {
    val minutes = Math.round(songs.sumOf { it.duration } / 60).toInt()
    val length = if (minutes >= 60) "${minutes / 60} hr ${minutes % 60} min" else count(minutes, "minute")
    return "${count(songs.size, "song")}, $length"
}

/**
 * A cover picture. Without one, a made-up cover as on desktop: a two-colour gradient and a
 * large, cropped first letter, different for every album (`seed`) and the same every time.
 */
@Composable
fun Cover(path: String?, modifier: Modifier = Modifier, shape: Shape = CoverShape, seed: String? = null) {
    Box(
        modifier
            .clip(shape)
            .background(MaterialTheme.colorScheme.surfaceContainerHigh),
        contentAlignment = Alignment.Center,
    ) {
        when {
            path != null -> AsyncImage(
                model = coverRequest(fileUri(path)),
                contentDescription = null,
                contentScale = ContentScale.Crop,
                modifier = Modifier.fillMaxSize(),
            )
            !seed.isNullOrBlank() -> MadeUpCover(seed)
            else -> Icon(
                Icons.Rounded.MusicNote,
                contentDescription = null,
                tint = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.6f),
                modifier = Modifier.fillMaxSize(0.36f),
            )
        }
    }
}

/** A playlist's picture: its own, a mosaic of four of its songs' covers, or one cover. */
@Composable
fun PlaylistCover(playlist: fyi.nnx.needle.core.Playlist, modifier: Modifier = Modifier, shape: Shape = SmallCoverShape) {
    Mosaic(playlist.mosaic, playlist.artwork, modifier, shape, seed = playlist.name)
}

/** Four covers in a square, two by two; fewer than four shows the one cover instead. */
@Composable
fun Mosaic(covers: List<String>, artwork: String?, modifier: Modifier = Modifier, shape: Shape = CoverShape, seed: String? = null) {
    if (covers.size < 4) {
        Cover(artwork ?: covers.firstOrNull(), modifier, shape, seed = seed)
        return
    }
    Column(modifier.clip(shape)) {
        covers.take(4).chunked(2).forEach { pair ->
            Row(Modifier.weight(1f)) {
                pair.forEach { c -> Cover(c, Modifier.weight(1f).fillMaxSize(), androidx.compose.ui.graphics.RectangleShape) }
            }
        }
    }
}

/** The same hash as desktop's, so an album's made-up cover matches on both. */
private fun seedHash(seed: String): Long =
    seed.lowercase().toByteArray().fold(2166136261L) { h, b -> ((h xor (b.toLong() and 0xff)) * 16777619L) and 0xffffffffL }

/** The main colour of an album's made-up cover, for tinting like a real one. */
fun seedColor(seed: String): Color = Color.hsl((seedHash(seed) % 360).toFloat(), 0.5f, 0.4f)

@Composable
private fun MadeUpCover(seed: String) {
    val hash = seedHash(seed)
    val h1 = (hash % 360).toFloat()
    val h2 = (h1 + 21.6f + ((hash shr 9) % 12) * 3.6f) % 360f
    val dark = MaterialTheme.colorScheme.background.luminance() < 0.5f
    val (a, b) = if (dark) Color.hsl(h1, 0.42f, 0.34f) to Color.hsl(h2, 0.5f, 0.17f) else Color.hsl(h1, 0.5f, 0.8f) to Color.hsl(h2, 0.42f, 0.62f)
    val letter = seed.firstOrNull { it.isLetterOrDigit() }?.uppercase() ?: "♪"
    androidx.compose.foundation.layout.BoxWithConstraints(
        Modifier.fillMaxSize().background(Brush.linearGradient(listOf(a, b))),
    ) {
        val size = maxWidth
        if (size >= 28.dp) {
            Text(
                letter,
                color = (if (dark) Color.White else Color.Black).copy(alpha = if (dark) 0.2f else 0.16f),
                fontSize = (size.value * 0.78f).sp,
                lineHeight = (size.value * 0.9f).sp,
                fontFamily = titleFamily(0.2f),
                fontWeight = FontWeight.SemiBold,
                maxLines = 1,
                softWrap = false,
                modifier = Modifier.align(Alignment.BottomStart).offset(x = size * 0.08f, y = size * 0.2f),
            )
        }
    }
}

/** A page's large title, left-aligned under the status bar. */
@Composable
fun LargeTitle(text: String, modifier: Modifier = Modifier, action: @Composable () -> Unit = {}) {
    val back = LocalCanGoBack.current
    Column(modifier.fillMaxWidth()) {
        // Pages opened from another get a way back at the top, as Android's own apps do.
        if (back) BackButton(Modifier.padding(start = 6.dp, top = 4.dp))
    Row(
        Modifier.fillMaxWidth().padding(start = Edge, end = 8.dp, top = if (back) 0.dp else 16.dp, bottom = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(
            text,
            style = MaterialTheme.typography.headlineLarge,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.weight(1f),
        )
        action()
    }
    }
}

/** A section's title; with `onMore`, a chevron leads to the whole list. */
@Composable
fun SectionHeader(text: String, modifier: Modifier = Modifier, onMore: (() -> Unit)? = null) {
    Row(
        modifier
            .fillMaxWidth()
            .then(if (onMore != null) Modifier.clickable(onClick = onMore) else Modifier)
            .padding(start = Edge, end = 12.dp, top = 28.dp, bottom = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(text, style = MaterialTheme.typography.titleLarge, fontWeight = FontWeight.Bold)
        if (onMore != null) {
            Icon(
                Icons.AutoMirrored.Rounded.KeyboardArrowRight,
                contentDescription = "See all",
                tint = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
    }
}

@Composable
@OptIn(ExperimentalFoundationApi::class)
fun AlbumTile(album: Album, modifier: Modifier = Modifier, open: ((Route) -> Unit)? = null, onClick: () -> Unit) {
    val interaction = remember { MutableInteractionSource() }
    var menu by remember { mutableStateOf(false) }
    val haptics = rememberHaptics()
    if (menu) AlbumSheet(album, open) { menu = false }
    Column(
        modifier
            .pressScale(interaction)
            .combinedClickable(interactionSource = interaction, indication = null, onClick = onClick, onLongClick = { haptics(true); menu = true }),
    ) {
        Cover(album.artwork, Modifier.fillMaxWidth().aspectRatio(1f).sharedCover("album-${album.key}"), seed = album.title + album.artist)
        Text(
            album.title.ifBlank { "Unknown album" },
            style = MaterialTheme.typography.bodyMedium,
            fontWeight = FontWeight.Medium,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.padding(top = 8.dp),
        )
        Text(
            album.artist,
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
    }
}

/** A song's menu: play it next, add it to the queue, or go to its album or artist. */
@Composable
fun SongMenu(song: Song, open: ((Route) -> Unit)?, remove: (() -> Unit)? = null) {
    var shown by remember { mutableStateOf(false) }
    SongMenu(song, open, shown, remove) { shown = it }
}

/** The menu's button; the menu itself is a sheet (see [SongSheet]). Opened from outside too. */
@Composable
fun SongMenu(
    song: Song,
    open: ((Route) -> Unit)?,
    shown: Boolean,
    remove: (() -> Unit)? = null,
    onShown: (Boolean) -> Unit,
) {
    IconButton(onClick = { onShown(true) }) {
        Icon(Icons.Rounded.MoreVert, contentDescription = "More", tint = MaterialTheme.colorScheme.onSurfaceVariant)
    }
    if (shown) SongSheet(song, open, remove) { onShown(false) }
}

@OptIn(ExperimentalFoundationApi::class)
/** How close list rows sit (Settings › Appearance › Density). */
data class Rows(
    /** A song row's height, its cover, and the gap between them. */
    val height: Dp,
    val cover: Dp,
    val gap: Dp,
    /** Space above and below the words of any other row (settings, menus, lists). */
    val pad: Dp,
    /** Space between covers in grids and shelves. */
    val grid: Dp,
) {
    companion object {
        fun of(density: String) = when (density) {
            "compact" -> Rows(48.dp, 38.dp, 12.dp, 6.dp, 10.dp)
            "spacious" -> Rows(76.dp, 60.dp, 18.dp, 20.dp, 30.dp)
            else -> Rows(60.dp, 48.dp, 14.dp, 12.dp, 20.dp)
        }
    }
}

val LocalRows = androidx.compose.runtime.compositionLocalOf { Rows.of("comfortable") }

/**
 * A song in a list: its cover (or its number on an album page), title and artist, and its menu,
 * over a hairline that starts where the text does.
 */
@Composable
fun SongRow(
    song: Song,
    playing: Boolean,
    open: ((Route) -> Unit)?,
    modifier: Modifier = Modifier,
    number: Boolean = false,
    divider: Boolean = true,
    remove: (() -> Unit)? = null,
    /** Shown at the end instead of the menu, such as a handle to drag the row by. */
    trailing: (@Composable () -> Unit)? = null,
    onClick: () -> Unit,
) {
    val rows = LocalRows.current
    // Choosing many songs: a tap adds the row or takes it away.
    val chosenSongs by Selection.songs.collectAsState()
    val choosing = chosenSongs.isNotEmpty()
    val chosen = chosenSongs.any { it.id == song.id }
    val lead: Dp = if (number) 36.dp else rows.cover + 4.dp
    var menu by remember { mutableStateOf(false) }
    val haptics = rememberHaptics()
    val isPlaying = playing && NeedleApp.instance.playback.collectAsState().value?.playing == true
    Column(
        modifier
            .fillMaxWidth()
            .background(if (chosen) MaterialTheme.colorScheme.primaryContainer.copy(alpha = 0.45f) else Color.Transparent)
            .combinedClickable(
                onClick = { if (choosing) Selection.toggle(song) else onClick() },
                onLongClick = { haptics(true); if (choosing) Selection.toggle(song) else menu = true },
            ),
    ) {
        Row(
            Modifier.fillMaxWidth().heightIn(min = rows.height).padding(start = Edge, end = 4.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(rows.gap),
        ) {
            if (number) {
                Box(Modifier.width(22.dp)) {
                    if (playing) {
                        PlayingBars(isPlaying)
                    } else {
                        Text(
                            if (song.trackNumber > 0) song.trackNumber.toString() else "–",
                            style = MaterialTheme.typography.bodyLarge,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                }
            } else {
                Box(Modifier.size(rows.cover), contentAlignment = Alignment.Center) {
                    Cover(song.artwork, Modifier.fillMaxSize(), SmallCoverShape, seed = song.album + song.artist)
                    if (playing) {
                        Box(Modifier.fillMaxSize().clip(SmallCoverShape).background(Color.Black.copy(alpha = 0.45f)))
                        PlayingBars(isPlaying, color = Color.White)
                    }
                }
            }
            Column(Modifier.weight(1f).padding(vertical = if (rows.height < 56.dp) 4.dp else 8.dp)) {
                Text(
                    song.title,
                    style = MaterialTheme.typography.bodyLarge,
                    color = if (playing) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurface,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
                if (!number) {
                    Text(
                        song.artist,
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
            }
            when {
                choosing -> androidx.compose.material3.Checkbox(chosen, { Selection.toggle(song) })
                trailing != null -> trailing()
                else -> SongMenu(song, open, menu, remove) { menu = it }
            }
        }
        if (divider) {
            HorizontalDivider(
                color = MaterialTheme.colorScheme.outlineVariant,
                modifier = Modifier.padding(start = Edge + lead + rows.gap),
            )
        }
    }
}

/** A row of the Library's list: an amber icon, a name, and a chevron. */
@Composable
fun NavRow(icon: androidx.compose.ui.graphics.vector.ImageVector, text: String, onClick: () -> Unit) {
    Column(Modifier.fillMaxWidth().clickable(onClick = onClick)) {
        Row(
            Modifier.fillMaxWidth().heightIn(min = LocalRows.current.height - 4.dp).padding(start = Edge, end = 12.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(18.dp),
        ) {
            Icon(icon, contentDescription = null, tint = MaterialTheme.colorScheme.primary)
            Text(text, style = MaterialTheme.typography.titleMedium, modifier = Modifier.weight(1f))
            Icon(
                Icons.AutoMirrored.Rounded.KeyboardArrowRight,
                contentDescription = null,
                tint = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        HorizontalDivider(
            color = MaterialTheme.colorScheme.outlineVariant,
            modifier = Modifier.padding(start = Edge + 24.dp + 18.dp),
        )
    }
}

@Composable
fun RoundCover(path: String?, size: Dp) {
    Cover(path, Modifier.size(size), CircleShape)
}
