package fyi.nnx.needle.ui

import android.content.Context
import android.os.Build
import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.snap
import androidx.compose.animation.core.tween
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.LocalIndication
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.indication
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.selected
import kotlinx.coroutines.flow.MutableStateFlow
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.grid.GridItemSpan
import androidx.compose.foundation.lazy.grid.LazyGridScope
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.QueueMusic
import androidx.compose.material.icons.rounded.Close
import androidx.compose.material.icons.rounded.History
import androidx.compose.material.icons.rounded.PlayArrow
import androidx.compose.material.icons.rounded.Search
import androidx.compose.material.icons.rounded.Shuffle
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.blur
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import fyi.nnx.needle.NeedleApp
import fyi.nnx.needle.core.LyricHit
import fyi.nnx.needle.core.Needle
import fyi.nnx.needle.core.RecentSearch
import fyi.nnx.needle.core.SearchBest
import fyi.nnx.needle.core.SearchFound
import fyi.nnx.needle.core.Song
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

/**
 * How Search's Browse shows the library before typing: by genre (the first time) or by decade,
 * for libraries whose genre tags are a mess. Kept on the phone, as the Library's Grid or List
 * ([LibraryView]); not a setting.
 */
enum class BrowseBy(val label: String) {
    Genres("Genres"), Decades("Decades");

    companion object {
        private val prefs get() = NeedleApp.instance.getSharedPreferences("search", Context.MODE_PRIVATE)
        val chosen = MutableStateFlow(if (prefs.getString("browse", null) == Decades.name) Decades else Genres)

        fun choose(by: BrowseBy) {
            chosen.value = by
            prefs.edit().putString("browse", by.name).apply()
        }
    }
}

/**
 * Genres | Decades at the end of Browse's title: a small bar of words, as the Library's, the
 * chosen word lit with the same soft pill, which fades across. One stop for TalkBack's "Browse
 * by", then a tab for each; each word takes the remote's focus.
 */
