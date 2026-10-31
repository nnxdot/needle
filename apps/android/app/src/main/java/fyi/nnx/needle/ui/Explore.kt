package fyi.nnx.needle.ui

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.ui.draw.clipToBounds
import androidx.compose.ui.unit.sp
import androidx.compose.foundation.border
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
 * The genre map (NEEDLE-2.0.md, "Genre map"), as on the computers and the iPhone: a bubble for
 * each of the first 40 genres (the core's order: most songs first) that has a place, as big as
 * its number of songs, where its songs sound; a tap plays the genre shuffled. The others, and
 * those with no song measured yet, are chips under "More genres".
 */
@OptIn(androidx.compose.foundation.layout.ExperimentalLayoutApi::class)
@Composable
fun MoodMapScreen() {
    val spotsLoad = rememberLoaded { genreMap() }
    val spots by spotsLoad
    val all = spots.orEmpty()
    val shown = all.filterIndexed { i, g -> i < MapBubbles && g.placed }
    val chips = all.filter { it !in shown }
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
        if (spots != null) item {
            Text(
                when {
                    all.isEmpty() -> "Your songs have no genres in their tags yet."
                    shown.isEmpty() -> "Needle measures each song's sound in the background (Settings › Playback › Radio › Measure songs for radio). Genres take their place as their songs are measured."
                    else -> "${count(all.size, "genre")}, each where its songs sound. Tap one to play it."
                },
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(horizontal = Edge),
            )
        }
        if (shown.isNotEmpty()) item { GenreBubbles(shown, all.first().songs.toFloat().coerceAtLeast(1f), ::playGenre) }
        if (chips.isNotEmpty()) {
            item { ShelfTitle("More genres", null) }
            item {
                FlowRow(
                    Modifier.fillMaxWidth().padding(horizontal = Edge),
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                    verticalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    chips.forEach { g ->
                        androidx.compose.material3.SuggestionChip(
                            onClick = { playGenre(g.genre) },
                            label = {
                                Text(g.genre)
                                Text("  ${g.songs}", color = MaterialTheme.colorScheme.onSurfaceVariant)
                            },
                            shape = RoundedCornerShape(50),
                        )
                    }
                }
            }
        }
    }
}

/** A bubble on the map: its genre, songs, middle (x, y), and size, in dp. */
private class Bubble(val genre: String, val songs: UInt, var x: Float, var y: Float, val size: Float)

/**
 * The bubbles on a map `w` × `h` (dp), as on the computers: each at its genre's place, then 200
 * passes nudging each overlapping pair apart along the line between them to a 4 dp gap (both
 * ways, in turn, where they share a row or column), kept inside the map after each pass, clear
 * of the corner words (28 dp at the top and foot).
 */
private fun layOut(spots: List<fyi.nnx.needle.core.GenreSpot>, biggest: Float, w: Float, h: Float, scale: Float): List<Bubble> {
    val b = spots.map { g ->
        Bubble(g.genre, g.songs, w * (0.06f + g.x * 0.88f), h * (0.94f - g.y * 0.88f), scale * (40f + 120f * kotlin.math.sqrt(g.songs.toFloat() / biggest)))
    }
    repeat(200) {
        for (a in b.indices) for (c in a + 1 until b.size) {
            var dx = b[c].x - b[a].x
            var dy = b[c].y - b[a].y
            // In one row or column (genres that sound alike): both ways, in turn.
            if (kotlin.math.abs(dy) < 1f) dy = if ((a + c) % 2 == 0) 6f else -6f
            if (kotlin.math.abs(dx) < 1f) dx = if (c % 2 == 0) 6f else -6f
            val gap = (b[a].size + b[c].size) / 2 + 4f
            val far = kotlin.math.sqrt(dx * dx + dy * dy)
            if (far < gap) {
                val ux = if (far > 0.01f) dx / far else 1f
                val uy = if (far > 0.01f) dy / far else 0f
                val push = (gap - far) / 2
                b[a].x -= ux * push; b[a].y -= uy * push
                b[c].x += ux * push; b[c].y += uy * push
            }
        }
        for (one in b) {
            val r = one.size / 2
            one.x = one.x.coerceIn(r + 4f, maxOf(r + 4f, w - r - 4f))
            one.y = one.y.coerceIn(r + 28f, maxOf(r + 28f, h - r - 28f))
        }
    }
    return b
}

/** A genre's colour, from its name as on the computers: the hue of its name's hash, HSL 0.55 and 0.6 (dark looks) or 0.42. */
private fun genreTone(genre: String, dark: Boolean): Color =
    Color.hsl(seedHue(genre.lowercase()), 0.55f, if (dark) 0.6f else 0.42f)

