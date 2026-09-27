package fyi.nnx.needle.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Folder
import androidx.compose.material3.FilterChip
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import fyi.nnx.needle.core.Ranked
import fyi.nnx.needle.core.Song
import fyi.nnx.needle.core.Stats
import java.text.DateFormat
import java.util.Date

private fun play(songs: List<Song>, start: Int = 0) {
    core.play(songs.map { it.id }, start.toUInt())
    fyi.nnx.needle.NeedleApp.instance.openPlayer.tryEmit(Unit)
}

/** "3 h 20 min", or "45 min". */
fun hoursMinutes(seconds: Double): String {
    val minutes = (seconds / 60).toLong()
    return if (minutes >= 60) "${minutes / 60} h ${minutes % 60} min" else "$minutes min"
}

// ---------- Songs lists with a title: favorites, recently added

@Composable
fun FavoritesScreen(open: (Route) -> Unit) {
    val songs by rememberLoaded { favorites() }
    TitledSongs("Favorites", songs, open, empty = "Songs you mark with the heart, or rate four or five stars, show here.")
}

@Composable
fun RecentlyAddedScreen(open: (Route) -> Unit) {
    val songs by rememberLoaded { recentlyAdded() }
    TitledSongs("Recently added", songs, open, empty = "Nothing added in the last 30 days.")
}

@Composable
fun TitledSongs(title: String, songs: List<Song>?, open: (Route) -> Unit, empty: String = "") {
    val playing = fyi.nnx.needle.NeedleApp.instance.playback.value?.current?.id
    val list = songs.orEmpty()
    LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(bottom = 24.dp)) {
        item { LargeTitle(title) }
        if (songs == null) items(8) { SongPlaceholder() }
        if (songs != null && list.isEmpty() && empty.isNotEmpty()) {
            item {
                Text(empty, color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.padding(horizontal = Edge, vertical = 8.dp))
            }
        }
        itemsIndexed(list, key = { i, s -> "$i-${s.id}" }) { i, song ->
            SongRow(song, playing == song.id, open) { play(list, i) }
        }
    }
}

// ---------- Folders

@Composable
fun FoldersScreen(path: String?, open: (Route) -> Unit) {
    // The music folders themselves, or the folders and songs inside one.
    val roots by rememberLoaded { folders() }
    val subfolders by rememberLoaded(path) { if (path == null) emptyList() else subfolders(path) }
    val songs by rememberLoaded(path) { if (path == null) emptyList() else folderSongs(path) }
    val playing = fyi.nnx.needle.NeedleApp.instance.playback.value?.current?.id
    val title = path?.substringAfterLast('/')?.ifBlank { path } ?: "Folders"
    // Every song in this folder and the folders in it, so a whole folder can be played.
    val here = songs.orEmpty()
    LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(bottom = 24.dp)) {
        item { LargeTitle(title) }
        if (path == null) {
            items(roots.orEmpty()) { root ->
                FolderRow(root.substringAfterLast('/').ifBlank { root }, root, null) { open(Route.Folder(root)) }
            }
        } else {
            items(subfolders.orEmpty(), key = { it.path }) { folder ->
                FolderRow(folder.name, count(folder.songs.toInt(), "song"), folder.artwork) { open(Route.Folder(folder.path)) }
            }
            if (here.isNotEmpty()) {
                item { SectionHeader("Every song in here") }
                itemsIndexed(here, key = { _, s -> s.id }) { i, song ->
                    SongRow(song, playing == song.id, open) { play(here, i) }
                }
            }
        }
    }
}

