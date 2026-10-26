package fyi.nnx.needle.ui

import androidx.compose.animation.Crossfade
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.snap
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsFocusedAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.RectangleShape
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import fyi.nnx.needle.core.Album
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

/** One shelf of Home on a TV: its name ("Jump back in"…) and its albums. */
private class TvShelf(val title: String, val albums: List<Album>)

/**
 * Home on Android TV (NEEDLE-2.0.md, "TV Home"): the album in focus fills the screen behind,
 * darkened at the left and the foot, and changes with a short fade as the focus moves; its
 * shelf's name, its name, artist, year, and songs sit large above the rows, with Play and
 * Shuffle; the tile in focus grows, with a 3 dp ring in the cover's colour.
 */
@Composable
fun TvHome(open: (Route) -> Unit) {
    val homeLoad = rememberLoaded { homeShelves(20u) }
    val home by homeLoad
    val daysLoad = rememberLoaded { onThisDay(java.util.TimeZone.getDefault().getOffset(System.currentTimeMillis()) / 1000, 12u) }
    val days by daysLoad
    val h = home
    val shelves = remember(h, days) {
        if (h == null) emptyList() else buildList {
            add(TvShelf("Jump back in", h.recent))
            add(TvShelf("New in your library", h.added))
            add(TvShelf("On repeat", h.mostPlayed))
            days.orEmpty().forEach { add(TvShelf(if (it.yearsAgo == 1) "On this day, a year ago" else "On this day, ${it.yearsAgo} years ago", it.albums)) }
            add(TvShelf("Favorites", h.favorites))
        }.filter { it.albums.isNotEmpty() }
    }
    // The album in focus, and the shelf it is on; the first album until the focus moves.
    var chosen by remember { mutableStateOf<Pair<String, Album>?>(null) }
    val shown = chosen ?: shelves.firstOrNull()?.let { it.title to it.albums.first() }
    val reduce = reduceMotion()
    Box(Modifier.fillMaxSize().background(MaterialTheme.colorScheme.background)) {
        // The album, wide, behind everything.
        Crossfade(shown?.second?.artwork, animationSpec = if (reduce) snap() else tween(450), label = "tv home backdrop") { art ->
            if (art != null) Cover(art, Modifier.fillMaxSize(), RectangleShape)
        }
        val bg = MaterialTheme.colorScheme.background
        Box(Modifier.fillMaxSize().background(Brush.horizontalGradient(0f to bg, 0.45f to bg.copy(alpha = 0.75f), 1f to bg.copy(alpha = 0.15f))))
        Box(Modifier.fillMaxSize().background(Brush.verticalGradient(0f to Color.Transparent, 0.45f to bg.copy(alpha = 0.35f), 1f to bg)))
        // The shelf in focus stays at the top of the rows, the next one's name under it.
        val rows = rememberLazyListState()
        val scope = rememberCoroutineScope()
        Column(Modifier.fillMaxSize()) {
            shown?.let { (shelf, album) -> TvHero(shelf, album, Modifier) }
            LazyColumn(
                Modifier.fillMaxWidth().weight(1f),
                state = rows,
                contentPadding = PaddingValues(bottom = 24.dp + LocalBottomInset.current),
            ) {
                shelves.forEachIndexed { index, shelf ->
                    item(key = shelf.title) {
                        Text(
                            shelf.title,
                            style = MaterialTheme.typography.titleMedium,
                            fontWeight = FontWeight.SemiBold,
                            modifier = Modifier.padding(start = 48.dp, top = 12.dp, bottom = 10.dp),
                        )
                        LazyRow(contentPadding = PaddingValues(horizontal = 48.dp), horizontalArrangement = Arrangement.spacedBy(22.dp)) {
                            items(shelf.albums, key = { shelf.title + it.key }) { album ->
                                TvTile(album, onFocus = {
                                    chosen = shelf.title to album
                                    if (rows.firstVisibleItemIndex != index || rows.firstVisibleItemScrollOffset != 0) {
                                        scope.launch { if (reduce) rows.scrollToItem(index) else rows.animateScrollToItem(index) }
                                    }
                                }) { open(Route.AlbumPage(album)) }
                            }
                        }
                    }
                }
            }
        }
    }
}

