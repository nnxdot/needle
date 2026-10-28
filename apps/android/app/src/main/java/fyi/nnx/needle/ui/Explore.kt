package fyi.nnx.needle.ui

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.ui.draw.clipToBounds
import androidx.compose.ui.graphics.luminance
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.detectDragGestures
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.pager.VerticalPager
import androidx.compose.foundation.pager.rememberPagerState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.graphics.lerp
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.zIndex
import fyi.nnx.needle.NeedleApp
import kotlinx.coroutines.launch
import kotlin.math.hypot

// ---------- Your library in numbers

/**
 * What the library is made of, from the core's count: how much there is, its formats and their
 * quality, its decades, genres, and artists, and the albums to fix (no cover, low quality).
 */
@Composable
fun NumbersScreen(open: (Route) -> Unit) {
    val numbersLoad = rememberLoaded { libraryNumbers() }
    val n by numbersLoad
    LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(bottom = 32.dp + LocalBottomInset.current)) {
        item { LargeTitle("Your library in numbers") }
        val numbers = n ?: return@LazyColumn
        item {
            Row(Modifier.fillMaxWidth().padding(horizontal = Edge, vertical = 8.dp), horizontalArrangement = Arrangement.spacedBy(20.dp)) {
                BigNumber(numbers.songs.toInt(), "songs")
                BigNumber(numbers.albums.toInt(), "albums")
                BigNumber(numbers.artists.toInt(), "artists")
                BigNumber((numbers.duration / 3600).toInt(), "hours")
            }
        }
        item {
            val parts = listOfNotNull(
                numbers.size.takeIf { it > 0 }?.let { android.text.format.Formatter.formatShortFileSize(NeedleApp.instance, it) + " on this phone" },
                numbers.serverSongs.takeIf { it > 0u }?.let { count(it.toInt(), "song") + " from your music server" },
            )
            if (parts.isNotEmpty()) Text(parts.joinToString(" · "), style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.padding(horizontal = Edge))
        }
        val songs = numbers.songs.toInt().coerceAtLeast(1)
        item { ShelfTitle("Quality", "How the songs are kept") }
        item {
            Column(Modifier.padding(horizontal = Edge), verticalArrangement = Arrangement.spacedBy(10.dp)) {
                CountBar("Lossless", numbers.lossless.toInt(), songs, percent = true)
                CountBar("Hi-res", numbers.hires.toInt(), songs, percent = true)
            }
        }
        countSection("Formats", "Songs in each", numbers.formats, songs)
        countSection("Decades", "When the songs came out", numbers.decades, songs)
        countSection("Genres", "The ten with the most songs", numbers.genres, songs)
        countSection("Artists", "The ten with the most songs", numbers.artistsBySongs, songs) { open(Route.Artist(it)) }
        if (numbers.albumsWithoutCover > 0u) {
            item { ShelfTitle("Without a cover", count(numbers.albumsWithoutCover.toInt(), "album")) }
            item { AlbumRow(numbers.withoutCover, open) }
        }
        if (numbers.lowQualityAlbums > 0u) {
            item { ShelfTitle("Low quality", "${count(numbers.lowQualityAlbums.toInt(), "album")} under 192 kbit/s") }
            item { AlbumRow(numbers.lowQuality, open) }
        }
    }
}

