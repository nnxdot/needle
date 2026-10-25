package fyi.nnx.needle.ui

import androidx.activity.compose.BackHandler
import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.ExperimentalSharedTransitionApi
import androidx.compose.animation.SharedTransitionLayout
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInHorizontally
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.slideOutHorizontally
import androidx.compose.animation.slideOutVertically
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.isImeVisible
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.only
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Home
import androidx.compose.material.icons.rounded.LibraryMusic
import androidx.compose.material.icons.rounded.Search
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.foundation.clickable
import androidx.compose.ui.Modifier
import androidx.compose.foundation.focusGroup
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.unit.dp
import dev.chrisbanes.haze.hazeSource
import fyi.nnx.needle.NeedleApp
import fyi.nnx.needle.core.Album

/** A page inside a tab. */
sealed interface Route {
    data object Home : Route
    data object Library : Route
    data object Search : Route
    data object Settings : Route
    data object Playlists : Route
    data object Artists : Route
    data object Albums : Route
    data object Songs : Route
    data object Genres : Route
    data class AlbumPage(val album: Album) : Route
    /** The album a song is on, found when the page opens. */
    data class AlbumOf(val title: String, val songId: String) : Route
    data class Artist(val name: String) : Route
    data class Playlist(val id: String, val name: String) : Route
    data class Genre(val name: String) : Route
    /** One section of Settings. */
    data class SettingsPage(val section: SettingsSection) : Route
    data object Favorites : Route
    data object RecentlyAdded : Route
    data object History : Route
    /** A music folder or a folder in one; `null` lists the music folders. */
    data class Folder(val path: String?) : Route
    data class Wrapped(val year: Int?) : Route
    data object Numbers : Route
    data object MoodMap : Route
    data object Crate : Route
    data object Plugins : Route
    data class Plugin(val id: String) : Route
    data object Connect : Route
    data object Sync : Route
    data object WhatsNew : Route
    /** Timing a song's lyrics by tapping along. */
    data class Timing(val songId: String) : Route
    /** Privacy or Help, in the app. */
    data class Document(val doc: Doc) : Route
    data object FixLibrary : Route
    data object Import : Route
    /** A theme of your own; `null` makes a new one. */
    data class ThemeEditor(val id: String?) : Route
}

enum class Tab(val label: String, val icon: ImageVector, val root: Route) {
    Home("Home", Icons.Rounded.Home, Route.Home),
    Library("Library", Icons.Rounded.LibraryMusic, Route.Library),
    Search("Search", Icons.Rounded.Search, Route.Search),
}

