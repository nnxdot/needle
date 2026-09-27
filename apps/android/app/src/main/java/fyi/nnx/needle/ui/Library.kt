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
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.QueueMusic
import androidx.compose.material.icons.rounded.Album
import androidx.compose.material.icons.rounded.Favorite
import androidx.compose.material.icons.rounded.Folder
import androidx.compose.material.icons.rounded.History
import androidx.compose.material.icons.rounded.LibraryAdd
import androidx.compose.material.icons.rounded.MusicNote
import androidx.compose.material.icons.rounded.Person
import androidx.compose.material.icons.rounded.Settings
import androidx.compose.material.icons.rounded.Style
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialShapes
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.carousel.HorizontalMultiBrowseCarousel
import androidx.compose.material3.carousel.rememberCarouselState
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
import androidx.compose.material3.toShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import fyi.nnx.needle.NeedleApp

/** How much there is of each, for the tiles. */
private data class Counts(val playlists: Int, val artists: Int, val albums: Int, val songs: Int)

/**
 * The library: four large tiles for what people open most, each with its own Expressive
 * shape; a row of smaller ways in; then the newest albums in a carousel.
 */
@OptIn(ExperimentalMaterial3Api::class, ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun LibraryScreen(open: (Route) -> Unit) {
    val home by rememberLoaded { home() }
    val counts by rememberLoaded {
        Counts(playlists().size, artists().size, albums().size, songCount().toInt())
    }
    val scan by NeedleApp.instance.scan.collectAsState()
    PullToRefreshBox(isRefreshing = scan.running, onRefresh = { core.rescan() }, modifier = Modifier.fillMaxSize()) {
        LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(bottom = 32.dp)) {
            item {
                LargeTitle("Library") {
                    IconButton(onClick = { open(Route.Settings) }) { Icon(Icons.Rounded.Settings, contentDescription = "Settings") }
                }
            }
            item {
                val c = counts
                Column(Modifier.padding(horizontal = 16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                    Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                        Tile("Albums", c?.albums?.let { count(it, "album") }, Icons.Rounded.Album, MaterialShapes.Cookie9Sided.toShape(), Modifier.weight(1f)) { open(Route.Albums) }
                        Tile("Artists", c?.artists?.let { count(it, "artist") }, Icons.Rounded.Person, MaterialShapes.Circle.toShape(), Modifier.weight(1f)) { open(Route.Artists) }
                    }
                    Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                        Tile("Songs", c?.songs?.let { count(it, "song") }, Icons.Rounded.MusicNote, MaterialShapes.Clover4Leaf.toShape(), Modifier.weight(1f)) { open(Route.Songs) }
                        Tile("Playlists", c?.playlists?.let { count(it, "playlist") }, Icons.AutoMirrored.Rounded.QueueMusic, MaterialShapes.Sunny.toShape(), Modifier.weight(1f)) { open(Route.Playlists) }
                    }
                }
            }
            item {
                LazyRow(
                    contentPadding = PaddingValues(horizontal = 16.dp),
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                    modifier = Modifier.padding(top = 16.dp),
                ) {
                    item { Way(Icons.Rounded.Favorite, "Favorites") { open(Route.Favorites) } }
                    item { Way(Icons.Rounded.History, "History") { open(Route.History) } }
                    item { Way(Icons.Rounded.LibraryAdd, "Recent") { open(Route.RecentlyAdded) } }
                    item { Way(Icons.Rounded.Style, "Genres") { open(Route.Genres) } }
                    item { Way(Icons.Rounded.Folder, "Folders") { open(Route.Folder(null)) } }
                }
            }
            val added = home?.added.orEmpty()
            if (added.isNotEmpty()) {
                item { SectionHeader("Recently added", onMore = { open(Route.RecentlyAdded) }) }
                item {
                    val state = rememberCarouselState { added.size }
                    HorizontalMultiBrowseCarousel(
                        state = state,
                        preferredItemWidth = 200.dp,
                        itemSpacing = 10.dp,
                        contentPadding = PaddingValues(horizontal = 16.dp),
                        modifier = Modifier.fillMaxWidth().height(220.dp),
                    ) { i ->
                        val album = added[i]
                        Box(
                            Modifier
                                .fillMaxSize()
                                .maskClip(MaterialTheme.shapes.extraLarge)
                                .clickable { open(Route.AlbumPage(album)) },
                        ) {
                            Cover(album.artwork, Modifier.fillMaxSize(), RoundedCornerShape(0.dp))
                        }
                    }
                }
            }
        }
    }
}

/** A large tile: an icon in an Expressive shape, a name, and how many there are. */
@Composable
private fun Tile(title: String, detail: String?, icon: ImageVector, shape: Shape, modifier: Modifier, onClick: () -> Unit) {
    val interaction = remember { MutableInteractionSource() }
    Column(
        modifier
            .pressScale(interaction)
            .clip(RoundedCornerShape(28.dp))
            .background(MaterialTheme.colorScheme.surfaceContainer)
            .clickable(interactionSource = interaction, indication = null, onClick = onClick)
            .padding(16.dp),
    ) {
        Box(
            Modifier.size(52.dp).clip(shape).background(MaterialTheme.colorScheme.primaryContainer),
            contentAlignment = Alignment.Center,
        ) {
            Icon(icon, contentDescription = null, tint = MaterialTheme.colorScheme.onPrimaryContainer)
        }
        Spacer(Modifier.height(20.dp))
        Text(title, style = MaterialTheme.typography.titleLarge, fontWeight = FontWeight.Bold)
        Text(detail ?: " ", style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant, maxLines = 1, overflow = TextOverflow.Ellipsis)
    }
}

/** A smaller way in: a pill with an icon. */
@Composable
private fun Way(icon: ImageVector, label: String, onClick: () -> Unit) {
    Row(
        Modifier
            .clip(RoundedCornerShape(50))
            .background(MaterialTheme.colorScheme.surfaceContainerHigh)
            .clickable(onClick = onClick)
            .padding(horizontal = 16.dp, vertical = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Icon(icon, contentDescription = null, tint = MaterialTheme.colorScheme.primary, modifier = Modifier.size(18.dp))
        Text(label, style = MaterialTheme.typography.labelLarge)
    }
}
