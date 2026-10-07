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
private enum class Sort(val label: String) { Recent("Recently added"), Title("Title"), Artist("Artist") }

/**
 * The library as one page, as Spotify and Apple Music do it: filter pills at the top, and the
 * music itself below, sorted as chosen. With no pill chosen: favorites and history, then what
 * came in lately.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun LibraryScreen(open: (Route) -> Unit) {
    var filter by rememberSaveable { mutableStateOf<Filter?>(null) }
    var sort by rememberSaveable { mutableStateOf(Sort.Recent) }
    val scan by NeedleApp.instance.scan.collectAsState()
    Page("Library", actions = {
        IconButton(onClick = { open(Route.Settings) }) { Icon(Icons.Rounded.Settings, contentDescription = "Settings") }
    }) { padding, scroll ->
        PullToRefreshBox(isRefreshing = scan.running, onRefresh = { core.rescan() }, modifier = Modifier.fillMaxSize().padding(padding)) {
            Column(Modifier.fillMaxSize()) {
                Pills(filter, { filter = it }, sort, { sort = it }, open)
                AnimatedContent(filter, transitionSpec = { fadeIn(tween(200)) togetherWith fadeOut(tween(120)) }, label = "filter") { f ->
                    when (f) {
                        null -> Overview(open, scroll)
                        Filter.Albums -> Albums(sort, open, scroll)
                        Filter.Artists -> Artists(open, scroll)
                        Filter.Songs -> Songs(sort, open, scroll)
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
        LazyRow(
            Modifier.weight(1f),
            contentPadding = PaddingValues(start = 16.dp, end = 8.dp),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            if (filter != null) {
                item {
                    // Clears the filter, as Spotify's ✕ does.
                    Box(Modifier.size(32.dp).clip(CircleShape).background(MaterialTheme.colorScheme.surfaceContainerHigh).clickable { choose(null) }, contentAlignment = Alignment.Center) {
                        Icon(Icons.Rounded.Close, contentDescription = "All", modifier = Modifier.size(18.dp))
                    }
                }
            }
            // Once one is chosen, only it shows, beside the ✕.
            val shown = if (filter == null) Filter.entries else listOf(filter)
            items(shown.size) { i ->
                val f = shown[i]
                FilterChip(
                    selected = filter == f,
                    onClick = { choose(if (filter == f) null else f) },
                    label = { Text(f.label) },
                    shape = RoundedCornerShape(50),
                    colors = FilterChipDefaults.filterChipColors(
                        selectedContainerColor = MaterialTheme.colorScheme.primary,
                        selectedLabelColor = MaterialTheme.colorScheme.onPrimary,
                    ),
                )
            }
            if (filter == null) item {
                Box {
                    FilterChip(selected = false, onClick = { more = true }, label = { Text("More") }, shape = RoundedCornerShape(50))
                    DropdownMenu(expanded = more, onDismissRequest = { more = false }, shape = RoundedCornerShape(20.dp)) {
                        DropdownMenuItem(text = { Text("Genres") }, leadingIcon = { Icon(Icons.Rounded.Style, null) }, onClick = { more = false; open(Route.Genres) })
                        DropdownMenuItem(text = { Text("Folders") }, leadingIcon = { Icon(Icons.Rounded.Folder, null) }, onClick = { more = false; open(Route.Folder(null)) })
                    }
                }
            }
        }
        if (filter == Filter.Albums || filter == Filter.Songs) {
            Box {
                IconButton(onClick = { sorting = true }) { Icon(Icons.AutoMirrored.Rounded.Sort, contentDescription = "Sort: ${sort.label}") }
                DropdownMenu(expanded = sorting, onDismissRequest = { sorting = false }, shape = RoundedCornerShape(20.dp)) {
                    Sort.entries.forEach { s ->
                        DropdownMenuItem(
                            text = { Text(s.label) },
                            trailingIcon = { if (s == sort) Icon(Icons.Rounded.Check, null, tint = MaterialTheme.colorScheme.primary) },
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
            Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                Pinned(Icons.Rounded.Favorite, "Favorites", favorites?.let { count(it, "song") }, Modifier.weight(1f)) { open(Route.Favorites) }
                Pinned(Icons.Rounded.History, "History", "What you played", Modifier.weight(1f)) { open(Route.History) }
            }
        }
        span { Text("Recently added", style = MaterialTheme.typography.titleLarge, fontWeight = FontWeight.Bold, modifier = Modifier.padding(top = 12.dp)) }
    }
}

/** A pinned place: its icon in the one icon shape, a name, and a line. */
@Composable
internal fun Pinned(icon: ImageVector, title: String, detail: String?, modifier: Modifier, onClick: () -> Unit) {
    val interaction = remember { MutableInteractionSource() }
    Row(
        modifier
            .pressScale(interaction)
            .clip(RoundedCornerShape(20.dp))
            .background(MaterialTheme.colorScheme.surfaceContainer)
            .clickable(interactionSource = interaction, indication = null, onClick = onClick)
            .padding(12.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        ShapedIcon(icon, 44)
        Column {
            Text(title, style = MaterialTheme.typography.titleSmall, fontWeight = FontWeight.Bold)
            Text(detail ?: " ", style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant, maxLines = 1, overflow = TextOverflow.Ellipsis)
        }
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
        items(artists.orEmpty(), key = { it.name }) { artist ->
            val photoLoad = rememberLoaded(artist.name) { artistPhoto(artist.name) }
            val photo by photoLoad
            Column(Modifier.clip(RoundedCornerShape(16.dp)).clickable { open(Route.Artist(artist.name)) }, horizontalAlignment = Alignment.CenterHorizontally) {
                Cover(photo ?: artist.artwork, Modifier.fillMaxWidth().aspectRatio(1f), CircleShape)
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
    }
}

@Composable
private fun Songs(sort: Sort, open: (Route) -> Unit, scroll: Modifier) {
    val songsLoad = rememberLoaded { songs() }
    val songs by songsLoad
    val playing = NeedleApp.instance.playback.collectAsState().value?.current?.id
    val list = songs?.let { sortedSongs(it, sort) }.orEmpty()
    androidx.compose.foundation.lazy.LazyColumn(Modifier.fillMaxSize().then(scroll), contentPadding = PaddingValues(bottom = 32.dp)) {
        if (songsLoad.loading) items(8) { SongPlaceholder() }
        items(list.size, key = { list[it].id }) { i ->
            val song = list[i]
            SongRow(song, playing == song.id, open) {
                core.play(list.map { it.id }, i.toUInt())
                NeedleApp.instance.openPlayer.tryEmit(Unit)
            }
        }
    }
}

@Composable
private fun Playlists(open: (Route) -> Unit, scroll: Modifier) {
    val playlistsLoad = rememberLoaded { playlists() }
    val playlists by playlistsLoad
    androidx.compose.foundation.lazy.LazyColumn(Modifier.fillMaxSize().then(scroll), contentPadding = PaddingValues(bottom = 32.dp)) {
        item {
            Row(
                Modifier.fillMaxWidth().clickable { Ui.sheet.value = Sheet.NewPlaylist(emptyList()) }.heightIn(min = LocalRows.current.height + 12.dp).padding(horizontal = 16.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(14.dp),
            ) {
                Box(Modifier.size(56.dp).clip(SmallCoverShape).background(MaterialTheme.colorScheme.surfaceContainerHigh), contentAlignment = Alignment.Center) {
                    Icon(Icons.Rounded.Add, contentDescription = null, tint = MaterialTheme.colorScheme.primary)
                }
                Text("New playlist", style = MaterialTheme.typography.bodyLarge, color = MaterialTheme.colorScheme.primary)
            }
        }
        items(playlists.orEmpty().size, key = { playlists!![it].id }) { i ->
            val playlist = playlists!![i]
            Row(
                Modifier.fillMaxWidth().clickable { open(Route.Playlist(playlist.id, playlist.name)) }.heightIn(min = LocalRows.current.height + 12.dp).padding(horizontal = 16.dp),
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
