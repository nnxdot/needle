package fyi.nnx.needle.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.PlaylistAdd
import androidx.compose.material.icons.automirrored.rounded.QueueMusic
import androidx.compose.material.icons.rounded.Album
import androidx.compose.material.icons.rounded.Delete
import androidx.compose.material.icons.rounded.Edit
import androidx.compose.material.icons.rounded.PlayArrow
import androidx.compose.material.icons.rounded.Shuffle
import androidx.compose.material.icons.rounded.Extension
import androidx.compose.material.icons.rounded.Lyrics
import androidx.compose.material.icons.rounded.Person
import androidx.compose.material.icons.rounded.QueuePlayNext
import androidx.compose.material.icons.rounded.Radio
import androidx.compose.material.icons.rounded.RemoveCircleOutline
import androidx.compose.material.icons.rounded.Star
import androidx.compose.material.icons.rounded.StarBorder
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MediumFlexibleTopAppBar
import androidx.compose.material3.MaterialShapes
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.material3.toShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.input.nestedscroll.nestedScroll
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import fyi.nnx.needle.NeedleApp
import fyi.nnx.needle.core.Song
import kotlinx.coroutines.launch

/** The one shape for icons set in a shape (see DESIGN.md). */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun iconShape(): androidx.compose.ui.graphics.Shape = MaterialShapes.Cookie9Sided.toShape()

/**
 * A page: its large title shrinks into a bar as the content scrolls (Material 3 Expressive's
 * flexible top bar). `content` gets the padding and the scroll to hand to its list.
 */
