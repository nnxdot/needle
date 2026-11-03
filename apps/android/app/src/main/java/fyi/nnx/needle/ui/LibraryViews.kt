package fyi.nnx.needle.ui

import android.content.Context
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.detectVerticalDragGestures
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Check
import androidx.compose.material.icons.rounded.MoreVert
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.dp
import fyi.nnx.needle.NeedleApp
import fyi.nnx.needle.core.Album
import fyi.nnx.needle.core.Artist
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.launch
import kotlin.math.roundToInt

/**
 * How the Library shows its albums and artists: covers in a grid, or rows in a list. Kept on the
 * phone, from the sort menu of each (DESIGN.md: small choices are dropdown menus).
 */
object LibraryView {
    private val prefs get() = NeedleApp.instance.getSharedPreferences("library-view", Context.MODE_PRIVATE)
    val albumsAsList = MutableStateFlow(prefs.getBoolean("albums", false))
    val artistsAsList = MutableStateFlow(prefs.getBoolean("artists", false))

    fun setAlbums(list: Boolean) {
        albumsAsList.value = list
        prefs.edit().putBoolean("albums", list).apply()
    }

    fun setArtists(list: Boolean) {
        artistsAsList.value = list
        prefs.edit().putBoolean("artists", list).apply()
    }
}

/** Grid and List in a sort menu, the one in use checked. */
@Composable
fun ViewChoices(asList: Boolean, close: () -> Unit, choose: (Boolean) -> Unit) {
    listOf(false to "Grid", true to "List").forEach { (list, label) ->
        DropdownMenuItem(
            text = { Text(label) },
            trailingIcon = { if (list == asList) Icon(Icons.Rounded.Check, null, tint = MaterialTheme.colorScheme.primary) },
            onClick = { close(); choose(list) },
        )
    }
}

/** The letter a name is filed under in a quick scroll's bubble: its first letter, or "#". */
fun indexLetter(text: String): String {
    val c = text.trim().firstOrNull { it.isLetterOrDigit() } ?: return "#"
    return if (c.isLetter()) c.uppercaseChar().toString() else "#"
}

