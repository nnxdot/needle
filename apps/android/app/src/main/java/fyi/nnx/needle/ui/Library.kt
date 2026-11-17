package fyi.nnx.needle.ui

import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.spring
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.LocalIndication
import androidx.compose.foundation.ScrollState
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.indication
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.GridItemSpan
import androidx.compose.foundation.lazy.grid.LazyGridScope
import androidx.compose.foundation.lazy.grid.LazyGridState
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.rememberLazyGridState
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.relocation.BringIntoViewRequester
import androidx.compose.foundation.relocation.bringIntoViewRequester
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.ViewList
import androidx.compose.material.icons.rounded.Add
import androidx.compose.material.icons.rounded.Album
import androidx.compose.material.icons.rounded.AutoAwesome
import androidx.compose.material.icons.rounded.BarChart
import androidx.compose.material.icons.rounded.BubbleChart
import androidx.compose.material.icons.rounded.Check
import androidx.compose.material.icons.rounded.Favorite
import androidx.compose.material.icons.rounded.GridView
import androidx.compose.material.icons.rounded.History
import androidx.compose.material.icons.rounded.KeyboardArrowDown
import androidx.compose.material.icons.rounded.Radio
import androidx.compose.material.icons.rounded.Settings
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.draw.drawWithContent
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.BlendMode
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.CompositingStrategy
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.layout.onPlaced
import androidx.compose.ui.layout.positionInParent
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.LayoutDirection
import androidx.compose.ui.unit.dp
import fyi.nnx.needle.NeedleApp
import fyi.nnx.needle.core.Album
import fyi.nnx.needle.core.Song
import kotlinx.coroutines.launch

/** The words of the Library's bar, in its order; All is the overview. */
private enum class Filter(val label: String) {
    All("All"), Playlists("Playlists"), Albums("Albums"), Artists("Artists"), Songs("Songs"), Genres("Genres"), Folders("Folders"),
}

private enum class Sort(val label: String) {
    Recent("Recently added"), Title("Title"), Artist("Artist");

    /** The order songs take: without a date they were added, newest first is by title. */
    fun forSongs(songs: Boolean) = if (songs && this == Recent) Title else this
}

/**
 * The library as one page: one bar of words at the top, a soft light under the chosen one (as
 * the dock's tabs), and the music itself below, sorted as chosen. All: a shelf of covers
 * (Favorites, History, Radio, the playlists), what came in lately, and the library's other pages.
 */
@OptIn(ExperimentalMaterial3Api::class, androidx.compose.material3.ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun LibraryScreen(open: (Route) -> Unit) {
    var filter by rememberSaveable { mutableStateOf(Filter.All) }
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
                FilterBar(filter) { filter = it }
                AnimatedContent(filter, transitionSpec = { fadeIn(tween(200)) togetherWith fadeOut(tween(120)) }, label = "filter") { f ->
                    when (f) {
                        Filter.All -> Overview(open, scroll) { sort = Sort.Recent; filter = Filter.Albums }
                        Filter.Albums -> Albums(sort, { sort = it }, open, scroll)
                        Filter.Artists -> Artists(open, scroll)
                        Filter.Songs -> Songs(sort.forSongs(true), { sort = it }, open, scroll)
                        Filter.Playlists -> Playlists(open, scroll)
                        Filter.Genres -> Genres(open, scroll)
                        Filter.Folders -> Folders(open, scroll)
                    }
                }
            }
        }
    }
}

// ---------- The bar

/** Where a word of the bar sits in its row, for the light under the chosen one. */
private data class Spot(val left: Float, val width: Float)

/**
 * The bar of words: one quiet pill, the words in it scrolling sideways where they do not fit
 * (their cut edges fade). The chosen word is in the accent on a soft light that slides from word
 * to word, as the dock's tab light does; with Reduce motion it moves at once. The bar looks 40 dp
 * high, and each word keeps 48 dp to touch. To TalkBack the words are tabs; a remote moves along
 * them with left and right.
 */
@Composable
private fun FilterBar(filter: Filter, choose: (Filter) -> Unit) = WordBar(Filter.entries, filter, { it.label }, choose)

/**
 * The Library's bar of words, for any set of choices (the Search page's kinds too), so "chosen"
 * looks the same wherever it shows.
 */
