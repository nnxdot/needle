package fyi.nnx.needle.ui

import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.GridItemSpan
import androidx.compose.foundation.lazy.grid.LazyGridScope
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.Sort
import androidx.compose.material.icons.rounded.Add
import androidx.compose.material.icons.rounded.ArrowDropDown
import androidx.compose.material.icons.rounded.Check
import androidx.compose.material.icons.rounded.Close
import androidx.compose.material.icons.rounded.Favorite
import androidx.compose.material.icons.rounded.Folder
import androidx.compose.material.icons.rounded.History
import androidx.compose.material.icons.rounded.Settings
import androidx.compose.material.icons.rounded.Style
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilterChip
import androidx.compose.material3.FilterChipDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.ui.unit.sp
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import fyi.nnx.needle.NeedleApp
import fyi.nnx.needle.core.Album
import fyi.nnx.needle.core.Song

private enum class Filter(val label: String) { Playlists("Playlists"), Albums("Albums"), Artists("Artists"), Songs("Songs") }
private enum class Sort(val label: String) {
    Recent("Recently added"), Title("Title"), Artist("Artist");

    /** The order songs take: without a date they were added, newest first is by title. */
    fun forSongs(songs: Boolean) = if (songs && this == Recent) Title else this
}

/**
 * The library as one page, as Spotify and Apple Music do it: filter pills at the top, and the
 * music itself below, sorted as chosen. With no pill chosen: favorites and history, then what
 * came in lately.
 */