@OptIn(ExperimentalSharedTransitionApi::class, androidx.compose.material3.ExperimentalMaterial3ExpressiveApi::class, androidx.compose.foundation.layout.ExperimentalLayoutApi::class)
@Composable
fun NeedleRoot() {
    var tab by rememberSaveable { mutableStateOf(Tab.Home) }
    // Each tab keeps its own pages, as in Apple Music.
    val stacks = remember { mutableStateMapOf<Tab, List<Route>>() }
    var playerOpen by rememberSaveable { mutableStateOf(false) }
    // Only whether a song is there, and its cover: the whole playback changes five times a
    // second as it plays, and would redraw every page with it.
    val playbackState = NeedleApp.instance.playback.collectAsState()
    val hasSong by remember { androidx.compose.runtime.derivedStateOf { playbackState.value?.current != null } }
    val playingArt by remember { androidx.compose.runtime.derivedStateOf { playbackState.value?.current?.artwork } }
    val reduce = reduceMotion()
    // The same list each time while the tab has only its first page, so what takes `open`
    // is not redrawn for nothing.
    val rootStack = remember(tab) { listOf(tab.root) }
    val stack = stacks[tab] ?: rootStack
    // A second tap while the page slides in (or OK pressed twice) does not open it twice.
    val open: (Route) -> Unit = { if (stack.lastOrNull() != it) stacks[tab] = stack + it }
    val snackbar = remember { SnackbarHostState() }
    // What the floating glass blurs (Dock.kt).
    val haze = dev.chrisbanes.haze.rememberHazeState()
    // How tall the mini player, the tabs, and the bar of chosen songs are together, so
    // messages show above them instead of over them.
    var barHeight by remember { mutableStateOf(0) }
    // Tablets and unfolded phones get a side bar of tabs, playlists, and albums (SideBar.kt).
    val wide = hasSideBar()
    // What each page in a tab's stack had (its scroll, its filter, its search words) is kept
    // while the page is under another, so Back finds it as it was; dropped once the page is.
    val pageStates = androidx.compose.runtime.saveable.rememberSaveableStateHolder()
    val kept = remember { HashSet<String>() }
    androidx.compose.runtime.SideEffect {
        val live = Tab.entries.flatMap { t -> (stacks[t] ?: listOf(t.root)).mapIndexed { i, r -> pageKey(t, i, r) } }.toSet()
        (kept - live).forEach(pageStates::removeState)
        kept.clear()
        kept += live
    }
    // A place chosen in a tablet's side bar: the page alone in its tab, as the Mac's side bar does.
    val show: (Tab, Route) -> Unit = { t, r ->
        stacks[t] = listOf(r)
        tab = t
    }
    val chooseTab: (Tab) -> Unit = { t ->
        // Tapping the open tab again goes back to its first page.
        if (t == tab) stacks[t] = listOf(t.root) else tab = t
    }

    LaunchedEffect(Unit) { Ui.showPlayer.collect { if (hasSong) playerOpen = true } }
    LaunchedEffect(Unit) {
        NeedleApp.instance.openPlayer.collect {
            if (NeedleApp.instance.app.value.openPlayerOnPlay) playerOpen = true
        }
    }
    // Nothing playing (the computer went, or Needle came back without its song): the player is
    // shut, so Back is not spent on it, and it does not open by itself with the next song.
    LaunchedEffect(hasSong) { if (!hasSong) playerOpen = false }
    // Back first stops choosing songs, when some are chosen.
    val choosing by Selection.songs.collectAsState()
    // Not while the player covers the bar of chosen songs: Back closes the player first.
    BackHandler(enabled = choosing.isNotEmpty() && !playerOpen) { Selection.clear() }
    BackHandler(enabled = playerOpen || (choosing.isEmpty() && stack.size > 1)) {
        if (playerOpen) playerOpen = false else stacks[tab] = stack.dropLast(1)
    }

    val app by NeedleApp.instance.app.collectAsState()
    // The welcome guide, the first time Needle opens with no music yet.
    val songsLoad = rememberLoaded { songCount() }
    val songs by songsLoad
    val again by Ui.welcome.collectAsState()
    // Once begun, it stays until it is finished: the songs the first folder brings in would
    // otherwise close it before its last step.
    var guiding by rememberSaveable { mutableStateOf(false) }
    if (!app.welcomed && songs == 0u) guiding = true
    val welcoming = again || (guiding && !app.welcomed)
    // With a remote (TvFocus.kt): the player and the guide hold the focus while they cover the
    // app. A page opened from the page takes the focus, and so does the page under the player
    // or the guide when it closes; otherwise Android would give it to the side bar's first
    // button when what had it goes.
    val covered = welcoming || (playerOpen && hasSong)
    val keys = usingKeys()
    val pageFocus = remember { androidx.compose.ui.focus.FocusRequester() }
    var inPage by remember { mutableStateOf(false) }
    var toPage by remember { androidx.compose.runtime.mutableIntStateOf(0) }
    // When a key was last pressed: the page losing the focus with no key pressed is what had
    // it going (a filter changing the page), not the remote moving on.
    var lastKey by remember { androidx.compose.runtime.mutableLongStateOf(0L) }
    LaunchedEffect(tab, stack.last()) { if (inPage) toPage++ }
    LaunchedEffect(covered) { if (!covered) toPage++ }
    LaunchedEffect(toPage, keys, covered) {
        if (!keys || covered) return@LaunchedEffect
        // After the page slides in, so the focus lands on the new page, not the old.
        kotlinx.coroutines.delay(350)
        // A page can take a moment to load its first buttons, the first time most.
        repeat(60) {
            // Something in the page has it already (its Play): asking the page again would
            // move it back to the page's first button.
            if (inPage) return@LaunchedEffect
            if (runCatching { pageFocus.requestFocus() }.getOrDefault(false)) return@LaunchedEffect
            kotlinx.coroutines.delay(50)
        }
    }
    // Settings › Appearance › Colours from what is playing: every page in the song's colours.
    CoverTheme(if (app.ambientColors) playingArt else null) {
    SharedTransitionLayout(Modifier.fillMaxSize()) {
        CompositionLocalProvider(LocalShared provides this) {
            Box(Modifier.fillMaxSize().onPreviewKeyEvent { lastKey = android.os.SystemClock.uptimeMillis(); false }) {
                // The keyboard takes room from the pages, so a text box low on a page shows above it.
                // On a tablet the side bar sits on the window, a shade darker, and the pages on a
                // sheet beside it with one round corner, as the Mac's and the computer's.
                Row(Modifier.fillMaxSize().imePadding().focusCovered(covered).background(if (wide) MaterialTheme.colorScheme.surfaceContainerLowest else MaterialTheme.colorScheme.background)) {
                if (wide) SideBar(tab, stack.last(), chooseTab, open, show)
                // Pages lay out by the room they have (see Adaptive.kt).
                androidx.compose.foundation.layout.BoxWithConstraints(Modifier.weight(1f).then(if (wide) Modifier.pane() else Modifier).focusRequester(pageFocus).onFocusChanged {
                    if (inPage && !it.hasFocus && android.os.SystemClock.uptimeMillis() - lastKey > 250) toPage++
                    inPage = it.hasFocus
                }.focusGroup()) {
                CompositionLocalProvider(LocalPageWidth provides maxWidth) {
                // The room the dock takes at the foot, with a little to spare, for the pages' lists.
                val dockRoom = with(LocalDensity.current) { barHeight.toDp() } + 12.dp
                // The pages run on under the floating dock (or a tablet's floating player); what
                // floats blurs them (Dock.kt).
                Scaffold(
                    containerColor = MaterialTheme.colorScheme.background,
                    // The top is left to each page: most keep clear of the status bar, and the
                    // album, artist, and playlist pages run their colour up under it. The foot is
                    // left to the dock, which floats over the page.
                    contentWindowInsets = androidx.compose.material3.ScaffoldDefaults.contentWindowInsets.only(
                        androidx.compose.foundation.layout.WindowInsetsSides.Horizontal,
                    ),
                    modifier = Modifier.hazeSource(haze),
                ) { padding ->
                    AnimatedContent(
                        targetState = tab to stack,
                        transitionSpec = {
                            val (fromTab, from) = initialState
                            val (toTab, to) = targetState
                            when {
                                reduce -> fadeIn(tween(120)) togetherWith fadeOut(tween(120))
                                // Deeper pages slide in from the side, as Android's shared axis.
                                fromTab == toTab && to.size > from.size ->
                                    (slideInHorizontally(tween(300)) { it / 5 } + fadeIn(tween(250))) togetherWith
                                        (slideOutHorizontally(tween(300)) { -it / 10 } + fadeOut(tween(200)))
                                fromTab == toTab && to.size < from.size ->
                                    (slideInHorizontally(tween(300)) { -it / 10 } + fadeIn(tween(250))) togetherWith
                                        (slideOutHorizontally(tween(300)) { it / 5 } + fadeOut(tween(200)))
                                // Tabs fade through.
                                else -> fadeIn(tween(220, delayMillis = 60)) togetherWith fadeOut(tween(120))
                            }
                        },
                        label = "page",
                    ) { (pageTab, pages) ->
                        val opened = remember { android.os.SystemClock.uptimeMillis() }
                        CompositionLocalProvider(
                            LocalAnimated provides this,
                            LocalPageOpened provides opened,
                            LocalCanGoBack provides (pages.size > 1),
                            // Covers fly only between pages of one tab, never across tabs.
                            LocalSharedScope provides pageTab.name,
                        ) {
                            val route = pages.last()
                            pageStates.SaveableStateProvider(pageKey(pageTab, pages.lastIndex, route)) {
                            val fullBleed = route is Route.AlbumPage || route is Route.AlbumOf || route is Route.Artist || route is Route.Playlist
                            // Every page runs on behind the dock; its list leaves room at the foot.
                            CompositionLocalProvider(LocalBottomInset provides dockRoom) {
                                Box(if (fullBleed) Modifier else Modifier.padding(padding).statusBarsPadding()) {
                                    Page(route, open)
                                    // What scrolls up fades into the page below the status bar,
                                    // rather than stopping at a hard edge.
                                    if (!fullBleed) {
                                        val page = MaterialTheme.colorScheme.background
                                        Box(
                                            Modifier
                                                .fillMaxWidth()
                                                .height(20.dp)
                                                .background(androidx.compose.ui.graphics.Brush.verticalGradient(listOf(page, page.copy(alpha = 0f)))),
                                        )
                                    }
                                }
                            }
                            }
                        }
                    }
                }
                // A tablet's side pane, floating over the page's right side above the player.
                if (wide) {
                    val pane by Ui.pane.collectAsState()
                    val shown = pane
                    Column(Modifier.align(androidx.compose.ui.Alignment.TopEnd)) {
                    AnimatedVisibility(
                        visible = shown != null && hasSong && !playerOpen,
                        enter = fadeIn(tween(200)) + slideInHorizontally(tween(260)) { it / 3 },
                        exit = fadeOut(tween(150)) + slideOutHorizontally(tween(200)) { it / 3 },
                    ) {
                        val bars = with(LocalDensity.current) { barHeight.toDp() }
                        // The last kind shown stays while the pane slides away.
                        var last by remember { mutableStateOf(SideKind.Lyrics) }
                        if (shown != null) last = shown
                        SidePane(last, open, Modifier.padding(top = 12.dp, end = 20.dp, bottom = bars + 8.dp))
                    }
                    }
                }
                // The bar of chosen songs, and under it the dock: the mini player and the tabs as
                // one floating glass bar on a phone; on a tablet or TV, the player alone.
                val typing = androidx.compose.foundation.layout.WindowInsets.isImeVisible
                CompositionLocalProvider(LocalHaze provides haze) {
                    Column(
                        Modifier
                            .align(androidx.compose.ui.Alignment.BottomCenter)
                            .onSizeChanged { barHeight = it.height }
                            .navigationBarsPadding()
                            .padding(horizontal = if (wide) 20.dp else 10.dp)
                            .padding(bottom = 10.dp),
                    ) {
                        if (!typing) SelectionBar()
                        if (!typing) {
                            if (!wide) {
                                Dock(tab, chooseTab, hasSong && !playerOpen, { playerOpen = true }, null)
                            } else {
                                AnimatedVisibility(
                                    visible = hasSong && !playerOpen,
                                    enter = fadeIn(tween(200)) + slideInVertically(tween(250)) { it / 2 },
                                    exit = fadeOut(tween(150)),
                                ) {
                                    CompositionLocalProvider(LocalAnimated provides this) {
                                        FloatingPlayer({ playerOpen = true })
                                    }
                                }
                            }
                        }
                    }
                }
                }
                }
                }
                AnimatedVisibility(
                    visible = playerOpen && hasSong,
                    enter = if (reduce) fadeIn(tween(150)) else slideInVertically(tween(380)) { it } + fadeIn(tween(200)),
                    exit = if (reduce) fadeOut(tween(150)) else slideOutVertically(tween(300)) { it } + fadeOut(tween(250)),
                ) {
                    CompositionLocalProvider(LocalAnimated provides this) {
                        Box(Modifier.focusLayer()) {
                            FullPlayer(onClose = { playerOpen = false }, open = { playerOpen = false; open(it) })
                        }
                    }
                }
                // Messages show above everything, the player too, and clear of the bars below.
                val bars = with(LocalDensity.current) { barHeight.toDp() }
                SnackbarHost(
                    snackbar,
                    // Over the player, at the top: at the foot it would cover the player's buttons.
                    if (playerOpen && hasSong) {
                        Modifier.align(androidx.compose.ui.Alignment.TopCenter).statusBarsPadding().padding(top = 56.dp)
                    } else {
                        Modifier
                            .align(androidx.compose.ui.Alignment.BottomCenter)
                            // Above the keyboard while it is up, where it would otherwise hide.
                            .imePadding()
                            // Above the dock, which already counts the navigation bar.
                            .then(if (bars == 0.dp) Modifier.navigationBarsPadding() else Modifier)
                            .padding(bottom = bars + 8.dp)
                    },
                ) { data ->
                    // A tap or a swipe sends it away at once.
                    val swipe = androidx.compose.material3.rememberSwipeToDismissBoxState()
                    LaunchedEffect(swipe.currentValue) {
                        if (swipe.currentValue != androidx.compose.material3.SwipeToDismissBoxValue.Settled) data.dismiss()
                    }
                    androidx.compose.material3.SwipeToDismissBox(swipe, backgroundContent = {}) {
                        androidx.compose.material3.Snackbar(
                            data,
                            modifier = Modifier.clickable(onClickLabel = "Dismiss") { data.dismiss() },
                        )
                    }
                }
                // The visualizer full screen, over everything, while a song is there.
                val fullVisualizer by Ui.fullVisualizer.collectAsState()
                if (fullVisualizer && hasSong) Box(Modifier.focusLayer()) { FullVisualizer { Ui.fullVisualizer.value = false } }
                Overlays(snackbar)
                if (welcoming) Box(Modifier.focusLayer()) { WelcomeGuide(open) }
            }
        }
    }
    }
}

