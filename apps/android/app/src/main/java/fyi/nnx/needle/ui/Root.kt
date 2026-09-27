package fyi.nnx.needle.ui

import androidx.activity.compose.BackHandler
import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.slideOutVertically
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.QueueMusic
import androidx.compose.material.icons.rounded.Home
import androidx.compose.material.icons.rounded.LibraryMusic
import androidx.compose.material.icons.rounded.Search
import androidx.compose.material3.Icon
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
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

/** A page inside a tab. */
sealed interface Route {
    data object Home : Route
    data object Library : Route
    data object Search : Route
    data object Playlists : Route
    data object Settings : Route
    data class Album(val key: String, val title: String) : Route
    data class Artist(val name: String) : Route
    data class Playlist(val id: String, val name: String) : Route
}

enum class Tab(val label: String, val icon: ImageVector, val root: Route) {
    Home("Home", Icons.Rounded.Home, Route.Home),
    Library("Library", Icons.Rounded.LibraryMusic, Route.Library),
    Search("Search", Icons.Rounded.Search, Route.Search),
    Playlists("Playlists", Icons.AutoMirrored.Rounded.QueueMusic, Route.Playlists),
}

@Composable
fun NeedleRoot() {
    var tab by rememberSaveable { mutableStateOf(Tab.Home) }
    // Each tab keeps its own pages, as in most music apps.
    val stacks = remember { mutableStateMapOf<Tab, List<Route>>() }
    var playerOpen by rememberSaveable { mutableStateOf(false) }
    val playback by NeedleApp.instance.playback.collectAsState()
    val stack = stacks[tab] ?: listOf(tab.root)
    val open: (Route) -> Unit = { stacks[tab] = stack + it }

    BackHandler(enabled = playerOpen || stack.size > 1) {
        if (playerOpen) playerOpen = false else stacks[tab] = stack.dropLast(1)
    }

    Box(Modifier.fillMaxSize()) {
        Scaffold(
            bottomBar = {
                Column {
                    if (playback?.current != null) MiniPlayer(onOpen = { playerOpen = true })
                    NavigationBar {
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
                targetState = stack.last(),
                transitionSpec = { fadeIn() togetherWith fadeOut() },
                modifier = Modifier.padding(padding),
                label = "page",
            ) { route ->
                when (route) {
                    Route.Home -> HomeScreen(open)
                    Route.Library -> LibraryScreen(open)
                    Route.Search -> SearchScreen(open)
                    Route.Playlists -> PlaylistsScreen(open)
                    Route.Settings -> SettingsScreen()
                    is Route.Album -> AlbumScreen(route, open)
                    is Route.Artist -> ArtistScreen(route, open)
                    is Route.Playlist -> PlaylistScreen(route)
                }
            }
        }
        AnimatedVisibility(
            visible = playerOpen && playback?.current != null,
            enter = slideInVertically { it },
            exit = slideOutVertically { it },
        ) {
            FullPlayer(onClose = { playerOpen = false })
        }
    }
}