/** A large number and what it counts. */
@Composable
private fun BigNumber(value: Int, label: String) {
    Column {
        Text("%,d".format(value), style = MaterialTheme.typography.headlineMedium, fontWeight = FontWeight.Bold)
        Text(label, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
    }
}

private fun androidx.compose.foundation.lazy.LazyListScope.countSection(
    title: String,
    detail: String,
    counts: List<fyi.nnx.needle.core.Count>,
    total: Int,
    onClick: ((String) -> Unit)? = null,
) {
    if (counts.isEmpty()) return
    item { ShelfTitle(title, detail) }
    item {
        val most = counts.maxOf { it.count.toInt() }.coerceAtLeast(1)
        Column(Modifier.padding(horizontal = Edge), verticalArrangement = Arrangement.spacedBy(10.dp)) {
            counts.forEach { c -> CountBar(c.name.ifBlank { "Unknown" }, c.count.toInt(), most, onClick = onClick?.let { { it(c.name) } }) }
        }
    }
}

/** A name, a thin bar as long as its share of `of`, and the count (or its percent). */
@Composable
private fun CountBar(name: String, value: Int, of: Int, percent: Boolean = false, onClick: (() -> Unit)? = null) {
    val share = (value.toFloat() / of).coerceIn(0f, 1f)
    Column(Modifier.fillMaxWidth().then(if (onClick != null) Modifier.clip(RowShape).clickable(onClick = onClick) else Modifier)) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Text(name, style = MaterialTheme.typography.bodyLarge, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.weight(1f))
            Text(
                if (percent) "${(share * 100).toInt()} %" else "%,d".format(value),
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        Box(Modifier.padding(top = 4.dp).fillMaxWidth().height(6.dp).clip(CircleShape).background(MaterialTheme.colorScheme.onSurface.copy(alpha = 0.08f))) {
            Box(Modifier.fillMaxWidth(share).height(6.dp).clip(CircleShape).background(MaterialTheme.colorScheme.primary))
        }
    }
}

@Composable
private fun AlbumRow(albums: List<fyi.nnx.needle.core.Album>, open: (Route) -> Unit) {
    LazyRow(contentPadding = PaddingValues(horizontal = Edge), horizontalArrangement = Arrangement.spacedBy(16.dp)) {
        items(albums, key = { it.key }) { album -> AlbumTile(album, Modifier.width(132.dp), open) { open(Route.AlbumPage(album)) } }
    }
}

// ---------- The genre map

/** How many genres the map shows as bubbles; the rest are chips under it. */
private const val MapBubbles = 40

/**
 * The genre map (NEEDLE-2.0.md, "Genre map"): one bubble per genre, as big as its number of
 * songs, at the middle of its songs' sound (dark to bright across, calm to lively up), its name
 * on it; a tap plays the genre shuffled. The 40 largest are on the map; the rest, and the
 * genres with no measured song yet, are chips under it.
 */
@OptIn(androidx.compose.foundation.layout.ExperimentalLayoutApi::class)
@Composable
fun MoodMapScreen() {
    val spotsLoad = rememberLoaded { genreMap() }
    val spots by spotsLoad
    val all = spots.orEmpty()
    val onMap = all.filter { it.placed }.take(MapBubbles)
    val chips = all.filter { it !in onMap }
    fun playGenre(name: String) {
        NeedleApp.instance.scope.launch(kotlinx.coroutines.Dispatchers.IO) {
            val songs = runCatching { NeedleApp.instance.core.genreSongs(name) }.getOrDefault(emptyList())
            if (songs.isEmpty()) showMessage("No songs in $name") else {
                playSongs(songs.shuffled().map { it.id })
                showMessage("Playing $name")
            }
        }
    }
    LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(bottom = 32.dp + LocalBottomInset.current)) {
        item { LargeTitle("Genre map") }
        item {
            Text(
                "Each genre by how its songs sound: calm or lively, dark or bright. The bigger the bubble, the more songs. Tap one to play it.",
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(horizontal = Edge),
            )
        }
        if (spots != null && all.isEmpty()) {
            item { Text("Your songs have no genres yet.", style = MaterialTheme.typography.bodyLarge, modifier = Modifier.padding(Edge)) }
            return@LazyColumn
        }
        if (spots != null && onMap.isEmpty()) {
            item {
                Text(
                    "No songs are measured yet. Turn on Measure songs for radio in Settings › Playback; Needle then measures them in the background.",
                    style = MaterialTheme.typography.bodyLarge,
                    modifier = Modifier.padding(Edge),
                )
            }
        }
        if (onMap.isNotEmpty()) item { GenreBubbles(onMap, ::playGenre) }
        if (chips.isNotEmpty()) {
            item { ShelfTitle(if (onMap.isEmpty()) "Genres" else "More genres", null) }
            item {
                FlowRow(
                    Modifier.fillMaxWidth().padding(horizontal = Edge),
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                    verticalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    chips.forEach { g ->
                        androidx.compose.material3.SuggestionChip(
                            onClick = { playGenre(g.genre) },
                            label = { Text("${g.genre} · ${g.songs}") },
                            shape = RoundedCornerShape(50),
                        )
                    }
                }
            }
        }
    }
}

