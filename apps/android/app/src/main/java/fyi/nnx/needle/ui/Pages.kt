package fyi.nnx.needle.ui

import androidx.compose.foundation.background
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.lazy.grid.rememberLazyGridState
import androidx.compose.material.icons.automirrored.rounded.ArrowBack
import androidx.compose.runtime.derivedStateOf
import androidx.compose.ui.draw.clipToBounds
import androidx.compose.ui.draw.shadow
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.GridItemSpan
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.PlaylistAdd
import androidx.compose.material.icons.rounded.AutoAwesome
import androidx.compose.material.icons.rounded.Close
import androidx.compose.material.icons.rounded.Computer
import androidx.compose.material.icons.rounded.Edit
import androidx.compose.material.icons.rounded.MoreVert
import androidx.compose.material.icons.rounded.PlayArrow
import androidx.compose.material.icons.rounded.Radio
import androidx.compose.material.icons.rounded.Search
import androidx.compose.material.icons.rounded.Settings
import androidx.compose.material.icons.rounded.Shuffle
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.FilledTonalIconButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.IconButtonDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TextField
import androidx.compose.material3.TextFieldDefaults
import androidx.compose.material3.carousel.HorizontalMultiBrowseCarousel
import androidx.compose.material3.carousel.rememberCarouselState
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
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import fyi.nnx.needle.NeedleApp
import fyi.nnx.needle.core.Album
import fyi.nnx.needle.core.Song
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

private fun play(songs: List<Song>, start: Int = 0) {
    core.play(songs.map { it.id }, start.toUInt())
    NeedleApp.instance.openPlayer.tryEmit(Unit)
}

@Composable
private fun playingId(): String? = NeedleApp.instance.playback.collectAsState().value?.current?.id

// ---------- Shared: album, playlist, and artist pages

