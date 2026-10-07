package fyi.nnx.needle.ui

import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.pager.HorizontalPager
import androidx.compose.foundation.pager.rememberPagerState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.FilterChip
import androidx.compose.material3.MaterialShapes
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.toShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import fyi.nnx.needle.NeedleApp
import fyi.nnx.needle.core.Wrapped
import kotlinx.coroutines.launch

private fun playTop(w: Wrapped, start: Int = 0) {
    core.play(w.topSongs.map { it.id }, start.toUInt())
    NeedleApp.instance.openPlayer.tryEmit(Unit)
}

/**
 * The year in music as a story: full-height cards to swipe through, coloured by the year's
 * top cover, as Wrapped and Replay do.
 */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun WrappedScreen(year: Int?, open: (Route) -> Unit) {
    val yearsLoad = rememberLoaded { wrappedYears() }
    val years by yearsLoad
    var chosen by rememberSaveable { mutableStateOf(year) }
    val shown = chosen ?: years?.firstOrNull()
    val wrappedLoad = rememberLoaded(shown) { shown?.let { wrapped(it) } }
    val wrapped by wrappedLoad
    val w = wrapped
    if (years != null && years!!.isEmpty()) {
        Column(Modifier.fillMaxSize()) {
            LargeTitle("Your year")
            Text("Play some music, and your year in music shows here.", color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.padding(Edge))
        }
        return
    }
    if (w == null) return
    val glow = coverColor(w.topSongs.firstOrNull()?.artwork)
    val pages = listOfNotNull<@Composable () -> Unit>(
        { Intro(w) },
        w.topSongs.firstOrNull()?.let { { TopSong(w) } },
        if (w.topSongs.size > 1) ({ TopFive(w) }) else null,
        if (w.stats.topArtists.isNotEmpty()) ({ Artists(w, open) }) else null,
        { Habits(w) },
        { Keep(w) },
    )
    val pager = rememberPagerState { pages.size }
    val scope = rememberCoroutineScope()
    Box(
        Modifier
            .fillMaxSize()
            .background(Brush.verticalGradient(listOf(glow.copy(alpha = 0.9f), MaterialTheme.colorScheme.background), endY = 1800f)),
    ) {
        Column(Modifier.fillMaxSize()) {
            // Story segments along the top: done, current, still to come.
            Row(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 12.dp), horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                repeat(pages.size) { i ->
                    val fill by animateFloatAsState(if (i <= pager.currentPage) 1f else 0f, tween(300), label = "segment")
                    Box(Modifier.weight(1f).height(4.dp).clip(RoundedCornerShape(2.dp)).background(Color.White.copy(alpha = 0.25f))) {
                        Box(Modifier.fillMaxHeight().fillMaxWidth(fill).background(Color.White))
                    }
                }
            }
            if ((years?.size ?: 0) > 1) {
                Row(Modifier.padding(horizontal = 16.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    years.orEmpty().take(4).forEach { y ->
                        FilterChip(selected = y == shown, onClick = { chosen = y; scope.launch { pager.scrollToPage(0) } }, label = { Text(y.toString()) })
                    }
                }
            }
            HorizontalPager(pager, Modifier.weight(1f), pageSpacing = 12.dp, contentPadding = androidx.compose.foundation.layout.PaddingValues(horizontal = 16.dp, vertical = 12.dp)) { page ->
                // Each card its own colour; the song card takes its cover's.
                val (top, bottom) = if (page == 1) glow to Color(0xFF0E0D0C) else CardColors[page % CardColors.size]
                Box(
                    Modifier
                        .fillMaxSize()
                        .clip(RoundedCornerShape(36.dp))
                        .background(Brush.linearGradient(listOf(top, bottom)))
                        // A tap moves on, as in a story.
                        .clickable { scope.launch { pager.animateScrollToPage((page + 1).coerceAtMost(pages.size - 1)) } }
                        .padding(28.dp),
                ) { pages[page]() }
            }
        }
    }
}

/** Rich, dark pairs, so white type always reads: amber, song, teal, plum, forest, amber. */
private val CardColors = listOf(
    Color(0xFF8A5718) to Color(0xFF1E1308),
    Color(0xFF3A2E20) to Color(0xFF0E0D0C),
    Color(0xFF0F5A6B) to Color(0xFF051D23),
    Color(0xFF6A2D7F) to Color(0xFF1A0B20),
    Color(0xFF2E6B3A) to Color(0xFF0B1E0F),
    Color(0xFF9A5B1C) to Color(0xFF1E1308),
)