@Composable
private fun FolderRow(name: String, detail: String, artwork: String?, onClick: () -> Unit) {
    Column(Modifier.fillMaxWidth().clickable(onClick = onClick)) {
        Row(
            Modifier.fillMaxWidth().heightIn(min = 64.dp).padding(horizontal = Edge, vertical = 8.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(14.dp),
        ) {
            if (artwork != null) {
                Cover(artwork, Modifier.size(48.dp), SmallCoverShape)
            } else {
                Box(Modifier.size(48.dp).clip(SmallCoverShape).background(MaterialTheme.colorScheme.surfaceContainerHigh), contentAlignment = Alignment.Center) {
                    Icon(Icons.Rounded.Folder, contentDescription = null, tint = MaterialTheme.colorScheme.primary)
                }
            }
            Column(Modifier.weight(1f)) {
                Text(name, style = MaterialTheme.typography.bodyLarge, maxLines = 1, overflow = TextOverflow.Ellipsis)
                Text(detail, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant, maxLines = 1, overflow = TextOverflow.Ellipsis)
            }
        }
        HorizontalDivider(color = MaterialTheme.colorScheme.outlineVariant, modifier = Modifier.padding(start = Edge + 62.dp))
    }
}

// ---------- History and its numbers

@Composable
fun HistoryScreen(open: (Route) -> Unit) {
    var view by rememberSaveable { mutableStateOf("Listens") }
    Column(Modifier.fillMaxSize()) {
        LargeTitle("History")
        Row(Modifier.padding(horizontal = Edge, vertical = 4.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            listOf("Listens", "Numbers").forEach { FilterChip(selected = view == it, onClick = { view = it }, label = { Text(it) }) }
        }
        if (view == "Listens") Listens(open) else Numbers(open)
    }
}

@Composable
private fun Listens(open: (Route) -> Unit) {
    val history by rememberLoaded { history(0u, 300u) }
    val format = DateFormat.getDateTimeInstance(DateFormat.MEDIUM, DateFormat.SHORT)
    val list = history.orEmpty()
    LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(bottom = 24.dp)) {
        if (history == null) items(8) { SongPlaceholder() }
        if (history != null && list.isEmpty()) {
            item { Text("What you play shows here.", color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.padding(Edge)) }
        }
        items(list) { listen ->
            Row(
                Modifier
                    .fillMaxWidth()
                    .clickable(enabled = listen.song != null) { listen.song?.let { play(listOf(it)) } }
                    .padding(horizontal = Edge, vertical = 8.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(14.dp),
            ) {
                Cover(listen.song?.artwork, Modifier.size(48.dp), SmallCoverShape)
                Column(Modifier.weight(1f)) {
                    Text(listen.title, style = MaterialTheme.typography.bodyLarge, maxLines = 1, overflow = TextOverflow.Ellipsis)
                    Text(
                        "${listen.artist} · ${format.format(Date(listen.startedAt * 1000))}",
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
                if (!listen.counted) Text("skipped", style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
        }
    }
}

@Composable
private fun Numbers(open: (Route) -> Unit) {
    var span by rememberSaveable { mutableStateOf(30) }
    val stats by rememberLoaded(span) { stats(if (span == 0) null else span.toUInt()) }
    LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(bottom = 24.dp)) {
        item {
            Row(Modifier.padding(horizontal = Edge, vertical = 8.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                listOf(7 to "Week", 30 to "Month", 365 to "Year", 0 to "All time").forEach { (days, label) ->
                    FilterChip(selected = span == days, onClick = { span = days }, label = { Text(label) })
                }
            }
        }
        stats?.let { s -> statsItems(s, open) }
    }
}

fun androidx.compose.foundation.lazy.LazyListScope.statsItems(s: Stats, open: (Route) -> Unit) {
    item {
        Row(Modifier.fillMaxWidth().padding(horizontal = Edge, vertical = 8.dp), horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            Figure(hoursMinutes(s.seconds), "listening", Modifier.weight(1f))
            Figure(s.plays.toString(), "plays", Modifier.weight(1f))
            Figure(s.artists.toString(), "artists", Modifier.weight(1f))
        }
    }
    item { SectionHeader("By hour") }
    item { HourBars(s.hours) }
    if (s.topSongs.isNotEmpty()) {
        item { SectionHeader("Top songs") }
        items(s.topSongs.take(10)) { RankedRow(it) }
    }
    if (s.topArtists.isNotEmpty()) {
        item { SectionHeader("Top artists") }
        items(s.topArtists.take(10)) { r -> RankedRow(r) { open(Route.Artist(r.artist)) } }
    }
    if (s.topAlbums.isNotEmpty()) {
        item { SectionHeader("Top albums") }
        item {
            LazyRow(contentPadding = PaddingValues(horizontal = Edge), horizontalArrangement = Arrangement.spacedBy(14.dp)) {
                items(s.topAlbums.take(10)) { r ->
                    Column(Modifier.width(140.dp).clickable { r.songId?.let { open(Route.AlbumOf(r.album, it)) } }) {
                        Cover(r.artwork, Modifier.size(140.dp))
                        Text(r.album, style = MaterialTheme.typography.bodyMedium, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.padding(top = 8.dp))
                        Text(count(r.plays.toInt(), "play"), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                    }
                }
            }
        }
    }
}

@Composable
fun Figure(value: String, label: String, modifier: Modifier = Modifier) {
    Column(modifier.clip(RoundedCornerShape(20.dp)).background(MaterialTheme.colorScheme.surfaceContainer).padding(14.dp)) {
        Text(value, style = MaterialTheme.typography.titleLarge, fontWeight = FontWeight.Bold, maxLines = 1)
        Text(label, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
    }
}

@Composable
private fun HourBars(hours: List<Double>) {
    val most = (hours.maxOrNull() ?: 0.0).coerceAtLeast(1.0)
    Row(
        Modifier.fillMaxWidth().height(96.dp).padding(horizontal = Edge),
        horizontalArrangement = Arrangement.spacedBy(3.dp),
        verticalAlignment = Alignment.Bottom,
    ) {
        hours.forEach { h ->
            Box(
                Modifier
                    .weight(1f)
                    .fillMaxHeight((h / most).toFloat().coerceIn(0.03f, 1f))
                    .clip(RoundedCornerShape(topStart = 3.dp, topEnd = 3.dp))
                    .background(MaterialTheme.colorScheme.primary.copy(alpha = if (h == most) 1f else 0.55f)),
            )
        }
    }
    Row(Modifier.fillMaxWidth().padding(horizontal = Edge, vertical = 4.dp)) {
        listOf("0", "6", "12", "18", "23").forEachIndexed { i, label ->
            Text(label, style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
            if (i < 4) Spacer(Modifier.weight(1f))
        }
    }
}

@Composable
private fun RankedRow(r: Ranked, onClick: (() -> Unit)? = null) {
    Row(
        Modifier
            .fillMaxWidth()
            .clickable(enabled = onClick != null || r.songId != null) {
                if (onClick != null) onClick() else r.songId?.let { core.play(listOf(it), 0u) }
            }
            .padding(horizontal = Edge, vertical = 6.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(14.dp),
    ) {
        Cover(r.artwork, Modifier.size(44.dp), SmallCoverShape)
        Column(Modifier.weight(1f)) {
            Text(r.title.ifBlank { r.artist }, style = MaterialTheme.typography.bodyLarge, maxLines = 1, overflow = TextOverflow.Ellipsis)
            if (r.title.isNotBlank()) Text(r.artist, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant, maxLines = 1, overflow = TextOverflow.Ellipsis)
        }
        Text(count(r.plays.toInt(), "play"), style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
    }
}

// ---------- Wrapped

@Composable
fun WrappedScreen(year: Int?, open: (Route) -> Unit) {
    val years by rememberLoaded { wrappedYears() }
    var chosen by rememberSaveable { mutableStateOf(year) }
    val shown = chosen ?: years?.firstOrNull()
    val wrapped by rememberLoaded(shown) { shown?.let { wrapped(it) } }
    LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(bottom = 32.dp)) {
        item { LargeTitle(if (shown != null) "Your $shown" else "Your year") }
        if (years != null && years!!.isEmpty()) {
            item { Text("Play some music, and your year in music shows here.", color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.padding(Edge)) }
        }
        if ((years?.size ?: 0) > 1) {
            item {
                LazyRow(contentPadding = PaddingValues(horizontal = Edge), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    items(years.orEmpty()) { y -> FilterChip(selected = y == shown, onClick = { chosen = y }, label = { Text(y.toString()) }) }
                }
            }
        }
        val w = wrapped ?: return@LazyColumn
        item {
            // The headline card, in the amber of Needle's record.
            Column(
                Modifier
                    .padding(Edge)
                    .fillMaxWidth()
                    .clip(RoundedCornerShape(28.dp))
                    .background(Brush.linearGradient(listOf(Color(0xFF5B4526), Color(0xFF2B2927))))
                    .padding(24.dp),
            ) {
                Text(hoursMinutes(w.stats.seconds), style = MaterialTheme.typography.displaySmall, fontWeight = FontWeight.Bold)
                Text("of music in ${w.year}", style = MaterialTheme.typography.titleMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                Spacer(Modifier.height(16.dp))
                Text("${w.stats.plays} plays · ${w.stats.songs} songs · ${w.stats.artists} artists", style = MaterialTheme.typography.bodyLarge)
                if (w.streak > 1u) Text("Longest streak: ${w.streak} days in a row", style = MaterialTheme.typography.bodyLarge)
                w.peakHour?.let { Text("You listen most around ${it}:00", style = MaterialTheme.typography.bodyLarge) }
                if (w.genres.isNotEmpty()) Text("Mostly ${w.genres.take(3).joinToString(", ")}", style = MaterialTheme.typography.bodyLarge)
            }
        }
        if (w.topSongs.isNotEmpty()) {
            item { SectionHeader("Your top songs") }
            item {
                Row(Modifier.padding(horizontal = Edge), horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                    androidx.compose.material3.Button(onClick = { play(w.topSongs) }) { Text("Play them") }
                    androidx.compose.material3.FilledTonalButton(onClick = {
                        core.createPlaylist("Top songs of ${w.year}", "Your most played songs of ${w.year}.", w.topSongs.map { it.id }, null)
                        fyi.nnx.needle.NeedleApp.instance.libraryVersion.value++
                        showMessage("Saved as a playlist")
                    }) { Text("Save as a playlist") }
                }
            }
            itemsIndexed(w.topSongs.take(25), key = { i, s -> "$i-${s.id}" }) { i, song ->
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Text("${i + 1}", style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.Bold, color = MaterialTheme.colorScheme.primary, modifier = Modifier.padding(start = Edge).width(28.dp))
                    Box(Modifier.weight(1f)) { SongRow(song, false, open) { play(w.topSongs, i) } }
                }
            }
        }
        if (w.newArtists.isNotEmpty()) {
            item { SectionHeader("New to you") }
            item { Text(w.newArtists.joinToString(" · "), style = MaterialTheme.typography.bodyLarge, modifier = Modifier.padding(horizontal = Edge)) }
        }
        statsItems(w.stats, open)
    }
}
