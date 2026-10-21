package fyi.nnx.needle.ui

import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.Spring
import androidx.compose.animation.core.animateDpAsState
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.snap
import androidx.compose.animation.core.spring
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.only
import androidx.compose.foundation.layout.union
import androidx.compose.foundation.layout.displayCutout
import androidx.compose.foundation.layout.systemBars
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.MenuOpen
import androidx.compose.material.icons.outlined.Home
import androidx.compose.material.icons.outlined.LibraryMusic
import androidx.compose.material.icons.outlined.Search
import androidx.compose.material.icons.outlined.Settings
import androidx.compose.material.icons.rounded.Add
import androidx.compose.material.icons.rounded.Menu
import androidx.compose.material.icons.rounded.Settings
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.foundation.selection.selectable
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawWithContent
import androidx.compose.ui.graphics.BlendMode
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.CompositingStrategy
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp

/** The side bar folded: icons and covers only, in a rail as wide as Material's. */
private val Folded = 96.dp

/** The side bar open: room for names beside the icons and covers. */
private val Unfolded = 288.dp

/**
 * The side bar on tablets, as Spotify's: the tabs, then your playlists and the albums played
 * last, and Settings at the foot. It folds to icons and covers for more room; it starts open
 * where the screen is wide enough for it and a tablet's page beside it.
 */
@Composable
fun SideBar(tab: Tab, page: Route, onTab: (Tab) -> Unit, open: (Route) -> Unit) {
    val roomy = LocalConfiguration.current.screenWidthDp >= 840
    var unfolded by rememberSaveable(roomy) { mutableStateOf(roomy) }
    val reduce = reduceMotion()
    val width by animateDpAsState(
        if (unfolded) Unfolded else Folded,
        if (reduce) snap() else spring(stiffness = Spring.StiffnessMediumLow),
        label = "side bar",
    )
    val words by animateFloatAsState(if (unfolded) 1f else 0f, tween(if (reduce) 0 else 220), label = "side bar words")
    val playlists by rememberLoaded { playlists() }
    val home by rememberLoaded { home() }
    val albums = home?.let { h -> h.recent.ifEmpty { h.added } }.orEmpty().take(8)
    val go: (Route) -> Unit = { if (it != page) open(it) }
    // A short window (a phone turned on its side): the tabs and Settings scroll with the lists,
    // or the lists would have no room left at all, and Settings would fall off the foot.
    val short = androidx.compose.ui.platform.LocalConfiguration.current.screenHeightDp < 480
    val inSettings = page == Route.Settings || page is Route.SettingsPage
    val tabs: @Composable () -> Unit = {
        Tab.entries.forEach { t ->
            SideItem(t.label, null, t == tab, words, CircleShape, tab = true, onClick = { onTab(t) }) {
                Icon(if (t == tab) t.icon else t.outline, contentDescription = null)
            }
        }
    }
    val settings: @Composable () -> Unit = {
        SideItem("Settings", null, inSettings, words, CircleShape, Modifier.padding(vertical = 8.dp), onClick = { go(Route.Settings) }) {
            Icon(if (inSettings) Icons.Rounded.Settings else Icons.Outlined.Settings, contentDescription = null)
        }
    }

    // The bar is on the frame, not in a Surface: its icons take the frame's colour for text.
    androidx.compose.runtime.CompositionLocalProvider(androidx.compose.material3.LocalContentColor provides MaterialTheme.colorScheme.onSurfaceVariant) {
    Column(
        Modifier
            .fillMaxHeight()
            // Clear of the bars and a camera cut-out on its own side only: a navigation bar at the
            // far side is the pane's to keep clear of, not the rail's. Added to the bar's width,
            // not taken from it, so its buttons keep their size beside a left bar or cut-out.
            .windowInsetsPadding(
                androidx.compose.foundation.layout.WindowInsets.systemBars
                    .union(androidx.compose.foundation.layout.WindowInsets.displayCutout)
                    .only(androidx.compose.foundation.layout.WindowInsetsSides.Top + androidx.compose.foundation.layout.WindowInsetsSides.Bottom + androidx.compose.foundation.layout.WindowInsetsSides.Start),
            )
            .width(width),
    ) {
        // The fold button, and Needle's record and name beside it while the bar is open.
        Row(Modifier.padding(start = 24.dp, top = 12.dp, bottom = 8.dp), verticalAlignment = Alignment.CenterVertically) {
            IconButton(onClick = { unfolded = !unfolded }) {
                Icon(
                    if (unfolded) Icons.AutoMirrored.Rounded.MenuOpen else Icons.Rounded.Menu,
                    contentDescription = if (unfolded) "Fold the side bar" else "Open the side bar",
                )
            }
            if (words > 0f) {
                Row(Modifier.padding(start = 8.dp).graphicsLayer { alpha = words }, verticalAlignment = Alignment.CenterVertically) {
                    androidx.compose.foundation.Image(
                        androidx.compose.ui.res.painterResource(fyi.nnx.needle.R.drawable.needle_logo),
                        contentDescription = null,
                        modifier = Modifier.size(30.dp).clip(RoundedCornerShape(8.dp)),
                    )
                    Text(
                        "Needle",
                        style = MaterialTheme.typography.titleLarge.copy(fontFamily = titleFamily(0.2f)),
                        color = MaterialTheme.colorScheme.onSurface,
                        maxLines = 1,
                        softWrap = false,
                        modifier = Modifier.padding(start = 10.dp),
                    )
                }
            }
        }
        if (!short) tabs()
        // Short, Settings is the list's last row: the fade at the foot is kept off it.
        LazyColumn(Modifier.weight(1f).padding(top = 8.dp).then(if (short) Modifier else Modifier.fadeFoot())) {
            if (short) item(key = "tabs") { Column { tabs() } }
            item { SideHeader("Playlists", words) { Ui.sheet.value = Sheet.NewPlaylist(emptyList()) } }
            items(playlists.orEmpty(), key = { "p" + it.id }) { p ->
                val route = Route.Playlist(p.id, p.name)
                // By its id: a page opened before a rename still has the old name.
                val here = page is Route.Playlist && page.id == p.id
                SideItem(p.name, if (p.smart) "Smart playlist" else count(p.songs.toInt(), "song"), here, words, onClick = { if (!here) go(route) }) {
                    PlaylistCover(p, Modifier.size(48.dp), SmallCoverShape)
                }
            }
            if (albums.isNotEmpty()) {
                item { SideHeader(if (home?.recent.isNullOrEmpty()) "Recently added" else "Jump back in", words) }
                items(albums, key = { "a" + it.key }) { a ->
                    val route = Route.AlbumPage(a)
                    SideItem(a.title, a.artist, page == route, words, onClick = { go(route) }) {
                        Cover(a.artwork, Modifier.size(48.dp), SmallCoverShape, seed = a.title + a.artist)
                    }
                }
            }
            if (short) item(key = "settings") { settings() }
        }
        if (!short) settings()
    }
    }
}

