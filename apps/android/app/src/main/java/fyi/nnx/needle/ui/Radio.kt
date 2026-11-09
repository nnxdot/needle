package fyi.nnx.needle.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.ShowChart
import androidx.compose.material.icons.rounded.AutoAwesome
import androidx.compose.material.icons.rounded.BubbleChart
import androidx.compose.material.icons.rounded.Favorite
import androidx.compose.material.icons.rounded.PlayArrow
import androidx.compose.material.icons.rounded.Radio
import androidx.compose.material.icons.rounded.Shuffle
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import fyi.nnx.needle.NeedleApp
import fyi.nnx.needle.core.Ranked
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch

/** The album a quick mix takes its colour from: its cover, or the name its made-up cover is drawn from. */
private class MixCover(val path: String?, val seed: String)

/**
 * Radio, as the iPhone's: your station (from the song you play most lately, or any song), quick
 * mixes of the whole library, and a station for each song you play most. Each mix's sign takes
 * the colour of a cover from it (the music is the colour); the tiles are plain fills, not glass.
 */
@Composable
fun RadioScreen(open: (Route) -> Unit) {
    // The same reads as the iPhone's page: the songs played most in 90 days start the stations.
    val seedsLoad = rememberLoaded { stats(90u).topSongs.filter { it.songId != null } }
    val seeds by seedsLoad
    val total by rememberLoaded { songCount() }
    val hearted by rememberLoaded { favorites().map { it.artwork to (it.album + it.artist) } }
    val covers by rememberLoaded {
        val home = runCatching { home() }.getOrNull()
        fun album(a: fyi.nnx.needle.core.Album?) = a?.let { MixCover(it.artwork, it.title + it.artist) }
        mapOf(
            "shuffle" to album(home?.mostPlayed?.firstOrNull() ?: home?.recent?.firstOrNull()),
            "new" to album(home?.added?.firstOrNull()),
            "listening" to (
                runCatching { stats(30u) }.getOrNull()?.topAlbums?.firstOrNull()?.let { MixCover(it.artwork, it.album + it.artist) }
                    ?: album(home?.recent?.drop(1)?.firstOrNull())
                ),
            "genres" to runCatching { genres() }.getOrNull()?.firstOrNull()?.let { MixCover(it.artwork, it.name) },
        )
    }
    val loved = hearted.orEmpty()
    LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(bottom = 32.dp + LocalBottomInset.current)) {
        item { LargeTitle("Radio") }
        item { YourStation(seeds?.firstOrNull(), loading = seedsLoad.loading) }
        item { SectionHeader("Quick mixes") }
        item {
            MixGrid(
                listOf(
                    Mix("Shuffle all", total?.let { count(it.toInt(), "song") } ?: "", Icons.Rounded.Shuffle, covers?.get("shuffle")) {
                        playMix { songs().map { it.id } }
                    },
                    Mix(
                        "Favorites",
                        if (loved.isNotEmpty()) "${count(loved.size, "song")}, shuffled" else "Heart songs to fill it",
                        Icons.Rounded.Favorite,
                        loved.firstOrNull()?.let { MixCover(it.first, it.second) },
                        enabled = loved.isNotEmpty(),
                    ) { playMix { favorites().map { it.id } } },
                    Mix("New to you", "Added lately, shuffled", Icons.Rounded.AutoAwesome, covers?.get("new")) {
                        playMix { home().added.take(12).flatMap { albumSongs(it.key) }.map { it.id } }
                    },
                    Mix("Your listening", "Top songs and artists", Icons.AutoMirrored.Rounded.ShowChart, covers?.get("listening")) { open(Route.History) },
                    Mix("Genre map", "Tap a genre to play it", Icons.Rounded.BubbleChart, covers?.get("genres")) { open(Route.MoodMap) },
                ),
            )
        }
        val list = seeds
        when {
            list == null && seedsLoad.loading -> item { Spacer(Modifier.height(24.dp)) }
            !list.isNullOrEmpty() -> {
                item { SectionHeader("Stations from your favorites") }
                item {
                    Text(
                        "Each starts with the song, then plays songs that flow from it.",
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.padding(start = Edge, end = Edge, bottom = 12.dp),
                    )
                }
                item {
                    LazyRow(contentPadding = PaddingValues(horizontal = Edge), horizontalArrangement = Arrangement.spacedBy(14.dp)) {
                        items(list.take(12), key = { it.songId.orEmpty() }) { seed -> StationCard(seed) }
                    }
                }
            }
            list != null -> item {
                Column(Modifier.fillMaxWidth().padding(horizontal = 32.dp, vertical = 32.dp), horizontalAlignment = Alignment.CenterHorizontally) {
                    ShapedIcon(Icons.Rounded.Radio, 64)
                    Text("Stations come from what you play", style = MaterialTheme.typography.titleMedium, modifier = Modifier.padding(top = 16.dp))
                    Text(
                        "Play some music, and a station for each song you play most shows here. Any song's menu can start one too: Start radio.",
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        textAlign = androidx.compose.ui.text.style.TextAlign.Center,
                        modifier = Modifier.padding(top = 6.dp),
                    )
                }
            }
        }
    }
}