/** The map: the bubbles, the largest drawn first so the small ones stay on top and in reach. */
@Composable
private fun GenreBubbles(spots: List<fyi.nnx.needle.core.GenreSpot>, play: (String) -> Unit) {
    val most = spots.maxOfOrNull { it.songs.toFloat() }?.coerceAtLeast(1f) ?: 1f
    val dark = Color(0xFF5B6CC8)
    val bright = Color(0xFFE2B46C)
    BoxWithConstraints(Modifier.fillMaxWidth().padding(Edge)) {
        val side = minOf(maxWidth, 560.dp)
        Column(Modifier.width(side).align(Alignment.Center)) {
            Text("Lively", style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.align(Alignment.CenterHorizontally))
            Box(
                Modifier
                    .fillMaxWidth()
                    .aspectRatio(1f)
                    .clip(RoundedCornerShape(16.dp))
                    .background(MaterialTheme.colorScheme.onSurface.copy(alpha = 0.04f))
                    .semantics { contentDescription = "Genre map of ${count(spots.size, "genre")}" },
            ) {
                val big = side * 0.26f
                val small = 44.dp
                spots.sortedByDescending { it.songs }.forEach { g ->
                    // Area by songs: the side grows with the square root.
                    val d = small + (big - small) * kotlin.math.sqrt(g.songs.toFloat() / most)
                    val room = side - d
                    val color = lerp(dark, bright, g.x.coerceIn(0f, 1f))
                    Box(
                        Modifier
                            .offset(x = room * g.x.coerceIn(0f, 1f), y = room * (1f - g.y.coerceIn(0f, 1f)))
                            .size(d)
                            .clip(CircleShape)
                            .background(color.copy(alpha = 0.88f))
                            .clickable(onClickLabel = "Play ${g.genre}") { play(g.genre) }
                            .semantics { contentDescription = "${g.genre}, ${count(g.songs.toInt(), "song")}" },
                        contentAlignment = Alignment.Center,
                    ) {
                        Text(
                            g.genre,
                            style = if (d > 90.dp) MaterialTheme.typography.titleSmall else MaterialTheme.typography.labelSmall,
                            fontWeight = FontWeight.SemiBold,
                            color = if (color.luminance() > 0.45f) Color(0xFF1B1816) else Color.White,
                            textAlign = TextAlign.Center,
                            maxLines = 2,
                            overflow = TextOverflow.Ellipsis,
                            modifier = Modifier.padding(4.dp),
                        )
                    }
                }
            }
            Row(Modifier.fillMaxWidth().padding(top = 4.dp)) {
                Text("Dark", style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                Text("Calm", style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.onSurfaceVariant, textAlign = TextAlign.Center, modifier = Modifier.weight(1f))
                Text("Bright", style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
        }
    }
}

// ---------- The record crate

/**
 * Flip through the albums like records in a box: one at the front, the ones behind leaning
 * back; a swipe flips to the next. In a new order each time, as digging through a crate.
 */
@Composable
fun CrateScreen(open: (Route) -> Unit) {
    val albumsLoad = rememberLoaded { albums().shuffled() }
    val albums by albumsLoad
    val list = albums.orEmpty()
    Column(Modifier.fillMaxSize()) {
        LargeTitle("Record crate")
        if (list.isEmpty()) return@Column
        val pager = rememberPagerState { list.size }
        val scope = androidx.compose.runtime.rememberCoroutineScope()
        // The crate in its own space, clipped to it: the records behind never reach the words.
        BoxWithConstraints(Modifier.weight(1f).fillMaxWidth().clipToBounds()) {
            val side = minOf(maxWidth * 0.78f, maxHeight * 0.55f, 420.dp)
            VerticalPager(
                state = pager,
                contentPadding = PaddingValues(top = 24.dp, bottom = maxHeight - side - 24.dp),
                pageSpacing = -(side * 0.82f),
                beyondViewportPageCount = 4,
                modifier = Modifier.fillMaxSize(),
            ) { page ->
                val album = list[page]
                Box(
                    Modifier
                        .fillMaxWidth()
                        .graphicsLayer {
                            // Behind the front record (offset > 0), each leans back and sinks a little;
                            // the ones flipped past fall forward and fade.
                            val offset = (pager.currentPage - page) + pager.currentPageOffsetFraction
                            val behind = (-offset).coerceAtLeast(0f)
                            val past = offset.coerceAtLeast(0f)
                            rotationX = -12f * behind.coerceAtMost(4f) + 60f * past.coerceAtMost(1f)
                            cameraDistance = 14f * density
                            transformOrigin = androidx.compose.ui.graphics.TransformOrigin(0.5f, 1f)
                            scaleX = 1f - 0.04f * behind.coerceAtMost(4f)
                            scaleY = scaleX
                            alpha = 1f - past.coerceAtMost(1f)
                        }
                        .zIndex(-page.toFloat()),
                    contentAlignment = Alignment.TopCenter,
                ) {
                    Cover(
                        album.artwork,
                        Modifier
                            .size(side)
                            .shadow(18.dp, CoverShape)
                            .clickable(onClickLabel = "Open ${album.title}") {
                                if (page == pager.currentPage) open(Route.AlbumPage(album)) else scope.launch { pager.animateScrollToPage(page) }
                            },
                        CoverShape,
                        seed = album.title + album.artist,
                    )
                }
            }
        }
        // The front record's name, and Play, under the crate on the page's own background, so
        // they read over any cover.
        val front = list.getOrNull(pager.currentPage)
        if (front != null) {
                Column(
                    Modifier
                        .fillMaxWidth()
                        .background(MaterialTheme.colorScheme.background)
                        .padding(horizontal = Edge)
                        .padding(top = 12.dp, bottom = 16.dp + LocalBottomInset.current),
                    horizontalAlignment = Alignment.CenterHorizontally,
                ) {
                    Text(front.title.ifBlank { "Unknown album" }, style = MaterialTheme.typography.titleLarge, fontWeight = FontWeight.SemiBold, maxLines = 1, overflow = TextOverflow.Ellipsis)
                    Text(front.artist, style = MaterialTheme.typography.bodyLarge, color = MaterialTheme.colorScheme.onSurfaceVariant, maxLines = 1)
                    Spacer(Modifier.height(12.dp))
                    PlayPill(Modifier.widthIn(min = 140.dp)) {
                        NeedleApp.instance.scope.launch(kotlinx.coroutines.Dispatchers.IO) {
                            runCatching { NeedleApp.instance.core.albumSongs(front.key).map { it.id } }.getOrNull()?.let { playSongs(it) }
                        }
                    }
                    Text(
                        "${pager.currentPage + 1} of ${list.size}",
                        style = MaterialTheme.typography.labelMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.padding(top = 8.dp),
                    )
                }
        }
    }
}