/** A tab's icon while another tab is open: outlined, as Material's navigation does. */
internal val Tab.outline: ImageVector
    get() = when (this) {
        Tab.Home -> Icons.Outlined.Home
        Tab.Library -> Icons.Outlined.LibraryMusic
        Tab.Search -> Icons.Outlined.Search
    }

/**
 * One row of the side bar: an icon or a cover, and its name beside it while the bar is open.
 * The icon or cover keeps one place, so folding only hides the words.
 */
@Composable
private fun SideItem(
    label: String,
    sub: String?,
    selected: Boolean,
    words: Float,
    shape: Shape = RoundedCornerShape(16.dp),
    modifier: Modifier = Modifier,
    /** A tab to TalkBack; the playlists, albums, and Settings are buttons. */
    tab: Boolean = false,
    onClick: () -> Unit,
    lead: @Composable () -> Unit,
) {
    val colors = MaterialTheme.colorScheme
    val fill by animateColorAsState(if (selected) colors.secondaryContainer else Color.Transparent, tween(200), label = "side item")
    Row(
        modifier
            .padding(horizontal = 12.dp, vertical = 2.dp)
            .fillMaxWidth()
            .clip(shape)
            .background(fill)
            // TalkBack says which one is open.
            .selectable(selected, role = if (tab) androidx.compose.ui.semantics.Role.Tab else androidx.compose.ui.semantics.Role.Button, onClick = onClick)
            .heightIn(min = 56.dp)
            .padding(horizontal = 12.dp, vertical = 4.dp)
            .then(if (words == 0f) Modifier.semantics { contentDescription = label } else Modifier),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Box(Modifier.size(48.dp), contentAlignment = Alignment.Center) {
            androidx.compose.runtime.CompositionLocalProvider(
                androidx.compose.material3.LocalContentColor provides if (selected) colors.onSecondaryContainer else colors.onSurfaceVariant,
            ) { lead() }
        }
        if (words > 0f) {
            Column(Modifier.weight(1f).padding(start = 14.dp).graphicsLayer { alpha = words }) {
                Text(
                    label,
                    style = if (sub == null) MaterialTheme.typography.labelLarge else MaterialTheme.typography.titleSmall,
                    color = if (selected) colors.onSecondaryContainer else colors.onSurface,
                    maxLines = 1,
                    softWrap = false,
                    overflow = TextOverflow.Ellipsis,
                )
                if (sub != null) {
                    Text(
                        sub,
                        style = MaterialTheme.typography.bodySmall,
                        color = colors.onSurfaceVariant,
                        maxLines = 1,
                        softWrap = false,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
            }
        }
    }
}

/** A heading in the side bar, with an add button; folded, a short line instead. */
@Composable
private fun SideHeader(title: String, words: Float, onAdd: (() -> Unit)? = null) {
    if (words == 0f) {
        HorizontalDivider(Modifier.padding(horizontal = 32.dp, vertical = 12.dp))
        return
    }
    Row(
        Modifier.fillMaxWidth().height(48.dp).padding(start = 24.dp, end = 12.dp).graphicsLayer { alpha = words },
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(
            title,
            style = MaterialTheme.typography.labelLarge,
            color = MaterialTheme.colorScheme.primary,
            maxLines = 1,
            softWrap = false,
            modifier = Modifier.weight(1f),
        )
        if (onAdd != null) {
            IconButton(onClick = onAdd) { Icon(Icons.Rounded.Add, contentDescription = "New playlist") }
        }
    }
}

/** The list fades out at its foot instead of stopping at a hard edge above Settings. */
private fun Modifier.fadeFoot(): Modifier = this
    .graphicsLayer { compositingStrategy = CompositingStrategy.Offscreen }
    .drawWithContent {
        drawContent()
        val fade = 32.dp.toPx()
        drawRect(
            Brush.verticalGradient(listOf(Color.Black, Color.Transparent), startY = size.height - fade, endY = size.height),
            topLeft = androidx.compose.ui.geometry.Offset(0f, size.height - fade),
            size = androidx.compose.ui.geometry.Size(size.width, fade),
            blendMode = BlendMode.DstIn,
        )
    }