@OptIn(ExperimentalMaterial3Api::class, ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun Page(
    title: String,
    subtitle: String? = null,
    actions: @Composable RowScope.() -> Unit = {},
    content: @Composable (padding: PaddingValues, scroll: Modifier) -> Unit,
) {
    val behavior = TopAppBarDefaults.exitUntilCollapsedScrollBehavior()
    Scaffold(
        containerColor = MaterialTheme.colorScheme.background,
        contentWindowInsets = WindowInsets(0),
        topBar = {
            MediumFlexibleTopAppBar(
                title = { Text(title, fontWeight = FontWeight.Bold, maxLines = 1, overflow = TextOverflow.Ellipsis) },
                subtitle = subtitle?.let { { Text(it, color = MaterialTheme.colorScheme.onSurfaceVariant) } },
                actions = actions,
                scrollBehavior = behavior,
                // The app's frame already keeps clear of the status bar.
                windowInsets = WindowInsets(0),
                colors = TopAppBarDefaults.topAppBarColors(
                    containerColor = MaterialTheme.colorScheme.background,
                    scrolledContainerColor = MaterialTheme.colorScheme.surfaceContainerLow,
                ),
            )
        },
    ) { padding -> content(padding, Modifier.nestedScroll(behavior.nestedScrollConnection)) }
}

/**
 * A song's menu, as a sheet from the bottom: the song and its stars at the top, then its
 * actions in rounded groups with gaps between them (Material 3 Expressive menus).
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SongSheet(song: Song, open: ((Route) -> Unit)?, remove: (() -> Unit)?, onDismiss: () -> Unit) {
    val state = rememberModalBottomSheetState(skipPartiallyExpanded = true)
    val scope = androidx.compose.runtime.rememberCoroutineScope()
    val haptics = rememberHaptics()
    fun act(work: () -> Unit) {
        scope.launch { state.hide() }.invokeOnCompletion { onDismiss(); work() }
    }
    ModalBottomSheet(onDismissRequest = onDismiss, sheetState = state, containerColor = MaterialTheme.colorScheme.surfaceContainerLow) {
        Column(Modifier.navigationBarsPadding().padding(horizontal = 16.dp).padding(bottom = 16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(14.dp), modifier = Modifier.padding(horizontal = 4.dp)) {
                Cover(song.artwork, Modifier.size(56.dp), SmallCoverShape)
                Column(Modifier.weight(1f)) {
                    Text(song.title, style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.Bold, maxLines = 1, overflow = TextOverflow.Ellipsis)
                    Text(song.artist, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant, maxLines = 1, overflow = TextOverflow.Ellipsis)
                }
            }
            // The stars: a tap sets them, a tap on the same one clears them.
            var stars by remember(song.id) { mutableIntStateOf(song.rating.toInt()) }
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.Center) {
                (1..5).forEach { n ->
                    IconButton(onClick = {
                        haptics(false)
                        stars = if (stars == n) 0 else n
                        runCatching { core.rate(song.id, stars.toUByte()) }
                        NeedleApp.instance.libraryVersion.value++
                    }) {
                        Icon(
                            if (n <= stars) Icons.Rounded.Star else Icons.Rounded.StarBorder,
                            contentDescription = "$n stars",
                            tint = if (n <= stars) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurfaceVariant,
                            modifier = Modifier.size(30.dp),
                        )
                    }
                }
            }
            MenuGroup {
                MenuRow(Icons.Rounded.QueuePlayNext, "Play next") { act { core.playNext(listOf(song.id)); showMessage("Plays next") } }
                MenuRow(Icons.AutoMirrored.Rounded.QueueMusic, "Add to queue") { act { core.enqueue(listOf(song.id)); showMessage("Added to the queue") } }
                MenuRow(Icons.Rounded.Radio, "Start radio") {
                    act {
                        showMessage("Starting radio from ${song.title}…")
                        NeedleApp.instance.scope.launch { runCatching { core.startRadio(song.id) } }
                    }
                }
            }
            MenuGroup {
                MenuRow(Icons.AutoMirrored.Rounded.PlaylistAdd, "Add to a playlist") { act { Ui.sheet.value = Sheet.AddToPlaylist(listOf(song.id)) } }
                if (remove != null) MenuRow(Icons.Rounded.RemoveCircleOutline, "Remove from this playlist") { act(remove) }
            }
            if (open != null) {
                MenuGroup {
                    MenuRow(Icons.Rounded.Album, "Go to album") { act { open(Route.AlbumOf(song.album, song.id)) } }
                    MenuRow(Icons.Rounded.Person, "Go to artist") { act { open(Route.Artist(song.artist)) } }
                    MenuRow(Icons.Rounded.Lyrics, "Time the lyrics") { act { open(Route.Timing(song.id)) } }
                }
            }
            // Commands from plugins that act on songs.
            val commands = remember { runCatching { core.plugins() }.getOrDefault(emptyList()).filter { it.enabled } }
                .flatMap { p -> p.commands.filter { it.forSongs }.map { p.id to it } }
            if (commands.isNotEmpty()) {
                MenuGroup {
                    commands.forEach { (plugin, command) ->
                        MenuRow(Icons.Rounded.Extension, command.title) { act { core.runPluginCommand(plugin, command.id, listOf(song.id)) } }
                    }
                }
            }
        }
    }
}

/** Menu rows on one rounded surface; groups have gaps between them. */
@Composable
fun MenuGroup(content: @Composable ColumnScope.() -> Unit) {
    Column(Modifier.fillMaxWidth().clip(RoundedCornerShape(24.dp)).background(MaterialTheme.colorScheme.surfaceContainerHigh), content = content)
}

@Composable
fun MenuRow(icon: ImageVector, label: String, tint: Color = MaterialTheme.colorScheme.onSurface, onClick: () -> Unit) {
    Row(
        Modifier.fillMaxWidth().heightIn(min = 56.dp).clickable(onClick = onClick).padding(horizontal = 20.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(18.dp),
    ) {
        Icon(icon, contentDescription = null, tint = MaterialTheme.colorScheme.onSurfaceVariant)
        Text(label, style = MaterialTheme.typography.bodyLarge, color = tint)
    }
}

/** An icon set in the one icon shape, for tiles and empty states. */
@Composable
fun ShapedIcon(icon: ImageVector, size: Int = 48) {
    androidx.compose.foundation.layout.Box(
        Modifier.size(size.dp).clip(iconShape()).background(MaterialTheme.colorScheme.primaryContainer),
        contentAlignment = Alignment.Center,
    ) { Icon(icon, contentDescription = null, tint = MaterialTheme.colorScheme.onPrimaryContainer, modifier = Modifier.size((size * 0.46).dp)) }
}

/**
 * A sheet from the bottom in the song menu's style: a cover and a name at the top, then groups
 * of actions. `act` closes the sheet, then does the work.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ActionSheet(
    art: String?,
    title: String,
    sub: String?,
    onDismiss: () -> Unit,
    content: @Composable ColumnScope.(act: (() -> Unit) -> Unit) -> Unit,
) {
    val state = rememberModalBottomSheetState(skipPartiallyExpanded = true)
    val scope = androidx.compose.runtime.rememberCoroutineScope()
    val act: (() -> Unit) -> Unit = { work -> scope.launch { state.hide() }.invokeOnCompletion { onDismiss(); work() } }
    ModalBottomSheet(onDismissRequest = onDismiss, sheetState = state, containerColor = MaterialTheme.colorScheme.surfaceContainerLow) {
        Column(Modifier.navigationBarsPadding().padding(horizontal = 16.dp).padding(bottom = 16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(14.dp), modifier = Modifier.padding(horizontal = 4.dp, vertical = 4.dp)) {
                Cover(art, Modifier.size(56.dp), SmallCoverShape)
                Column(Modifier.weight(1f)) {
                    Text(title, style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.Bold, maxLines = 1, overflow = TextOverflow.Ellipsis)
                    if (sub != null) Text(sub, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant, maxLines = 1, overflow = TextOverflow.Ellipsis)
                }
            }
            content(act)
        }
    }
}

/** An album's menu: from a long press on its cover, or the More button on its page. */
@Composable
fun AlbumSheet(album: fyi.nnx.needle.core.Album, open: ((Route) -> Unit)?, onDismiss: () -> Unit) {
    fun ids() = runCatching { core.albumSongs(album.key).map { it.id } }.getOrDefault(emptyList())
    fun start(list: List<String>) {
        if (list.isEmpty()) return
        core.play(list, 0u)
        NeedleApp.instance.openPlayer.tryEmit(Unit)
    }
    ActionSheet(album.artwork, album.title.ifBlank { "Unknown album" }, album.artist, onDismiss) { act ->
        MenuGroup {
            MenuRow(Icons.Rounded.PlayArrow, "Play") { act { start(ids()) } }
            MenuRow(Icons.Rounded.Shuffle, "Shuffle") { act { start(ids().shuffled()) } }
        }
        MenuGroup {
            MenuRow(Icons.Rounded.QueuePlayNext, "Play next") { act { core.playNext(ids()); showMessage("Plays next") } }
            MenuRow(Icons.AutoMirrored.Rounded.QueueMusic, "Add to queue") { act { core.enqueue(ids()); showMessage("Added to the queue") } }
            MenuRow(Icons.AutoMirrored.Rounded.PlaylistAdd, "Add to a playlist") { act { Ui.sheet.value = Sheet.AddToPlaylist(ids()) } }
        }
        if (open != null) {
            MenuGroup {
                MenuRow(Icons.Rounded.Album, "Go to album") { act { open(Route.AlbumPage(album)) } }
                MenuRow(Icons.Rounded.Person, "Go to artist") { act { open(Route.Artist(album.artist)) } }
            }
        }
    }
}

/** A playlist's menu, from the More button on its page. */
@Composable
fun PlaylistSheet(detail: fyi.nnx.needle.core.PlaylistDetail, art: String?, onDelete: () -> Unit, onDismiss: () -> Unit) {
    val ids = detail.songs.map { it.id }
    ActionSheet(art, detail.name, if (detail.rule != null) "Smart playlist" else count(ids.size, "song"), onDismiss) { act ->
        if (ids.isNotEmpty()) {
            MenuGroup {
                MenuRow(Icons.Rounded.QueuePlayNext, "Play next") { act { core.playNext(ids); showMessage("Plays next") } }
                MenuRow(Icons.AutoMirrored.Rounded.QueueMusic, "Add to queue") { act { core.enqueue(ids); showMessage("Added to the queue") } }
            }
        }
        MenuGroup {
            MenuRow(Icons.Rounded.Edit, "Edit") { act { Ui.sheet.value = Sheet.EditPlaylist(detail.id) } }
            MenuRow(Icons.Rounded.Delete, "Delete playlist", tint = MaterialTheme.colorScheme.error) { act(onDelete) }
        }
    }
}