/**
 * An album, playlist, or artist page: all of it in the cover's colours, the page running up
 * under the status bar. A bar with the name fades in once the top has scrolled away.
 */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun DetailFrame(art: String?, title: String, scrolled: () -> Float, menu: (() -> Unit)?, content: @Composable () -> Unit) {
    CoverTheme(art) {
        Box(Modifier.fillMaxSize().background(MaterialTheme.colorScheme.background)) {
            content()
            val limit = with(androidx.compose.ui.platform.LocalDensity.current) { 340.dp.toPx() }
            val solid by remember { derivedStateOf { scrolled() > limit } }
            val fill by animateFloatAsState(if (solid) 1f else 0f, tween(220), label = "bar")
            val back = androidx.activity.compose.LocalOnBackPressedDispatcherOwner.current
            val scheme = MaterialTheme.colorScheme
            Row(
                Modifier
                    .fillMaxWidth()
                    .background(scheme.surfaceContainer.copy(alpha = fill))
                    .statusBarsPadding()
                    .height(56.dp)
                    .padding(horizontal = 6.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                // Over the cover, the buttons sit on soft circles so they always read.
                val round = IconButtonDefaults.filledTonalIconButtonColors(
                    containerColor = scheme.surfaceContainerHighest.copy(alpha = 0.55f * (1f - fill)),
                    contentColor = scheme.onSurface,
                )
                FilledTonalIconButton(onClick = { back?.onBackPressedDispatcher?.onBackPressed() }, colors = round, shapes = IconButtonDefaults.shapes()) {
                    Icon(Icons.AutoMirrored.Rounded.ArrowBack, contentDescription = "Back")
                }
                Text(
                    title,
                    style = MaterialTheme.typography.titleLarge,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                    modifier = Modifier.weight(1f).padding(horizontal = 8.dp).graphicsLayer { alpha = fill },
                )
                if (menu != null) {
                    FilledTonalIconButton(onClick = menu, colors = round, shapes = IconButtonDefaults.shapes()) {
                        Icon(Icons.Rounded.MoreVert, contentDescription = "More")
                    }
                }
            }
        }
    }
}

/**
 * The top of an album or playlist page: the cover drifting on its own blurred self, then the
 * name large and narrow. As the page scrolls, the cover moves slower than the page and shrinks
 * back, and the name grows narrower.
 */
@Composable
fun DetailHeader(art: String?, title: String, sub: String?, meta: List<String>, scrolled: () -> Float, onSub: (() -> Unit)? = null, key: String? = null) {
    val density = androidx.compose.ui.platform.LocalDensity.current
    val squeeze by remember { derivedStateOf { (scrolled() / with(density) { 260.dp.toPx() }).coerceIn(0f, 1f) } }
    Box(Modifier.fillMaxWidth()) {
        CoverBackdrop(art, Modifier.matchParentSize())
        Column(
            Modifier.fillMaxWidth().statusBarsPadding().padding(top = 56.dp, start = Edge, end = Edge, bottom = 4.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            Cover(
                art,
                Modifier
                    .fillMaxWidth(0.72f)
                    .aspectRatio(1f)
                    .graphicsLayer {
                        // Shrinks toward its top and fades, so it leaves ahead of the name.
                        val s = scrolled().coerceAtMost(2000f)
                        val k = (1f - s / 1600f).coerceAtLeast(0.6f)
                        transformOrigin = androidx.compose.ui.graphics.TransformOrigin(0.5f, 0f)
                        scaleX = k
                        scaleY = k
                        alpha = 1f - (s / 900f).coerceIn(0f, 1f)
                    }
                    .then(if (key != null) Modifier.sharedCover(key) else Modifier)
                    .shadow(28.dp, CoverShape, ambientColor = Color.Black, spotColor = Color.Black),
            )
            Text(
                title,
                style = MaterialTheme.typography.displaySmall.copy(fontFamily = titleFamily(0.3f + squeeze * 0.7f)),
                textAlign = TextAlign.Center,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier.padding(top = 24.dp),
            )
            if (sub != null) {
                Text(
                    sub,
                    style = MaterialTheme.typography.titleMedium,
                    color = MaterialTheme.colorScheme.primary,
                    textAlign = TextAlign.Center,
                    modifier = Modifier
                        .padding(top = 2.dp)
                        .clip(CircleShape)
                        .then(if (onSub != null) Modifier.clickable(onClick = onSub) else Modifier)
                        .padding(horizontal = 10.dp, vertical = 4.dp),
                )
            }
            if (meta.isNotEmpty()) MetaLine(meta, Modifier.padding(top = 6.dp))
        }
    }
}

/** Small facts in a row: "Album · 2026 · 16 min". */
@Composable
fun MetaLine(parts: List<String>, modifier: Modifier = Modifier) {
    Row(modifier, verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        parts.forEachIndexed { i, part ->
            if (i > 0) Box(Modifier.size(3.dp).clip(CircleShape).background(MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.6f)))
            Text(part, style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
    }
}

/**
 * The page's buttons, as in Apple Music: a round Shuffle at the left, the one large Play in
 * the middle, and a round button of the page's own at the right. Pressed, each one squares
 * its corners a little (Material 3 Expressive's shape morph).
 */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun PlayRow(songs: List<Song>, side: (@Composable () -> Unit)? = null) {
    val haptics = rememberHaptics()
    Row(
        Modifier.fillMaxWidth().padding(horizontal = Edge, vertical = 16.dp),
        horizontalArrangement = Arrangement.spacedBy(14.dp, Alignment.CenterHorizontally),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        RoundAction(Icons.Rounded.Shuffle, "Shuffle") { haptics(false); play(songs.shuffled()) }
        Button(
            onClick = { haptics(false); play(songs) },
            shapes = ButtonDefaults.shapes(),
            modifier = Modifier.height(64.dp).width(172.dp),
        ) {
            Icon(Icons.Rounded.PlayArrow, contentDescription = null, modifier = Modifier.size(30.dp))
            Spacer(Modifier.width(8.dp))
            Text("Play", style = MaterialTheme.typography.titleLarge)
        }
        if (side != null) side() else Spacer(Modifier.size(64.dp))
    }
}

@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun RoundAction(icon: androidx.compose.ui.graphics.vector.ImageVector, label: String, onClick: () -> Unit) {
    FilledTonalIconButton(onClick = onClick, shapes = IconButtonDefaults.shapes(), modifier = Modifier.size(64.dp)) {
        Icon(icon, contentDescription = label)
    }
}

private fun LazyListScope.songList(list: List<Song>, playing: String?, open: (Route) -> Unit, numbered: Boolean, remove: ((Int) -> Unit)? = null) {
    itemsIndexed(list, key = { i, s -> "$i-${s.id}" }) { i, song ->
        Box(Modifier.entrance(i)) {
            SongRow(song, playing == song.id, open, number = numbered, remove = remove?.let { r -> { r(i) } }) { play(list, i) }
        }
    }
}

/** Minutes, as the top of a page says them. */
private fun length(songs: List<Song>): String {
    val minutes = (songs.sumOf { it.duration } / 60).toInt()
    return if (minutes >= 60) "${minutes / 60} h ${minutes % 60} min" else "$minutes min"
}

// ---------- Album

@Composable
fun AlbumScreen(album: Album, open: (Route) -> Unit) {
    val songs by rememberLoaded(album.key) { albumSongs(album.key) }
    val others by rememberLoaded(album.artist) { artistAlbums(album.artist) }
    val list = songs.orEmpty()
    val playing = playingId()
    val state = rememberLazyListState()
    var menu by remember { mutableStateOf(false) }
    if (menu) AlbumSheet(album, open) { menu = false }
    // No cover: look for one online (Cover Art Archive), when online lookups are on.
    LaunchedEffect(album.key, list.isNotEmpty()) {
        val first = list.firstOrNull() ?: return@LaunchedEffect
        if (album.artwork == null && list.all { it.artwork == null }) {
            val found = withContext(Dispatchers.IO) {
                runCatching { if (core.playbackSettings().onlineMedia) core.fetchCover(first.id) else 0u }.getOrDefault(0u)
            }
            if (found > 0u) NeedleApp.instance.libraryVersion.value++
        }
    }
    val art = album.artwork ?: list.firstNotNullOfOrNull { it.artwork }
    val title = album.title.ifBlank { "Unknown album" }
    DetailFrame(art, title, { state.headerScroll() }, { menu = true }) {
        LazyColumn(Modifier.fillMaxSize(), state = state, contentPadding = PaddingValues(bottom = 32.dp + LocalBottomInset.current)) {
            item {
                DetailHeader(
                    art, title, album.artist,
                    listOfNotNull(if (list.size in 2..6) "EP" else "Album", album.year.takeIf { it > 0 }?.toString(), songs?.let { length(it) }),
                    { state.headerScroll() },
                    onSub = { open(Route.Artist(album.artist)) },
                    key = "album-${album.key}",
                )
            }
            if (list.isNotEmpty()) {
                item { PlayRow(list) { RoundAction(Icons.AutoMirrored.Rounded.PlaylistAdd, "Add to a playlist") { Ui.sheet.value = Sheet.AddToPlaylist(list.map { it.id }) } } }
            }
            if (songs == null) items(6) { SongPlaceholder() }
            songList(list, playing, open, numbered = true)
            if (list.isNotEmpty()) {
                item {
                    val year = album.year.takeIf { it > 0 }?.let { "Released $it\n" } ?: ""
                    Text(
                        year + songsAndLength(list),
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.padding(horizontal = Edge, vertical = 20.dp),
                    )
                }
            }
            val more = others.orEmpty().filter { it.key != album.key }
            if (more.isNotEmpty()) {
                item { SectionHeader("More by ${album.artist}", onMore = { open(Route.Artist(album.artist)) }) }
                item {
                    LazyRow(contentPadding = PaddingValues(horizontal = 16.dp), horizontalArrangement = Arrangement.spacedBy(14.dp)) {
                        items(more, key = { it.key }) { a -> AlbumTile(a, Modifier.width(150.dp), open) { open(Route.AlbumPage(a)) } }
                    }
                }
            }
        }
    }
}

@Composable
fun AlbumOfScreen(route: Route.AlbumOf, open: (Route) -> Unit) {
    val album by rememberLoaded(route.songId) { albumOf(route.songId) }
    album?.let { AlbumScreen(it, open) }
}

// ---------- Playlist

@Composable
fun PlaylistScreen(route: Route.Playlist, open: (Route) -> Unit) {
    val detail by rememberLoaded(route.id) { playlistDetail(route.id) }
    val summary by rememberLoaded(route.id) { playlists().firstOrNull { it.id == route.id } }
    val playing = playingId()
    val state = rememberLazyListState()
    var deleting by remember { mutableStateOf(false) }
    var menu by remember { mutableStateOf(false) }
    val d = detail
    val list = d?.songs.orEmpty()
    val art = summary?.artwork ?: list.firstNotNullOfOrNull { it.artwork }
    val name = d?.name ?: route.name
    DetailFrame(art, name, { state.headerScroll() }, { menu = true }) {
        LazyColumn(Modifier.fillMaxSize(), state = state, contentPadding = PaddingValues(bottom = 32.dp + LocalBottomInset.current)) {
            item {
                DetailHeader(
                    art, name, d?.description?.ifBlank { null },
                    listOfNotNull(if (d?.rule != null) "Smart playlist" else "Playlist", d?.let { count(list.size, "song") }, d?.takeIf { list.isNotEmpty() }?.let { length(list) }),
                    { state.headerScroll() },
                )
            }
            if (d != null) {
                item { PlayRow(list) { RoundAction(Icons.Rounded.Edit, "Edit") { Ui.sheet.value = Sheet.EditPlaylist(d.id) } } }
            }
            if (detail == null) items(6) { SongPlaceholder() }
            if (d != null && list.isEmpty()) {
                item {
                    Column(Modifier.fillMaxWidth().padding(32.dp), horizontalAlignment = Alignment.CenterHorizontally) {
                        ShapedIcon(Icons.AutoMirrored.Rounded.PlaylistAdd, 64)
                        Text(
                            if (d.rule != null) "No songs match its rule yet." else "Add songs from any song's menu: Add to a playlist.",
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                            textAlign = TextAlign.Center,
                            modifier = Modifier.padding(top = 16.dp),
                        )
                    }
                }
            }
            songList(list, playing, open, numbered = false, remove = if (d?.rule == null) { i ->
                runCatching { core.removeFromPlaylist(route.id, i.toUInt()) }
                NeedleApp.instance.libraryVersion.value++
            } else null)
        }
        if (menu && d != null) PlaylistSheet(d, art, onDelete = { deleting = true }) { menu = false }
    }
    if (deleting && d != null) {
        AlertDialog(
            onDismissRequest = { deleting = false },
            title = { Text("Delete ${d.name}?") },
            text = { Text("The playlist goes. Its songs stay in your library.") },
            confirmButton = {
                TextButton(onClick = {
                    runCatching { core.deletePlaylist(d.id) }
                    NeedleApp.instance.libraryVersion.value++
                    deleting = false
                    showMessage("Deleted ${d.name}")
                }) { Text("Delete") }
            },
            dismissButton = { TextButton(onClick = { deleting = false }) { Text("Cancel") } },
        )
    }
}

// ---------- Artist

/**
 * An artist: their photo across the whole top, their name large over its foot, as Apple Music
 * and Spotify show artists. Then top songs and albums.
 */
@Composable
fun ArtistScreen(name: String, open: (Route) -> Unit) {
    val albums by rememberLoaded(name) { artistAlbums(name) }
    val top by rememberLoaded(name) { artistSongs(name) }
    val photo by rememberLoaded(name) { artistPhoto(name) }
    val playing = playingId()
    val songs = top.orEmpty()
    val state = rememberLazyGridState()
    val art = photo ?: albums?.firstNotNullOfOrNull { it.artwork }
    val shown = name.ifBlank { "Unknown artist" }
    DetailFrame(art, shown, { state.headerScroll() }, null) {
        LazyVerticalGrid(
            columns = GridCells.Fixed(2),
            state = state,
            contentPadding = PaddingValues(bottom = 32.dp + LocalBottomInset.current),
            horizontalArrangement = Arrangement.spacedBy(14.dp),
            verticalArrangement = Arrangement.spacedBy(18.dp),
            modifier = Modifier.fillMaxSize(),
        ) {
            full { ArtistHero(art, shown, albums?.size, songs.size, { state.headerScroll() }) }
            if (songs.isNotEmpty()) {
                full {
                    PlayRow(songs) {
                        RoundAction(Icons.Rounded.Radio, "Artist radio") {
                            showMessage("Starting radio from $name…")
                            NeedleApp.instance.scope.launch { runCatching { core.startArtistRadio(name) } }
                        }
                    }
                }
                full { SectionHeader("Top songs", Modifier.padding(top = 0.dp)) }
                items(minOf(5, songs.size), span = { GridItemSpan(maxLineSpan) }) { i ->
                    val song = songs[i]
                    Row(Modifier.entrance(i), verticalAlignment = Alignment.CenterVertically) {
                        // The rank, as charts show it.
                        Text(
                            "${i + 1}",
                            style = MaterialTheme.typography.headlineSmall,
                            color = MaterialTheme.colorScheme.primary,
                            textAlign = TextAlign.Center,
                            modifier = Modifier.padding(start = 8.dp).width(28.dp),
                        )
                        Box(Modifier.weight(1f)) { SongRow(song, playing == song.id, open, divider = false) { play(songs, i) } }
                    }
                }
            }
            full { SectionHeader("Albums", Modifier.padding(top = 8.dp)) }
            items(albums.orEmpty().size) { i ->
                val album = albums!![i]
                AlbumTile(album, Modifier.entrance(i + 5).padding(start = if (i % 2 == 0) Edge else 0.dp, end = if (i % 2 == 1) Edge else 0.dp), open) { open(Route.AlbumPage(album)) }
            }
        }
    }
}

@Composable
private fun ArtistHero(art: String?, name: String, albums: Int?, songs: Int, scrolled: () -> Float) {
    val density = androidx.compose.ui.platform.LocalDensity.current
    val squeeze by remember { derivedStateOf { (scrolled() / with(density) { 240.dp.toPx() }).coerceIn(0f, 1f) } }
    val page = MaterialTheme.colorScheme.background
    Box(Modifier.fillMaxWidth().aspectRatio(0.9f).clipToBounds()) {
        Cover(
            art,
            Modifier.fillMaxSize().graphicsLayer {
                val s = scrolled().coerceAtMost(3000f)
                translationY = s * 0.5f
                val k = 1f + s / 4000f
                scaleX = k
                scaleY = k
            },
            RoundedCornerShape(0.dp),
        )
        Box(Modifier.fillMaxSize().background(Brush.verticalGradient(0f to Color.Black.copy(alpha = 0.35f), 0.25f to Color.Transparent, 0.6f to Color.Transparent, 1f to page)))
        Column(Modifier.align(Alignment.BottomStart).padding(horizontal = Edge, vertical = 8.dp)) {
            Text(
                name,
                style = MaterialTheme.typography.displayMedium.copy(fontFamily = titleFamily(0.5f + squeeze * 0.5f)),
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
            )
            MetaLine(listOfNotNull("Artist", albums?.let { count(it, "album") }, songs.takeIf { it > 0 }?.let { count(it, "top song") }))
        }
    }
}

// ---------- Home

@Composable
fun HomeScreen(open: (Route) -> Unit) {
    val home by rememberLoaded { home() }
    val count by rememberLoaded { songCount() }
    val scan by NeedleApp.instance.scan.collectAsState()
    Page(greeting(), actions = {
        IconButton(onClick = { open(Route.Settings) }) { Icon(Icons.Rounded.Settings, contentDescription = "Settings") }
    }) { padding, scroll ->
        LazyColumn(Modifier.fillMaxSize().padding(padding).then(scroll), contentPadding = PaddingValues(bottom = 32.dp)) {
            if (count == 0u) {
                item { HomeEmpty(scan.running, open) }
                return@LazyColumn
            }
            val h = home ?: return@LazyColumn
            val lead = h.recent.ifEmpty { h.added }
            if (lead.isNotEmpty()) {
                item { SectionHeader(if (h.recent.isNotEmpty()) "Jump back in" else "Recently added", Modifier.padding(top = 0.dp)) }
                item { Hero(lead, open) }
            }
            item {
                Row(Modifier.fillMaxWidth().padding(horizontal = 16.dp).padding(top = 20.dp), horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                    Pinned(Icons.Rounded.AutoAwesome, "Your year", "Told back", Modifier.weight(1f)) { open(Route.Wrapped(null)) }
                    Pinned(Icons.Rounded.Computer, "Computer", "Play from there", Modifier.weight(1f)) { open(Route.Connect) }
                }
            }
            if (h.recent.isNotEmpty()) shelf("Recently added", h.added, open)
            shelf("Most played", h.mostPlayed, open)
        }
    }
}

/** Home's title: a greeting for the time of day. */
@Composable
private fun greeting(): String {
    val hour = remember { java.time.LocalTime.now().hour }
    return when (hour) {
        in 5..11 -> "Good morning"
        in 12..17 -> "Good afternoon"
        in 18..22 -> "Good evening"
        else -> "Up late"
    }
}

/** The first shelf: large covers in Material's carousel, the name over each. */
@Composable
private fun Hero(albums: List<Album>, open: (Route) -> Unit) {
    val state = rememberCarouselState { albums.size }
    HorizontalMultiBrowseCarousel(
        state = state,
        preferredItemWidth = 280.dp,
        itemSpacing = 10.dp,
        contentPadding = PaddingValues(horizontal = 16.dp),
        modifier = Modifier.fillMaxWidth().height(300.dp),
    ) { i ->
        val album = albums[i]
        Box(Modifier.fillMaxSize().maskClip(RoundedCornerShape(28.dp)).clickable { open(Route.AlbumPage(album)) }) {
            Cover(album.artwork, Modifier.fillMaxSize(), RoundedCornerShape(0.dp))
            Box(Modifier.fillMaxSize().background(Brush.verticalGradient(0.6f to Color.Transparent, 1f to Color.Black.copy(alpha = 0.7f))))
            // The name shows only on the large card; the narrow ones are just covers.
            val info = carouselItemDrawInfo
            Column(Modifier.align(Alignment.BottomStart).padding(16.dp).graphicsLayer { alpha = ((info.size / info.maxSize - 0.8f) / 0.2f).coerceIn(0f, 1f) }) {
                Text(album.title, color = Color.White, style = MaterialTheme.typography.titleLarge, fontWeight = FontWeight.Bold, maxLines = 1, overflow = TextOverflow.Ellipsis)
                Text(album.artist, color = Color.White.copy(alpha = 0.8f), style = MaterialTheme.typography.bodyMedium, maxLines = 1, overflow = TextOverflow.Ellipsis)
            }
        }
    }
}

private fun LazyListScope.shelf(title: String, albums: List<Album>, open: (Route) -> Unit) {
    if (albums.isEmpty()) return
    item { SectionHeader(title, onMore = { open(Route.Albums) }) }
    item {
        LazyRow(contentPadding = PaddingValues(horizontal = 16.dp), horizontalArrangement = Arrangement.spacedBy(14.dp)) {
            items(albums, key = { it.key }) { album -> AlbumTile(album, Modifier.width(150.dp), open) { open(Route.AlbumPage(album)) } }
        }
    }
}

@Composable
private fun HomeEmpty(scanning: Boolean, open: (Route) -> Unit) {
    Column(Modifier.fillMaxWidth().padding(horizontal = 32.dp, vertical = 48.dp), horizontalAlignment = Alignment.CenterHorizontally) {
        ShapedIcon(Icons.Rounded.AutoAwesome, 72)
        Text(
            if (scanning) "Reading your music…" else "Your music, on your phone",
            style = MaterialTheme.typography.headlineSmall,
            fontWeight = FontWeight.Bold,
            textAlign = TextAlign.Center,
            modifier = Modifier.padding(top = 20.dp),
        )
        Text(
            if (scanning) "Songs appear here as Needle finds them." else "Choose the folder your music is in. Needle reads it and keeps it up to date.",
            style = MaterialTheme.typography.bodyLarge,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            textAlign = TextAlign.Center,
            modifier = Modifier.padding(top = 8.dp, bottom = 24.dp),
        )
        if (!scanning) {
            Button(onClick = { open(Route.Settings) }) { Text("Choose a music folder") }
            val context = androidx.compose.ui.platform.LocalContext.current
            TextButton(onClick = {
                val folder = java.io.File(context.filesDir, "demo").absolutePath
                NeedleApp.instance.scope.launch(Dispatchers.IO) { runCatching { core.addDemoLibrary(folder) } }
            }) { Text("Or try a few demo songs") }
        }
    }
}

// ---------- Search

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SearchScreen(open: (Route) -> Unit) {
    var query by rememberSaveable { mutableStateOf("") }
    var songs by remember { mutableStateOf<List<Song>>(emptyList()) }
    var server by remember { mutableStateOf<Pair<ULong, List<Song>>?>(null) }
    val genres by rememberLoaded { genres() }
    val playing = playingId()
    LaunchedEffect(query) {
        delay(200)
        songs = if (query.isBlank()) emptyList() else withContext(Dispatchers.IO) { runCatching { core.search(query) }.getOrDefault(emptyList()) }
        // Music servers that search (octo-fiesta) answer a moment later.
        server = null
        if (query.isNotBlank()) {
            val number = withContext(Dispatchers.IO) { core.searchServers(query) }
            repeat(8) {
                delay(500)
                val found = withContext(Dispatchers.IO) { core.serverResults(number) }
                if (found.isNotEmpty()) { server = number to found; return@LaunchedEffect }
            }
        }
    }
    Page("Search") { padding, scroll ->
        LazyVerticalGrid(
            columns = GridCells.Fixed(2),
            contentPadding = PaddingValues(bottom = 32.dp),
            horizontalArrangement = Arrangement.spacedBy(12.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
            modifier = Modifier.fillMaxSize().padding(padding).then(scroll),
        ) {
            full {
                TextField(
                    value = query,
                    onValueChange = { query = it },
                    placeholder = { Text("Songs, albums, artists") },
                    leadingIcon = { Icon(Icons.Rounded.Search, contentDescription = null) },
                    trailingIcon = { if (query.isNotEmpty()) IconButton(onClick = { query = "" }) { Icon(Icons.Rounded.Close, contentDescription = "Clear") } },
                    singleLine = true,
                    keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search),
                    shape = RoundedCornerShape(28.dp),
                    colors = TextFieldDefaults.colors(
                        focusedIndicatorColor = Color.Transparent,
                        unfocusedIndicatorColor = Color.Transparent,
                        focusedContainerColor = MaterialTheme.colorScheme.surfaceContainerHigh,
                        unfocusedContainerColor = MaterialTheme.colorScheme.surfaceContainerHigh,
                    ),
                    modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp),
                )
            }
            if (query.isBlank()) {
                val list = genres.orEmpty()
                if (list.isNotEmpty()) {
                    full { SectionHeader("Browse by genre", Modifier.padding(top = 4.dp)) }
                    items(list.size, key = { list[it].name }) { i ->
                        GenreTile(list[i].name, list[i].artwork, Modifier.padding(start = if (i % 2 == 0) 16.dp else 0.dp, end = if (i % 2 == 1) 16.dp else 0.dp)) { open(Route.Genre(list[i].name)) }
                    }
                }
            } else {
                if (songs.isEmpty()) {
                    full { Text("Nothing found for “$query”.", color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.padding(horizontal = Edge, vertical = 16.dp)) }
                }
                items(songs.size, key = { songs[it].id }, span = { GridItemSpan(maxLineSpan) }) { i ->
                    SongRow(songs[i], playing == songs[i].id, open) { play(songs, i) }
                }
                server?.let { (number, found) ->
                    full { SectionHeader("On your server") }
                    items(found.size, key = { "server-" + found[it].id }, span = { GridItemSpan(maxLineSpan) }) { i ->
                        SongRow(found[i], playing == found[i].id, null) { core.playServerSongs(number, i.toUInt()) }
                    }
                }
            }
        }
    }
}

/** A genre as Spotify shows one: its colour from a cover of it, and the cover tilted in the corner. */
@Composable
internal fun GenreTile(name: String, art: String?, modifier: Modifier, onClick: () -> Unit) {
    val glow = coverColor(art)
    Box(
        modifier
            .fillMaxWidth()
            .heightIn(min = 100.dp)
            .clip(RoundedCornerShape(16.dp))
            .background(Brush.linearGradient(listOf(glow, glow.copy(alpha = 0.6f))))
            .clickable(onClick = onClick),
    ) {
        Text(name, color = Color.White, style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.Bold, modifier = Modifier.padding(14.dp))
        if (art != null) {
            Cover(
                art,
                Modifier
                    .align(Alignment.BottomEnd)
                    .offset(x = 14.dp, y = 10.dp)
                    .size(72.dp)
                    .graphicsLayer { rotationZ = 22f },
                RoundedCornerShape(8.dp),
            )
        }
    }
}
