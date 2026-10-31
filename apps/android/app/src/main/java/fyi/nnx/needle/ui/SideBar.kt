package fyi.nnx.needle.ui

import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.Spring
import androidx.compose.animation.core.animateDpAsState
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.snap
import androidx.compose.animation.core.spring
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsFocusedAsState
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.displayCutout
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.only
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.systemBars
import androidx.compose.foundation.layout.union
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.ViewSidebar
import androidx.compose.material.icons.outlined.Album
import androidx.compose.material.icons.outlined.FavoriteBorder
import androidx.compose.material.icons.outlined.Folder
import androidx.compose.material.icons.outlined.Home
import androidx.compose.material.icons.outlined.LibraryMusic
import androidx.compose.material.icons.outlined.MusicNote
import androidx.compose.material.icons.outlined.Person
import androidx.compose.material.icons.outlined.Schedule
import androidx.compose.material.icons.outlined.Search
import androidx.compose.material.icons.outlined.Settings
import androidx.compose.material.icons.outlined.BarChart
import androidx.compose.material.icons.outlined.Build
import androidx.compose.material.icons.outlined.CreateNewFolder
import androidx.compose.material.icons.outlined.Download
import androidx.compose.material.icons.rounded.Add
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.ripple
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.composed
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawWithContent
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.BlendMode
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.CompositingStrategy
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

/** The side bar open, as the Mac's and the computer's. */
private val Open = 240.dp

/** The side bar folded to its icons (a TV until the focus moves to it; a narrow tablet). */
private val Folded = 64.dp

/** A row's corners, and its soft highlight's. */
private val RowCorners = RoundedCornerShape(8.dp)

/**
 * The side bar on tablets and TVs, as on the Mac and the computer: the search field at the top;
 * Home, Songs, Albums, Artists, and Folders; a grey "Collections" label with Favorites,
 * Recently added, and Listening history; a grey "Playlists" label with a + and the playlists
 * with small covers; and the library's tools and Settings at the foot. Small rows, grey
 * icons; the row open has a soft highlight and an amber icon. On a TV it is folded to its
 * icons and opens while the focus is in it; on a narrow tablet the button at its top folds it.
 */