/** Plays a mix, shuffled; the songs are read off the main thread. */
private fun playMix(ids: fyi.nnx.needle.core.Needle.() -> List<String>) {
    NeedleApp.instance.scope.launch(Dispatchers.IO) {
        val list = runCatching { core.ids() }.getOrElse { showMessage(it.message ?: "Could not play these songs"); return@launch }
        if (list.isEmpty()) showMessage("Nothing to play yet") else playSongs(list.shuffled())
    }
}

/** Starts radio from a song: it first, then songs that flow from it. */
private fun startStation(id: String, title: String) {
    showMessage("Starting radio from $title…")
    NeedleApp.instance.scope.launch(Dispatchers.IO) {
        runCatching { core.startRadio(id) }
            .onSuccess { NeedleApp.instance.openPlayer.tryEmit(Unit) }
            .onFailure { showMessage(it.message ?: "Could not start the radio") }
    }
}

/**
 * Your station: from the song played most lately (the first that can start one), or any song
 * when nothing has played yet. Its cover large, darkened at its foot, with a round Play.
 */
@Composable
private fun YourStation(seed: Ranked?, loading: Boolean) {
    val interaction = remember { MutableInteractionSource() }
    // As wide as the page: it shrinks a little when pressed, but does not grow with the focus
    // (it would run past the page's edge on a TV); the ring alone shows the focus.
    val pressed by interaction.collectIsPressedAsState()
    val reduce = reduceMotion()
    val press by androidx.compose.animation.core.animateFloatAsState(if (pressed && !reduce) 0.98f else 1f, label = "press")
    Box(
        Modifier
            .padding(horizontal = Edge, vertical = 8.dp)
            .fillMaxWidth()
            .height(220.dp)
            .firstFocus()
            .graphicsLayer { scaleX = press; scaleY = press }
            .focusRing(interaction, 12.dp)
            .clip(RoundedCornerShape(12.dp))
            .clickable(interactionSource = interaction, indication = null, role = Role.Button, onClickLabel = "Start a radio station", enabled = !loading) {
                showMessage("Tuning in…")
                NeedleApp.instance.scope.launch(Dispatchers.IO) {
                    val top = runCatching { core.stats(30u).topSongs.mapNotNull { it.songId } }.getOrDefault(emptyList())
                    val any = runCatching { core.songs().randomOrNull()?.id }.getOrNull()
                    // The first that can start one (a song whose file is gone cannot).
                    val started = (top + listOfNotNull(any)).firstOrNull { runCatching { core.startRadio(it) }.isSuccess }
                    if (started == null) showMessage("Add music first") else NeedleApp.instance.openPlayer.tryEmit(Unit)
                }
            }
            .semantics { contentDescription = seed?.let { "Your station, from ${it.title} by ${it.artist}" } ?: "Your station" },
    ) {
        Cover(seed?.artwork, Modifier.fillMaxSize(), RoundedCornerShape(12.dp), seed = (seed?.album.orEmpty()) + (seed?.artist ?: "Needle"))
        Box(Modifier.fillMaxSize().background(Brush.verticalGradient(0.4f to Color.Transparent, 1f to Color.Black.copy(alpha = 0.75f))))
        Row(Modifier.align(Alignment.BottomStart).fillMaxWidth().padding(18.dp), verticalAlignment = Alignment.Bottom) {
            Column(Modifier.weight(1f)) {
                Text("YOUR STATION", style = MaterialTheme.typography.labelMedium, fontWeight = FontWeight.Bold, color = Color.White.copy(alpha = 0.8f))
                Text(
                    seed?.let { "From ${it.title}" } ?: "From anything in your library",
                    style = MaterialTheme.typography.headlineSmall, fontWeight = FontWeight.Bold, color = Color.White,
                    maxLines = 2, overflow = TextOverflow.Ellipsis,
                )
                Text(
                    seed?.artist ?: "Needle picks a song to start",
                    style = MaterialTheme.typography.bodyMedium, color = Color.White.copy(alpha = 0.8f), maxLines = 1, overflow = TextOverflow.Ellipsis,
                )
            }
            Box(Modifier.size(52.dp).clip(CircleShape).background(Color.White), contentAlignment = Alignment.Center) {
                Icon(Icons.Rounded.PlayArrow, contentDescription = null, tint = Color.Black, modifier = Modifier.size(30.dp))
            }
        }
    }
}

