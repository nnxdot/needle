package fyi.nnx.needle.ui

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
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.GridItemSpan
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.PlayArrow
import androidx.compose.material.icons.rounded.Settings
import androidx.compose.material.icons.rounded.Shuffle
import androidx.compose.material3.Button
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.FilterChip
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import fyi.nnx.needle.NeedleApp
import fyi.nnx.needle.core.Album
import fyi.nnx.needle.core.Song
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.withContext
import java.util.Calendar

private fun count(n: Int, word: String) = if (n == 1) "1 $word" else "$n ${word}s"

private fun songsLabel(n: Int) = count(n, "song")

private fun play(songs: List<Song>, start: Int = 0) = core.play(songs.map { it.id }, start.toUInt())

private fun shuffle(songs: List<Song>) {
    play(songs.shuffled())
}

@Composable
private fun PageTitle(text: String, action: @Composable () -> Unit = {}) {
    Row(
        Modifier.fillMaxWidth().statusBarsPadding().padding(start = 20.dp, end = 8.dp, top = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(
            text,
            style = MaterialTheme.typography.displaySmall,
            fontWeight = FontWeight.Bold,
            modifier = Modifier.weight(1f),
        )
        action()
    }
}

@Composable
fun HomeScreen(open: (Route) -> Unit) {
    val home by rememberLoaded { home() }
    val count by rememberLoaded { songCount() }
    val scan by NeedleApp.instance.scan.collectAsState()
    val hour = Calendar.getInstance().get(Calendar.HOUR_OF_DAY)
    val greeting = when (hour) {
        in 5..11 -> "Good morning"
        in 12..17 -> "Good afternoon"
        else -> "Good evening"
    }
    LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(bottom = 24.dp)) {
        item {
            PageTitle(greeting) {
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
        albumRow("Jump back in", h.recent, open)
        albumRow("Recently added", h.added, open)
        albumRow("Most played", h.mostPlayed, open)
    }
}

private fun androidx.compose.foundation.lazy.LazyListScope.albumRow(
    title: String,
    albums: List<Album>,
    open: (Route) -> Unit,
) {
    if (albums.isEmpty()) return
    item { SectionTitle(title) }
    item {
        LazyRow(
            contentPadding = PaddingValues(horizontal = 16.dp),
            horizontalArrangement = Arrangement.spacedBy(14.dp),
        ) {
            items(albums, key = { it.key }) { album ->
                AlbumTile(album, Modifier.width(148.dp)) { open(Route.Album(album.key, album.title)) }
            }
        }
    }
}

@Composable
private fun EmptyLibrary(scanning: Boolean, open: (Route) -> Unit) {
    Column(
        Modifier.fillMaxWidth().padding(24.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Spacer(Modifier.height(48.dp))
        Text(
            if (scanning) "Reading your music…" else "Bring your music home",
            style = MaterialTheme.typography.headlineSmall,
            fontWeight = FontWeight.Bold,
        )
        Text(
            if (scanning) "Your songs appear as Needle finds them."
            else "Choose the folder your music is in. Needle reads it and plays it from your phone.",
            style = MaterialTheme.typography.bodyLarge,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            textAlign = TextAlign.Center,
            modifier = Modifier.padding(top = 8.dp, bottom = 20.dp),
        )
        if (!scanning) Button(onClick = { open(Route.Settings) }) { Text("Add your music") }
    }
}

@Composable
fun LibraryScreen(open: (Route) -> Unit) {
    var view by rememberSaveable { mutableStateOf("Albums") }
    Column(Modifier.fillMaxSize()) {
        PageTitle("Library")
        Row(
            Modifier.padding(horizontal = 16.dp, vertical = 12.dp),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            listOf("Albums", "Artists", "Songs").forEach {
                FilterChip(selected = view == it, onClick = { view = it }, label = { Text(it) })
            }
        }
        when (view) {
            "Albums" -> AlbumGrid(open)
            "Artists" -> ArtistList(open)
            else -> SongList()
        }
    }
}

@Composable
private fun AlbumGrid(open: (Route) -> Unit, artist: String? = null, header: @Composable () -> Unit = {}) {
    val albums by rememberLoaded(artist) { if (artist == null) albums() else artistAlbums(artist) }
    LazyVerticalGrid(
        columns = GridCells.Adaptive(150.dp),
        contentPadding = PaddingValues(16.dp),
        horizontalArrangement = Arrangement.spacedBy(16.dp),
        verticalArrangement = Arrangement.spacedBy(20.dp),
        modifier = Modifier.fillMaxSize(),
    ) {
        item(span = { GridItemSpan(maxLineSpan) }) { header() }
        items(albums.orEmpty(), key = { it.key }) { album ->
            AlbumTile(album) { open(Route.Album(album.key, album.title)) }
        }
    }
}

@Composable
private fun ArtistList(open: (Route) -> Unit) {
    val artists by rememberLoaded { artists() }
    LazyColumn(Modifier.fillMaxSize()) {
        items(artists.orEmpty(), key = { it.name }) { artist ->
            Row(
                Modifier
                    .fillMaxWidth()
                    .clickable { open(Route.Artist(artist.name)) }
                    .padding(horizontal = 16.dp, vertical = 8.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(14.dp),
            ) {
                RoundCover(artist.artwork, 56)
                Column(Modifier.weight(1f)) {
                    Text(artist.name.ifBlank { "Unknown artist" }, style = MaterialTheme.typography.bodyLarge)
                    Text(
                        "${count(artist.albums.toInt(), "album")} · ${songsLabel(artist.songs.toInt())}",
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
            }
        }
    }
}

@Composable
private fun SongList() {
    val songs by rememberLoaded { songs() }
    val playback by NeedleApp.instance.playback.collectAsState()
    val list = songs.orEmpty()
    LazyColumn(Modifier.fillMaxSize()) {
        if (list.isNotEmpty()) item { PlayShuffle(list) }
        itemsIndexed(list, key = { _, s -> s.id }) { i, song ->
            SongRow(song, playing = playback?.current?.id == song.id) { play(list, i) }
        }
    }
}

@Composable
private fun PlayShuffle(songs: List<Song>) {
    Row(
        Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 8.dp),
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Button(onClick = { play(songs) }, modifier = Modifier.weight(1f)) {
            Icon(Icons.Rounded.PlayArrow, contentDescription = null)
            Spacer(Modifier.width(8.dp))
            Text("Play")
        }
        FilledTonalButton(onClick = { shuffle(songs) }, modifier = Modifier.weight(1f)) {
            Icon(Icons.Rounded.Shuffle, contentDescription = null)
            Spacer(Modifier.width(8.dp))
            Text("Shuffle")
        }
    }
}

@Composable
fun AlbumScreen(route: Route.Album, open: (Route) -> Unit) {
    val songs by rememberLoaded(route.key) { albumSongs(route.key) }
    val playback by NeedleApp.instance.playback.collectAsState()
    val list = songs.orEmpty()
    val first = list.firstOrNull()
    LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(bottom = 24.dp)) {
        item {
            Column(
                Modifier.fillMaxWidth().statusBarsPadding().padding(24.dp),
                horizontalAlignment = Alignment.CenterHorizontally,
            ) {
                Cover(
                    list.firstNotNullOfOrNull { it.artwork },
                    Modifier.fillMaxWidth(0.72f).aspectRatio(1f),
                    RoundedCornerShape(28.dp),
                )
                Text(
                    route.title.ifBlank { "Unknown album" },
                    style = MaterialTheme.typography.headlineSmall,
                    fontWeight = FontWeight.Bold,
                    textAlign = TextAlign.Center,
                    modifier = Modifier.padding(top = 20.dp),
                )
                if (first != null) {
                    Text(
                        first.artist,
                        style = MaterialTheme.typography.titleMedium,
                        color = MaterialTheme.colorScheme.primary,
                        modifier = Modifier.clickable { open(Route.Artist(first.artist)) },
                    )
                    Text(
                        "${songsLabel(list.size)} · ${time(list.sumOf { it.duration })}",
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.padding(top = 4.dp),
                    )
                }
            }
        }
        if (list.isNotEmpty()) item { PlayShuffle(list) }
        itemsIndexed(list, key = { _, s -> s.id }) { i, song ->
            SongRow(song, playing = playback?.current?.id == song.id, showCover = false) { play(list, i) }
        }
    }
}

@Composable
fun ArtistScreen(route: Route.Artist, open: (Route) -> Unit) {
    AlbumGrid(open, artist = route.name) {
        Text(
            route.name.ifBlank { "Unknown artist" },
            style = MaterialTheme.typography.displaySmall,
            fontWeight = FontWeight.Bold,
            modifier = Modifier.statusBarsPadding().padding(bottom = 8.dp),
        )
    }
}

@Composable
fun SearchScreen(open: (Route) -> Unit) {
    var query by rememberSaveable { mutableStateOf("") }
    var songs by androidx.compose.runtime.remember { mutableStateOf<List<Song>>(emptyList()) }
    val playback by NeedleApp.instance.playback.collectAsState()
    LaunchedEffect(query) {
        delay(250)
        songs = if (query.isBlank()) emptyList()
        else withContext(Dispatchers.IO) { runCatching { core.search(query) }.getOrDefault(emptyList()) }
    }
    Column(Modifier.fillMaxSize()) {
        PageTitle("Search")
        OutlinedTextField(
            value = query,
            onValueChange = { query = it },
            placeholder = { Text("Songs, albums, artists…") },
            singleLine = true,
            shape = RoundedCornerShape(28.dp),
            modifier = Modifier.fillMaxWidth().padding(16.dp),
        )
        if (query.isBlank()) {
            Text(
                "Try",
                style = MaterialTheme.typography.titleMedium,
                modifier = Modifier.padding(horizontal = 16.dp, vertical = 4.dp),
            )
            Row(
                Modifier.padding(horizontal = 16.dp),
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                listOf("rating is at least 4", "added this month").forEach {
                    FilterChip(selected = false, onClick = { query = it }, label = { Text(it) })
                }
            }
        }
        LazyColumn(Modifier.fillMaxSize()) {
            itemsIndexed(songs, key = { _, s -> s.id }) { i, song ->
                SongRow(song, playing = playback?.current?.id == song.id) { play(songs, i) }
            }
        }
    }
}

@Composable
fun PlaylistsScreen(open: (Route) -> Unit) {
    val playlists by rememberLoaded { playlists() }
    LazyVerticalGrid(
        columns = GridCells.Adaptive(150.dp),
        contentPadding = PaddingValues(16.dp),
        horizontalArrangement = Arrangement.spacedBy(16.dp),
        verticalArrangement = Arrangement.spacedBy(20.dp),
        modifier = Modifier.fillMaxSize(),
    ) {
        item(span = { GridItemSpan(maxLineSpan) }) { Box(Modifier.padding(start = 4.dp)) { PageTitle("Playlists") } }
        val list = playlists.orEmpty()
        if (playlists != null && list.isEmpty()) {
            item(span = { GridItemSpan(maxLineSpan) }) {
                Text(
                    "No playlists yet.",
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }
        items(list, key = { it.id }) { playlist ->
            Column(Modifier.clickable { open(Route.Playlist(playlist.id, playlist.name)) }) {
                Cover(playlist.artwork, Modifier.fillMaxWidth().aspectRatio(1f), RoundedCornerShape(20.dp))
                Text(
                    playlist.name,
                    style = MaterialTheme.typography.titleSmall,
                    fontWeight = FontWeight.SemiBold,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                    modifier = Modifier.padding(top = 8.dp),
                )
                Text(
                    if (playlist.smart) "Smart playlist" else songsLabel(playlist.songs.toInt()),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }
    }
}

@Composable
fun PlaylistScreen(route: Route.Playlist) {
    val songs by rememberLoaded(route.id) { playlistSongs(route.id) }
    val playback by NeedleApp.instance.playback.collectAsState()
    val list = songs.orEmpty()
    LazyColumn(Modifier.fillMaxSize()) {
        item { PageTitle(route.name) }
        if (list.isNotEmpty()) item { PlayShuffle(list) }
        itemsIndexed(list, key = { _, s -> s.id }) { i, song ->
            SongRow(song, playing = playback?.current?.id == song.id) { play(list, i) }
        }
    }
}