@Composable
internal fun <T : Any> WordBar(options: List<T>, filter: T, label: (T) -> String, choose: (T) -> Unit) {
    val reduce = reduceMotion()
    val scroll = rememberScrollState()
    val spots = remember { mutableStateMapOf<T, Spot>() }
    val left = remember { Animatable(0f) }
    val width = remember { Animatable(0f) }
    var placed by remember { mutableStateOf(false) }
    val target = spots[filter]
    LaunchedEffect(target, reduce) {
        val to = target ?: return@LaunchedEffect
        // The first time it is placed it is simply there; after that it slides.
        if (!placed || reduce) {
            left.snapTo(to.left)
            width.snapTo(to.width)
            placed = true
        } else {
            val slide = spring<Float>(dampingRatio = 0.85f, stiffness = 500f)
            launch { left.animateTo(to.left, slide) }
            width.animateTo(to.width, slide)
        }
    }
    val light = MaterialTheme.colorScheme.onSurface.copy(alpha = 0.09f)
    Box(Modifier.fillMaxWidth().padding(horizontal = 16.dp).height(48.dp), contentAlignment = Alignment.Center) {
        // The bar to see: the page's raised tone with a faint edge, 40 dp of the 48 dp place.
        Box(
            Modifier
                .fillMaxWidth()
                .height(40.dp)
                .clip(CircleShape)
                .background(MaterialTheme.colorScheme.surfaceContainerLow)
                .border(1.dp, MaterialTheme.colorScheme.onSurface.copy(alpha = 0.08f), CircleShape),
        )
        Row(
            Modifier
                .fillMaxSize()
                .fadeSides(scroll)
                .horizontalScroll(scroll)
                .selectableGroup()
                .padding(horizontal = 4.dp)
                // Read only where it is drawn: the slide does not compose the words again.
                .drawBehind {
                    if (!placed) return@drawBehind
                    val h = 32.dp.toPx()
                    drawRoundRect(
                        light,
                        topLeft = Offset(left.value, (size.height - h) / 2),
                        size = Size(width.value, h),
                        cornerRadius = CornerRadius(h / 2),
                    )
                },
            verticalAlignment = Alignment.CenterVertically,
        ) {
            options.forEach { f ->
                FilterWord(
                    label(f),
                    chosen = f == filter,
                    modifier = Modifier.onPlaced { c ->
                        val spot = Spot(c.positionInParent().x, c.size.width.toFloat())
                        if (spots[f] != spot) spots[f] = spot
                    },
                ) { choose(f) }
            }
        }
    }
}

/** One word of the bar: 48 dp to touch, its ripple and focus ring on the light's 32 dp pill. */
@Composable
private fun FilterWord(label: String, chosen: Boolean, modifier: Modifier, onClick: () -> Unit) {
    val interaction = remember { MutableInteractionSource() }
    val bring = remember { BringIntoViewRequester() }
    // The chosen word is brought into view when it is cut off (chosen from See all, or come back to).
    LaunchedEffect(chosen) { if (chosen) bring.bringIntoView() }
    Box(
        modifier
            .bringIntoViewRequester(bring)
            .height(48.dp)
            .semantics { selected = chosen }
            .clickable(interactionSource = interaction, indication = null, role = Role.Tab, onClick = onClick),
        contentAlignment = Alignment.Center,
    ) {
        Box(
            Modifier
                .height(32.dp)
                .clip(CircleShape)
                .indication(interaction, LocalIndication.current)
                .padding(horizontal = 13.dp),
            contentAlignment = Alignment.Center,
        ) {
            Text(
                label,
                style = MaterialTheme.typography.labelLarge,
                fontWeight = if (chosen) FontWeight.SemiBold else FontWeight.Medium,
                color = if (chosen) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurfaceVariant,
                maxLines = 1,
            )
        }
    }
}

/** The bar's ends fade where words run on past them, so a word cut in half looks soft. */
private fun Modifier.fadeSides(scroll: ScrollState): Modifier = this
    .graphicsLayer { compositingStrategy = CompositingStrategy.Offscreen }
    .drawWithContent {
        drawContent()
        val fade = 24.dp.toPx()
        // Words cut at the start, and at the end; the start is at the right in a right-to-left language.
        val start = scroll.value > 0
        val end = scroll.value < scroll.maxValue
        val rtl = layoutDirection == LayoutDirection.Rtl
        if (if (rtl) end else start) {
            drawRect(Brush.horizontalGradient(0f to Color.Transparent, 1f to Color.Black, endX = fade), size = Size(fade, size.height), blendMode = BlendMode.DstIn)
        }
        if (if (rtl) start else end) {
            val x = size.width - fade
            drawRect(
                Brush.horizontalGradient(0f to Color.Black, 1f to Color.Transparent, startX = x, endX = size.width),
                topLeft = Offset(x, 0f),
                size = Size(fade, size.height),
                blendMode = BlendMode.DstIn,
            )
        }
    }

