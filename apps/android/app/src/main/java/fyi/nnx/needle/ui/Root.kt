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
import androidx.compose.foundation.layout.padding
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Home
import androidx.compose.material.icons.rounded.LibraryMusic
import androidx.compose.material.icons.rounded.Search
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
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
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
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
}

enum class Tab(val label: String, val icon: ImageVector, val root: Route) {
    Home("Home", Icons.Rounded.Home, Route.Home),
    Library("Library", Icons.Rounded.LibraryMusic, Route.Library),
    Search("Search", Icons.Rounded.Search, Route.Search),
}

@OptIn(ExperimentalSharedTransitionApi::class)
@Composable
fun NeedleRoot() {
    var tab by rememberSaveable { mutableStateOf(Tab.Home) }
    // Each tab keeps its own pages, as in Apple Music.
    val stacks = remember { mutableStateMapOf<Tab, List<Route>>() }
    var playerOpen by rememberSaveable { mutableStateOf(false) }
    val playback by NeedleApp.instance.playback.collectAsState()
    val reduce = reduceMotion()
    val stack = stacks[tab] ?: listOf(tab.root)
    val open: (Route) -> Unit = { stacks[tab] = stack + it }
    val hasSong = playback?.current != null

    LaunchedEffect(Unit) {
        NeedleApp.instance.openPlayer.collect {
            if (NeedleApp.instance.app.value.openPlayerOnPlay) playerOpen = true
        }
    }
    BackHandler(enabled = playerOpen || stack.size > 1) {
        if (playerOpen) playerOpen = false else stacks[tab] = stack.dropLast(1)
    }

    SharedTransitionLayout(Modifier.fillMaxSize()) {
        CompositionLocalProvider(LocalShared provides this) {
            Box(Modifier.fillMaxSize()) {
                Scaffold(
                    containerColor = MaterialTheme.colorScheme.background,
                    bottomBar = {
                        Column {
                            AnimatedVisibility(
                                visible = hasSong && !playerOpen,
                                enter = fadeIn(tween(200)) + slideInVertically(tween(250)) { it / 2 },
                                exit = fadeOut(tween(150)),
                            ) {
                                CompositionLocalProvider(LocalAnimated provides this) {
                                    MiniPlayer(onOpen = { playerOpen = true })
                                }
                            }
                            NavigationBar(containerColor = MaterialTheme.colorScheme.surfaceContainerLow) {
                                Tab.entries.forEach { t ->
                                    NavigationBarItem(
                                        selected = t == tab,
                                        onClick = {
                                            // Tapping the open tab again goes back to its first page.
                                            if (t == tab) stacks[t] = listOf(t.root) else tab = t
                                        },
                                        icon = { Icon(t.icon, contentDescription = null) },
                                        label = { Text(t.label) },
                                    )
                                }
                            }
                        }
                    },
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
                        modifier = Modifier.padding(padding),
                        label = "page",
                    ) { (_, pages) ->
                        CompositionLocalProvider(LocalAnimated provides this) {
                            Page(pages.last(), open)
                        }
                    }
                }
                AnimatedVisibility(
                    visible = playerOpen && hasSong,
                    enter = if (reduce) fadeIn(tween(150)) else slideInVertically(tween(380)) { it } + fadeIn(tween(200)),
                    exit = if (reduce) fadeOut(tween(150)) else slideOutVertically(tween(300)) { it } + fadeOut(tween(250)),
                ) {
                    CompositionLocalProvider(LocalAnimated provides this) {
                        FullPlayer(onClose = { playerOpen = false })
                    }
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
        is Route.SettingsPage -> SettingsSectionScreen(route.section)
    }
}
