package fyi.nnx.needle.ui

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
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

// ---------- The mood map

/**
 * Every measured song a dot, dark to bright across and calm to lively up; draw a circle (or
 * tap) on an area to play the songs in it, nearest its middle first.
 */
@Composable
fun MoodMapScreen() {
    val dotsLoad = rememberLoaded { moodMap() }
    val dots by dotsLoad
    var center by remember { mutableStateOf<Offset?>(null) }
    var radius by remember { mutableStateOf(0f) }
    val accent = MaterialTheme.colorScheme.primary
    val dark = Color(0xFF5B6CC8)
    val bright = Color(0xFFE2B46C)
    fun playArea(x: Float, y: Float, r: Float) {
        NeedleApp.instance.scope.launch(kotlinx.coroutines.Dispatchers.IO) {
            val songs = runCatching { NeedleApp.instance.core.moodArea(x, y, r.coerceAtLeast(0.06f), 100u) }.getOrDefault(emptyList())
            if (songs.isEmpty()) showMessage("No songs there") else {
                playSongs(songs.map { it.id })
                showMessage("Playing ${count(songs.size, "song")} from that mood")
            }
        }
    }
    LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(bottom = 32.dp + LocalBottomInset.current)) {
        item { LargeTitle("Mood map") }
        item {
            Text(
                "Every song by how calm or lively, dark or bright it sounds. Circle an area, or tap one, to play it.",
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(horizontal = Edge),
            )
        }
        val list = dots.orEmpty()
        if (dots != null && list.isEmpty()) {
            item {
                Text(
                    "No songs are measured yet. Turn on Measure songs for radio in Settings › Playback; Needle then measures them in the background.",
                    style = MaterialTheme.typography.bodyLarge,
                    modifier = Modifier.padding(Edge),
                )
            }
            return@LazyColumn
        }
        item {
            BoxWithConstraints(Modifier.fillMaxWidth().padding(Edge)) {
                val side = minOf(maxWidth, 560.dp)
                Column(Modifier.width(side).align(Alignment.Center)) {
                    Text("Lively", style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.align(Alignment.CenterHorizontally))
                    Canvas(
                        Modifier
                            .fillMaxWidth()
                            .aspectRatio(1f)
                            .clip(RoundedCornerShape(16.dp))
                            .background(MaterialTheme.colorScheme.onSurface.copy(alpha = 0.04f))
                            .semantics { contentDescription = "Mood map of ${count(list.size, "song")}" }
                            .pointerInput(Unit) {
                                detectTapGestures { p -> playArea(p.x / size.width, 1f - p.y / size.height, 0.08f) }
                            }
                            .pointerInput(Unit) {
                                detectDragGestures(
                                    onDragStart = { center = it; radius = 0f },
                                    onDragEnd = {
                                        val c = center
                                        if (c != null) playArea(c.x / size.width, 1f - c.y / size.height, radius / size.width)
                                        center = null
                                    },
                                    onDragCancel = { center = null },
                                ) { change, _ -> center?.let { radius = hypot(change.position.x - it.x, change.position.y - it.y) } }
                            },
                    ) {
                        list.forEach { d ->
                            val color = lerp(dark, bright, d.x).copy(alpha = 0.45f + d.y * 0.45f)
                            drawCircle(color, radius = 3.5.dp.toPx(), center = Offset(d.x * size.width, (1f - d.y) * size.height))
                        }
                        center?.let { c ->
                            drawCircle(accent.copy(alpha = 0.18f), radius, c)
                            drawCircle(accent, radius, c, style = Stroke(2.dp.toPx()))
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
        BoxWithConstraints(Modifier.weight(1f).fillMaxWidth()) {
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
            // The front record's name, and Play.
            val front = list.getOrNull(pager.currentPage)
            if (front != null) {
                Column(
                    Modifier.align(Alignment.BottomCenter).fillMaxWidth().padding(horizontal = Edge).padding(bottom = 16.dp + LocalBottomInset.current),
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
}
