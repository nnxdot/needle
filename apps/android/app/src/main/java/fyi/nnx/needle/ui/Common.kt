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

/** A cover picture, or a note on a quiet surface when there is none. */
@Composable
fun Cover(path: String?, modifier: Modifier = Modifier, shape: Shape = CoverShape) {
    Box(
        modifier
            .clip(shape)
            .background(MaterialTheme.colorScheme.surfaceContainerHigh),
        contentAlignment = Alignment.Center,
    ) {
        if (path == null) {
            Icon(
                Icons.Rounded.MusicNote,
                contentDescription = null,
                tint = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.6f),
                modifier = Modifier.fillMaxSize(0.36f),
            )
        } else {
            AsyncImage(
                model = coverRequest(fileUri(path)),
                contentDescription = null,
                contentScale = ContentScale.Crop,
                modifier = Modifier.fillMaxSize(),
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
        Cover(album.artwork, Modifier.fillMaxWidth().aspectRatio(1f).sharedCover("album-${album.key}"))
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
    onClick: () -> Unit,
) {
    val rows = LocalRows.current
    val lead: Dp = if (number) 36.dp else rows.cover + 4.dp
    var menu by remember { mutableStateOf(false) }
    val haptics = rememberHaptics()
    val isPlaying = playing && NeedleApp.instance.playback.collectAsState().value?.playing == true
    Column(
        modifier
            .fillMaxWidth()
            .combinedClickable(onClick = onClick, onLongClick = { haptics(true); menu = true }),
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
                    Cover(song.artwork, Modifier.fillMaxSize(), SmallCoverShape)
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
            SongMenu(song, open, menu, remove) { menu = it }
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
