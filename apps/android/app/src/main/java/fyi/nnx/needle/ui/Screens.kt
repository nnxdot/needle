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
import androidx.compose.material.icons.rounded.Favorite
import androidx.compose.material.icons.rounded.Folder
import androidx.compose.material.icons.rounded.History
import androidx.compose.material.icons.rounded.LibraryAdd
import androidx.compose.material.icons.rounded.Computer
import androidx.compose.material.icons.rounded.AutoAwesome
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
import androidx.compose.material3.TextButton
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
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

private fun play(songs: List<Song>, start: Int = 0) {
    core.play(songs.map { it.id }, start.toUInt())
    NeedleApp.instance.openPlayer.tryEmit(Unit)
}

@Composable
private fun currentId(): String? = NeedleApp.instance.playback.collectAsState().value?.current?.id

internal fun LazyGridScope.full(content: @Composable () -> Unit) =
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
            open,
        ) { open(Route.AlbumPage(album)) }
    }
}

@Composable
fun AlbumsScreen(open: (Route) -> Unit) {
    val albums by rememberLoaded { albums() }
    Page("Albums", albums?.let { count(it.size, "album") }) { padding, scroll ->
        LazyVerticalGrid(
            columns = GridCells.Fixed(2),
            contentPadding = PaddingValues(bottom = 24.dp),
            horizontalArrangement = Arrangement.spacedBy(16.dp),
            verticalArrangement = Arrangement.spacedBy(20.dp),
            modifier = Modifier.fillMaxSize().padding(padding).then(scroll),
        ) { albumGrid(albums, open) }
    }
}

@Composable
fun ArtistsScreen(open: (Route) -> Unit) {
    val artists by rememberLoaded { artists() }
    Page("Artists") { padding, scroll ->
        LazyColumn(Modifier.fillMaxSize().padding(padding).then(scroll), contentPadding = PaddingValues(bottom = 24.dp)) {
            items(artists.orEmpty(), key = { it.name }) { artist ->
                Row(
                    Modifier.fillMaxWidth().clickable { open(Route.Artist(artist.name)) }.padding(horizontal = Edge, vertical = 6.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(14.dp),
                ) {
                    RoundCover(artist.artwork, 52.dp)
                    Text(artist.name.ifBlank { "Unknown artist" }, style = MaterialTheme.typography.bodyLarge, maxLines = 1, overflow = TextOverflow.Ellipsis)
                }
            }
        }
    }
}

@Composable
private fun SongListPage(title: String, songs: List<Song>?, open: (Route) -> Unit) {
    val playing = currentId()
    val list = songs.orEmpty()
    Page(title, songs?.let { songsAndLength(it) }) { padding, scroll ->
        LazyColumn(Modifier.fillMaxSize().padding(padding).then(scroll), contentPadding = PaddingValues(bottom = 24.dp)) {
            if (songs == null) items(8) { SongPlaceholder() }
            if (list.isNotEmpty()) item { PlayRow(list) }
            itemsIndexed(list, key = { _, s -> s.id }) { i, song ->
                SongRow(song, playing == song.id, open) { play(list, i) }
            }
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
fun PlaylistsScreen(open: (Route) -> Unit) {
    val playlists by rememberLoaded { playlists() }
    val list = playlists.orEmpty()
    Page("Playlists", actions = {
        IconButton(onClick = { Ui.sheet.value = Sheet.NewPlaylist(emptyList()) }) { Icon(Icons.Rounded.LibraryAdd, contentDescription = "New playlist") }
    }) { padding, scroll ->
        LazyColumn(Modifier.fillMaxSize().padding(padding).then(scroll), contentPadding = PaddingValues(bottom = 24.dp)) {
            item {
                Row(Modifier.padding(horizontal = Edge, vertical = 8.dp), horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                    Button(onClick = { Ui.sheet.value = Sheet.NewPlaylist(emptyList()) }) { Text("New playlist") }
                    FilledTonalButton(onClick = { Ui.sheet.value = Sheet.NewPlaylist(emptyList(), smart = true) }) { Text("New smart playlist") }
                }
            }
            if (playlists != null && list.isEmpty()) {
                item { Text("No playlists yet.", color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.padding(horizontal = Edge, vertical = 8.dp)) }
            }
            items(list, key = { it.id }) { playlist ->
                Row(
                    Modifier.fillMaxWidth().clickable { open(Route.Playlist(playlist.id, playlist.name)) }.padding(horizontal = Edge, vertical = 4.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(14.dp),
                ) {
                    Cover(playlist.artwork, Modifier.size(52.dp), SmallCoverShape)
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
}

/** Genres as tiles in their own colours, as on Search. */
@Composable
fun GenresScreen(open: (Route) -> Unit) {
    val genres by rememberLoaded { genres() }
    val list = genres.orEmpty()
    Page("Genres") { padding, scroll ->
        LazyVerticalGrid(
            columns = GridCells.Fixed(2),
            contentPadding = PaddingValues(bottom = 24.dp),
            horizontalArrangement = Arrangement.spacedBy(12.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
            modifier = Modifier.fillMaxSize().padding(padding).then(scroll),
        ) {
            gridItemsIndexed(list, key = { _, g -> g.name }) { i, genre ->
                GenreTile(genre.name, genre.artwork, Modifier.padding(start = if (i % 2 == 0) 16.dp else 0.dp, end = if (i % 2 == 1) 16.dp else 0.dp)) { open(Route.Genre(genre.name)) }
            }
        }
    }
}