@Composable
private fun Big(text: String, size: Int = 88) {
    Text(text, color = Color.White, fontSize = size.sp, lineHeight = (size * 0.95).sp, fontWeight = FontWeight.Black, letterSpacing = (-2).sp)
}

@Composable
private fun Small(text: String, alpha: Float = 0.75f) {
    Text(text, color = Color.White.copy(alpha = alpha), style = MaterialTheme.typography.titleMedium)
}

@Composable
private fun Intro(w: Wrapped) {
    val minutes = (w.stats.seconds / 60).toLong()
    Column(Modifier.fillMaxSize(), verticalArrangement = Arrangement.Center) {
        Collage(w)
        Spacer(Modifier.height(32.dp))
        Small("Your ${w.year} in music")
        Spacer(Modifier.height(16.dp))
        Big("%,d".format(minutes))
        Text("minutes of music", color = Color.White, style = MaterialTheme.typography.headlineSmall, fontWeight = FontWeight.Bold)
        Spacer(Modifier.height(28.dp))
        Small("${w.stats.plays} plays · ${w.stats.songs} songs · ${w.stats.artists} artists")
    }
}

@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun TopSong(w: Wrapped) {
    val song = w.topSongs.first()
    // The cover turns slowly in a scalloped frame, like a record.
    val spin by rememberInfiniteTransition(label = "record").animateFloat(0f, 360f, infiniteRepeatable(tween(24000, easing = LinearEasing)), label = "spin")
    val reduce = reduceMotion()
    Column(Modifier.fillMaxSize(), horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.Center) {
        Small("Your song of the year")
        Spacer(Modifier.height(24.dp))
        Cover(
            song.artwork,
            Modifier.fillMaxWidth(0.8f).aspectRatio(1f).graphicsLayer { rotationZ = if (reduce) 0f else spin },
            MaterialShapes.Cookie12Sided.toShape(),
        )
        Spacer(Modifier.height(28.dp))
        Text(song.title, color = Color.White, style = MaterialTheme.typography.headlineMedium, fontWeight = FontWeight.Black, textAlign = TextAlign.Center, maxLines = 2, overflow = TextOverflow.Ellipsis)
        Small(song.artist)
        Spacer(Modifier.height(20.dp))
        Button(onClick = { playTop(w) }, colors = ButtonDefaults.buttonColors(containerColor = Color.White, contentColor = Color.Black)) { Text("Play it") }
    }
}