/** The album in focus, large: its shelf, its name, its artist, year, and songs, then Play and Shuffle. */
@Composable
private fun TvHero(shelf: String, album: Album, modifier: Modifier) {
    val scope = rememberCoroutineScope()
    fun play(shuffled: Boolean) {
        scope.launch {
            val ids = withContext(Dispatchers.IO) { runCatching { core.albumSongs(album.key).map { it.id } }.getOrDefault(emptyList()) }
            if (ids.isNotEmpty()) playSongs(if (shuffled) ids.shuffled() else ids)
        }
    }
    Column(modifier.fillMaxWidth().padding(start = 48.dp, end = 48.dp, top = 28.dp, bottom = 4.dp)) {
        Text(shelf, style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.SemiBold, color = MaterialTheme.colorScheme.primary)
        Text(
            album.title.ifBlank { "Unknown album" },
            style = MaterialTheme.typography.displaySmall.copy(fontFamily = titleFamily(1f)),
            fontWeight = FontWeight.Bold,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.semantics { heading() },
        )
        Text(
            listOfNotNull(album.artist.ifBlank { null }, album.year.takeIf { it > 0 }?.toString(), count(album.songs.toInt(), "song")).joinToString(" · "),
            style = MaterialTheme.typography.titleLarge,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
        Row(Modifier.padding(top = 14.dp), horizontalArrangement = Arrangement.spacedBy(14.dp)) {
            PlayPill { play(false) }
            ShufflePill { play(true) }
        }
    }
}

/** A TV Home tile's cover: one row of them, the hero, and the player fit on a 540 dp screen. */
private val TileSide = 108.dp

/** An album on a TV's Home: it grows while it has the focus, with a 3 dp ring in its cover's colour. */
@Composable
private fun TvTile(album: Album, onFocus: () -> Unit, onClick: () -> Unit) {
    val interaction = remember { MutableInteractionSource() }
    val focused by interaction.collectIsFocusedAsState()
    val reduce = reduceMotion()
    val grow by animateFloatAsState(if (focused) 1.08f else 1f, if (reduce) snap() else tween(180), label = "tv tile")
    val ring = ringColor(album.artwork)
    Column(
        Modifier
            .width(TileSide)
            .onFocusChanged { if (it.isFocused) onFocus() }
            .clickable(interactionSource = interaction, indication = null, onClick = onClick),
    ) {
        Cover(
            album.artwork,
            Modifier
                .size(TileSide)
                .graphicsLayer { scaleX = grow; scaleY = grow }
                .then(if (focused) Modifier.border(3.dp, ring, CoverShape) else Modifier),
            CoverShape,
            seed = album.title + album.artist,
        )
        Text(
            album.title.ifBlank { "Unknown album" },
            style = MaterialTheme.typography.bodyMedium,
            fontWeight = FontWeight.Medium,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.padding(top = 10.dp),
        )
        Text(album.artist, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant, maxLines = 1, overflow = TextOverflow.Ellipsis)
    }
}

/** A cover's colour, bright enough to ring it on a dark page; amber when it has none. */
@Composable
private fun ringColor(path: String?): Color {
    val context = LocalContext.current
    val amber = MaterialTheme.colorScheme.primary
    var color by remember(path) { mutableStateOf(amber) }
    LaunchedEffect(path) {
        val seed = path?.let { coverSeed(context, it) } ?: return@LaunchedEffect
        color = Color(com.materialkolor.hct.Hct.fromInt(seed).let { if (it.tone < 70) it.withTone(70.0) else it }.toInt())
    }
    return color
}
