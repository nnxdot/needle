package fyi.nnx.needle.ui

import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.grid.GridItemSpan
import androidx.compose.foundation.lazy.grid.LazyGridScope
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.rounded.PlayArrow
import androidx.compose.material3.FilterChip
import androidx.compose.material3.FilterChipDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.foundation.background
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import fyi.nnx.needle.core.LyricHit
import fyi.nnx.needle.core.SearchBest
import fyi.nnx.needle.core.SearchFound
import fyi.nnx.needle.core.Song
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

/** The chips under the search field: everything found, or one kind of it. */
enum class SearchKind(val label: String) { All("All"), Songs("Songs"), Albums("Albums"), Artists("Artists"), Lyrics("Lyrics") }

/** What a search shows, by kind (NEEDLE-2.0.md, "Search page"). */
class SearchAnswer(
    val query: String,
    /** The core's find: the best match, then a few of each kind; null while it is asked. */
    val found: SearchFound?,
    /** Every song the search finds, for the Songs chip. */
    val songs: List<Song>,
    /** A rule that does not compile: its error, as the Songs page says it. */
    val error: String?,
)

/**
 * The Search page's results under the field: the chips, then, under All, the best match large,
 * Songs (up to 5), Albums, Artists, In the lyrics (up to 3), and Try a rule; a section with
 * nothing is left out. The other chips show all of their kind.
 */
internal fun LazyGridScope.searchResults(
    answer: SearchAnswer,
    kind: SearchKind,
    onKind: (SearchKind) -> Unit,
    rules: List<Pair<String, String>>,
    onRule: (String) -> Unit,
    playing: String?,
    open: (Route) -> Unit,
) {
    edgeToEdge(16.dp, key = "kinds") { SearchKinds(kind, onKind) }
    val found = answer.found
    answer.error?.let { error ->
        edgeToEdge(16.dp, key = "error") {
            Text(error, color = MaterialTheme.colorScheme.error, modifier = Modifier.padding(horizontal = Edge, vertical = 12.dp))
        }
        tryARule(rules, onRule)
        return
    }
    if (found == null) return
    val songs = answer.songs
    when (kind) {
        SearchKind.All -> {
            val nothing = found.best == null && found.songs.isEmpty() && found.albums.isEmpty() && found.artists.isEmpty() && found.lyrics.isEmpty()
            if (nothing) edgeToEdge(16.dp, key = "nothing") { NothingMatches() }
            found.best?.let { best -> edgeToEdge(16.dp, key = "best") { BestMatch(best, open) } }
            if (found.songs.isNotEmpty()) {
                edgeToEdge(16.dp, key = "songs-title") { ShelfTitle("Songs", null) { onKind(SearchKind.Songs) } }
                val five = found.songs.take(5)
                items(five.size, key = { "song-" + five[it].id }, span = { GridItemSpan(maxLineSpan) }) { i ->
                    SongRow(five[i], playing == five[i].id, open, Modifier.bleed(16.dp), place = Place(i, five.size)) { playSongs(five.map { it.id }, i) }
                }
            }
            if (found.albums.isNotEmpty()) {
                edgeToEdge(16.dp, key = "albums-title") { ShelfTitle("Albums", null) { onKind(SearchKind.Albums) } }
                edgeToEdge(16.dp, key = "albums") {
                    val tile = if (isWide()) 176.dp else 148.dp
                    LazyRow(contentPadding = PaddingValues(horizontal = Edge), horizontalArrangement = Arrangement.spacedBy(16.dp)) {
                        items(found.albums, key = { it.key }) { album -> AlbumTile(album, Modifier.width(tile), open) { open(Route.AlbumPage(album)) } }
                    }
                }
            }
            if (found.artists.isNotEmpty()) {
                edgeToEdge(16.dp, key = "artists-title") { ShelfTitle("Artists", null) { onKind(SearchKind.Artists) } }
                edgeToEdge(16.dp, key = "artists") {
                    LazyRow(contentPadding = PaddingValues(horizontal = Edge), horizontalArrangement = Arrangement.spacedBy(18.dp)) {
                        items(found.artists, key = { it.name }) { a -> ArtistTile(a, Modifier.width(if (isWide()) 132.dp else 112.dp), open) }
                    }
                }
            }
            if (found.lyrics.isNotEmpty()) {
                edgeToEdge(16.dp, key = "lyrics-title") { ShelfTitle("In the lyrics", null) { onKind(SearchKind.Lyrics) } }
                val three = found.lyrics.take(3)
                items(three.size, key = { "lyric-" + three[it].song.id }, span = { GridItemSpan(maxLineSpan) }) { i ->
                    LyricHitRow(three[i], answer.query, playing == three[i].song.id)
                }
            }
            tryARule(rules, onRule)
        }
        SearchKind.Songs -> {
            if (songs.isEmpty()) edgeToEdge(16.dp, key = "nothing") { NothingMatches() }
            items(songs.size, key = { "song-" + songs[it].id }, span = { GridItemSpan(maxLineSpan) }) { i ->
                SongRow(songs[i], playing == songs[i].id, open, Modifier.bleed(16.dp), place = Place(i, songs.size)) { playSongs(songs.map { it.id }, i) }
            }
        }
        SearchKind.Albums -> {
            if (found.albums.isEmpty()) edgeToEdge(16.dp, key = "nothing") { NothingMatches() }
            items(found.albums.size, key = { "album-" + found.albums[it].key }) { i ->
                val album = found.albums[i]
                AlbumTile(album, Modifier, open) { open(Route.AlbumPage(album)) }
            }
        }
        SearchKind.Artists -> {
            if (found.artists.isEmpty()) edgeToEdge(16.dp, key = "nothing") { NothingMatches() }
            items(found.artists.size, key = { "artist-" + found.artists[it].name }) { i ->
                ArtistTile(found.artists[i], Modifier, open)
            }
        }
        SearchKind.Lyrics -> {
            if (found.lyrics.isEmpty()) edgeToEdge(16.dp, key = "nothing") { NothingMatches() }
            items(found.lyrics.size, key = { "lyric-" + found.lyrics[it].song.id }, span = { GridItemSpan(maxLineSpan) }) { i ->
                LyricHitRow(found.lyrics[i], answer.query, playing == found.lyrics[i].song.id)
            }
        }
    }
}