@Composable
private fun Page(route: Route, open: (Route) -> Unit) {
    when (route) {
        Route.Home -> HomeScreen(open)
        Route.Library -> LibraryScreen(open)
        Route.Search -> SearchScreen(open)
        Route.Settings -> SettingsScreen(open)
        Route.Playlists -> PlaylistsScreen(open)
        Route.Artists -> ArtistsScreen(open)
        Route.Albums -> AlbumsScreen(open)
        Route.Songs -> SongsScreen(open)
        Route.Genres -> GenresScreen(open)
        is Route.AlbumPage -> AlbumScreen(route.album, open)
        is Route.AlbumOf -> AlbumOfScreen(route, open)
        is Route.Artist -> ArtistScreen(route.name, open)
        is Route.Playlist -> PlaylistScreen(route, open)
        is Route.Genre -> GenreScreen(route.name, open)
        is Route.SettingsPage -> SettingsSectionScreen(route.section, open)
        Route.Favorites -> FavoritesScreen(open)
        Route.RecentlyAdded -> RecentlyAddedScreen(open)
        Route.History -> HistoryScreen(open)
        is Route.Folder -> FoldersScreen(route.path, open)
        is Route.Wrapped -> WrappedScreen(route.year, open)
        Route.Numbers -> NumbersScreen(open)
        Route.MoodMap -> MoodMapScreen()
        Route.Crate -> CrateScreen(open)
        Route.Plugins -> PluginsScreen(open)
        is Route.Plugin -> PluginScreen(route.id)
        Route.Connect -> ConnectScreen()
        Route.Sync -> SyncScreen()
        Route.WhatsNew -> WhatsNewScreen()
        is Route.Timing -> TimingScreen(route.songId)
        is Route.Document -> DocScreen(route.doc)
        Route.FixLibrary -> FixLibraryScreen()
        Route.Import -> ImportScreen()
        is Route.ThemeEditor -> {
            val back = androidx.activity.compose.LocalOnBackPressedDispatcherOwner.current
            ThemeEditorScreen(route.id) { back?.onBackPressedDispatcher?.onBackPressed() }
        }
    }
}

/** A page's place in its tab's stack, the key its kept state is held under. */
private fun pageKey(tab: Tab, index: Int, route: Route) = "${tab.name}/$index/$route"