@Composable
internal fun BrowseSwitch(by: BrowseBy) {
    val scheme = MaterialTheme.colorScheme
    val reduce = reduceMotion()
    Row(
        Modifier
            .semantics { contentDescription = "Browse by" }
            .selectableGroup()
            .height(48.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Row(
            Modifier
                .height(36.dp)
                .clip(CircleShape)
                .background(scheme.surfaceContainerLow)
                .border(1.dp, scheme.onSurface.copy(alpha = 0.08f), CircleShape)
                .padding(horizontal = 2.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            BrowseBy.entries.forEach { option ->
                val chosen = option == by
                val interaction = remember { MutableInteractionSource() }
                val light by animateColorAsState(
                    if (chosen) scheme.onSurface.copy(alpha = 0.09f) else scheme.onSurface.copy(alpha = 0f),
                    if (reduce) snap() else tween(180),
                    label = "browse-light",
                )
                val ink by animateColorAsState(
                    if (chosen) scheme.primary else scheme.onSurfaceVariant,
                    if (reduce) snap() else tween(180),
                    label = "browse-ink",
                )
                Box(
                    Modifier
                        // 48 dp tall to touch; the pill drawn is 28 dp.
                        .height(48.dp)
                        .clickable(interactionSource = interaction, indication = null, role = Role.Tab) { if (!chosen) BrowseBy.choose(option) }
                        .semantics { selected = chosen },
                    contentAlignment = Alignment.Center,
                ) {
                    Box(
                        Modifier
                            .height(28.dp)
                            .clip(CircleShape)
                            .background(light)
                            .indication(interaction, LocalIndication.current)
                            .padding(horizontal = 11.dp),
                        contentAlignment = Alignment.Center,
                    ) {
                        Text(
                            option.label,
                            style = MaterialTheme.typography.labelLarge,
                            // One weight for both: the words keep their width when the choice changes.
                            fontWeight = FontWeight.SemiBold,
                            color = ink,
                            maxLines = 1,
                        )
                    }
                }
            }
        }
    }
}

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

/** How many of `kind` the search found in all; none for All. */
fun SearchFound.countOf(kind: SearchKind): Int? = when (kind) {
    SearchKind.All -> null
    SearchKind.Songs -> songCount.toInt()
    SearchKind.Albums -> albumCount.toInt()
    SearchKind.Artists -> artistCount.toInt()
    SearchKind.Lyrics -> lyricCount.toInt()
}

/** The search found nothing at all. */
fun SearchFound.nothing(): Boolean =
    best == null && songs.isEmpty() && albums.isEmpty() && artists.isEmpty() && lyrics.isEmpty() &&
        songCount == 0u && albumCount == 0u && artistCount == 0u && lyricCount == 0u

/**
 * Whether the words typed are a rule (rating >= 4, played(30d)): a rule's words are not lit in
 * the results. `hinted`: the core offers what can come next, as it does only in a rule.
 */
fun looksLikeRule(query: String, hinted: Boolean): Boolean = hinted || query.any { it in "=<>()" }

// ---------- What Search remembers

/** Raised after each change to the recent searches, so Search reads them again. */
internal val recentVersion = mutableIntStateOf(0)

/** A result opened or played from Search, or words searched with the keyboard's Search key. */
internal fun keepSearch(kind: String, key: String) {
    if (key.isNotBlank()) changeRecent { rememberSearch(kind, key) }
}

private fun changeRecent(work: Needle.() -> Unit) {
    NeedleApp.instance.scope.launch(Dispatchers.IO) {
        runCatching { core.work() }
        withContext(Dispatchers.Main) { recentVersion.intValue++ }
    }
}

/** A song from Search: kept, then played with the songs around it. */
private fun playFound(songs: List<Song>, i: Int) {
    keepSearch("song", songs[i].id)
    playSongs(songs.map { it.id }, i)
}

/** Opens a page from Search, keeping an artist or an album it opens as a recent search. */
private fun keepAndOpen(open: (Route) -> Unit): (Route) -> Unit = { route ->
    when (route) {
        is Route.Artist -> keepSearch("artist", route.name)
        is Route.AlbumPage -> keepSearch("album", route.album.key)
        else -> Unit
    }
    open(route)
}

// ---------- The parts of the page

/** A part of the page across the whole grid, to its edges; a new one fades in. */
private fun LazyGridScope.whole(key: Any, content: @Composable () -> Unit) =
    item(key = key, span = { GridItemSpan(maxLineSpan) }) {
        Box(Modifier.animateItem(fadeInSpec = tween(180), placementSpec = null, fadeOutSpec = null).bleed(16.dp)) { content() }
    }

/** A section's title, as the shelves', with the action at its end ("Clear", "All 24 ›"). */
@Composable
private fun Title(text: String, action: String? = null, onAction: (() -> Unit)? = null) =
    ShelfTitle(text, null, more = action ?: "See all", onMore = onAction)

/**
 * Recent: the words searched for and the results opened from Search, newest first. A phone has
 * a row for each (up to 8); a wide page one line of up to 4. "Clear" drops them all, at once.
 */
internal fun LazyGridScope.recentSection(recent: List<RecentSearch>, onWords: (String) -> Unit, open: (Route) -> Unit) {
    if (recent.isEmpty()) return
    whole("recent-title") { Title("Recent", "Clear") { changeRecent { clearSearches() } } }
    whole("recent") {
        if (isWide()) {
            Row(Modifier.fillMaxWidth().padding(horizontal = Edge), horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                recent.take(4).forEach { r -> RecentTile(r, Modifier.weight(1f), onWords, open) }
                repeat(4 - recent.take(4).size) { Spacer(Modifier.weight(1f)) }
            }
        } else {
            Column { recent.take(8).forEach { r -> RecentRow(r, onWords, open) } }
        }
    }
}

/** What a press on a recent search does: words go in the field, a song plays, an album or artist opens. */
private fun pressRecent(r: RecentSearch, onWords: (String) -> Unit, open: (Route) -> Unit) {
    val song = r.song
    val album = r.album
    val artist = r.artist
    when {
        r.kind == "words" -> onWords(r.key)
        song != null -> { keepSearch("song", song.id); playSongs(listOf(song.id)) }
        album != null -> keepAndOpen(open)(Route.AlbumPage(album))
        artist != null -> keepAndOpen(open)(Route.Artist(artist.name))
    }
}

/** A recent search's name and grey line ("Album · Nora Vale"); short on a wide page's tile. */
private fun recentWords(r: RecentSearch, short: Boolean): Pair<String, String> = when {
    r.song != null -> r.song!!.title to (if (short) "Song" else "Song · ${r.song!!.artist}")
    r.album != null -> r.album!!.title.ifBlank { "Unknown album" } to (if (short) "Album" else "Album · ${r.album!!.artist}")
    r.artist != null -> r.artist!!.name.ifBlank { "Unknown artist" } to "Artist"
    else -> r.key to (if (short) "Search" else "You searched for this")
}

/** A recent search's picture: the cover, an artist's round photo, or a clock for words. */
@Composable
private fun RecentCover(r: RecentSearch, size: Dp) {
    val artist = r.artist
    val photo by rememberLoaded(artist?.name, cache = false) { artist?.let { artistPhoto(it.name) } }
    when {
        r.kind == "words" -> Box(
            Modifier.size(size).clip(SmallCoverShape).background(MaterialTheme.colorScheme.surfaceContainerHigh),
            contentAlignment = Alignment.Center,
        ) { Icon(Icons.Rounded.History, contentDescription = null, tint = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.size(size * 0.5f)) }
        artist != null -> Cover(photo ?: artist.artwork, Modifier.size(size), artistShape(), seed = artist.name)
        r.album != null -> Cover(r.album!!.artwork, Modifier.size(size), SmallCoverShape, seed = r.album!!.title + r.album!!.artist)
        else -> Cover(r.song?.artwork, Modifier.size(size), SmallCoverShape, seed = r.song?.album + r.song?.artist)
    }
}

@Composable
private fun RecentRow(r: RecentSearch, onWords: (String) -> Unit, open: (Route) -> Unit) {
    val rows = LocalRows.current
    val (name, line) = recentWords(r, short = false)
    Row(
        Modifier
            .fillMaxWidth()
            .clickable { pressRecent(r, onWords, open) }
            .heightIn(min = rows.height)
            .padding(start = Edge, end = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(rows.gap),
    ) {
        RecentCover(r, rows.cover)
        Column(Modifier.weight(1f).padding(vertical = 6.dp)) {
            Text(name, style = MaterialTheme.typography.bodyLarge, maxLines = 1, overflow = TextOverflow.Ellipsis)
            Text(line, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant, maxLines = 1, overflow = TextOverflow.Ellipsis)
        }
        ForgetButton(r)
    }
}

/** On a wide page: a recent search as a small tile, four in a line. */
@Composable
private fun RecentTile(r: RecentSearch, modifier: Modifier, onWords: (String) -> Unit, open: (Route) -> Unit) {
    val (name, line) = recentWords(r, short = true)
    Row(
        modifier
            .clip(RoundedCornerShape(12.dp))
            .background(MaterialTheme.colorScheme.surfaceContainerLow)
            .clickable { pressRecent(r, onWords, open) }
            .padding(start = 8.dp, top = 8.dp, bottom = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        RecentCover(r, 40.dp)
        Column(Modifier.weight(1f)) {
            Text(name, style = MaterialTheme.typography.bodyMedium, fontWeight = FontWeight.Medium, maxLines = 1, overflow = TextOverflow.Ellipsis)
            Text(line, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant, maxLines = 1, overflow = TextOverflow.Ellipsis)
        }
        ForgetButton(r)
    }
}

/** The ×: drops this one recent search. */
@Composable
private fun ForgetButton(r: RecentSearch) {
    IconButton(onClick = { changeRecent { forgetSearch(r.kind, r.key) } }) {
        Icon(Icons.Rounded.Close, contentDescription = "Remove from Recent", tint = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.size(20.dp))
    }
}

/** Rules: each chip puts its rule in the search field. A wide page shows the rule too. */
internal fun LazyGridScope.rulesSection(rules: List<Pair<String, String>>, onRule: (String) -> Unit) {
    if (rules.isEmpty()) return
    whole("rules-title") { Title("Rules") }
    whole("rules") { RuleChips(rules, onRule) }
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun RuleChips(rules: List<Pair<String, String>>, onRule: (String) -> Unit) {
    val wide = isWide()
    // One line, which scrolls sideways on a narrow phone: the rules never take two lines.
    Row(
        Modifier.fillMaxWidth().horizontalScroll(androidx.compose.foundation.rememberScrollState()).padding(horizontal = Edge),
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        rules.forEach { (name, rule) ->
            androidx.compose.material3.SuggestionChip(
                onClick = { onRule(rule) },
                label = {
                    Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
                        Text(name)
                        if (wide) Text(rule, fontFamily = FontFamily.Monospace, fontSize = 12.sp, color = MaterialTheme.colorScheme.onSurfaceVariant)
                    }
                },
                shape = RoundedCornerShape(50),
            )
        }
    }
}

/** The kinds, pinned under the field: the Library's bar of words, each but All with its count. */
@Composable
internal fun SearchKinds(found: SearchFound?, kind: SearchKind, onKind: (SearchKind) -> Unit) {
    WordBar(SearchKind.entries, kind, { it.label }, count = { found?.countOf(it) }, choose = onKind)
}

/**
 * The results under the pinned field and kinds. All: the top result, Songs (5), Albums,
 * Artists, In the lyrics (3); a part with nothing is left out. Each other kind shows all of
 * its kind. Nothing found: a note, then Rules and Recent; a rule with an error: its error, then
 * Rules.
 */
internal fun LazyGridScope.searchResults(
    answer: SearchAnswer,
    kind: SearchKind,
    onKind: (SearchKind) -> Unit,
    rules: List<Pair<String, String>>,
    onRule: (String) -> Unit,
    recent: List<RecentSearch>,
    rule: Boolean,
    /** A wide page with room for two columns (the top result beside the songs). */
    duo: Boolean,
    playing: String?,
    open: (Route) -> Unit,
    /** A music server found songs: they show under these results, so this is not "Nothing". */
    serverHas: Boolean = false,
) {
    answer.error?.let { error ->
        whole("error") {
            Text(error, color = MaterialTheme.colorScheme.error, modifier = Modifier.padding(horizontal = Edge, vertical = 12.dp))
        }
        rulesSection(rules, onRule)
        return
    }
    val found = answer.found ?: return
    if (found.nothing()) {
        if (serverHas) return
        whole("nothing") { NothingFor(answer.query) }
        rulesSection(rules, onRule)
        recentSection(recent, onRule, open)
        return
    }
    // A rule's words are not lit: they are not in the names.
    val lit = answer.query.takeUnless { rule }
    val go = keepAndOpen(open)
    val songs = answer.songs
    when (kind) {
        SearchKind.All -> allResults(found, lit, onKind, playing, open, go, answer.query, duo)
        SearchKind.Songs -> {
            whole("songs-line") { SongsLine(found.songCount.toInt().coerceAtLeast(songs.size), songs) }
            items(songs.size, key = { "song-" + songs[it].id }, span = { GridItemSpan(maxLineSpan) }) { i ->
                val columns = LocalPageWidth.current >= SongColumnsFrom
                // The album on the grey line, where the row has no column for it.
                SongRow(
                    songs[i], playing == songs[i].id, open,
                    Modifier.animateItem(fadeInSpec = tween(180), placementSpec = null, fadeOutSpec = null).bleed(16.dp),
                    place = songs.place(i),
                    detail = if (columns) null else songs[i].album.ifBlank { null },
                    lit = lit,
                ) { playFound(songs, i) }
            }
        }
        SearchKind.Albums -> {
            items(found.albums.size, key = { "album-" + found.albums[it].key }) { i ->
                val album = found.albums[i]
                AlbumTile(album, Modifier.animateItem(fadeInSpec = tween(180), placementSpec = null, fadeOutSpec = null).padding(bottom = 16.dp), open, lit = lit) { go(Route.AlbumPage(album)) }
            }
        }
        SearchKind.Artists -> {
            items(found.artists.size, key = { "artist-" + found.artists[it].name }) { i ->
                ArtistTile(found.artists[i], Modifier.animateItem(fadeInSpec = tween(180), placementSpec = null, fadeOutSpec = null).padding(bottom = 16.dp), go, lit = lit)
            }
        }
        SearchKind.Lyrics -> {
            items(found.lyrics.size, key = { "lyric-" + found.lyrics[it].song.id }, span = { GridItemSpan(maxLineSpan) }) { i ->
                Box(Modifier.animateItem(fadeInSpec = tween(180), placementSpec = null, fadeOutSpec = null).bleed(16.dp)) {
                    LyricHitRow(found.lyrics[i], answer.query, playing == found.lyrics[i].song.id)
                }
            }
        }
    }
}

/** All: the top result, Songs, Albums, Artists, In the lyrics. */
private fun LazyGridScope.allResults(
    found: SearchFound,
    lit: String?,
    onKind: (SearchKind) -> Unit,
    playing: String?,
    open: (Route) -> Unit,
    go: (Route) -> Unit,
    query: String,
    duo: Boolean,
) {
    val five = found.songs.take(5)
    val best = found.best
    val songsTitle: @Composable () -> Unit = {
        Title("Songs", "All ${number(found.songCount.toLong())} ›") { onKind(SearchKind.Songs) }
    }
    // The best match's own key: a new one fades in.
    val bestKey = best?.let { "best-" + (it.album?.key ?: it.artist?.name ?: it.song?.id) }
    whole(bestKey ?: "songs-head") {
        // A wide page with room: the top result at the left, the songs beside it.
        if (duo && best != null) {
            Row(Modifier.fillMaxWidth().padding(horizontal = 16.dp), horizontalArrangement = Arrangement.spacedBy(20.dp)) {
                TopResult(best, lit, open, wide = true, modifier = Modifier.width(292.dp))
                if (five.isNotEmpty()) {
                    Column(Modifier.weight(1f)) {
                        Row(Modifier.fillMaxWidth().padding(start = 4.dp), verticalAlignment = Alignment.CenterVertically) {
                            Text("Songs", style = MaterialTheme.typography.titleLarge, fontWeight = FontWeight.SemiBold, modifier = Modifier.weight(1f).semantics { heading() })
                            androidx.compose.material3.TextButton(onClick = { onKind(SearchKind.Songs) }) { Text("All ${number(found.songCount.toLong())} ›") }
                        }
                        five.forEachIndexed { i, s -> DuoSongRow(s, lit, playing == s.id, open) { playFound(five, i) } }
                    }
                }
            }
        } else {
            Column {
                if (best != null) TopResult(best, lit, open, wide = false, modifier = Modifier.padding(horizontal = 16.dp).padding(top = 8.dp).widthIn(max = 720.dp).fillMaxWidth())
                if (five.isNotEmpty()) songsTitle()
            }
        }
    }
    // The songs under the card (beside it on a wide page: drawn above).
    if (!(duo && best != null)) {
        items(five.size, key = { "song-" + five[it].id }, span = { GridItemSpan(maxLineSpan) }) { i ->
            SongRow(
                five[i], playing == five[i].id, open,
                Modifier.animateItem(fadeInSpec = tween(180), placementSpec = null, fadeOutSpec = null).bleed(16.dp),
                place = five.place(i),
                lit = lit,
            ) { playFound(five, i) }
        }
    }
    if (found.albums.isNotEmpty()) {
        whole("albums-title") { Title("Albums", "All ${number(found.albumCount.toLong())} ›") { onKind(SearchKind.Albums) } }
        whole("albums") {
            val tile = if (isWide()) 176.dp else 148.dp
            LazyRow(contentPadding = PaddingValues(horizontal = Edge), horizontalArrangement = Arrangement.spacedBy(16.dp)) {
                items(found.albums, key = { it.key }) { album -> AlbumTile(album, Modifier.width(tile), open, lit = lit) { go(Route.AlbumPage(album)) } }
            }
        }
    }
    val artists = found.artists
    val lyrics = found.lyrics.take(3)
    val artistsPart: @Composable () -> Unit = {
        Column {
            Title("Artists")
            LazyRow(contentPadding = PaddingValues(horizontal = Edge), horizontalArrangement = Arrangement.spacedBy(18.dp)) {
                items(artists, key = { it.name }) { a -> ArtistTile(a, Modifier.width(if (isWide()) 132.dp else 112.dp), go, lit = lit) }
            }
        }
    }
    val lyricsPart: @Composable () -> Unit = {
        Column {
            Title("In the lyrics", "All ${number(found.lyricCount.toLong())} ›") { onKind(SearchKind.Lyrics) }
            lyrics.forEach { hit -> LyricHitRow(hit, query, playing == hit.song.id) }
        }
    }
    when {
        artists.isNotEmpty() && lyrics.isNotEmpty() -> whole("artists-lyrics") {
            // A wide page: Artists and In the lyrics share one line.
            if (duo) {
                Row(Modifier.fillMaxWidth()) {
                    Box(Modifier.weight(1f)) { artistsPart() }
                    Box(Modifier.weight(1.2f)) { lyricsPart() }
                }
            } else {
                Column { artistsPart(); lyricsPart() }
            }
        }
        artists.isNotEmpty() -> whole("artists") { artistsPart() }
        lyrics.isNotEmpty() -> whole("lyrics") { lyricsPart() }
    }
}

/** The Songs chip's line: how many, and Shuffle (all that match, shuffled). */
@Composable
private fun SongsLine(count: Int, songs: List<Song>) {
    Row(
        Modifier.fillMaxWidth().heightIn(min = 56.dp).padding(start = Edge, end = 16.dp, top = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(count(count, "song"), style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.weight(1f))
        SmallPill(Icons.Rounded.Shuffle, "Shuffle", filled = true, enabled = songs.isNotEmpty()) {
            playSongs(songs.map { it.id }.shuffled())
        }
    }
}

/** Nothing found: a round sign, the words, and what to try. */
@Composable
private fun NothingFor(query: String) {
    Column(
        Modifier.fillMaxWidth().padding(start = 30.dp, end = 30.dp, top = 40.dp, bottom = 4.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Box(
            Modifier.size(64.dp).clip(CircleShape).background(MaterialTheme.colorScheme.surfaceContainerHigh),
            contentAlignment = Alignment.Center,
        ) { Icon(Icons.Rounded.Search, contentDescription = null, tint = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.size(28.dp)) }
        Text(
            "Nothing for “${query.trim()}”",
            style = MaterialTheme.typography.titleLarge,
            fontWeight = FontWeight.Bold,
            textAlign = TextAlign.Center,
            modifier = Modifier.padding(top = 14.dp),
        )
        Text(
            "Look at the spelling. Or search with a rule.",
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            textAlign = TextAlign.Center,
            modifier = Modifier.padding(top = 4.dp),
        )
    }
}

/** A small pill button: Play in the accent, or a quiet one (Shuffle, Play next). */
@Composable
private fun SmallPill(icon: ImageVector, label: String, filled: Boolean, enabled: Boolean = true, onClick: () -> Unit) {
    val content: @Composable androidx.compose.foundation.layout.RowScope.() -> Unit = {
        Icon(icon, contentDescription = null, modifier = Modifier.size(18.dp))
        Spacer(Modifier.width(4.dp))
        Text(label, style = MaterialTheme.typography.labelLarge, fontWeight = FontWeight.SemiBold, maxLines = 1, softWrap = false)
    }
    val padding = PaddingValues(start = 10.dp, end = 14.dp)
    if (filled) {
        Button(onClick = onClick, enabled = enabled, shape = CircleShape, contentPadding = padding, modifier = Modifier.height(40.dp), content = content)
    } else {
        val scheme = MaterialTheme.colorScheme
        OutlinedButton(
            onClick = onClick,
            enabled = enabled,
            shape = CircleShape,
            colors = ButtonDefaults.outlinedButtonColors(containerColor = scheme.onSurface.copy(alpha = 0.08f), contentColor = scheme.onSurface),
            border = androidx.compose.foundation.BorderStroke(0.5.dp, scheme.onSurface.copy(alpha = 0.14f)),
            contentPadding = padding,
            modifier = Modifier.height(40.dp),
            content = content,
        )
    }
}

/**
 * The top result: its cover blurred behind a veil, so the card takes the cover's colour, and a
 * thin glass edge. Its kind, its name, a grey line, and Play with Shuffle (an album, an artist)
 * or Play next (a song). A press on the card opens the album or artist, or plays the song.
 * Phone: the cover at the left; wide: the cover above the words.
 */
@OptIn(ExperimentalFoundationApi::class)
@Composable
private fun TopResult(best: SearchBest, lit: String?, open: (Route) -> Unit, wide: Boolean, modifier: Modifier) {
    val scope = rememberCoroutineScope()
    val song = best.song
    val album = best.album
    val artist = best.artist
    val photo by rememberLoaded(artist?.name, cache = false) { artist?.let { artistPhoto(it.name) } }
    val art = when {
        album != null -> album.artwork
        artist != null -> photo ?: artist.artwork
        else -> song?.artwork
    }
    val kind = when {
        album != null -> "Album"
        artist != null -> "Artist"
        else -> "Song"
    }
    val name = when {
        album != null -> album.title.ifBlank { "Unknown album" }
        artist != null -> artist.name.ifBlank { "Unknown artist" }
        else -> song?.title.orEmpty()
    }
    val detail = when {
        album != null -> listOfNotNull(album.artist.ifBlank { null }, album.year.takeIf { it > 0 }?.toString(), album.songs.takeIf { it > 0u }?.let { count(it.toInt(), "song") })
        artist != null -> listOfNotNull(artist.songs.takeIf { it > 0u }?.let { count(it.toInt(), "song") }, artist.albums.takeIf { it > 0u }?.let { count(it.toInt(), "album") })
        else -> listOfNotNull(song?.artist?.ifBlank { null }, song?.album?.ifBlank { null })
    }.joinToString(" · ")
    val tile = rememberTileId()
    val go: () -> Unit = {
        when {
            album != null -> { tapTile(tile); keepSearch("album", album.key); open(Route.AlbumPage(album)) }
            artist != null -> { keepSearch("artist", artist.name); open(Route.Artist(artist.name)) }
            song != null -> { keepSearch("song", song.id); playSongs(listOf(song.id)) }
        }
    }
    val play: (Boolean) -> Unit = { shuffled ->
        when {
            album != null -> keepSearch("album", album.key)
            artist != null -> keepSearch("artist", artist.name)
            song != null -> keepSearch("song", song.id)
        }
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
            if (ids.isNotEmpty()) playSongs(if (shuffled) ids.shuffled() else ids)
        }
    }
    val shape = RoundedCornerShape(22.dp)
    val dark = darkPage()
    val interaction = remember { MutableInteractionSource() }
    val coverShape = if (artist != null) artistShape() else RoundedCornerShape(10.dp)
    val coverSize = if (wide) 116.dp else 96.dp
    val glow = coverColor(art)
    Box(
        modifier
            .pressScale(interaction, 0.98f)
            .shadow(14.dp, shape, ambientColor = Color.Black.copy(alpha = 0.3f), spotColor = Color.Black.copy(alpha = 0.3f))
            .clip(shape)
            .background(glow)
            .combinedClickable(interactionSource = interaction, indication = androidx.compose.foundation.LocalIndication.current, onClick = go),
    ) {
        // Behind: the same cover, blurred (Android 12 and newer), under a veil.
        if (art != null && Build.VERSION.SDK_INT >= 31) {
            Cover(
                art,
                Modifier
                    .matchParentSize()
                    .graphicsLayer { scaleX = 1.6f; scaleY = 1.6f; alpha = 0.8f }
                    .blur(26.dp),
                androidx.compose.ui.graphics.RectangleShape,
            )
        }
        Box(Modifier.matchParentSize().background(if (dark) Color.Black.copy(alpha = 0.32f) else Color.White.copy(alpha = 0.68f)))
        val words: @Composable () -> Unit = {
            Text(
                "TOP RESULT · ${kind.uppercase()}",
                style = MaterialTheme.typography.labelSmall,
                fontWeight = FontWeight.SemiBold,
                letterSpacing = 0.8.sp,
                color = MaterialTheme.colorScheme.onSurface.copy(alpha = 0.72f),
            )
            Text(
                litText(name, lit, MaterialTheme.colorScheme.primary),
                style = if (wide) MaterialTheme.typography.headlineSmall else MaterialTheme.typography.titleLarge,
                fontWeight = FontWeight.ExtraBold,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier.padding(top = 2.dp),
            )
            if (detail.isNotEmpty()) {
                Text(detail, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurface.copy(alpha = 0.78f), maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.padding(top = 2.dp))
            }
            Row(Modifier.padding(top = if (wide) 14.dp else 10.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                SmallPill(Icons.Rounded.PlayArrow, "Play", filled = true) { play(false) }
                if (song != null && album == null && artist == null) {
                    SmallPill(Icons.AutoMirrored.Rounded.QueueMusic, "Play next", filled = false) {
                        keepSearch("song", song.id)
                        addToQueue(true) { listOf(song.id) }
                    }
                } else {
                    SmallPill(Icons.Rounded.Shuffle, "Shuffle", filled = false) { play(true) }
                }
            }
        }
        val cover: @Composable () -> Unit = {
            Cover(
                art,
                Modifier
                    .size(coverSize)
                    .shadow(10.dp, coverShape)
                    .then(if (album != null) Modifier.tileCover(tile, "album-${album.key}") else Modifier),
                coverShape,
                seed = name,
            )
        }
        if (wide) {
            Column(Modifier.fillMaxWidth().padding(18.dp)) {
                cover()
                Spacer(Modifier.height(14.dp))
                words()
            }
        } else {
            Row(Modifier.fillMaxWidth().padding(14.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(14.dp)) {
                cover()
                Column(Modifier.weight(1f)) { words() }
            }
        }
        // The thin glass edge.
        Box(Modifier.matchParentSize().edgeLight(shape, dark))
    }
}

/** A song beside the top result on a wide page: cover, title above artist, length at the end. */
@OptIn(ExperimentalFoundationApi::class)
@Composable
private fun DuoSongRow(song: Song, lit: String?, playing: Boolean, open: (Route) -> Unit, onClick: () -> Unit) {
    var menu by remember { mutableStateOf(false) }
    if (menu) SongSheet(song, open, null) { menu = false }
    val interaction = remember { MutableInteractionSource() }
    Row(
        Modifier
            .fillMaxWidth()
            .clip(RowShape)
            .background(if (playing) MaterialTheme.colorScheme.onSurface.copy(alpha = 0.06f) else Color.Transparent)
            .combinedClickable(interactionSource = interaction, indication = androidx.compose.foundation.LocalIndication.current, onClick = onClick, onLongClick = { menu = true })
            .padding(horizontal = 8.dp, vertical = 5.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Cover(song.artwork, Modifier.size(42.dp), SmallCoverShape, seed = song.album + song.artist)
        Column(Modifier.weight(1f)) {
            Text(
                litText(song.title, lit, MaterialTheme.colorScheme.primary),
                style = MaterialTheme.typography.bodyLarge,
                color = if (playing) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurface,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            Text(song.artist, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant, maxLines = 1, overflow = TextOverflow.Ellipsis)
        }
        Text(time(song.duration), style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
    }
}

/** A song whose lyrics hold the search: the line with the words lit in amber, the song under it; a tap plays it from the start. */
@Composable
private fun LyricHitRow(hit: LyricHit, query: String, playing: Boolean) {
    val lit = MaterialTheme.colorScheme.primary
    Row(
        Modifier
            .fillMaxWidth()
            .clickable { keepSearch("song", hit.song.id); playSongs(listOf(hit.song.id)) }
            .padding(horizontal = Edge, vertical = 8.dp),
    ) {
        Column(Modifier.weight(1f)) {
            Text(litText("“" + hit.line.trim() + "”", query, lit), style = MaterialTheme.typography.titleMedium, maxLines = 2, overflow = TextOverflow.Ellipsis)
            Row(Modifier.padding(top = 5.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(7.dp)) {
                Cover(hit.song.artwork, Modifier.size(18.dp), RoundedCornerShape(4.dp), seed = hit.song.album + hit.song.artist)
                Text(
                    "${hit.song.title} · ${hit.song.artist}",
                    style = MaterialTheme.typography.bodyMedium,
                    color = if (playing) lit else MaterialTheme.colorScheme.onSurfaceVariant,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
        }
    }
}