@OptIn(ExperimentalMaterial3Api::class, androidx.compose.material3.ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun LibraryScreen(open: (Route) -> Unit) {
    var filter by rememberSaveable { mutableStateOf<Filter?>(null) }
    var sort by rememberSaveable { mutableStateOf(Sort.Recent) }
    val scan by NeedleApp.instance.scan.collectAsState()
    // Under the title, what the library holds: the flexible top bar's subtitle.
    val sizesLoad = rememberLoaded {
        listOf(count(albums().size, "album"), count(artists().size, "artist"), count(playlists().size, "playlist"))
    }
    val sizes by sizesLoad
    // With the side bar, Settings is at its foot instead.
    Page("Library", subtitle = sizes?.joinToString(" · "), actions = {
        if (!hasSideBar()) IconButton(onClick = { open(Route.Settings) }) { Icon(Icons.Rounded.Settings, contentDescription = "Settings") }
    }) { padding, scroll ->
        // Pulled down, it reads the music folders again; Material 3 Expressive's shape-changing
        // loading indicator shows while it does.
        val pull = androidx.compose.material3.pulltorefresh.rememberPullToRefreshState()
        PullToRefreshBox(
            isRefreshing = scan.running,
            onRefresh = { core.rescan() },
            state = pull,
            indicator = {
                androidx.compose.material3.pulltorefresh.PullToRefreshDefaults.LoadingIndicator(
                    state = pull,
                    isRefreshing = scan.running,
                    modifier = Modifier.align(Alignment.TopCenter),
                )
            },
            modifier = Modifier.fillMaxSize().padding(padding),
        ) {
            Column(Modifier.fillMaxSize()) {
                Pills(filter, { filter = it }, sort, { sort = it }, open)
                AnimatedContent(filter, transitionSpec = { fadeIn(tween(200)) togetherWith fadeOut(tween(120)) }, label = "filter") { f ->
                    when (f) {
                        null -> Overview(open, scroll)
                        Filter.Albums -> Albums(sort, open, scroll)
                        Filter.Artists -> Artists(open, scroll)
                        Filter.Songs -> Songs(sort.forSongs(true), open, scroll)
                        Filter.Playlists -> Playlists(open, scroll)
                    }
                }
            }
        }
    }
}

@Composable
private fun Pills(filter: Filter?, choose: (Filter?) -> Unit, sort: Sort, sortBy: (Sort) -> Unit, open: (Route) -> Unit) {
    var more by remember { mutableStateOf(false) }
    var sorting by remember { mutableStateOf(false) }
    Row(Modifier.fillMaxWidth().padding(bottom = 8.dp), verticalAlignment = Alignment.CenterVertically) {
        // Tonal pills, no outlines: the page's own tone a step up, the chosen one in amber.
        val pill = FilterChipDefaults.filterChipColors(
            containerColor = MaterialTheme.colorScheme.surfaceContainerHigh,
            labelColor = MaterialTheme.colorScheme.onSurface,
            iconColor = MaterialTheme.colorScheme.onSurfaceVariant,
            selectedContainerColor = MaterialTheme.colorScheme.primary,
            selectedLabelColor = MaterialTheme.colorScheme.onPrimary,
        )
        LazyRow(
            Modifier.weight(1f),
            contentPadding = PaddingValues(start = 16.dp, end = 8.dp),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
            // Pills keep a 48 dp place to touch; the ✕ beside them is centred on them, not set
            // at the top of that place.
            verticalAlignment = Alignment.CenterVertically,
        ) {
            if (filter != null) {
                item(key = "clear") {
                    // Clears the filter, as Spotify's ✕ does: a pill's size to see, a finger's to touch.
                    Box(
                        Modifier
                            .size(48.dp)
                            .clip(CircleShape)
                            .clickable(role = androidx.compose.ui.semantics.Role.Button, onClickLabel = "Show all") { choose(null) },
                        contentAlignment = Alignment.Center,
                    ) {
                        Box(Modifier.size(32.dp).clip(CircleShape).background(MaterialTheme.colorScheme.surfaceContainerHigh), contentAlignment = Alignment.Center) {
                            Icon(Icons.Rounded.Close, contentDescription = "Show all", modifier = Modifier.size(18.dp))
                        }
                    }
                }
            }
            // Once one is chosen, only it shows, beside the ✕. Each pill keeps its key, so the
            // chosen one is the same pill afterwards and a remote's focus stays on it.
            val shown = if (filter == null) Filter.entries else listOf(filter)
            items(shown.size, key = { shown[it].name }) { i ->
                val f = shown[i]
                FilterChip(
                    selected = filter == f,
                    onClick = { choose(if (filter == f) null else f) },
                    label = { Text(f.label) },
                    shape = RoundedCornerShape(50),
                    colors = pill,
                    border = null,
                )
            }
            if (filter == null) item(key = "more") {
                Box {
                    FilterChip(
                        selected = false,
                        onClick = { more = true },
                        label = { Text("More") },
                        // It opens a menu, so it says so, as Material's menu chips do.
                        trailingIcon = { Icon(Icons.Rounded.ArrowDropDown, contentDescription = null, modifier = Modifier.size(FilterChipDefaults.IconSize)) },
                        shape = RoundedCornerShape(50),
                        colors = pill,
                        border = null,
                    )
                    DropdownMenu(expanded = more, onDismissRequest = { more = false }, shape = RoundedCornerShape(20.dp)) {
                        DropdownMenuItem(text = { Text("Genres") }, leadingIcon = { Icon(Icons.Rounded.Style, null) }, onClick = { more = false; open(Route.Genres) })
                        DropdownMenuItem(text = { Text("Folders") }, leadingIcon = { Icon(Icons.Rounded.Folder, null) }, onClick = { more = false; open(Route.Folder(null)) })
                    }
                }
            }
        }
        if (filter == Filter.Albums || filter == Filter.Songs) {
            // Songs carry no date they were added, so they sort by name or artist only.
            val orders = if (filter == Filter.Songs) Sort.entries - Sort.Recent else Sort.entries
            val current = sort.forSongs(filter == Filter.Songs)
            Box {
                IconButton(onClick = { sorting = true }) { Icon(Icons.AutoMirrored.Rounded.Sort, contentDescription = "Sort: ${current.label}") }
                DropdownMenu(expanded = sorting, onDismissRequest = { sorting = false }, shape = RoundedCornerShape(20.dp)) {
                    orders.forEach { s ->
                        DropdownMenuItem(
                            text = { Text(s.label) },
                            trailingIcon = { if (s == current) Icon(Icons.Rounded.Check, null, tint = MaterialTheme.colorScheme.primary) },
                            onClick = { sorting = false; sortBy(s) },
                        )
                    }
                }
            }
        }
    }
}

private fun sorted(albums: List<Album>, sort: Sort) = when (sort) {
    Sort.Recent -> albums.sortedByDescending { it.addedAt }
    Sort.Title -> albums.sortedBy { it.title.lowercase() }
    Sort.Artist -> albums.sortedBy { it.artist.lowercase() }
}

private fun sortedSongs(songs: List<Song>, sort: Sort) = when (sort) {
    Sort.Recent -> songs
    Sort.Title -> songs.sortedBy { it.title.lowercase() }
    Sort.Artist -> songs.sortedBy { it.artist.lowercase() }
}

private fun LazyGridScope.span(content: @Composable () -> Unit) =
    item(span = { GridItemSpan(maxLineSpan) }) { content() }

/** Two covers across, with the page's margins. */
@Composable
private fun AlbumGrid(albums: List<Album>?, open: (Route) -> Unit, scroll: Modifier, loading: Boolean, header: LazyGridScope.() -> Unit = {}) {
    LazyVerticalGrid(
        columns = GridCells.Adaptive(160.dp),
        contentPadding = PaddingValues(start = 16.dp, end = 16.dp, top = 8.dp, bottom = 32.dp),
        horizontalArrangement = Arrangement.spacedBy(14.dp),
        verticalArrangement = Arrangement.spacedBy(LocalRows.current.grid),
        modifier = Modifier.fillMaxSize().then(scroll),
    ) {
        header()
        if (loading) items(6) { AlbumPlaceholder() }
        items(albums.orEmpty(), key = { it.key }) { album -> AlbumTile(album, open = open) { open(Route.AlbumPage(album)) } }
    }
}

@Composable
private fun Overview(open: (Route) -> Unit, scroll: Modifier) {
    val albumsLoad = rememberLoaded { albums() }
    val albums by albumsLoad
    val favoritesLoad = rememberLoaded { favorites().size }
    val favorites by favoritesLoad
    AlbumGrid(albums?.let { sorted(it, Sort.Recent).take(24) }, open, scroll, albumsLoad.loading) {
        span {
            // The two the same height, when one's words take two lines.
            Row(Modifier.height(androidx.compose.foundation.layout.IntrinsicSize.Min), horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                val colors = MaterialTheme.colorScheme
                Pinned(Icons.Rounded.Favorite, "Favorites", favorites?.let { count(it, "song") }, Modifier.weight(1f).fillMaxHeight(), colors.tertiaryContainer, colors.tertiary) { open(Route.Favorites) }
                Pinned(Icons.Rounded.History, "History", "What you played", Modifier.weight(1f).fillMaxHeight()) { open(Route.History) }
            }
        }
        span { Text("Recently added", style = MaterialTheme.typography.titleLargeEmphasized, modifier = Modifier.padding(top = 12.dp)) }
    }
}

/**
 * A pinned place: its icon in the one icon shape, a name, and a line, on a tone of its own
 * (Material's secondary or tertiary container), the icon's shape in the tone's strong colour.
 */
@Composable
internal fun Pinned(
    icon: ImageVector,
    title: String,
    detail: String?,
    modifier: Modifier,
    container: androidx.compose.ui.graphics.Color = MaterialTheme.colorScheme.secondaryContainer,
    accent: androidx.compose.ui.graphics.Color = MaterialTheme.colorScheme.secondary,
    onClick: () -> Unit,
) {
    val interaction = remember { MutableInteractionSource() }
    val text = androidx.compose.material3.contentColorFor(container)
    Row(
        modifier
            .pressScale(interaction)
            .clip(RoundedCornerShape(24.dp))
            .background(container)
            .clickable(interactionSource = interaction, indication = null, onClick = onClick)
            .padding(12.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        ShapedIcon(icon, 44, container = accent, content = androidx.compose.material3.contentColorFor(accent))
        Column {
            // Smaller rather than broken mid-word on a narrow phone.
            Text(
                title,
                style = MaterialTheme.typography.titleSmall,
                fontWeight = FontWeight.Bold,
                color = text,
                maxLines = 1,
                softWrap = false,
                autoSize = androidx.compose.foundation.text.TextAutoSize.StepBased(minFontSize = 10.sp, maxFontSize = MaterialTheme.typography.titleSmall.fontSize),
            )
            // Two lines on a narrow phone, so the words are not cut short.
            Text(detail ?: " ", style = MaterialTheme.typography.bodySmall, color = text.copy(alpha = 0.75f), maxLines = 2, overflow = TextOverflow.Ellipsis)
        }
    }
}

/** An artist: the photo (or a cover of theirs) in the artists' arch, the name under it. */
@Composable
internal fun ArtistTile(artist: fyi.nnx.needle.core.Artist, modifier: Modifier, open: (Route) -> Unit) {
    val photoLoad = rememberLoaded(artist.name, cache = false) { artistPhoto(artist.name) }
    val photo by photoLoad
    val interaction = remember { MutableInteractionSource() }
    Column(
        modifier.pressScale(interaction).clickable(interactionSource = interaction, indication = null) { open(Route.Artist(artist.name)) },
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Cover(photo ?: artist.artwork, Modifier.fillMaxWidth().aspectRatio(1f), artistShape(), seed = artist.name)
        Text(
            artist.name.ifBlank { "Unknown artist" },
            style = MaterialTheme.typography.bodyMedium,
            fontWeight = FontWeight.Medium,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.padding(top = 8.dp),
        )
    }
}

@Composable
private fun Albums(sort: Sort, open: (Route) -> Unit, scroll: Modifier) {
    val albumsLoad = rememberLoaded { albums() }
    val albums by albumsLoad
    AlbumGrid(albums?.let { sorted(it, sort) }, open, scroll, albumsLoad.loading)
}

@Composable
private fun Artists(open: (Route) -> Unit, scroll: Modifier) {
    val artistsLoad = rememberLoaded { artists() }
    val artists by artistsLoad
    LazyVerticalGrid(
        columns = GridCells.Adaptive(104.dp),
        contentPadding = PaddingValues(start = 16.dp, end = 16.dp, top = 8.dp, bottom = 32.dp),
        horizontalArrangement = Arrangement.spacedBy(12.dp),
        verticalArrangement = Arrangement.spacedBy(LocalRows.current.grid),
        modifier = Modifier.fillMaxSize().then(scroll),
    ) {
        items(artists.orEmpty(), key = { it.name }) { artist -> ArtistTile(artist, Modifier, open) }
    }
}

@Composable
private fun Songs(sort: Sort, open: (Route) -> Unit, scroll: Modifier) {
    val songsLoad = rememberLoaded { songs() }
    val songs by songsLoad
    val playing = playingSongId()
    // Sorted once per list and order, not at every redraw.
    val list = remember(songs, sort) { songs?.let { sortedSongs(it, sort) }.orEmpty() }
    androidx.compose.foundation.lazy.LazyColumn(Modifier.fillMaxSize().then(scroll), contentPadding = PaddingValues(bottom = 32.dp)) {
        if (songsLoad.loading) items(8) { SongPlaceholder() }
        items(list.size, key = { list[it].id }) { i ->
            val song = list[i]
            SongRow(song, playing == song.id, open, place = Place(i, list.size)) {
                playSongs(list.map { it.id }, i)
            }
        }
    }
}

@Composable
private fun Playlists(open: (Route) -> Unit, scroll: Modifier) {
    val playlistsLoad = rememberLoaded { playlists() }
    val playlists by playlistsLoad
    androidx.compose.foundation.lazy.LazyColumn(Modifier.fillMaxSize().then(scroll), contentPadding = PaddingValues(bottom = 32.dp)) {
        // A new playlist: Material's tonal button, above the group of playlists.
        item {
            androidx.compose.material3.FilledTonalButton(
                onClick = { Ui.sheet.value = Sheet.NewPlaylist(emptyList()) },
                modifier = Modifier.padding(start = 16.dp, top = 4.dp, bottom = 14.dp).heightIn(min = 48.dp),
            ) {
                Icon(Icons.Rounded.Add, contentDescription = null)
                androidx.compose.foundation.layout.Spacer(Modifier.size(8.dp))
                Text("New playlist")
            }
        }
        // One list, read once: a reload that fails leaves it, not a null, to the rows.
        val list = playlists.orEmpty()
        val count = list.size
        items(count, key = { list[it].id }) { i ->
            val playlist = list[i]
            Row(
                Modifier.fillMaxWidth().segment(Place(i, count), groupColor()).clickable { open(Route.Playlist(playlist.id, playlist.name)) }
                    .heightIn(min = LocalRows.current.height + 12.dp).padding(horizontal = 12.dp, vertical = 8.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(14.dp),
            ) {
                PlaylistCover(playlist, Modifier.size(56.dp), SmallCoverShape)
                Column(Modifier.weight(1f)) {
                    Text(playlist.name, style = MaterialTheme.typography.bodyLarge, maxLines = 1, overflow = TextOverflow.Ellipsis)
                    Text(
                        if (playlist.smart) "Smart playlist" else count(playlist.songs.toInt(), "song"),
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
            }
        }
    }
}