@Composable
fun SideBar(tab: Tab, page: Route, onTab: (Tab) -> Unit, open: (Route) -> Unit, show: (Tab, Route) -> Unit) {
    val tv = isTv()
    val roomy = LocalConfiguration.current.screenWidthDp >= 840
    var chosenOpen by rememberSaveable(roomy) { mutableStateOf(roomy) }
    var focused by remember { mutableStateOf(false) }
    val unfolded = if (tv) focused else chosenOpen
    val reduce = reduceMotion()
    val width by animateDpAsState(
        if (unfolded) Open else Folded,
        if (reduce) snap() else spring(stiffness = Spring.StiffnessMediumLow),
        label = "side bar",
    )
    val words by animateFloatAsState(if (unfolded) 1f else 0f, tween(if (reduce) 0 else 180), label = "side bar words")
    val playlists by rememberLoaded { playlists() }
    val inSettings = page == Route.Settings || page is Route.SettingsPage
    val colors = MaterialTheme.colorScheme
    // A place in the library opens as the Library tab's page; one open already is left be.
    val library: (Route) -> Unit = { r -> if (page != r) show(Tab.Library, r) }

    Column(
        Modifier
            .fillMaxHeight()
            // Clear of the bars and a camera cut-out on its own side only.
            .windowInsetsPadding(
                androidx.compose.foundation.layout.WindowInsets.systemBars
                    .union(androidx.compose.foundation.layout.WindowInsets.displayCutout)
                    .only(androidx.compose.foundation.layout.WindowInsetsSides.Top + androidx.compose.foundation.layout.WindowInsetsSides.Bottom + androidx.compose.foundation.layout.WindowInsetsSides.Start),
            )
            .width(width)
            .onFocusChanged { focused = it.hasFocus }
            .padding(horizontal = 10.dp),
    ) {
        // The search field (folded: its icon), and the fold button on a tablet.
        Row(Modifier.fillMaxWidth().padding(top = 10.dp, bottom = 8.dp), verticalAlignment = Alignment.CenterVertically) {
            SearchField(tab == Tab.Search, words, Modifier.weight(1f)) { onTab(Tab.Search) }
            if (!tv && words > 0.5f) {
                SmallButton(Icons.AutoMirrored.Rounded.ViewSidebar, "Fold the side bar") { chosenOpen = false }
            }
        }
        if (!tv && words < 0.5f) {
            SideRow("Open the side bar", Icons.AutoMirrored.Rounded.ViewSidebar, false, words) { chosenOpen = true }
        }
        val list = rememberLazyListState()
        LazyColumn(Modifier.weight(1f).fadeCut(list.canScrollBackward, list.canScrollForward), list) {
            item { SideRow("Home", Icons.Outlined.Home, tab == Tab.Home && page == Route.Home, words) { show(Tab.Home, Route.Home) } }
            item { SideRow("Songs", Icons.Outlined.MusicNote, page == Route.Songs, words) { library(Route.Songs) } }
            item { SideRow("Albums", Icons.Outlined.Album, page == Route.Albums, words) { library(Route.Albums) } }
            item { SideRow("Artists", Icons.Outlined.Person, page == Route.Artists, words) { library(Route.Artists) } }
            item { SideRow("Folders", Icons.Outlined.Folder, page is Route.Folder, words) { library(Route.Folder(null)) } }
            item { SideLabel("Collections", words) }
            item { SideRow("Favorites", Icons.Outlined.FavoriteBorder, page == Route.Favorites, words) { library(Route.Favorites) } }
            item { SideRow("Recently added", Icons.Outlined.Schedule, page == Route.RecentlyAdded, words) { library(Route.RecentlyAdded) } }
            item { SideRow("Listening history", Icons.Outlined.BarChart, page == Route.History, words) { library(Route.History) } }
            item { SideLabel("Playlists", words) { Ui.sheet.value = Sheet.NewPlaylist(emptyList()) } }
            val mine = playlists.orEmpty()
            if (playlists != null && mine.isEmpty() && words > 0.5f) {
                item {
                    Text(
                        "Make one with the +, or from a song's menu.",
                        style = MaterialTheme.typography.bodySmall,
                        color = colors.onSurfaceVariant,
                        modifier = Modifier.padding(start = 10.dp, end = 10.dp, bottom = 6.dp).graphicsLayer { alpha = words },
                    )
                }
            }
            items(mine, key = { "p" + it.id }) { p ->
                // By its id: a page opened before a rename still has the old name.
                val here = page is Route.Playlist && page.id == p.id
                SideRow(p.name, null, here, words, lead = { PlaylistCover(p, Modifier.size(18.dp), RoundedCornerShape(4.dp)) }) {
                    if (!here) library(Route.Playlist(p.id, p.name))
                }
            }
        }
        // The library's tools and Settings, at the foot.
        Column(Modifier.padding(top = 6.dp, bottom = 8.dp)) {
            SideRow("Add music folder", Icons.Outlined.CreateNewFolder, false, words) { show(tab, Route.SettingsPage(SettingsSection.Library)) }
            SideRow("Fix my library", Icons.Outlined.Build, page == Route.FixLibrary, words) { open(Route.FixLibrary) }
            SideRow("Import", Icons.Outlined.Download, page == Route.Import, words) { open(Route.Import) }
            SideRow("Settings", Icons.Outlined.Settings, inSettings, words) { if (page != Route.Settings) open(Route.Settings) }
        }
    }
}

/** The list's edges fade where rows go on past them, so a row cut in half looks soft (a short TV). */
private fun Modifier.fadeCut(above: Boolean, below: Boolean): Modifier = this
    .graphicsLayer { compositingStrategy = CompositingStrategy.Offscreen }
    .drawWithContent {
        drawContent()
        val fade = 28.dp.toPx()
        if (above) {
            drawRect(Brush.verticalGradient(0f to Color.Transparent, 1f to Color.Black, endY = fade), size = Size(size.width, fade), blendMode = BlendMode.DstIn)
        }
        if (below) {
            val top = size.height - fade
            drawRect(
                Brush.verticalGradient(0f to Color.Black, 1f to Color.Transparent, startY = top, endY = size.height),
                topLeft = Offset(0f, top),
                size = Size(size.width, fade),
                blendMode = BlendMode.DstIn,
            )
        }
    }

/** A tab's icon while another tab is open: outlined, as on the dock. */
internal val Tab.outline: ImageVector
    get() = when (this) {
        Tab.Home -> Icons.Outlined.Home
        Tab.Library -> Icons.Outlined.LibraryMusic
        Tab.Search -> Icons.Outlined.Search
    }