/** An album as a row of a list: its cover, its name, and its artist and year; its menu at the right. */
@Composable
@OptIn(ExperimentalFoundationApi::class)
fun AlbumRow(album: Album, place: Place, open: (Route) -> Unit) {
    val rows = LocalRows.current
    var menu by remember { mutableStateOf(false) }
    val haptics = rememberHaptics()
    if (menu) AlbumSheet(album, open) { menu = false }
    Row(
        Modifier
            .fillMaxWidth()
            .segment(place, groupColor(), lineStart = 14.dp + rows.cover + rows.gap)
            .combinedClickable(onClick = { open(Route.AlbumPage(album)) }, onLongClick = { haptics(true); menu = true })
            .heightIn(min = rows.height)
            .padding(start = 14.dp, end = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(rows.gap),
    ) {
        Cover(album.artwork, Modifier.size(rows.cover), SmallCoverShape, seed = album.title + album.artist)
        Column(Modifier.weight(1f).padding(vertical = if (rows.height < 56.dp) 4.dp else 8.dp)) {
            Text(album.title.ifBlank { "Unknown album" }, style = MaterialTheme.typography.bodyLarge, maxLines = 1, overflow = TextOverflow.Ellipsis)
            Text(
                listOfNotNull(album.artist.ifBlank { null }, album.year.takeIf { it > 0 }?.toString()).joinToString(" · "),
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
        }
        IconButton(onClick = { menu = true }) {
            Icon(Icons.Rounded.MoreVert, contentDescription = "More", tint = MaterialTheme.colorScheme.onSurfaceVariant)
        }
    }
}

/** An artist as a row of a list: their picture in a circle, the name, and how much of theirs there is. */
@Composable
fun ArtistRow(artist: Artist, place: Place, open: (Route) -> Unit) {
    val rows = LocalRows.current
    val photoLoad = rememberLoaded(artist.name, cache = false) { artistPhoto(artist.name) }
    val photo by photoLoad
    Row(
        Modifier
            .fillMaxWidth()
            .segment(place, groupColor(), lineStart = 14.dp + rows.cover + rows.gap)
            .clickable { open(Route.Artist(artist.name)) }
            .heightIn(min = rows.height)
            .padding(start = 14.dp, end = 16.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(rows.gap),
    ) {
        Cover(photo ?: artist.artwork, Modifier.size(rows.cover), artistShape(), seed = artist.name)
        Column(Modifier.weight(1f).padding(vertical = if (rows.height < 56.dp) 4.dp else 8.dp)) {
            Text(artist.name.ifBlank { "Unknown artist" }, style = MaterialTheme.typography.bodyLarge, maxLines = 1, overflow = TextOverflow.Ellipsis)
            Text(
                listOfNotNull(
                    artist.albums.toInt().takeIf { it > 0 }?.let { count(it, "album") },
                    count(artist.songs.toInt(), "song"),
                ).joinToString(" · "),
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
        }
    }
}

/** The bubble beside a quick scroll's thumb: the letter it is at. */
@Composable
fun ScrollBubble(letter: String, modifier: Modifier = Modifier) {
    Box(
        modifier
            .size(56.dp)
            // Round but for the corner toward the thumb, as Android's own fast scroll.
            .clip(RoundedCornerShape(topStart = 28.dp, topEnd = 28.dp, bottomStart = 28.dp, bottomEnd = 4.dp))
            .background(MaterialTheme.colorScheme.primaryContainer),
        contentAlignment = Alignment.Center,
    ) {
        Text(letter, style = MaterialTheme.typography.headlineSmall, fontWeight = FontWeight.SemiBold, color = MaterialTheme.colorScheme.onPrimaryContainer)
    }
}

/** Fewer rows than this scroll through quickly enough by hand. */
private const val QuickFrom = 40

/**
 * A quick scroll at the right edge of a long list: a thin thumb that shows while the list
 * scrolls; dragged, it jumps through the list, with the letter it is at in a bubble beside it
 * when `letter` gives one (a list in A–Z order). Touch only: TalkBack and a remote scroll the
 * list itself. Place it over the list, along its right edge.
 */
@Composable
fun QuickScroll(
    count: Int,
    first: () -> Int,
    scrolling: Boolean,
    scrollTo: suspend (Int) -> Unit,
    modifier: Modifier = Modifier,
    letter: ((Int) -> String?)? = null,
) {
    if (count < QuickFrom) return
    val scope = rememberCoroutineScope()
    var dragged by remember { mutableStateOf<Float?>(null) }
    // One jump at a time, to a place read when the finger moved: a jump still waiting when the
    // finger lets go must not read the drag (by then none).
    var jump by remember { mutableStateOf<kotlinx.coroutines.Job?>(null) }
    fun jumpTo(at: Float) {
        val to = (at * (count - 1)).roundToInt().coerceIn(0, count - 1)
        jump?.cancel()
        jump = scope.launch { scrollTo(to) }
    }
    var height by remember { mutableIntStateOf(1) }
    val thumb = with(LocalDensity.current) { 44.dp.toPx() }
    val shown = scrolling || dragged != null
    // Out at once, and away a moment after the list stops.
    val alpha by animateFloatAsState(if (shown) 1f else 0f, tween(if (shown) 120 else 400, delayMillis = if (shown) 0 else 900), label = "quick scroll")
    val at = dragged ?: (first().toFloat() / (count - 1).coerceAtLeast(1)).coerceIn(0f, 1f)
    val top = { (at * (height - thumb).coerceAtLeast(0f)).roundToInt() }
    fun index(y: Float) = ((y - thumb / 2) / (height - thumb).coerceAtLeast(1f)).coerceIn(0f, 1f)
    Box(
        modifier
            .fillMaxHeight()
            .width(if (dragged != null) 96.dp else 28.dp)
            .onSizeChanged { height = it.height.coerceAtLeast(1) }
            .clearAndSetSemantics {}
            // Only while it shows: hidden, the edge is the list's to scroll.
            .then(
                if (alpha > 0f) {
                    Modifier.pointerInput(count) {
                        detectVerticalDragGestures(
                            onDragStart = { o ->
                                val at = index(o.y)
                                dragged = at
                                jumpTo(at)
                            },
                            onDragEnd = { dragged = null },
                            onDragCancel = { dragged = null },
                        ) { change, _ ->
                            change.consume()
                            val at = index(change.position.y)
                            dragged = at
                            jumpTo(at)
                        }
                    }
                } else {
                    Modifier
                },
            )
            .graphicsLayer { this.alpha = alpha },
    ) {
        val bubble = dragged?.let { d -> letter?.invoke((d * (count - 1)).roundToInt()) }
        if (bubble != null) {
            ScrollBubble(bubble, Modifier.align(Alignment.TopEnd).offset { IntOffset(-(20.dp.roundToPx()), top() - 6.dp.roundToPx()) })
        }
        Box(
            Modifier
                .align(Alignment.TopEnd)
                .offset { IntOffset(0, top()) }
                .padding(end = 4.dp)
                .size(width = if (dragged != null) 8.dp else 5.dp, height = 44.dp)
                .clip(CircleShape)
                .background(if (dragged != null) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.6f)),
        )
    }
}

/** [QuickScroll] for a list. */
@Composable
fun QuickScroll(state: androidx.compose.foundation.lazy.LazyListState, count: Int, modifier: Modifier = Modifier, letter: ((Int) -> String?)? = null) =
    QuickScroll(count, { state.firstVisibleItemIndex }, state.isScrollInProgress, { state.scrollToItem(it) }, modifier, letter)

/** [QuickScroll] for a grid. */
@Composable
fun QuickScroll(state: androidx.compose.foundation.lazy.grid.LazyGridState, count: Int, modifier: Modifier = Modifier, letter: ((Int) -> String?)? = null) =
    QuickScroll(count, { state.firstVisibleItemIndex }, state.isScrollInProgress, { state.scrollToItem(it) }, modifier, letter)