/** Try a rule: each chip puts its rule in the search field. */
private fun LazyGridScope.tryARule(rules: List<Pair<String, String>>, onRule: (String) -> Unit) {
    if (rules.isEmpty()) return
    edgeToEdge(16.dp, key = "rules-title") { ShelfTitle("Try a rule", null) }
    edgeToEdge(16.dp, key = "rules") { RuleChips(rules, onRule) }
}

@Composable
private fun NothingMatches() {
    Text("Nothing matches", color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.padding(horizontal = Edge, vertical = 16.dp))
}

/** The chips, as the Library's pills: the page's tone a step up, the chosen one in amber. */
@Composable
private fun SearchKinds(kind: SearchKind, onKind: (SearchKind) -> Unit) {
    val pill = FilterChipDefaults.filterChipColors(
        containerColor = MaterialTheme.colorScheme.surfaceContainerHigh,
        labelColor = MaterialTheme.colorScheme.onSurface,
        selectedContainerColor = MaterialTheme.colorScheme.primary,
        selectedLabelColor = MaterialTheme.colorScheme.onPrimary,
    )
    LazyRow(contentPadding = PaddingValues(horizontal = 16.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        items(SearchKind.entries, key = { it.name }) { k ->
            FilterChip(
                selected = kind == k,
                onClick = { onKind(k) },
                label = { Text(k.label) },
                shape = RoundedCornerShape(50),
                colors = pill,
                border = null,
            )
        }
    }
}

/** The match that heads the results, large: its cover (an artist's photo, round), its kind, its name, and Play. */
@Composable
private fun BestMatch(best: SearchBest, open: (Route) -> Unit) {
    val scope = rememberCoroutineScope()
    val song = best.song
    val album = best.album
    val artist = best.artist
    val photo by rememberLoaded(artist?.name, cache = false) { artist?.let { artistPhoto(it.name) } }
    val (kind, name, sub, art) = when {
        album != null -> listOf("Album", album.title.ifBlank { "Unknown album" }, album.artist, album.artwork)
        artist != null -> listOf("Artist", artist.name.ifBlank { "Unknown artist" }, count(artist.songs.toInt(), "song"), photo ?: artist.artwork)
        else -> listOf("Song", song?.title.orEmpty(), song?.artist.orEmpty(), song?.artwork)
    }
    val shape = if (artist != null) artistShape() else CoverShape
    val go: () -> Unit = {
        when {
            album != null -> open(Route.AlbumPage(album))
            artist != null -> open(Route.Artist(artist.name))
            song != null -> playSongs(listOf(song.id))
        }
    }
    val play: () -> Unit = {
        scope.launch {
            val ids = withContext(Dispatchers.IO) {
                runCatching {
                    when {
                        album != null -> core.albumSongs(album.key).map { it.id }
                        artist != null -> core.artistSongs(artist.name).map { it.id }
                        else -> listOfNotNull(song?.id)
                    }
                }.getOrDefault(emptyList())
            }
            if (ids.isNotEmpty()) playSongs(ids)
        }
    }
    Row(
        Modifier
            .fillMaxWidth()
            .padding(horizontal = 16.dp, vertical = 8.dp)
            .clip(RoundedCornerShape(16.dp))
            .background(MaterialTheme.colorScheme.surfaceContainerHigh)
            .clickable(onClick = go)
            .padding(16.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        Cover(art, Modifier.size(if (isWide()) 120.dp else 96.dp), shape, seed = name)
        Column(Modifier.weight(1f)) {
            Text(kind.orEmpty(), style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.primary)
            Text(name.orEmpty(), style = MaterialTheme.typography.headlineSmall, fontWeight = FontWeight.Bold, maxLines = 2, overflow = TextOverflow.Ellipsis)
            if (!sub.isNullOrBlank()) Text(sub, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant, maxLines = 1, overflow = TextOverflow.Ellipsis)
        }
        // A round amber Play, as on the player: the name keeps the room.
        androidx.compose.material3.FilledIconButton(
            onClick = play,
            shape = androidx.compose.foundation.shape.CircleShape,
            colors = androidx.compose.material3.IconButtonDefaults.filledIconButtonColors(
                containerColor = MaterialTheme.colorScheme.primary,
                contentColor = MaterialTheme.colorScheme.onPrimary,
            ),
            modifier = Modifier.size(56.dp),
        ) { androidx.compose.material3.Icon(androidx.compose.material.icons.Icons.Rounded.PlayArrow, contentDescription = "Play", modifier = Modifier.size(30.dp)) }
    }
}

/** A song whose lyrics hold the search: the line with the words lit in amber, the song under it; a tap plays it from the start. */
@Composable
private fun LyricHitRow(hit: LyricHit, query: String, playing: Boolean) {
    val lit = MaterialTheme.colorScheme.primary
    Column(
        Modifier
            .bleed(16.dp)
            .fillMaxWidth()
            .clickable { playSongs(listOf(hit.song.id)) }
            .padding(horizontal = Edge, vertical = 10.dp),
    ) {
        Text(litWords(hit.line, query, SpanStyle(color = lit, fontWeight = FontWeight.SemiBold)), style = MaterialTheme.typography.titleMedium, maxLines = 2, overflow = TextOverflow.Ellipsis)
        Text(
            "${hit.song.title} · ${hit.song.artist}",
            style = MaterialTheme.typography.bodyMedium,
            color = if (playing) lit else MaterialTheme.colorScheme.onSurfaceVariant,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.padding(top = 2.dp),
        )
    }
}

/** `line` with each of the search's words lit, wherever it stands (any case). */
private fun litWords(line: String, query: String, style: SpanStyle): AnnotatedString {
    val words = query.split(' ').map { it.trim('"', ',', '.') }.filter { it.isNotEmpty() }
    val lit = BooleanArray(line.length)
    val lower = line.lowercase()
    words.forEach { w ->
        val needle = w.lowercase()
        var at = lower.indexOf(needle)
        while (at >= 0) {
            for (i in at until (at + needle.length).coerceAtMost(line.length)) lit[i] = true
            at = lower.indexOf(needle, at + needle.length)
        }
    }
    return buildAnnotatedString {
        line.forEachIndexed { i, c -> if (lit[i]) withStyle(style) { append(c) } else append(c) }
    }
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun RuleChips(rules: List<Pair<String, String>>, onRule: (String) -> Unit) {
    FlowRow(
        Modifier.fillMaxWidth().padding(horizontal = Edge),
        horizontalArrangement = Arrangement.spacedBy(8.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        rules.forEach { (name, rule) ->
            androidx.compose.material3.SuggestionChip(onClick = { onRule(rule) }, label = { Text(name) }, shape = RoundedCornerShape(50))
        }
    }
}