/** The search field at the top: a rounded box saying "Search"; it opens the Search page. */
@Composable
private fun SearchField(here: Boolean, words: Float, modifier: Modifier, onClick: () -> Unit) {
    val colors = MaterialTheme.colorScheme
    val interaction = remember { MutableInteractionSource() }
    Row(
        modifier
            .height(36.dp)
            .softRing(interaction, 10.dp)
            .clip(RoundedCornerShape(10.dp))
            .background(colors.onSurface.copy(alpha = if (here) 0.12f else 0.07f))
            .selectable(here, interactionSource = interaction, indication = ripple(), role = androidx.compose.ui.semantics.Role.Button, onClick = onClick)
            .semantics { contentDescription = "Search" }
            .padding(horizontal = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Icon(Icons.Outlined.Search, contentDescription = null, tint = if (here) colors.primary else colors.onSurfaceVariant, modifier = Modifier.size(18.dp))
        if (words > 0f) {
            Text(
                "Search",
                fontSize = 14.sp,
                color = colors.onSurfaceVariant,
                maxLines = 1,
                softWrap = false,
                modifier = Modifier.padding(start = 10.dp).graphicsLayer { alpha = words },
            )
        }
    }
}

/** A small icon button beside the search field. */
@Composable
private fun SmallButton(icon: ImageVector, label: String, onClick: () -> Unit) {
    val interaction = remember { MutableInteractionSource() }
    Box(
        Modifier
            .padding(start = 6.dp)
            .size(36.dp)
            .softRing(interaction, 8.dp)
            .clip(RowCorners)
            .selectable(false, interactionSource = interaction, indication = ripple(), role = androidx.compose.ui.semantics.Role.Button, onClick = onClick)
            .semantics { contentDescription = label },
        contentAlignment = Alignment.Center,
    ) { Icon(icon, contentDescription = null, tint = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.size(18.dp)) }
}

/**
 * One row of the side bar: a small grey icon (or a playlist's cover) and the name; the row open
 * takes a soft highlight and an amber icon. Folded, the icon alone, named for TalkBack.
 */
@Composable
private fun SideRow(
    label: String,
    icon: ImageVector?,
    selected: Boolean,
    words: Float,
    lead: (@Composable () -> Unit)? = null,
    onClick: () -> Unit,
) {
    val colors = MaterialTheme.colorScheme
    val fill by animateColorAsState(if (selected) colors.onSurface.copy(alpha = 0.08f) else Color.Transparent, tween(160), label = "side row")
    val interaction = remember { MutableInteractionSource() }
    Row(
        Modifier
            .padding(vertical = 1.dp)
            .fillMaxWidth()
            .height(34.dp)
            .softRing(interaction, 8.dp)
            .clip(RowCorners)
            .background(fill)
            // TalkBack says which one is open.
            .selectable(selected, interactionSource = interaction, indication = ripple(), role = androidx.compose.ui.semantics.Role.Tab, onClick = onClick)
            .then(if (words < 0.5f) Modifier.semantics { contentDescription = label } else Modifier)
            .padding(horizontal = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Box(Modifier.size(20.dp), contentAlignment = Alignment.Center) {
            if (lead != null) lead() else if (icon != null) {
                Icon(icon, contentDescription = null, tint = if (selected) colors.primary else colors.onSurfaceVariant, modifier = Modifier.size(18.dp))
            }
        }
        if (words > 0f) {
            Text(
                label,
                fontSize = 14.sp,
                fontWeight = if (selected) FontWeight.SemiBold else FontWeight.Normal,
                color = colors.onSurface,
                maxLines = 1,
                softWrap = false,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier.weight(1f).padding(start = 12.dp).graphicsLayer { alpha = words },
            )
        }
    }
}

/** A small grey label over a part of the side bar, with a + where something can be added. */
@Composable
private fun SideLabel(title: String, words: Float, onAdd: (() -> Unit)? = null) {
    if (words < 0.5f) {
        Spacer(Modifier.height(14.dp))
        return
    }
    Row(
        Modifier.fillMaxWidth().padding(start = 12.dp, top = 14.dp, bottom = 2.dp).height(28.dp).graphicsLayer { alpha = words },
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(
            title,
            fontSize = 12.sp,
            fontWeight = FontWeight.Medium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            maxLines = 1,
            softWrap = false,
            modifier = Modifier.weight(1f).semantics { heading() },
        )
        if (onAdd != null) SmallButton(Icons.Rounded.Add, "New playlist", onAdd)
    }
}

/** A remote's focus: a ring in the text colour inside the row, on the soft highlight's corners. */
private fun Modifier.softRing(interaction: androidx.compose.foundation.interaction.InteractionSource, corner: androidx.compose.ui.unit.Dp): Modifier = composed {
    val focused by interaction.collectIsFocusedAsState()
    val ring = MaterialTheme.colorScheme.onSurface
    drawWithContent {
        drawContent()
        if (!focused) return@drawWithContent
        val w = 2.dp.toPx()
        drawRoundRect(ring, Offset(w / 2, w / 2), Size(size.width - w, size.height - w), CornerRadius(corner.toPx()), style = Stroke(w))
    }
}