// ---------- The line under the bar

/** Under the bar once a kind is chosen: how many in grey at the left; its order and button at the right. */
@Composable
private fun KindLine(count: String?, controls: @Composable RowScope.() -> Unit = {}) {
    Row(
        Modifier.fillMaxWidth().heightIn(min = 48.dp).padding(start = 16.dp, end = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(
            count.orEmpty(),
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            maxLines = 1,
            modifier = Modifier.weight(1f),
        )
        controls()
    }
}

/** The order in words with a small chevron; a tap opens its choices (DESIGN.md: small choices are dropdown menus). */
@Composable
private fun OrderButton(label: String, items: @Composable (close: () -> Unit) -> Unit) {
    var open by remember { mutableStateOf(false) }
    Box {
        Row(
            Modifier
                .heightIn(min = 48.dp)
                .clip(CircleShape)
                .clickable(role = Role.Button, onClickLabel = "Change the order") { open = true }
                .semantics { contentDescription = "Order: $label" }
                .padding(start = 10.dp, end = 6.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(2.dp),
        ) {
            Text(label, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant, maxLines = 1, modifier = Modifier.clearAndSetSemantics {})
            Icon(Icons.Rounded.KeyboardArrowDown, contentDescription = null, tint = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.size(16.dp))
        }
        GlassMenu(expanded = open, onDismissRequest = { open = false }) { items { open = false } }
    }
}

/** One order in the menu, the one in use checked. */
@Composable
private fun OrderItem(label: String, chosen: Boolean, onClick: () -> Unit) {
    DropdownMenuItem(
        text = { Text(label) },
        trailingIcon = { if (chosen) Icon(Icons.Rounded.Check, null, tint = MaterialTheme.colorScheme.primary) },
        onClick = onClick,
    )
}

/** A round quiet button at the line's right: 36 dp to see, 48 dp to touch. */
@Composable
private fun RoundButton(icon: ImageVector, label: String, onClick: () -> Unit) {
    val interaction = remember { MutableInteractionSource() }
    Box(
        Modifier
            .size(48.dp)
            .clickable(interactionSource = interaction, indication = null, role = Role.Button, onClick = onClick)
            .semantics { contentDescription = label },
        contentAlignment = Alignment.Center,
    ) {
        Box(
            Modifier
                .size(36.dp)
                .clip(CircleShape)
                .background(MaterialTheme.colorScheme.surfaceContainerHigh)
                .indication(interaction, LocalIndication.current),
            contentAlignment = Alignment.Center,
        ) {
            Icon(icon, contentDescription = null, tint = MaterialTheme.colorScheme.onSurface, modifier = Modifier.size(18.dp))
        }
    }
}

/** Grid or list in one tap: the icon shows what a tap gives. */
@Composable
private fun ViewToggle(asList: Boolean, choose: (Boolean) -> Unit) {
    if (asList) {
        RoundButton(Icons.Rounded.GridView, "Show as a grid") { choose(false) }
    } else {
        RoundButton(Icons.AutoMirrored.Rounded.ViewList, "Show as a list") { choose(true) }
    }
}

// ---------- Albums in a grid

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

private fun LazyGridScope.span(key: Any, content: @Composable () -> Unit) =
    item(key = key, span = { GridItemSpan(maxLineSpan) }) { content() }

/** The album grid's columns: three across a phone, as many 160 dp ones as fit beside the side bar. */
@Composable
private fun albumColumns(): Int? = if (hasSideBar()) null else 3

/**
 * Albums with the page's margins: three across on a phone (their words a size smaller), as many
 * as fit on a tablet. `entrance`: the tiles come in one after another as the page opens.
 */
@Composable
private fun AlbumGrid(
    albums: List<Album>?,
    open: (Route) -> Unit,
    modifier: Modifier,
    loading: Boolean,
    state: LazyGridState = rememberLazyGridState(),
    entrance: Boolean = false,
    header: LazyGridScope.() -> Unit = {},
    footer: LazyGridScope.() -> Unit = {},
) {
    val fixed = albumColumns()
    val list = albums.orEmpty()
    LazyVerticalGrid(
        columns = if (fixed != null) GridCells.Fixed(fixed) else GridCells.Adaptive(160.dp),
        state = state,
        contentPadding = PaddingValues(start = 16.dp, end = 16.dp, top = 4.dp, bottom = 32.dp + LocalBottomInset.current),
        horizontalArrangement = Arrangement.spacedBy(if (fixed != null) 10.dp else 14.dp),
        verticalArrangement = Arrangement.spacedBy(LocalRows.current.grid),
        modifier = Modifier.fillMaxSize().then(modifier),
    ) {
        header()
        if (loading) items(6) { AlbumPlaceholder() }
        items(list.size, key = { list[it].key }) { i ->
            val album = list[i]
            AlbumTile(album, if (entrance) Modifier.entrance(i + 4) else Modifier, open, small = fixed != null) { open(Route.AlbumPage(album)) }
        }
        footer()
    }
}

// ---------- All

@Composable
private fun Overview(open: (Route) -> Unit, scroll: Modifier, seeAll: () -> Unit) {
    val albumsLoad = rememberLoaded { albums() }
    val albums by albumsLoad
    // Four rows on a phone; two are enough on a wide screen (columns as GridCells.Adaptive counts them).
    val fixed = albumColumns()
    val page = LocalPageWidth.current
    val take = if (fixed != null) fixed * 4 else ((page - 32.dp + 14.dp) / (160.dp + 14.dp)).toInt().coerceAtLeast(2) * 2
    val shown = remember(albums, take) { albums?.let { sorted(it, Sort.Recent).take(take) } }
    val loading = albumsLoad.loading
    AlbumGrid(
        shown,
        open,
        scroll,
        loading,
        entrance = true,
        header = {
            edgeToEdge(16.dp, key = "library/shelf") { LibraryShelf(open) }
            // An empty library has no "Recently added" with nothing under it.
            if (loading || !shown.isNullOrEmpty()) span("library/recent") { RecentTitle(seeAll) }
        },
        footer = {
            span("library/explore") { ExploreLibrary(open, shown) }
        },
    )
}

/** "Recently added", with a grey See all at its right: the Albums, newest first. */
@Composable
private fun RecentTitle(seeAll: () -> Unit) {
    Row(Modifier.fillMaxWidth().padding(top = 8.dp), verticalAlignment = Alignment.CenterVertically) {
        Text(
            "Recently added",
            style = MaterialTheme.typography.titleLarge,
            fontWeight = FontWeight.SemiBold,
            modifier = Modifier.weight(1f).semantics { heading() },
        )
        // Grey, not the accent: the accent marks only what plays and the one main button.
        TextButton(onClick = seeAll, colors = ButtonDefaults.textButtonColors(contentColor = MaterialTheme.colorScheme.onSurfaceVariant)) {
            Text("See all")
        }
    }
}

/** What the shelf shows of Favorites, History, and Radio: covers and a count, read off the main thread. */
private class Places(val favorites: Int, val loved: List<String>, val played: List<String>, val station: String?)

/** A shelf card's width: three cards and a clear slice of the fourth on a phone. */
@Composable
private fun shelfCardWidth(): Dp {
    if (isWide()) return 132.dp
    val fit = (LocalPageWidth.current - 16.dp - 36.dp) / 3.4f
    return maxOf(92.dp, minOf(132.dp, fit))
}

/**
 * The shelf at the top of All: Favorites, History, and Radio as covers (each with a small round
 * sign, so they read as places, not albums), then the playlists, in the library's order.
 */
@Composable
private fun LibraryShelf(open: (Route) -> Unit) {
    val placesLoad = rememberLoaded {
        val hearted = favorites()
        val loved = hearted.mapNotNull { it.artwork }.distinct()
        // The covers of what played last, newest first; a song gone from the library has none.
        val played = runCatching { history(0u, 60u) }.getOrDefault(emptyList()).mapNotNull { it.song?.artwork }.distinct().take(4)
        // Radio's cover: one more from the favourites than Favorites shows, else one played lately.
        Places(hearted.size, loved.take(4), played, loved.getOrNull(4) ?: played.firstOrNull() ?: loved.firstOrNull())
    }
    val places by placesLoad
    val playlistsLoad = rememberLoaded { playlists().take(12) }
    val playlists by playlistsLoad
    val list = playlists.orEmpty()
    val width = shelfCardWidth()
    LazyRow(contentPadding = PaddingValues(horizontal = 16.dp), horizontalArrangement = Arrangement.spacedBy(12.dp)) {
        item(key = "favorites") {
            ShelfCard(width, "Favorites", places?.let { count(it.favorites, "song") }, Icons.Rounded.Favorite, 0, onClick = { open(Route.Favorites) }) {
                Mosaic(places?.loved.orEmpty(), null, it, CoverShape, seed = "Favorites")
            }
        }
        item(key = "history") {
            ShelfCard(width, "History", "What you played", Icons.Rounded.History, 1, onClick = { open(Route.History) }) {
                Mosaic(places?.played.orEmpty(), null, it, CoverShape, seed = "History")
            }
        }
        item(key = "radio") {
            ShelfCard(width, "Radio", "Your station", Icons.Rounded.Radio, 2, darkFoot = true, onClick = { open(Route.Radio) }) {
                Cover(places?.station, it, CoverShape, seed = "Radio")
            }
        }
        items(list.size, key = { "playlist/" + list[it].id }) { i ->
            val playlist = list[i]
            ShelfCard(
                width,
                playlist.name,
                null,
                null,
                3 + i,
                line = { PlaylistLine(playlist, MaterialTheme.typography.bodySmall) },
                onClick = { open(Route.Playlist(playlist.id, playlist.name)) },
            ) {
                PlaylistCover(playlist, it, CoverShape)
            }
        }
    }
}

/** A card on the shelf: a square cover with a soft shadow, the name, and one grey line. */
@Composable
private fun ShelfCard(
    width: Dp,
    title: String,
    detail: String?,
    badge: ImageVector?,
    index: Int,
    darkFoot: Boolean = false,
    line: (@Composable () -> Unit)? = null,
    onClick: () -> Unit,
    cover: @Composable (Modifier) -> Unit,
) {
    val interaction = remember { MutableInteractionSource() }
    Column(
        Modifier
            .width(width)
            .entrance(index)
            .pressScale(interaction)
            .clickable(interactionSource = interaction, indication = null, role = Role.Button, onClick = onClick),
    ) {
        Box(
            Modifier
                .fillMaxWidth()
                .aspectRatio(1f)
                .shadow(6.dp, CoverShape, ambientColor = Color.Black.copy(alpha = 0.3f), spotColor = Color.Black.copy(alpha = 0.3f)),
        ) {
            cover(Modifier.fillMaxSize())
            // Radio's cover darkens at its foot, as the station on the Radio page.
            if (darkFoot) {
                Box(Modifier.fillMaxSize().background(Brush.verticalGradient(0.45f to Color.Transparent, 1f to Color.Black.copy(alpha = 0.55f))))
            }
            if (badge != null) PlaceBadge(badge, Modifier.align(Alignment.BottomStart).padding(6.dp))
        }
        Text(
            title,
            style = MaterialTheme.typography.bodyMedium,
            fontWeight = FontWeight.Medium,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.padding(top = 8.dp),
        )
        when {
            line != null -> line()
            detail != null -> Text(
                detail,
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
        }
    }
}

/** The small round sign on a place's cover: white on dark, so it reads on any cover. */
@Composable
private fun PlaceBadge(icon: ImageVector, modifier: Modifier) {
    Box(
        modifier
            .size(26.dp)
            .clip(CircleShape)
            .background(Color.Black.copy(alpha = 0.52f))
            .border(1.dp, Color.White.copy(alpha = 0.2f), CircleShape),
        contentAlignment = Alignment.Center,
    ) {
        Icon(icon, contentDescription = null, tint = Color.White, modifier = Modifier.size(14.dp))
    }
}

private class ExploreTile(val title: String, val detail: String, val icon: ImageVector, val route: Route)

/**
 * The foot of All: the library's other pages, as plain tiles like the Radio page's quick mixes,
 * each sign in the colour of a cover from what came in lately. Two across a phone, four wide.
 */
@Composable
private fun ExploreLibrary(open: (Route) -> Unit, albums: List<Album>?) {
    val factsLoad = rememberLoaded { genres().size to wrappedYears().firstOrNull() }
    val facts by factsLoad
    val picks = remember(albums) { albums.orEmpty().distinctBy { it.artwork ?: (it.title + it.artist) }.take(4) }
    val tiles = listOf(
        ExploreTile("Record crate", "Flip the covers", Icons.Rounded.Album, Route.Crate),
        ExploreTile("Genre map", facts?.let { count(it.first, "genre") } ?: "Your genres", Icons.Rounded.BubbleChart, Route.MoodMap),
        ExploreTile("In numbers", "Your library, counted", Icons.Rounded.BarChart, Route.Numbers),
        ExploreTile("Your year", facts?.second?.toString() ?: "What you played", Icons.Rounded.AutoAwesome, Route.Wrapped(null)),
    )
    val columns = if (isWide()) 4 else 2
    Column(Modifier.fillMaxWidth().padding(top = 12.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
        Text(
            "Explore your library",
            style = MaterialTheme.typography.titleLarge,
            fontWeight = FontWeight.SemiBold,
            modifier = Modifier.semantics { heading() },
        )
        tiles.chunked(columns).forEachIndexed { r, row ->
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                row.forEachIndexed { c, tile ->
                    val pick = picks.getOrNull(r * columns + c)
                    PlainTile(tile.title, tile.detail, tile.icon, pick?.artwork, pick?.let { it.title + it.artist }, Modifier.weight(1f)) { open(tile.route) }
                }
            }
        }
    }
}

// ---------- The kinds

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

/** The letter a row is filed under, in this order; none when the order is not by name. */
private fun letterOf(sort: Sort, title: String, artist: String): String? = when (sort) {
    Sort.Recent -> null
    Sort.Title -> indexLetter(title)
    Sort.Artist -> indexLetter(artist)
}

@Composable
private fun Albums(sort: Sort, sortBy: (Sort) -> Unit, open: (Route) -> Unit, scroll: Modifier) {
    val albumsLoad = rememberLoaded { albums() }
    val albums by albumsLoad
    val asList by LibraryView.albumsAsList.collectAsState()
    // Sorted once per list and order, not at every redraw.
    val list = remember(albums, sort) { albums?.let { sorted(it, sort) } }
    val all = list.orEmpty()
    val letter = { i: Int -> all.getOrNull(i)?.let { letterOf(sort, it.title, it.artist) } }
    Column(Modifier.fillMaxSize().then(scroll)) {
        KindLine(albums?.let { count(it.size, "album") }) {
            OrderButton(sort.label) { close ->
                Sort.entries.forEach { s -> OrderItem(s.label, s == sort) { close(); sortBy(s) } }
            }
            ViewToggle(asList, LibraryView::setAlbums)
        }
        Box(Modifier.weight(1f).fillMaxWidth()) {
            if (asList) {
                val state = rememberLazyListState()
                LazyColumn(Modifier.fillMaxSize(), state = state, contentPadding = PaddingValues(bottom = 32.dp + LocalBottomInset.current)) {
                    if (albumsLoad.loading) items(8) { SongPlaceholder() }
                    items(all.size, key = { all[it].key }) { i -> AlbumRow(all[i], Place(i, all.size), open) }
                }
                QuickScroll(state, all.size, Modifier.align(Alignment.TopEnd).padding(top = 8.dp, bottom = LocalBottomInset.current), letter)
            } else {
                val state = rememberLazyGridState()
                AlbumGrid(list, open, Modifier, albumsLoad.loading, state)
                QuickScroll(state, all.size, Modifier.align(Alignment.TopEnd).padding(top = 8.dp, bottom = LocalBottomInset.current), letter)
            }
        }
    }
}

@Composable
private fun Artists(open: (Route) -> Unit, scroll: Modifier) {
    val more by NeedleApp.instance.more.collectAsState()
    val asList by LibraryView.artistsAsList.collectAsState()
    ArtistGrid(open, scroll, top = 4.dp, above = { n ->
        KindLine(n?.let { count(it, "artist") }) {
            OrderButton(artistOrderLabel(more.artistOrder)) { close -> ArtistMenuItems(close) }
            ViewToggle(asList, LibraryView::setArtists)
        }
    })
}

@Composable
private fun Songs(sort: Sort, sortBy: (Sort) -> Unit, open: (Route) -> Unit, scroll: Modifier) {
    val songsLoad = rememberLoaded { songs() }
    val songs by songsLoad
    val playing = playingSongId()
    // Sorted once per list and order, not at every redraw.
    val list = remember(songs, sort) { songs?.let { sortedSongs(it, sort) }.orEmpty() }
    val state = rememberLazyListState()
    Column(Modifier.fillMaxSize().then(scroll)) {
        KindLine(songs?.let { count(it.size, "song") }) {
            // Songs carry no date they were added, so they sort by name or artist only.
            OrderButton(sort.label) { close ->
                (Sort.entries - Sort.Recent).forEach { s -> OrderItem(s.label, s == sort) { close(); sortBy(s) } }
            }
        }
        Box(Modifier.weight(1f).fillMaxWidth()) {
            LazyColumn(Modifier.fillMaxSize(), state = state, contentPadding = PaddingValues(bottom = 32.dp + LocalBottomInset.current)) {
                if (songsLoad.loading) items(8) { SongPlaceholder() }
                items(list.size, key = { list[it].id }) { i ->
                    val song = list[i]
                    SongRow(song, playing == song.id, open, place = list.place(i)) {
                        playSongs(list.map { it.id }, i)
                    }
                }
            }
            QuickScroll(state, list.size, Modifier.align(Alignment.TopEnd).padding(top = 8.dp, bottom = LocalBottomInset.current)) { i ->
                list.getOrNull(i)?.let { letterOf(sort, it.title, it.artist) }
            }
        }
    }
}

@Composable
private fun Playlists(open: (Route) -> Unit, scroll: Modifier) {
    val playlistsLoad = rememberLoaded { playlists() }
    val playlists by playlistsLoad
    // One list, read once: a reload that fails leaves it, not a null, to the rows.
    val list = playlists.orEmpty()
    val count = list.size
    Column(Modifier.fillMaxSize().then(scroll)) {
        // The core gives playlists no order to choose: the count, and + for a new one.
        KindLine(playlists?.let { count(it.size, "playlist") }) {
            RoundButton(Icons.Rounded.Add, "New playlist") { Ui.sheet.value = Sheet.NewPlaylist(emptyList()) }
        }
        LazyColumn(Modifier.weight(1f).fillMaxWidth(), contentPadding = PaddingValues(bottom = 32.dp + LocalBottomInset.current)) {
            if (playlists != null && list.isEmpty()) item {
                Text(
                    "No playlists yet. Tap + to make one.",
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp),
                )
            }
            items(count, key = { list[it].id }) { i ->
                val playlist = list[i]
                Row(
                    Modifier.fillMaxWidth().segment(Place(i, count), groupColor(), lineStart = 82.dp).clickable { open(Route.Playlist(playlist.id, playlist.name)) }
                        .heightIn(min = LocalRows.current.height + 12.dp).padding(horizontal = 12.dp, vertical = 8.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(14.dp),
                ) {
                    PlaylistCover(playlist, Modifier.size(56.dp), SmallCoverShape)
                    Column(Modifier.weight(1f)) {
                        Text(playlist.name, style = MaterialTheme.typography.bodyLarge, maxLines = 1, overflow = TextOverflow.Ellipsis)
                        PlaylistLine(playlist)
                    }
                }
            }
        }
    }
}

/** The genres in place under the bar, as on the Genres page; a tap opens the genre's page. */
@Composable
private fun Genres(open: (Route) -> Unit, scroll: Modifier) {
    val genresLoad = rememberLoaded { genres() }
    val genres by genresLoad
    Column(Modifier.fillMaxSize().then(scroll)) {
        KindLine(genres?.let { count(it.size, "genre") })
        GenreGrid(genres.orEmpty(), open, Modifier.weight(1f).fillMaxWidth(), top = 4.dp)
    }
}

/** The music folders in place under the bar, as at the top of the Folders page; a tap opens one. */
@Composable
private fun Folders(open: (Route) -> Unit, scroll: Modifier) {
    val rootsLoad = rememberLoaded { folders() }
    val roots by rootsLoad
    Column(Modifier.fillMaxSize().then(scroll)) {
        KindLine(roots?.let { count(it.size, "folder") })
        LazyColumn(Modifier.weight(1f).fillMaxWidth(), contentPadding = PaddingValues(bottom = 32.dp + LocalBottomInset.current)) {
            folderRoots(roots, open)
        }
    }
}