private class Mix(
    val title: String,
    val detail: String,
    val icon: ImageVector,
    val cover: MixCover?,
    val enabled: Boolean = true,
    val onClick: () -> Unit,
)

/** The quick mixes, two to a row on a phone, more on a tablet or TV. */
@Composable
private fun MixGrid(mixes: List<Mix>) {
    val columns = if (isWide()) (LocalPageWidth.current / 200.dp).toInt().coerceIn(2, 5) else 2
    Column(Modifier.padding(horizontal = Edge), verticalArrangement = Arrangement.spacedBy(12.dp)) {
        mixes.chunked(columns).forEach { row ->
            Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                row.forEach { mix -> MixTile(mix, Modifier.weight(1f)) }
                repeat(columns - row.size) { Spacer(Modifier.weight(1f)) }
            }
        }
    }
}

/**
 * A mix's tile: a plain fill in the page's raised colour (content, not glass), its sign in the
 * colour of a cover from the mix, a made-up cover's colour when it has no picture, or the accent
 * before one is known.
 */
@Composable
private fun MixTile(mix: Mix, modifier: Modifier) {
    val interaction = remember { MutableInteractionSource() }
    val scheme = rememberCoverScheme(mix.cover?.path)
    val seeded = mix.cover?.takeIf { it.path == null }?.seed?.let { seedColor(it) }
    val sign = scheme?.primaryContainer ?: seeded ?: MaterialTheme.colorScheme.primary
    val ink = scheme?.onPrimaryContainer ?: if (seeded != null) Color.White else MaterialTheme.colorScheme.onPrimary
    Column(
        modifier
            .pressScale(interaction)
            .clip(RoundedCornerShape(14.dp))
            .background(MaterialTheme.colorScheme.surfaceContainer)
            .clickable(interactionSource = interaction, indication = null, enabled = mix.enabled, role = Role.Button, onClick = mix.onClick)
            .padding(14.dp),
        verticalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        Box(
            Modifier.size(40.dp).clip(RoundedCornerShape(10.dp)).background(Brush.verticalGradient(listOf(sign, sign.copy(alpha = 0.78f)))),
            contentAlignment = Alignment.Center,
        ) {
            Icon(mix.icon, contentDescription = null, tint = ink, modifier = Modifier.size(22.dp))
        }
        Column {
            Text(mix.title, style = MaterialTheme.typography.bodyLarge, fontWeight = FontWeight.SemiBold, maxLines = 1, overflow = TextOverflow.Ellipsis)
            Text(
                mix.detail, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant,
                maxLines = 1, overflow = TextOverflow.Ellipsis,
            )
        }
    }
}

/** A station from one song: its cover with a small Radio mark; a tap starts it. */
@Composable
private fun StationCard(seed: Ranked) {
    val interaction = remember { MutableInteractionSource() }
    Column(
        Modifier
            .width(150.dp)
            .pressScale(interaction)
            .clickable(interactionSource = interaction, indication = null, role = Role.Button) {
                seed.songId?.let { startStation(it, seed.title) }
            }
            .semantics(mergeDescendants = true) { contentDescription = "${seed.title} radio, by ${seed.artist}" },
    ) {
        Box {
            Cover(seed.artwork, Modifier.size(150.dp), CoverShape, seed = seed.album + seed.artist)
            Row(
                Modifier
                    .align(Alignment.BottomStart)
                    .padding(8.dp)
                    .clip(CircleShape)
                    .background(MaterialTheme.colorScheme.surfaceContainer.copy(alpha = 0.92f))
                    .padding(horizontal = 8.dp, vertical = 4.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(4.dp),
            ) {
                Icon(Icons.Rounded.Radio, contentDescription = null, modifier = Modifier.size(12.dp))
                Text("Radio", style = MaterialTheme.typography.labelSmall, fontWeight = FontWeight.Bold)
            }
        }
        Text(seed.title, style = MaterialTheme.typography.bodyMedium, fontWeight = FontWeight.Medium, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.padding(top = 6.dp))
        Text(seed.artist, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant, maxLines = 1, overflow = TextOverflow.Ellipsis)
    }
}
