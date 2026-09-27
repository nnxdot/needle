package fyi.nnx.needle.ui

import androidx.compose.foundation.background
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
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.GridItemSpan
import androidx.compose.foundation.lazy.grid.LazyGridScope
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.lazy.grid.itemsIndexed as gridItemsIndexed
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.QueueMusic
import androidx.compose.material.icons.rounded.Album
import androidx.compose.material.icons.rounded.Close
import androidx.compose.material.icons.rounded.MusicNote
import androidx.compose.material.icons.rounded.Person
import androidx.compose.material.icons.rounded.PlayArrow
import androidx.compose.material.icons.rounded.Search
import androidx.compose.material.icons.rounded.Settings
import androidx.compose.material.icons.rounded.Shuffle
import androidx.compose.material.icons.rounded.Style
import androidx.compose.material3.Button
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextField
import androidx.compose.material3.TextFieldDefaults
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
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.graphics.Color
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
import kotlinx.coroutines.withContext

private fun play(songs: List<Song>, start: Int = 0) {
    core.play(songs.map { it.id }, start.toUInt())
    NeedleApp.instance.openPlayer.tryEmit(Unit)
}

private fun shuffle(songs: List<Song>) = play(songs.shuffled())

@Composable
private fun currentId(): String? = NeedleApp.instance.playback.collectAsState().value?.current?.id

// ---------- Home

@Composable
fun HomeScreen(open: (Route) -> Unit) {
    val home by rememberLoaded { home() }
    val count by rememberLoaded { songCount() }
    val scan by NeedleApp.instance.scan.collectAsState()
    LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(bottom = 24.dp)) {
        item {
            LargeTitle("Home") {
                IconButton(onClick = { open(Route.Settings) }) {
                    Icon(Icons.Rounded.Settings, contentDescription = "Settings")
                }
            }
        }
        if (count == 0u) {
            item { EmptyLibrary(scan.running, open) }
            return@LazyColumn
        }
        val h = home ?: return@LazyColumn
        // The first shelf is larger, as Apple Music leads with its top picks.
        val lead = h.recent.ifEmpty { h.added }
        albumShelf(if (h.recent.isNotEmpty()) "Jump back in" else "Recently added", lead, 220, open)
        if (h.recent.isNotEmpty()) albumShelf("Recently added", h.added, 150, open)
        albumShelf("Most played", h.mostPlayed, 150, open)
    }
}

private fun LazyListScope.albumShelf(title: String, albums: List<Album>, size: Int, open: (Route) -> Unit) {
    if (albums.isEmpty()) return
    item { SectionHeader(title, onMore = { open(Route.Albums) }) }
    item {
        LazyRow(
            contentPadding = PaddingValues(horizontal = Edge),
            horizontalArrangement = Arrangement.spacedBy(14.dp),
        ) {
            items(albums, key = { it.key }) { album ->
                AlbumTile(album, Modifier.width(size.dp)) { open(Route.AlbumPage(album)) }
            }
        }
    }
}

