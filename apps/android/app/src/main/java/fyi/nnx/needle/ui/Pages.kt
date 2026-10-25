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
import androidx.compose.foundation.layout.widthIn
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
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.foundation.border
import androidx.compose.material3.ButtonShapes
import androidx.compose.material3.ButtonGroupDefaults
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
import androidx.compose.foundation.combinedClickable
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.foundation.layout.wrapContentHeight
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.graphics.luminance
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.coerceIn
import androidx.compose.ui.unit.dp
import fyi.nnx.needle.NeedleApp
import fyi.nnx.needle.core.Album
import fyi.nnx.needle.core.Song
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

private fun play(songs: List<Song>, start: Int = 0) = playSongs(songs.map { it.id }, start)

@Composable
private fun playingId(): String? = playingSongId()

// ---------- Shared: album, playlist, and artist pages

/**
 * An album, playlist, or artist page: all of it in the cover's colours, the page running up
 * under the status bar. A bar with the name fades in once the top has scrolled away.
 */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun DetailFrame(art: String?, title: String, scrolled: () -> Float, menu: (() -> Unit)?, content: @Composable () -> Unit) {
    val app by NeedleApp.instance.app.collectAsState()
    CoverTheme(if (app.coverColors) art else null) {
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
fun DetailHeader(
    art: String?,
    title: String,
    sub: String?,
    meta: List<String>,
    scrolled: () -> Float,
    onSub: (() -> Unit)? = null,
    key: String? = null,
    mosaic: List<String> = emptyList(),
    /** The page's buttons, beside the cover on a tablet (on a phone they follow the header). */
    buttons: (@Composable () -> Unit)? = null,
) {
    val density = androidx.compose.ui.platform.LocalDensity.current
    val squeeze by remember { derivedStateOf { (scrolled() / with(density) { 260.dp.toPx() }).coerceIn(0f, 1f) } }
    if (isWide()) {
        WideDetailHeader(art, title, sub, meta, squeeze, onSub, key, mosaic, buttons)
        return
    }
    Box(Modifier.fillMaxWidth()) {
        CoverBackdrop(art, Modifier.matchParentSize())
        Column(
            Modifier.fillMaxWidth().statusBarsPadding().padding(top = 56.dp, start = Edge, end = Edge, bottom = 4.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            Mosaic(
                mosaic,
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
                seed = title,
            )
            Text(
                title,
                style = MaterialTheme.typography.displaySmall.copy(fontFamily = titleFamily(0.3f + squeeze * 0.7f)),
                textAlign = TextAlign.Center,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier.padding(top = 24.dp).semantics { heading() },
            )
            if (sub != null) {
                Text(
                    sub,
                    style = MaterialTheme.typography.titleMedium,
                    color = MaterialTheme.colorScheme.primary,
                    textAlign = TextAlign.Center,
                    // A full finger's height to tap, and a button to TalkBack.
                    modifier = Modifier
                        .then(if (onSub != null) Modifier.heightIn(min = 48.dp) else Modifier.padding(top = 2.dp))
                        // Rounded, not a circle: a description of three lines keeps its corners.
                        .clip(RoundedCornerShape(24.dp))
                        .then(if (onSub != null) Modifier.clickable(role = androidx.compose.ui.semantics.Role.Button, onClick = onSub) else Modifier)
                        .wrapContentHeight()
                        .padding(horizontal = 10.dp, vertical = 4.dp),
                )
            }
            if (meta.isNotEmpty()) MetaLine(meta, Modifier.padding(top = 6.dp))
        }
    }
}

/**
 * The top of an album or playlist page on a tablet, as Apple Music shows it there: the cover at
 * the left, and the name, the facts, and the buttons beside it, rather than a cover the width of
 * the screen.
 */
@Composable
private fun WideDetailHeader(
    art: String?,
    title: String,
    sub: String?,
    meta: List<String>,
    squeeze: Float,
    onSub: (() -> Unit)?,
    key: String?,
    mosaic: List<String>,
    buttons: (@Composable () -> Unit)?,
) {
    val cover = (LocalPageWidth.current * 0.28f).coerceIn(200.dp, 320.dp)
    Box(Modifier.fillMaxWidth()) {
        CoverBackdrop(art, Modifier.matchParentSize())
        Row(
            Modifier.fillMaxWidth().statusBarsPadding().padding(top = 64.dp, start = Edge + 8.dp, end = Edge, bottom = 8.dp),
            verticalAlignment = Alignment.Bottom,
            horizontalArrangement = Arrangement.spacedBy(32.dp),
        ) {
            Mosaic(
                mosaic,
                art,
                Modifier
                    .size(cover)
                    .then(if (key != null) Modifier.sharedCover(key) else Modifier)
                    .shadow(28.dp, CoverShape, ambientColor = Color.Black, spotColor = Color.Black),
                seed = title,
            )
            Column(Modifier.weight(1f)) {
                Text(
                    title,
                    style = MaterialTheme.typography.displayMedium.copy(fontFamily = titleFamily(0.3f + squeeze * 0.7f)),
                    maxLines = 3,
                    overflow = TextOverflow.Ellipsis,
                    modifier = Modifier.semantics { heading() },
                )
                if (sub != null) {
                    Text(
                        sub,
                        style = MaterialTheme.typography.titleLarge,
                        color = MaterialTheme.colorScheme.primary,
                        modifier = Modifier
                            .then(if (onSub != null) Modifier.heightIn(min = 48.dp) else Modifier.padding(top = 4.dp))
                            .offset(x = (-10).dp)
                            .clip(RoundedCornerShape(24.dp))
                            .then(if (onSub != null) Modifier.clickable(role = androidx.compose.ui.semantics.Role.Button, onClick = onSub) else Modifier)
                            .wrapContentHeight()
                            .padding(horizontal = 10.dp, vertical = 4.dp),
                    )
                }
                if (meta.isNotEmpty()) MetaLine(meta, Modifier.padding(top = 6.dp))
                buttons?.invoke()
            }
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
 * The page's buttons, calm as on the Mac: a filled Play pill in the page's colour, a glass
 * Shuffle pill beside it, and the page's own button (if any) as a round glass button.
 */
@Composable
fun PlayRow(songs: List<Song>, inline: Boolean = false, side: (@Composable () -> Unit)? = null) {
    val haptics = rememberHaptics()
    Row(
        // Inline: under the name beside a tablet's cover, from its left edge.
        if (inline) Modifier.padding(top = 20.dp) else Modifier.fillMaxWidth().padding(horizontal = Edge, vertical = 16.dp),
        horizontalArrangement = if (inline) Arrangement.spacedBy(12.dp) else Arrangement.spacedBy(12.dp, Alignment.CenterHorizontally),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        PlayPill(Modifier.weight(1f, fill = false).firstFocus()) { haptics(false); play(songs) }
        ShufflePill(Modifier.weight(1f, fill = false)) { haptics(false); play(songs.shuffled()) }
        side?.invoke()
    }
}

/** The page's one main button: Play, a pill filled in the page's colour. */
@Composable
fun PlayPill(modifier: Modifier = Modifier, onClick: () -> Unit) {
    Button(
        onClick = onClick,
        shape = CircleShape,
        contentPadding = PaddingValues(horizontal = 24.dp),
        modifier = modifier.height(50.dp).widthIn(min = 120.dp, max = 180.dp),
    ) {
        Icon(Icons.Rounded.PlayArrow, contentDescription = null, modifier = Modifier.size(24.dp))
        Spacer(Modifier.width(6.dp))
        Text("Play", style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.SemiBold)
    }
}

/** Shuffle beside Play: a pill of glass, the page showing through it. */
@Composable
fun ShufflePill(modifier: Modifier = Modifier, onClick: () -> Unit) {
    val scheme = MaterialTheme.colorScheme
    androidx.compose.material3.OutlinedButton(
        onClick = onClick,
        shape = CircleShape,
        colors = ButtonDefaults.outlinedButtonColors(containerColor = scheme.onSurface.copy(alpha = 0.08f), contentColor = scheme.onSurface),
        border = androidx.compose.foundation.BorderStroke(0.5.dp, scheme.onSurface.copy(alpha = 0.14f)),
        contentPadding = PaddingValues(horizontal = 22.dp),
        modifier = modifier.height(50.dp).widthIn(min = 120.dp, max = 180.dp),
    ) {
        Icon(Icons.Rounded.Shuffle, contentDescription = null, modifier = Modifier.size(22.dp))
        Spacer(Modifier.width(6.dp))
        Text("Shuffle", style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.SemiBold)
    }
}

/** A round glass button beside the pills, for the page's own action. */
@Composable
fun RoundAction(icon: androidx.compose.ui.graphics.vector.ImageVector, label: String, onClick: () -> Unit) {
    val scheme = MaterialTheme.colorScheme
    FilledTonalIconButton(
        onClick = onClick,
        shape = CircleShape,
        colors = IconButtonDefaults.filledTonalIconButtonColors(containerColor = scheme.onSurface.copy(alpha = 0.08f), contentColor = scheme.onSurface),
        modifier = Modifier.size(50.dp).border(0.5.dp, scheme.onSurface.copy(alpha = 0.14f), CircleShape),
    ) {
        Icon(icon, contentDescription = label, modifier = Modifier.size(22.dp))
    }
}

private fun LazyListScope.songList(
    list: List<Song>,
    playing: String?,
    open: (Route) -> Unit,
    numbered: Boolean,
    remove: ((Int) -> Unit)? = null,
    reorder: Reorder? = null,
) {
    // Keys that follow each song, so a row moved with a remote keeps its focus as it moves.
    val keys = songKeys(list)
    itemsIndexed(list, key = { i, _ -> keys[i] }) { i, song ->
        Box(Modifier.entrance(i).then(if (reorder != null) Modifier.reorderRow(reorder, i) else Modifier)) {
            SongRow(
                song, playing == song.id, open,
                number = numbered,
                remove = remove?.let { r -> { r(i) } },
                trailing = reorder?.let { r -> { DragHandle(r, i) } },
                place = Place(i, list.size),
            ) { play(list, i) }
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
    // Its menus, and its songs' menus, know they are on this album already.
    CompositionLocalProvider(LocalAlbumHere provides album.title) { AlbumPage(album, open) }
}

@Composable
private fun AlbumPage(album: Album, open: (Route) -> Unit) {
    val songsLoad = rememberLoaded(album.key) { albumSongs(album.key) }
    val songs by songsLoad
    val othersLoad = rememberLoaded(album.artist) { artistAlbums(album.artist) }
    val others by othersLoad
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
            if (found > 0u) NeedleApp.instance.libraryChanged()
        }
    }
    val art = album.artwork ?: list.firstNotNullOfOrNull { it.artwork }
    val title = album.title.ifBlank { "Unknown album" }
    val wide = isWide()
    val buttons: (@Composable () -> Unit)? = if (list.isEmpty()) null else {
        { PlayRow(list, inline = wide) { RoundAction(Icons.AutoMirrored.Rounded.PlaylistAdd, "Add to a playlist") { Ui.sheet.value = Sheet.AddToPlaylist(list.map { it.id }) } } }
    }
    DetailFrame(art, title, { state.headerScroll() }, { menu = true }) {
        LazyColumn(Modifier.fillMaxSize(), state = state, contentPadding = PaddingValues(bottom = 32.dp + LocalBottomInset.current)) {
            item {
                DetailHeader(
                    art, title, album.artist,
                    listOfNotNull(if (list.size in 2..6) "EP" else "Album", album.year.takeIf { it > 0 }?.toString(), songs?.let { length(it) }),
                    { state.headerScroll() },
                    onSub = { open(Route.Artist(album.artist)) },
                    key = "album-${album.key}",
                    buttons = buttons?.takeIf { wide },
                )
            }
            if (!wide && buttons != null) item { buttons() }
            if (songsLoad.loading) items(6) { SongPlaceholder() }
            // Its songs were given another album name (in their tags), or taken out.
            if (songs != null && list.isEmpty()) item { GoneNote("This album is no longer in your library.") }
            // No cover found: it may be a picture in the album's folder that Needle may not see yet.
            if (art == null && list.isNotEmpty()) item { FolderCoversAccess(Modifier.padding(horizontal = Edge, vertical = 8.dp)) }
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

@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun AlbumOfScreen(route: Route.AlbumOf, open: (Route) -> Unit) {
    val albumLoad = rememberLoaded(route.songId) { albumOf(route.songId) }
    val album by albumLoad
    album?.let { AlbumScreen(it, open) }
    // While it is found, and if there is none: the way back, and a word on what is happening.
    if (album == null) Column(Modifier.fillMaxSize().statusBarsPadding()) {
        LargeTitle(route.title.ifBlank { "Album" })
        if (albumLoad.loading) {
            androidx.compose.material3.LoadingIndicator(Modifier.align(Alignment.CenterHorizontally).padding(top = 32.dp))
        } else if (!albumLoad.failed) {
            Text("This song has no album in the library.", color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.padding(Edge))
        }
    }
}

// ---------- Playlist

@Composable
fun PlaylistScreen(route: Route.Playlist, open: (Route) -> Unit) {
    val back = androidx.activity.compose.LocalOnBackPressedDispatcherOwner.current
    val detailLoad = rememberLoaded(route.id) { playlistDetail(route.id) }
    val detail by detailLoad
    val summaryLoad = rememberLoaded(route.id) { playlists().firstOrNull { it.id == route.id } }
    val summary by summaryLoad
    val playing = playingId()
    val state = rememberLazyListState()
    var deleting by remember { mutableStateOf(false) }
    var menu by remember { mutableStateOf(false) }
    val d = detail
    // A song dragged to a new place stays there until the list is read again, rather than
    // going back to its old place for a moment.
    var moved by remember(d) { mutableStateOf<List<Song>?>(null) }
    val list = moved ?: d?.songs.orEmpty()
    val context = androidx.compose.ui.platform.LocalContext.current
    val picture = androidx.activity.compose.rememberLauncherForActivityResult(androidx.activity.result.contract.ActivityResultContracts.OpenDocument()) { uri ->
        pickedFile(context, uri, { core.setPlaylistPicture(route.id, it) }) {
            NeedleApp.instance.libraryChanged()
        }
    }
    val export = androidx.activity.compose.rememberLauncherForActivityResult(androidx.activity.result.contract.ActivityResultContracts.CreateDocument("audio/x-mpegurl")) { uri ->
        uri ?: return@rememberLauncherForActivityResult
        NeedleApp.instance.scope.launch(Dispatchers.IO) {
            runCatching {
                val file = java.io.File(context.cacheDir, "export.m3u8")
                core.exportPlaylist(route.id, file.absolutePath)
                context.contentResolver.openOutputStream(uri)!!.use { out -> file.inputStream().use { it.copyTo(out) } }
                file.delete()
            }.onSuccess { showMessage("Exported") }.onFailure { showMessage(it.message ?: "Could not export it") }
        }
    }
    // A change on its way to the library: the list's places are the old ones until it is read
    // again, so no other change goes until then (or a moment, if it reads the same).
    var writing by remember { mutableStateOf(false) }
    LaunchedEffect(d) { writing = false }
    // Should the list not come back changed (it could not be read), the page goes back to the
    // order it has, with its places, after a while: the two always agree.
    LaunchedEffect(writing) {
        if (writing) {
            delay(10_000)
            writing = false
            moved = null
        }
    }
    val written: (Unit) -> Unit = {}
    // Put in order: each row gets a handle to drag it by.
    var arranging by remember { mutableStateOf(false) }
    val reorder = rememberReorder(list.size) { from, to ->
        val positions = d?.positions.orEmpty()
        // One change at a time: until the list is read again, its places are the old ones.
        if (!writing && from in positions.indices && to in positions.indices) {
            writing = true
            moved = list.toMutableList().apply { if (from in indices && to in indices) add(to, removeAt(from)) }
            libraryWrite({ core.moveInPlaylist(route.id, positions[from], positions[to]) }, { writing = false; moved = null; showMessage(it.message ?: "Could not reorder it") }, written)
        }
    }
    val art = summary?.artwork ?: list.firstNotNullOfOrNull { it.artwork }
    val name = d?.name ?: route.name
    val wide = isWide()
    // No Play or Shuffle for a playlist with nothing in it (its menu still edits it).
    val buttons: (@Composable () -> Unit)? = if (d == null || list.isEmpty()) null else {
        { PlayRow(list, inline = wide) { RoundAction(Icons.Rounded.Edit, "Edit") { Ui.sheet.value = Sheet.EditPlaylist(d.id) } } }
    }
    DetailFrame(art, name, { state.headerScroll() }, { menu = true }) {
        LazyColumn(Modifier.fillMaxSize(), state = state, contentPadding = PaddingValues(bottom = 32.dp + LocalBottomInset.current)) {
            item {
                DetailHeader(
                    art, name, d?.description?.ifBlank { null },
                    listOfNotNull(if (d?.rule != null) "Smart playlist" else "Playlist", d?.let { count(list.size, "song") }, d?.takeIf { list.isNotEmpty() }?.let { length(list) }),
                    { state.headerScroll() },
                    mosaic = summary?.mosaic.orEmpty(),
                    buttons = buttons?.takeIf { wide },
                )
            }
            if (d != null) {
                if (!wide && buttons != null) item { buttons() }
                if (arranging) item {
                    Row(Modifier.fillMaxWidth().padding(horizontal = Edge), verticalAlignment = Alignment.CenterVertically) {
                        Text("Drag the handles to put songs in order", style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.weight(1f))
                        TextButton(onClick = { arranging = false }) { Text("Done") }
                    }
                }
            }
            if (detailLoad.loading) items(6) { SongPlaceholder() }
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
                if (!writing) d?.positions?.getOrNull(i)?.let { position ->
                    writing = true
                    libraryWrite({ core.removeFromPlaylist(route.id, position) }, { writing = false; showMessage(it.message ?: "Could not remove it") }, written)
                }
            } else null, reorder = if (d?.rule == null && arranging) reorder else null)
        }
        if (menu && d != null) PlaylistSheet(
            d, art,
            onDelete = { deleting = true },
            onPicture = { tryOpen("file picker") { picture.launch(arrayOf("image/*")) } },
            onCovers = { libraryWrite({ core.setPlaylistPicture(d.id, null) }) },
            onExport = { tryOpen("file picker") { export.launch("${d.name}.m3u8") } },
            onArrange = if (d.rule == null && list.size > 1) ({ arranging = true }) else null,
        ) { menu = false }
    }
    if (deleting && d != null) {
        AlertDialog(
            onDismissRequest = { deleting = false },
            title = { Text("Delete ${d.name}?") },
            text = { Text("The playlist goes. Its songs stay in your library.") },
            confirmButton = {
                TextButton(onClick = {
                    deleting = false
                    NeedleApp.instance.scope.launch(kotlinx.coroutines.Dispatchers.Main) {
                        withContext(Dispatchers.IO) { runCatching { core.deletePlaylist(d.id) } }
                            .onSuccess {
                                // Away from the page first, and the lists told only once it has
                                // gone: a playlist that is gone has nothing to show, and would say so.
                                // Songs chosen on the page would take the Back for themselves.
                                Selection.clear()
                                back?.onBackPressedDispatcher?.onBackPressed()
                                delay(400)
                                NeedleApp.instance.libraryChanged()
                                showMessage("Deleted ${d.name}")
                            }
                            .onFailure { showMessage("Could not delete it: ${it.message}") }
                    }
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
    val albumsLoad = rememberLoaded(name) { artistAlbums(name) }
    val albums by albumsLoad
    val topLoad = rememberLoaded(name) { artistSongs(name) }
    val top by topLoad
    // Read again when a picture of theirs is chosen or reset.
    var pictureVersion by remember { androidx.compose.runtime.mutableIntStateOf(0) }
    val photoLoad = rememberLoaded(name, pictureVersion) { artistPhoto(name) }
    val photo by photoLoad
    // Songs without an artist: nothing to look up online for them.
    val real = name.isNotBlank() && name != "Unknown artist"
    // From fanart.tv (with online lookups on), or chosen: a wide picture of them, and their logo.
    val bannerLoad = rememberLoaded(name, "banner", pictureVersion) { if (real) artistBanner(name) else null }
    val banner by bannerLoad
    val logoLoad = rememberLoaded(name, "logo", pictureVersion) { if (real) artistLogo(name) else null }
    val logo by logoLoad
    val playing = playingId()
    val songs = top.orEmpty()
    val state = rememberLazyGridState()
    val art = banner ?: photo ?: albums?.firstNotNullOfOrNull { it.artwork }
    val shown = name.ifBlank { "Unknown artist" }
    // The newest album with a year, for its own box under the buttons.
    val latest = albums?.filter { it.year > 0 }?.maxWithOrNull(compareBy({ it.year }, { it.title }))
    var pictures by remember { mutableStateOf(false) }
    ArtistPictures(name, pictures, { pictures = it }) { pictureVersion++ }
    // Their pictures can be chosen (More › Pictures), not for songs with no artist.
    DetailFrame(art, shown, { state.headerScroll() }, if (real) ({ pictures = true }) else null) {
        // Two covers across on a phone, more on a tablet; the rows and the photo run to the edges.
        KeepTop(state) {
        LazyVerticalGrid(
            columns = gridColumns(180.dp),
            state = state,
            contentPadding = PaddingValues(start = Edge, end = Edge, bottom = 32.dp + LocalBottomInset.current),
            horizontalArrangement = Arrangement.spacedBy(14.dp),
            verticalArrangement = Arrangement.spacedBy(LocalRows.current.grid),
            modifier = Modifier.fillMaxSize(),
        ) {
            edgeToEdge(Edge) {
                ArtistHero(art, logo, shown, albums?.size, songs.size, { state.headerScroll() }) {
                    if (songs.isNotEmpty()) ArtistButtons(
                        onShuffle = { play(songs.shuffled(), 0) },
                        onPlay = { play(songs, 0) },
                        onRadio = {
                            showMessage("Starting radio from $name…")
                            NeedleApp.instance.scope.launch { runCatching { core.startArtistRadio(name) }.onFailure { showMessage(it.message ?: "Could not start the radio") } }
                        },
                    )
                }
            }
            if (latest != null) edgeToEdge(Edge) { LatestRelease(latest, open) }
            if (songs.isNotEmpty()) {
                // The title and the songs as one group, as in a song list, not apart by the grid's
                // gap for covers; the grid's own gap above is space enough.
                edgeToEdge(Edge) {
                Column {
                SectionHeader("Top songs", top = 0.dp)
                repeat(minOf(5, songs.size)) { i ->
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
                        Box(Modifier.weight(1f)) { SongRow(song, playing == song.id, open, place = Place(i, minOf(5, songs.size))) { play(songs, i) } }
                    }
                }
                }
                }
            }
            // Not for songs with no artist in their tags: their page is simply empty.
            if (name.isNotBlank() && top != null && albums != null && songs.isEmpty() && albums.orEmpty().isEmpty()) {
                edgeToEdge(Edge) { GoneNote("This artist is no longer in your library.") }
            }
            if (albums.orEmpty().isNotEmpty()) edgeToEdge(Edge) { SectionHeader("Albums", top = 12.dp) }
            // One list, read once: a reload that fails leaves it, not a null, to the rows.
            val albumList = albums.orEmpty()
            items(albumList.size) { i ->
                val album = albumList[i]
                AlbumTile(album, Modifier.entrance(i + 5), open) { open(Route.AlbumPage(album)) }
            }
        }
        }
    }
}

/**
 * An artist's top, after Apple Music's (and Needle's on the Mac and iPhone): their wide picture
 * (or photo, or a cover of theirs) tall across the top, fading into the page; their logo (or
 * name) in the middle near its foot, and under it the page's buttons.
 */
@Composable
private fun ArtistHero(art: String?, logo: String?, name: String, albums: Int?, songs: Int, scrolled: () -> Float, buttons: @Composable () -> Unit) {
    val density = androidx.compose.ui.platform.LocalDensity.current
    val squeeze by remember { derivedStateOf { (scrolled() / with(density) { 240.dp.toPx() }).coerceIn(0f, 1f) } }
    val page = MaterialTheme.colorScheme.background
    // On a tablet a picture the width of the screen would be taller than the screen: a band instead.
    Box(Modifier.fillMaxWidth().aspectRatio(if (isWide()) 2.1f else 0.72f).clipToBounds()) {
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
            // No photo: the made-up cover in the artist's own colours, as their tiles have.
            seed = name,
        )
        Box(Modifier.fillMaxSize().background(Brush.verticalGradient(0f to Color.Black.copy(alpha = 0.35f), 0.2f to Color.Transparent, 0.45f to Color.Transparent, 0.75f to page.copy(alpha = 0.75f), 1f to page)))
        Column(
            Modifier.align(Alignment.BottomCenter).fillMaxWidth().padding(horizontal = Edge, vertical = 4.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.spacedBy(10.dp),
        ) {
            if (logo != null) {
                // Their own logo, as Apple Music shows it. Logos are mostly white: on a light
                // page, the logo takes the page's ink.
                val light = page.luminance() > 0.5f
                coil3.compose.AsyncImage(
                    model = fileUri(logo),
                    contentDescription = name,
                    contentScale = androidx.compose.ui.layout.ContentScale.Fit,
                    colorFilter = if (light) androidx.compose.ui.graphics.ColorFilter.tint(MaterialTheme.colorScheme.onBackground) else null,
                    modifier = Modifier.widthIn(max = 300.dp).heightIn(max = 120.dp).fillMaxWidth(),
                )
            } else {
                Text(
                    name,
                    style = MaterialTheme.typography.displayMedium.copy(fontFamily = titleFamily(0.5f + squeeze * 0.5f)),
                    textAlign = TextAlign.Center,
                    maxLines = 2,
                    overflow = TextOverflow.Ellipsis,
                    modifier = Modifier.semantics { heading() },
                )
            }
            MetaLine(listOfNotNull("Artist", albums?.let { count(it, "album") }, songs.takeIf { it > 0 }?.let { count(it, "top song") }))
            buttons()
        }
    }
}

/** Play and Shuffle pills, and a round Radio, as on the album pages. */
@Composable
private fun ArtistButtons(onShuffle: () -> Unit, onPlay: () -> Unit, onRadio: () -> Unit) {
    Row(
        Modifier.padding(top = 6.dp, bottom = 4.dp),
        horizontalArrangement = Arrangement.spacedBy(12.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        PlayPill(Modifier.firstFocus(), onPlay)
        ShufflePill(onClick = onShuffle)
        RoundAction(Icons.Rounded.Radio, "Artist radio", onRadio)
    }
}

/** Their newest album in a box of its own: its cover, year, name, and a Play. */
@Composable
private fun LatestRelease(album: Album, open: (Route) -> Unit) {
    Column(Modifier.padding(horizontal = Edge)) {
        Text("Latest release", style = MaterialTheme.typography.titleSmall, color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.padding(bottom = 8.dp))
        Row(
            Modifier
                .fillMaxWidth()
                .clip(RowShape)
                .clickable(onClickLabel = "Open ${album.title}") { open(Route.AlbumPage(album)) }
                .padding(vertical = 6.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(14.dp),
        ) {
            Cover(album.artwork, Modifier.size(84.dp), SmallCoverShape, seed = album.title + album.artist)
            Column(Modifier.weight(1f)) {
                Text("${album.year}", style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.onSurfaceVariant)
                Text(album.title.ifBlank { "Unknown album" }, style = MaterialTheme.typography.titleMedium, maxLines = 2, overflow = TextOverflow.Ellipsis)
                Text(count(album.songs.toInt(), "song"), style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
            RoundAction(Icons.Rounded.PlayArrow, "Play ${album.title}") {
                NeedleApp.instance.scope.launch(kotlinx.coroutines.Dispatchers.IO) {
                    runCatching { core.albumSongs(album.key).map { it.id } }.getOrNull()?.let { playSongs(it) }
                }
            }
        }
    }
}

// ---------- Home

/**
 * Home, as on the Mac: a greeting, the song playing now, then shelves of covers (albums played
 * lately, new, played most, loved) and the artists played most, each shelf with a grey line
 * under its title.
 */
@Composable
fun HomeScreen(open: (Route) -> Unit) {
    val homeLoad = rememberLoaded { homeShelves() }
    val home by homeLoad
    val countLoad = rememberLoaded { songCount() }
    val count by countLoad
    val scan by NeedleApp.instance.scan.collectAsState()
    val daysLoad = rememberLoaded { onThisDay(java.util.TimeZone.getDefault().getOffset(System.currentTimeMillis()) / 1000, 12u) }
    val days by daysLoad
    LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(bottom = 32.dp + LocalBottomInset.current)) {
        item {
            Row(
                Modifier.fillMaxWidth().padding(start = Edge, end = 8.dp, top = 20.dp, bottom = 8.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Text(
                    greeting(),
                    style = MaterialTheme.typography.headlineMedium.copy(fontFamily = titleFamily(0.5f)),
                    fontWeight = FontWeight.Bold,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                    modifier = Modifier.weight(1f).semantics { heading() },
                )
                // With the side bar, Settings is at its foot instead.
                if (!hasSideBar()) IconButton(onClick = { open(Route.Settings) }) { Icon(Icons.Rounded.Settings, contentDescription = "Settings") }
            }
        }
        if (count == 0u) {
            item { HomeEmpty(scan.running, scan.scanned, open) }
            return@LazyColumn
        }
        item { NowCard() }
        val h = home ?: return@LazyColumn
        shelf("Jump back in", "Albums you played last", h.recent, open)
        shelf("New in your library", "Added most recently", h.added, open, Route.RecentlyAdded)
        shelf("On repeat", "Your most played albums", h.mostPlayed, open)
        // What played on this day a year and five years ago.
        days.orEmpty().forEach { day ->
            shelf("On this day", if (day.yearsAgo == 1) "A year ago" else "${day.yearsAgo} years ago", day.albums, open)
        }
        shelf("Favorites", "Albums with the songs you love", h.favorites, open)
        artistShelf(h.artists, open)
    }
}

/** Home's title: a greeting for the time of day. */
@Composable
private fun greeting(): String {
    val hour = remember { java.time.LocalTime.now().hour }
    return when (hour) {
        in 5..11 -> "Good morning"
        in 12..17 -> "Good afternoon"
        else -> "Good evening"
    }
}

/**
 * The song playing, large, at the top of Home: its cover, "Playing now" in the accent, the
 * song, and a round Play/Pause, on a wash of the cover's colour.
 */
@Composable
private fun NowCard() {
    val playbackState = NeedleApp.instance.playback.collectAsState()
    val song by remember { derivedStateOf { playbackState.value?.current } }
    val playing by remember { derivedStateOf { playbackState.value?.playing == true } }
    val s = song ?: return
    val haptics = rememberHaptics()
    CoverTheme(s.artwork) {
        val scheme = MaterialTheme.colorScheme
        Row(
            Modifier
                .fillMaxWidth()
                .padding(horizontal = 16.dp, vertical = 8.dp)
                .clip(RoundedCornerShape(16.dp))
                .background(Brush.horizontalGradient(listOf(scheme.primaryContainer.copy(alpha = 0.55f), scheme.surfaceContainer.copy(alpha = 0.6f))))
                .clickable(onClickLabel = "Open the player") { Ui.showPlayer.tryEmit(Unit) }
                .padding(14.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(14.dp),
        ) {
            Cover(s.artwork, Modifier.size(76.dp).shadow(8.dp, CoverShape), CoverShape, seed = s.album + s.artist)
            Column(Modifier.weight(1f)) {
                Text(
                    if (playing) "Playing now" else "Paused",
                    style = MaterialTheme.typography.labelMedium,
                    fontWeight = FontWeight.SemiBold,
                    color = scheme.primary,
                )
                Text(s.title, style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.SemiBold, maxLines = 1, overflow = TextOverflow.Ellipsis)
                Text(
                    listOf(s.artist, s.album).filter { it.isNotBlank() }.joinToString(" · "),
                    style = MaterialTheme.typography.bodyMedium,
                    color = scheme.onSurfaceVariant,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
            androidx.compose.material3.FilledIconButton(
                onClick = { haptics(false); Controls.toggle() },
                shape = CircleShape,
                modifier = Modifier.size(52.dp).playPauseSemantics(playing) { haptics(false); Controls.toggle() },
            ) { PlayPauseIcon(playing, 28.dp) }
        }
    }
}

/** A shelf's title, with a grey line under it, and See all where there is a page of it. */
@Composable
internal fun ShelfTitle(title: String, detail: String?, onMore: (() -> Unit)? = null) {
    Row(
        Modifier.fillMaxWidth().padding(start = Edge, end = 8.dp, top = 26.dp, bottom = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(Modifier.weight(1f)) {
            Text(title, style = MaterialTheme.typography.titleLarge, fontWeight = FontWeight.SemiBold, modifier = Modifier.semantics { heading() })
            if (detail != null) Text(detail, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
        if (onMore != null) TextButton(onClick = onMore) { Text("See all") }
    }
}

private fun LazyListScope.shelf(title: String, detail: String, albums: List<Album>, open: (Route) -> Unit, more: Route? = null) {
    if (albums.isEmpty()) return
    // "See all" only where there is a page of the same albums in the same order.
    item { ShelfTitle(title, detail, more?.let { { open(it) } }) }
    item {
        val tile = if (isWide()) 176.dp else 148.dp
        LazyRow(contentPadding = PaddingValues(horizontal = Edge), horizontalArrangement = Arrangement.spacedBy(16.dp)) {
            items(albums, key = { it.key }) { album -> AlbumTile(album, Modifier.width(tile), open) { open(Route.AlbumPage(album)) } }
        }
    }
}

/** The artists played most, on Home: each in a circle, the name under it. */
private fun LazyListScope.artistShelf(artists: List<fyi.nnx.needle.core.Artist>, open: (Route) -> Unit) {
    if (artists.size < 2) return
    item { ShelfTitle("Artists you play most", null) { open(Route.Artists) } }
    item {
        LazyRow(contentPadding = PaddingValues(horizontal = Edge), horizontalArrangement = Arrangement.spacedBy(18.dp)) {
            items(artists, key = { it.name }) { a -> ArtistTile(a, Modifier.width(if (isWide()) 132.dp else 112.dp), open) }
        }
    }
}

@Composable
private fun HomeEmpty(scanning: Boolean, read: UInt, open: (Route) -> Unit) {
    Column(Modifier.fillMaxWidth().padding(horizontal = 32.dp, vertical = 48.dp), horizontalAlignment = Alignment.CenterHorizontally) {
        MusicAccessNotice(Modifier.padding(bottom = 24.dp))
        ShapedIcon(Icons.Rounded.AutoAwesome, 72)
        Text(
            if (scanning) "Reading your music…" else "Your music, on your phone",
            style = MaterialTheme.typography.headlineSmall,
            fontWeight = FontWeight.Bold,
            textAlign = TextAlign.Center,
            modifier = Modifier.padding(top = 20.dp),
        )
        Text(
            // The lists are read again when the scan ends, so say that, and how far it is.
            if (scanning) "Your songs show here once Needle has read them all. ${if (read > 0u) count(read.toInt(), "file") + " read so far." else ""}".trim() else "Choose the folder your music is in. Needle reads it and keeps it up to date.",
            style = MaterialTheme.typography.bodyLarge,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            textAlign = TextAlign.Center,
            modifier = Modifier.padding(top = 8.dp, bottom = 24.dp),
        )
        if (!scanning) {
            Button(onClick = { open(Route.Settings) }) { Text("Choose a music folder") }
        }
    }
}

// ---------- Search

/** The last search and what it found (see [SearchScreen]). */
private var lastSearch: Pair<String, List<Song>>? = null

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SearchScreen(open: (Route) -> Unit) {
    var query by rememberSaveable { mutableStateOf("") }
    val keyboard = androidx.compose.ui.platform.LocalSoftwareKeyboardController.current
    // Back on the page (from an album found), the last answer shows at once, so the list is
    // there to scroll back to; it is asked again quietly.
    val last = lastSearch?.takeIf { it.first == query }
    var songs by remember { mutableStateOf(last?.second ?: emptyList()) }
    // What the songs shown were found for, so "Nothing found" waits for its own answer.
    var searched by remember { mutableStateOf(last?.first) }
    var server by remember { mutableStateOf<Pair<ULong, List<Song>>?>(null) }
    val genresLoad = rememberLoaded { genres() }
    val genres by genresLoad
    val playing = playingId()
    LaunchedEffect(query) {
        delay(200)
        songs = if (query.isBlank()) emptyList() else withContext(Dispatchers.IO) { runCatching { core.search(query) }.getOrDefault(emptyList()) }
        searched = query
        lastSearch = query to songs
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
    // Off the main thread, as each letter is typed.
    val hints by produceState(emptyList<fyi.nnx.needle.core.Suggestion>(), query) {
        value = if (query.isBlank()) emptyList() else withContext(Dispatchers.IO) { runCatching { core.suggest(query, query.length.toUInt()) }.getOrDefault(emptyList()) }
    }
    Page("Search") { padding, scroll ->
        // Two genres across on a phone, more on a tablet; the rows run to the edges.
        LazyVerticalGrid(
            columns = gridColumns(210.dp),
            contentPadding = PaddingValues(start = 16.dp, end = 16.dp, bottom = 32.dp + LocalBottomInset.current),
            horizontalArrangement = Arrangement.spacedBy(12.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
            modifier = Modifier.fillMaxSize().padding(padding).then(scroll),
        ) {
            edgeToEdge(16.dp) {
                RemoteTyping(Modifier.fillMaxWidth().padding(horizontal = 16.dp)) { field ->
                TextField(
                    value = query,
                    onValueChange = { query = it },
                    placeholder = { Text("Songs, albums, artists") },
                    leadingIcon = { Icon(Icons.Rounded.Search, contentDescription = null) },
                    trailingIcon = { if (query.isNotEmpty()) IconButton(onClick = { query = "" }) { Icon(Icons.Rounded.Close, contentDescription = "Clear") } },
                    singleLine = true,
                    keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search),
                    // The results come as it is typed; the keyboard's Search puts the keyboard away.
                    keyboardActions = androidx.compose.foundation.text.KeyboardActions(onSearch = { keyboard?.hide() }),
                    shape = RoundedCornerShape(28.dp),
                    colors = TextFieldDefaults.colors(
                        focusedIndicatorColor = Color.Transparent,
                        unfocusedIndicatorColor = Color.Transparent,
                        focusedContainerColor = MaterialTheme.colorScheme.surfaceContainerHigh,
                        unfocusedContainerColor = MaterialTheme.colorScheme.surfaceContainerHigh,
                    ),
                    modifier = field.fillMaxWidth(),
                )
                }
            }
            // Typing a smart rule (rating >= 4, genre = "K-pop"): what can come next, with
            // the library's own names.
            if (hints.isNotEmpty()) edgeToEdge(16.dp) {
                androidx.compose.foundation.lazy.LazyRow(
                    contentPadding = PaddingValues(horizontal = 16.dp),
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    items(hints) { h ->
                        androidx.compose.material3.SuggestionChip(
                            onClick = { query = query.substring(0, h.start.toInt()) + h.insert + query.substring(h.end.toInt().coerceAtMost(query.length)) },
                            label = { Text(h.label) },
                        )
                    }
                }
            }
            if (query.isBlank()) {
                val list = genres.orEmpty()
                if (list.isNotEmpty()) {
                    edgeToEdge(16.dp) { SectionHeader("Browse by genre", Modifier.padding(top = 4.dp)) }
                    items(list.size, key = { list[it].name }) { i ->
                        GenreTile(list[i].name, list[i].artwork, Modifier) { open(Route.Genre(list[i].name)) }
                    }
                }
            } else {
                if (songs.isEmpty() && searched == query && server == null) {
                    edgeToEdge(16.dp) { Text("Nothing found for “$query”.", color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.padding(horizontal = Edge, vertical = 16.dp)) }
                }
                items(songs.size, key = { songs[it].id }, span = { GridItemSpan(maxLineSpan) }) { i ->
                    SongRow(songs[i], playing == songs[i].id, open, Modifier.bleed(16.dp), place = Place(i, songs.size)) { play(songs, i) }
                }
                server?.let { (number, found) ->
                    edgeToEdge(16.dp) { SectionHeader("On your server") }
                    items(found.size, key = { "server-$it-" + found[it].id }, span = { GridItemSpan(maxLineSpan) }) { i ->
                        SongRow(found[i], playing == found[i].id, null, Modifier.bleed(16.dp), place = Place(i, found.size)) {
                            core.playServerSongs(number, i.toUInt())
                            NeedleApp.instance.openPlayer.tryEmit(Unit)
                        }
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

/** A page whose album or artist has gone from the library (new tags, or taken out). */
@Composable
private fun GoneNote(text: String) {
    Text(
        text,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        textAlign = TextAlign.Center,
        modifier = Modifier.fillMaxWidth().padding(32.dp),
    )
}