@Composable
private fun TopFive(w: Wrapped) {
    Column(Modifier.fillMaxSize(), verticalArrangement = Arrangement.Center) {
        Small("Your top songs")
        Spacer(Modifier.height(20.dp))
        w.topSongs.take(5).forEachIndexed { i, song ->
            Row(
                Modifier.fillMaxWidth().clip(RoundedCornerShape(16.dp)).clickable { playTop(w, i) }.padding(vertical = 8.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(14.dp),
            ) {
                Text("${i + 1}", color = Color.White, fontSize = 40.sp, fontWeight = FontWeight.Black, modifier = Modifier.width(40.dp))
                Cover(song.artwork, Modifier.size(56.dp), RoundedCornerShape(12.dp))
                Column(Modifier.weight(1f)) {
                    Text(song.title, color = Color.White, style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.Bold, maxLines = 1, overflow = TextOverflow.Ellipsis)
                    Text(song.artist, color = Color.White.copy(alpha = 0.7f), style = MaterialTheme.typography.bodyMedium, maxLines = 1, overflow = TextOverflow.Ellipsis)
                }
            }
        }
    }
}

@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun Artists(w: Wrapped, open: (Route) -> Unit) {
    val shapes = listOf(MaterialShapes.Circle, MaterialShapes.Cookie9Sided, MaterialShapes.Clover8Leaf, MaterialShapes.Sunny, MaterialShapes.Flower)
    Column(Modifier.fillMaxSize(), verticalArrangement = Arrangement.Center) {
        Small("Your top artists")
        Spacer(Modifier.height(20.dp))
        w.stats.topArtists.take(5).forEachIndexed { i, a ->
            // Their photo, or else a cover of theirs.
            val photoLoad = rememberLoaded(a.artist) { artistPhoto(a.artist) ?: artistAlbums(a.artist).firstNotNullOfOrNull { it.artwork } }
            val photo by photoLoad
            Row(
                Modifier.fillMaxWidth().clip(RoundedCornerShape(16.dp)).clickable { open(Route.Artist(a.artist)) }.padding(vertical = 8.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(16.dp),
            ) {
                Cover(photo ?: a.artwork, Modifier.size(if (i == 0) 84.dp else 56.dp), shapes[i % shapes.size].toShape())
                Column(Modifier.weight(1f)) {
                    Text(a.artist, color = Color.White, style = if (i == 0) MaterialTheme.typography.headlineSmall else MaterialTheme.typography.titleMedium, fontWeight = FontWeight.Bold, maxLines = 1, overflow = TextOverflow.Ellipsis)
                    Text(count(a.plays.toInt(), "play"), color = Color.White.copy(alpha = 0.7f), style = MaterialTheme.typography.bodyMedium)
                }
            }
        }
    }
}

@Composable
private fun Habits(w: Wrapped) {
    Column(Modifier.fillMaxSize(), verticalArrangement = Arrangement.Center) {
        Small("How you listened")
        Spacer(Modifier.height(16.dp))
        w.peakHour?.let {
            val label = when (it.toInt()) {
                in 5..11 -> "Mornings"
                in 12..16 -> "Afternoons"
                in 17..21 -> "Evenings"
                else -> "Late nights"
            }
            Big(label, 56)
            Small("Most of all around ${it}:00")
        }
        Spacer(Modifier.height(24.dp))
        // The day's hours as bars, the busiest lit.
        val most = (w.stats.hours.maxOrNull() ?: 0.0).coerceAtLeast(1.0)
        Row(Modifier.fillMaxWidth().height(90.dp), horizontalArrangement = Arrangement.spacedBy(3.dp), verticalAlignment = Alignment.Bottom) {
            w.stats.hours.forEach { h ->
                Box(Modifier.weight(1f).fillMaxHeight((h / most).toFloat().coerceIn(0.04f, 1f)).clip(RoundedCornerShape(3.dp)).background(Color.White.copy(alpha = if (h == most) 1f else 0.4f)))
            }
        }
        Spacer(Modifier.height(24.dp))
        if (w.streak > 1u) Small("Your longest streak: ${w.streak} days in a row", 0.9f)
        if (w.genres.isNotEmpty()) Small("Mostly ${w.genres.take(3).joinToString(", ")}", 0.9f)
        if (w.newArtists.isNotEmpty()) Small("New to you: ${w.newArtists.take(3).joinToString(", ")}", 0.9f)
    }
}

@Composable
private fun Keep(w: Wrapped) {
    Column(Modifier.fillMaxSize(), verticalArrangement = Arrangement.Center, horizontalAlignment = Alignment.CenterHorizontally) {
        Big("${w.year}", 96)
        Small("That was your year.")
        Spacer(Modifier.height(32.dp))
        Button(onClick = { playTop(w) }, colors = ButtonDefaults.buttonColors(containerColor = Color.White, contentColor = Color.Black)) { Text("Play your top songs") }
        Spacer(Modifier.height(8.dp))
        FilledTonalButton(onClick = {
            core.createPlaylist("Top songs of ${w.year}", "Your most played songs of ${w.year}.", w.topSongs.map { it.id }, null)
            NeedleApp.instance.libraryVersion.value++
            showMessage("Saved as a playlist")
        }) { Text("Save them as a playlist") }
    }
}

/** The year's top covers, each in its own Expressive shape, bobbing gently. */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun Collage(w: Wrapped) {
    val covers = w.topSongs.mapNotNull { it.artwork }.distinct().take(5)
    if (covers.isEmpty()) return
    val shapes = listOf(MaterialShapes.Cookie9Sided, MaterialShapes.Circle, MaterialShapes.Clover4Leaf, MaterialShapes.Sunny, MaterialShapes.Pill)
    val sizes = listOf(120, 84, 100, 72, 90)
    val time by rememberInfiniteTransition(label = "bob").animateFloat(0f, (2 * Math.PI).toFloat(), infiniteRepeatable(tween(6000, easing = LinearEasing)), label = "t")
    val reduce = reduceMotion()
    Row(Modifier.fillMaxWidth().height(150.dp), horizontalArrangement = Arrangement.spacedBy((-14).dp), verticalAlignment = Alignment.CenterVertically) {
        covers.forEachIndexed { i, art ->
            Cover(
                art,
                Modifier
                    .size(sizes[i].dp)
                    .graphicsLayer {
                        translationY = if (reduce) 0f else kotlin.math.sin(time + i * 1.3f) * 10f
                        rotationZ = if (reduce) 0f else kotlin.math.cos(time + i) * 6f
                    },
                shapes[i].toShape(),
            )
        }
    }
}