@Composable
private fun EmptyLibrary(scanning: Boolean, open: (Route) -> Unit) {
    Column(
        Modifier.fillMaxWidth().padding(horizontal = 32.dp, vertical = 64.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Icon(
            Icons.Rounded.MusicNote,
            contentDescription = null,
            tint = MaterialTheme.colorScheme.primary,
            modifier = Modifier.size(56.dp),
        )
        Text(
            if (scanning) "Reading your music…" else "Your music, on your phone",
            style = MaterialTheme.typography.headlineSmall,
            fontWeight = FontWeight.Bold,
            textAlign = TextAlign.Center,
            modifier = Modifier.padding(top = 20.dp),
        )
        Text(
            if (scanning) "Songs appear here as Needle finds them."
            else "Choose the folder your music is in. Needle reads it and keeps it up to date.",
            style = MaterialTheme.typography.bodyLarge,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            textAlign = TextAlign.Center,
            modifier = Modifier.padding(top = 8.dp, bottom = 24.dp),
        )
        if (!scanning) Button(onClick = { open(Route.Settings) }) { Text("Choose a music folder") }
    }
}

// ---------- Library

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun LibraryScreen(open: (Route) -> Unit) {
    val home by rememberLoaded { home() }
    val scan by NeedleApp.instance.scan.collectAsState()
    // Pull down to read the music folders again.
    PullToRefreshBox(isRefreshing = scan.running, onRefresh = { core.rescan() }, modifier = Modifier.fillMaxSize()) {
    LazyVerticalGrid(
        columns = GridCells.Fixed(2),
        contentPadding = PaddingValues(bottom = 24.dp),
        horizontalArrangement = Arrangement.spacedBy(16.dp),
        verticalArrangement = Arrangement.spacedBy(20.dp),
        modifier = Modifier.fillMaxSize(),
    ) {
        full {
            LargeTitle("Library") {
                IconButton(onClick = { open(Route.Settings) }) { Icon(Icons.Rounded.Settings, contentDescription = "Settings") }
            }
        }
        full {
            Column {
                NavRow(Icons.AutoMirrored.Rounded.QueueMusic, "Playlists") { open(Route.Playlists) }
                NavRow(Icons.Rounded.Person, "Artists") { open(Route.Artists) }
                NavRow(Icons.Rounded.Album, "Albums") { open(Route.Albums) }
                NavRow(Icons.Rounded.MusicNote, "Songs") { open(Route.Songs) }
                NavRow(Icons.Rounded.Style, "Genres") { open(Route.Genres) }
            }
        }
        val added = home?.added.orEmpty()
        if (added.isNotEmpty()) {
            full { SectionHeader("Recently added", Modifier.padding(top = 4.dp)) }
            albumGrid(added, open)
        }
    }
    }
}

private fun LazyGridScope.full(content: @Composable () -> Unit) =
    item(span = { GridItemSpan(maxLineSpan) }) { content() }

/** Albums two to a row, with the page's side margins; soft placeholders while they load. */
private fun LazyGridScope.albumGrid(albums: List<Album>?, open: (Route) -> Unit) {
    if (albums == null) {
        items(6) { i ->
            AlbumPlaceholder(Modifier.padding(start = if (i % 2 == 0) Edge else 0.dp, end = if (i % 2 == 1) Edge else 0.dp))
        }
        return
    }
    gridItemsIndexed(albums, key = { _, a -> a.key }) { i, album ->
        AlbumTile(
            album,
            Modifier.padding(start = if (i % 2 == 0) Edge else 0.dp, end = if (i % 2 == 1) Edge else 0.dp),
        ) { open(Route.AlbumPage(album)) }
    }
}

@Composable
fun AlbumsScreen(open: (Route) -> Unit) {
    val albums by rememberLoaded { albums() }
    LazyVerticalGrid(
        columns = GridCells.Fixed(2),
        contentPadding = PaddingValues(bottom = 24.dp),
        horizontalArrangement = Arrangement.spacedBy(16.dp),
        verticalArrangement = Arrangement.spacedBy(20.dp),
        modifier = Modifier.fillMaxSize(),
    ) {
        full { LargeTitle("Albums") }
        albumGrid(albums, open)
    }
}

@Composable
fun ArtistsScreen(open: (Route) -> Unit) {
    val artists by rememberLoaded { artists() }
    LazyColumn(Modifier.fillMaxSize()) {
        item { LargeTitle("Artists") }
        items(artists.orEmpty(), key = { it.name }) { artist ->
            Column(Modifier.fillMaxWidth().clickable { open(Route.Artist(artist.name)) }) {
                Row(
                    Modifier.fillMaxWidth().padding(horizontal = Edge, vertical = 8.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(14.dp),
                ) {
                    RoundCover(artist.artwork, 48.dp)
                    Text(
                        artist.name.ifBlank { "Unknown artist" },
                        style = MaterialTheme.typography.bodyLarge,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
                HorizontalDivider(color = MaterialTheme.colorScheme.outlineVariant, modifier = Modifier.padding(start = Edge + 62.dp))
            }
        }
    }
}

@Composable
private fun SongListPage(title: String, songs: List<Song>?, open: (Route) -> Unit) {
    val playing = currentId()
    val list = songs.orEmpty()
    LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(bottom = 24.dp)) {
        item { LargeTitle(title) }
        if (songs == null) items(8) { SongPlaceholder() }
        if (list.isNotEmpty()) item { PlayShuffle(list) }
        itemsIndexed(list, key = { _, s -> s.id }) { i, song ->
            SongRow(song, playing == song.id, open) { play(list, i) }
        }
    }
}

@Composable
fun SongsScreen(open: (Route) -> Unit) {
    val songs by rememberLoaded { songs() }
    SongListPage("Songs", songs, open)
}

@Composable
fun GenreScreen(name: String, open: (Route) -> Unit) {
    val songs by rememberLoaded(name) { genreSongs(name) }
    SongListPage(name, songs, open)
}

@Composable
fun PlaylistScreen(route: Route.Playlist, open: (Route) -> Unit) {
    val songs by rememberLoaded(route.id) { playlistSongs(route.id) }
    SongListPage(route.name, songs, open)
}

/** Play and Shuffle, side by side: soft pills with amber words, as in Apple Music. */
@Composable
private fun PlayShuffle(songs: List<Song>) {
    Row(
        Modifier.fillMaxWidth().padding(horizontal = Edge, vertical = 12.dp),
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        val colors = ButtonDefaults.filledTonalButtonColors(
            containerColor = MaterialTheme.colorScheme.surfaceContainerHigh,
            contentColor = MaterialTheme.colorScheme.primary,
        )
        FilledTonalButton(onClick = { play(songs) }, colors = colors, modifier = Modifier.weight(1f).height(48.dp)) {
            Icon(Icons.Rounded.PlayArrow, contentDescription = null)
            Spacer(Modifier.width(6.dp))
            Text("Play", fontWeight = FontWeight.SemiBold)
        }
        FilledTonalButton(onClick = { shuffle(songs) }, colors = colors, modifier = Modifier.weight(1f).height(48.dp)) {
            Icon(Icons.Rounded.Shuffle, contentDescription = null)
            Spacer(Modifier.width(6.dp))
            Text("Shuffle", fontWeight = FontWeight.SemiBold)
        }
    }
}

@Composable
fun PlaylistsScreen(open: (Route) -> Unit) {
    val playlists by rememberLoaded { playlists() }
    val list = playlists.orEmpty()
    LazyColumn(Modifier.fillMaxSize()) {
        item { LargeTitle("Playlists") }
        if (playlists != null && list.isEmpty()) {
            item {
                Text(
                    "No playlists yet.",
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.padding(horizontal = Edge, vertical = 8.dp),
                )
            }
        }
        items(list, key = { it.id }) { playlist ->
            Column(Modifier.fillMaxWidth().clickable { open(Route.Playlist(playlist.id, playlist.name)) }) {
                Row(
                    Modifier.fillMaxWidth().padding(horizontal = Edge, vertical = 8.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(14.dp),
                ) {
                    Cover(playlist.artwork, Modifier.size(64.dp))
                    Column(Modifier.weight(1f)) {
                        Text(playlist.name, style = MaterialTheme.typography.bodyLarge, maxLines = 1, overflow = TextOverflow.Ellipsis)
                        Text(
                            if (playlist.smart) "Smart playlist" else count(playlist.songs.toInt(), "song"),
                            style = MaterialTheme.typography.bodyMedium,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                }
                HorizontalDivider(color = MaterialTheme.colorScheme.outlineVariant, modifier = Modifier.padding(start = Edge + 78.dp))
            }
        }
    }
}

// ---------- Album and artist

@Composable
fun AlbumScreen(album: Album, open: (Route) -> Unit) {
    val songs by rememberLoaded(album.key) { albumSongs(album.key) }
    val playing = currentId()
    val list = songs.orEmpty()
    LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(bottom = 32.dp)) {
        item {
            Column(
                Modifier.fillMaxWidth().padding(top = 24.dp, start = Edge, end = Edge),
                horizontalAlignment = Alignment.CenterHorizontally,
            ) {
                Cover(
                    album.artwork ?: list.firstNotNullOfOrNull { it.artwork },
                    Modifier
                        .fillMaxWidth(0.7f)
                        .aspectRatio(1f)
                        .sharedCover("album-${album.key}")
                        .shadow(24.dp, CoverShape, ambientColor = Color.Black, spotColor = Color.Black),
                )
                Text(
                    album.title.ifBlank { "Unknown album" },
                    style = MaterialTheme.typography.titleLarge,
                    fontWeight = FontWeight.Bold,
                    textAlign = TextAlign.Center,
                    modifier = Modifier.padding(top = 20.dp),
                )
                Text(
                    album.artist,
                    style = MaterialTheme.typography.titleLarge,
                    color = MaterialTheme.colorScheme.primary,
                    textAlign = TextAlign.Center,
                    modifier = Modifier.clip(RoundedCornerShape(6.dp)).clickable { open(Route.Artist(album.artist)) },
                )
                if (album.year > 0) {
                    Text(
                        album.year.toString(),
                        style = MaterialTheme.typography.labelLarge,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.padding(top = 4.dp),
                    )
                }
            }
        }
        if (list.isNotEmpty()) item { PlayShuffle(list) }
        item { HorizontalDivider(color = MaterialTheme.colorScheme.outlineVariant, modifier = Modifier.padding(start = Edge)) }
        itemsIndexed(list, key = { _, s -> s.id }) { i, song ->
            SongRow(song, playing == song.id, open, number = true) { play(list, i) }
        }
        if (list.isNotEmpty()) {
            item {
                Text(
                    songsAndLength(list),
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.padding(horizontal = Edge, vertical = 16.dp),
                )
            }
        }
    }
}

@Composable
fun AlbumOfScreen(route: Route.AlbumOf, open: (Route) -> Unit) {
    val album by rememberLoaded(route.songId) { albumOf(route.songId) }
    album?.let { AlbumScreen(it, open) }
}

@Composable
fun ArtistScreen(name: String, open: (Route) -> Unit) {
    val albums by rememberLoaded(name) { artistAlbums(name) }
    val top by rememberLoaded(name) { artistSongs(name) }
    val playing = currentId()
    val songs = top.orEmpty()
    LazyVerticalGrid(
        columns = GridCells.Fixed(2),
        contentPadding = PaddingValues(bottom = 24.dp),
        horizontalArrangement = Arrangement.spacedBy(16.dp),
        verticalArrangement = Arrangement.spacedBy(20.dp),
        modifier = Modifier.fillMaxSize(),
    ) {
        full {
            Column(Modifier.fillMaxWidth().padding(top = 24.dp), horizontalAlignment = Alignment.CenterHorizontally) {
                RoundCover(albums?.firstNotNullOfOrNull { it.artwork }, 160.dp)
                Text(
                    name.ifBlank { "Unknown artist" },
                    style = MaterialTheme.typography.headlineMedium,
                    fontWeight = FontWeight.Bold,
                    textAlign = TextAlign.Center,
                    modifier = Modifier.padding(top = 16.dp, start = Edge, end = Edge),
                )
            }
        }
        if (songs.isNotEmpty()) {
            full { PlayShuffle(songs) }
            full { SectionHeader("Top songs", Modifier.padding(top = 0.dp)) }
            gridItemsIndexed(songs.take(5), key = { _, s -> "top-" + s.id }, span = { _, _ -> GridItemSpan(maxLineSpan) }) { i, song ->
                SongRow(song, playing == song.id, open, divider = i < minOf(5, songs.size) - 1) { play(songs, i) }
            }
        }
        full { SectionHeader("Albums", Modifier.padding(top = 0.dp)) }
        albumGrid(albums, open)
    }
}

// ---------- Search

/** A steady colour for each genre tile, from its name. */
private val TileColors = listOf(
    Color(0xFF8E4A2F), Color(0xFF2F5E8E), Color(0xFF6B3F8E), Color(0xFF2F7A5C),
    Color(0xFF8E2F4A), Color(0xFF7A6A2F), Color(0xFF3F4F8E), Color(0xFF8E5E2F),
)

@Composable
fun SearchScreen(open: (Route) -> Unit) {
    var query by rememberSaveable { mutableStateOf("") }
    var songs by remember { mutableStateOf<List<Song>>(emptyList()) }
    val genres by rememberLoaded { genres() }
    val playing = currentId()
    LaunchedEffect(query) {
        delay(200)
        songs = if (query.isBlank()) emptyList()
        else withContext(Dispatchers.IO) { runCatching { core.search(query) }.getOrDefault(emptyList()) }
    }
    LazyVerticalGrid(
        columns = GridCells.Fixed(2),
        contentPadding = PaddingValues(bottom = 24.dp),
        horizontalArrangement = Arrangement.spacedBy(12.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp),
        modifier = Modifier.fillMaxSize(),
    ) {
        full { LargeTitle("Search") }
        full {
            TextField(
                value = query,
                onValueChange = { query = it },
                placeholder = { Text("Songs, albums, artists") },
                leadingIcon = { Icon(Icons.Rounded.Search, contentDescription = null) },
                trailingIcon = {
                    if (query.isNotEmpty()) {
                        IconButton(onClick = { query = "" }) { Icon(Icons.Rounded.Close, contentDescription = "Clear") }
                    }
                },
                singleLine = true,
                keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search),
                shape = RoundedCornerShape(28.dp),
                colors = TextFieldDefaults.colors(
                    focusedIndicatorColor = Color.Transparent,
                    unfocusedIndicatorColor = Color.Transparent,
                    focusedContainerColor = MaterialTheme.colorScheme.surfaceContainerHigh,
                    unfocusedContainerColor = MaterialTheme.colorScheme.surfaceContainerHigh,
                ),
                modifier = Modifier.fillMaxWidth().padding(horizontal = Edge),
            )
        }
        if (query.isBlank()) {
            val list = genres.orEmpty()
            if (list.isNotEmpty()) {
                full { SectionHeader("Browse by genre", Modifier.padding(top = 4.dp)) }
                gridItemsIndexed(list, key = { _, g -> g.name }) { i, genre ->
                    val color = TileColors[Math.floorMod(genre.name.lowercase().hashCode(), TileColors.size)]
                    Box(
                        Modifier
                            .padding(start = if (i % 2 == 0) Edge else 0.dp, end = if (i % 2 == 1) Edge else 0.dp)
                            .fillMaxWidth()
                            .heightIn(min = 96.dp)
                            .clip(RoundedCornerShape(12.dp))
                            .background(color)
                            .clickable { open(Route.Genre(genre.name)) }
                            .padding(14.dp),
                    ) {
                        Text(genre.name, style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.Bold, color = Color.White)
                    }
                }
            }
        } else {
            if (songs.isEmpty()) {
                full {
                    Text(
                        "Nothing found for “$query”.",
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.padding(horizontal = Edge, vertical = 16.dp),
                    )
                }
            }
            itemsIndexedFull(songs) { i, song ->
                SongRow(song, playing == song.id, open) { play(songs, i) }
            }
        }
    }
}

private fun LazyGridScope.itemsIndexedFull(songs: List<Song>, content: @Composable (Int, Song) -> Unit) {
    gridItemsIndexed(songs, key = { _, s -> s.id }, span = { _, _ -> GridItemSpan(maxLineSpan) }) { i, song ->
        content(i, song)
    }
}

@Composable
fun GenresScreen(open: (Route) -> Unit) {
    val genres by rememberLoaded { genres() }
    LazyColumn(Modifier.fillMaxSize()) {
        item { LargeTitle("Genres") }
        items(genres.orEmpty(), key = { it.name }) { genre ->
            Column(Modifier.fillMaxWidth().clickable { open(Route.Genre(genre.name)) }) {
                Row(
                    Modifier.fillMaxWidth().heightIn(min = 56.dp).padding(horizontal = Edge),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Text(genre.name, style = MaterialTheme.typography.bodyLarge, modifier = Modifier.weight(1f))
                    Text(
                        count(genre.songs.toInt(), "song"),
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
                HorizontalDivider(color = MaterialTheme.colorScheme.outlineVariant, modifier = Modifier.padding(start = Edge))
            }
        }
    }
}