/**
 * The map: taller than wide on a phone (1.6 × its width, up to 760 dp), as wide as a
 * computer's on a tablet; the bubbles smaller on a small map, by the square root of its area
 * against the widest computer map's (980 × 600), 0.6 to 1. The largest drawn first, so the small
 * ones stay on top. The corner words inside the map, small and faint.
 */
@Composable
private fun GenreBubbles(spots: List<fyi.nnx.needle.core.GenreSpot>, biggest: Float, play: (String) -> Unit) {
    val dark = MaterialTheme.colorScheme.background.luminance() < 0.5f
    val ink = MaterialTheme.colorScheme.onSurface
    val faint = MaterialTheme.colorScheme.onSurfaceVariant
    val measurer = androidx.compose.ui.text.rememberTextMeasurer()
    val density = androidx.compose.ui.platform.LocalDensity.current
    BoxWithConstraints(Modifier.fillMaxWidth().padding(horizontal = Edge, vertical = 12.dp)) {
        val w = minOf(maxWidth, 980.dp).value
        val h = if (w >= 600f) (w * 0.62f).coerceIn(280f, 600f) else (w * 1.6f).coerceIn(300f, 760f)
        val scale = kotlin.math.sqrt(w * h / (980f * 600f)).coerceIn(0.6f, 1f)
        val bubbles = remember(spots, w, h) { layOut(spots, biggest, w, h, scale).sortedByDescending { it.size } }
        Box(
            Modifier
                .align(Alignment.Center)
                .width(w.dp)
                .height(h.dp)
                .clip(RoundedCornerShape(18.dp))
                .background(MaterialTheme.colorScheme.onSurface.copy(alpha = 0.04f))
                .semantics { contentDescription = "Genre map of ${count(spots.size, "genre")}" },
        ) {
            val corner = MaterialTheme.typography.labelSmall
            Text("Lively", style = corner, color = faint, modifier = Modifier.align(Alignment.TopStart).padding(start = 12.dp, top = 8.dp))
            Text("Calm", style = corner, color = faint, modifier = Modifier.align(Alignment.BottomStart).padding(start = 12.dp, bottom = 8.dp))
            Text("Dark", style = corner, color = faint, modifier = Modifier.align(Alignment.BottomStart).padding(start = 60.dp, bottom = 8.dp))
            Text("Bright", style = corner, color = faint, modifier = Modifier.align(Alignment.BottomEnd).padding(end = 12.dp, bottom = 8.dp))
            bubbles.forEach { one ->
                val d = one.size
                val tone = genreTone(one.genre, dark)
                // A seventh of the bubble (11 to 17), or smaller so the longest word fits on one line.
                val label = remember(one.genre, d) {
                    val words = one.genre.replace("-", "- ").split(' ').filter { it.isNotEmpty() }
                    var sp = (d / 7f).coerceIn(11f, 17f)
                    while (sp > 8f) {
                        val widest = words.maxOf { word ->
                            with(density) { measurer.measure(word, androidx.compose.ui.text.TextStyle(fontSize = sp.sp, fontWeight = FontWeight.SemiBold)).size.width.toDp().value }
                        }
                        if (widest <= d - 12f) break
                        sp -= 0.5f
                    }
                    sp
                }
                val many = one.genre.contains(' ') || one.genre.contains('-')
                Column(
                    Modifier
                        .offset(x = (one.x - d / 2).dp, y = (one.y - d / 2).dp)
                        .size(d.dp)
                        .clip(CircleShape)
                        .background(tone.copy(alpha = 0.22f))
                        .border(1.dp, tone.copy(alpha = 0.65f), CircleShape)
                        .clickable(onClickLabel = "Play ${one.genre}") { play(one.genre) }
                        .semantics(mergeDescendants = true) { contentDescription = "${one.genre}, ${count(one.songs.toInt(), "song")}" }
                        .padding(horizontal = 4.dp),
                    horizontalAlignment = Alignment.CenterHorizontally,
                    verticalArrangement = Arrangement.Center,
                ) {
                    Text(
                        one.genre,
                        fontSize = label.sp,
                        lineHeight = (label * 1.15f).sp,
                        fontWeight = FontWeight.SemiBold,
                        color = ink,
                        textAlign = TextAlign.Center,
                        maxLines = if (many) 2 else 1,
                        overflow = TextOverflow.Clip,
                    )
                    if (d > 74f) Text(count(one.songs.toInt(), "song"), style = MaterialTheme.typography.labelSmall, color = faint, maxLines = 1)
                }
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
