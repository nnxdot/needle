package fyi.nnx.needle.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.detectVerticalDragGestures
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.rememberLazyGridState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.Sort
import androidx.compose.material.icons.rounded.Check
import androidx.compose.material.icons.rounded.Collections
import androidx.compose.material.icons.rounded.Image
import androidx.compose.material.icons.rounded.Restore
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import fyi.nnx.needle.NeedleApp
import fyi.nnx.needle.core.Artist
import fyi.nnx.needle.core.ArtistEntry
import kotlinx.coroutines.launch

/** The orders the Artists list can take (the core's `artist_order`), as people see them. */
val ArtistOrders = listOf("name" to "Name", "most_played" to "Most played", "most_songs" to "Most songs", "recently_added" to "Recently added")

private fun ArtistEntry.asArtist() = Artist(name, albums, songs, artwork)

/**
 * Sort and Only album artists for the Artists list: a menu from the sort button, as the other
 * lists' sorts. The choice is the core's, the same as the computer's.
 */
@Composable
fun ArtistSortButton() {
    val more by NeedleApp.instance.more.collectAsState()
    var open by remember { mutableStateOf(false) }
    val current = ArtistOrders.firstOrNull { it.first == more.artistOrder } ?: ArtistOrders.first()
    Box {
        IconButton(onClick = { open = true }) { Icon(Icons.AutoMirrored.Rounded.Sort, contentDescription = "Sort: ${current.second}") }
        DropdownMenu(expanded = open, onDismissRequest = { open = false }, shape = RoundedCornerShape(16.dp)) {
            ArtistOrders.forEach { (id, label) ->
                DropdownMenuItem(
                    text = { Text(label) },
                    trailingIcon = { if (id == more.artistOrder) Icon(Icons.Rounded.Check, null, tint = MaterialTheme.colorScheme.primary) },
                    onClick = { open = false; NeedleApp.instance.updateMore { it.copy(artistOrder = id) } },
                )
            }
            HorizontalDivider(color = MaterialTheme.colorScheme.outlineVariant)
            DropdownMenuItem(
                text = { Text("Only album artists") },
                trailingIcon = { if (more.albumArtistsOnly) Icon(Icons.Rounded.Check, null, tint = MaterialTheme.colorScheme.primary) },
                onClick = { open = false; NeedleApp.instance.updateMore { it.copy(albumArtistsOnly = !it.albumArtistsOnly) } },
            )
        }
    }
}

/**
 * Every artist, in the order chosen: circles as many across as fit, and (ordered by name) an
 * A–Z bar at the right edge that jumps to a letter, tapped or slid along.
 */
@Composable
fun ArtistGrid(open: (Route) -> Unit, modifier: Modifier = Modifier, top: androidx.compose.ui.unit.Dp = 8.dp) {
    val more by NeedleApp.instance.more.collectAsState()
    val listLoad = rememberLoaded(more.artistOrder, more.albumArtistsOnly) { artistList() }
    val loaded by listLoad
    val list = loaded.orEmpty()
    val state = rememberLazyGridState()
    val scope = rememberCoroutineScope()
    // The letters there are, each with the first artist filed under it.
    val letters = remember(list) {
        val firsts = LinkedHashMap<String, Int>()
        list.forEachIndexed { i, a -> firsts.putIfAbsent(a.letter, i) }
        firsts
    }
    val alphabetical = more.artistOrder == "name" && letters.size > 3
    Box(modifier.fillMaxSize()) {
        LazyVerticalGrid(
            columns = GridCells.Adaptive(104.dp),
            state = state,
            contentPadding = PaddingValues(start = 16.dp, end = if (alphabetical) 36.dp else 16.dp, top = top, bottom = 32.dp + LocalBottomInset.current),
            horizontalArrangement = Arrangement.spacedBy(12.dp),
            verticalArrangement = Arrangement.spacedBy(LocalRows.current.grid),
            modifier = Modifier.fillMaxSize(),
        ) {
            items(list.size, key = { list[it].name }) { i -> ArtistTile(list[i].asArtist(), Modifier, open) }
        }
        if (alphabetical) {
            AlphabetBar(
                letters.keys.toList(),
                Modifier.align(Alignment.CenterEnd).padding(end = 4.dp, bottom = LocalBottomInset.current),
            ) { letter -> letters[letter]?.let { scope.launch { state.scrollToItem(it) } } }
        }
    }
}

/**
 * The letters down the right edge: a tap jumps to one; sliding a finger along them jumps as it
 * goes, as a phone's contacts do. Each letter is its own button to TalkBack.
 */
@Composable
private fun AlphabetBar(letters: List<String>, modifier: Modifier, onLetter: (String) -> Unit) {
    var height by remember { mutableIntStateOf(1) }
    var last by remember { mutableStateOf<String?>(null) }
    fun at(y: Float) = letters[((y / height) * letters.size).toInt().coerceIn(0, letters.size - 1)]
    Column(
        modifier
            .width(28.dp)
            .clip(RoundedCornerShape(14.dp))
            .onSizeChanged { height = it.height.coerceAtLeast(1) }
            .pointerInput(letters) {
                detectVerticalDragGestures(
                    onDragStart = { o -> at(o.y).let { last = it; onLetter(it) } },
                    onDragEnd = { last = null },
                ) { change, _ ->
                    val l = at(change.position.y)
                    if (l != last) { last = l; onLetter(l) }
                }
            }
            .padding(vertical = 4.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        letters.forEach { letter ->
            Text(
                letter,
                style = MaterialTheme.typography.labelSmall,
                fontWeight = if (letter == last) FontWeight.Bold else FontWeight.SemiBold,
                color = if (letter == last) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier
                    .clickable { onLetter(letter) }
                    .semantics { contentDescription = "Jump to $letter" }
                    .padding(vertical = 1.dp),
            )
        }
    }
}

/** An artist's three pictures, as their page's menu names them. */
private val PictureKinds = listOf("photo" to "Photo", "banner" to "Wide picture", "logo" to "Logo")

/**
 * An artist's pictures, from the More button on their page: for the photo, the wide picture,
 * and the logo, Choose picture… (a file), Pick from fanart.tv (all of fanart.tv's choices), and
 * Reset (the automatic one again). A chosen picture is kept and never replaced.
 */
@Composable
fun ArtistPictures(name: String, shown: Boolean, onShown: (Boolean) -> Unit, onChanged: () -> Unit) {
    // Kept on the artist's page, not in the sheet: the file picker answers after the sheet has gone.
    val context = androidx.compose.ui.platform.LocalContext.current
    var choosing by remember { mutableStateOf<String?>(null) }
    var fanart by remember { mutableStateOf<String?>(null) }
    // Which kinds have a picture of their own now (Reset only for those).
    val ownLoad = rememberLoaded(name, shown, cache = false) { PictureKinds.map { it.first }.filter { ownArtistPicture(name, it) != null } }
    val own by ownLoad
    val pick = androidx.activity.compose.rememberLauncherForActivityResult(androidx.activity.result.contract.ActivityResultContracts.OpenDocument()) { uri ->
        val kind = choosing ?: return@rememberLauncherForActivityResult
        pickedFile(context, uri, { core.setArtistPicture(name, kind, it) }) { onChanged(); showMessage("Picture kept for $name") }
    }
    if (shown) ActionSheet(null, name, "Pictures", { onShown(false) }) { act ->
        PictureKinds.forEach { (kind, label) ->
            Text(label, style = MaterialTheme.typography.titleSmall, color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.padding(start = 16.dp, top = 8.dp))
            MenuGroup {
                MenuRow(Icons.Rounded.Image, "Choose picture…") {
                    choosing = kind
                    act { tryOpen("file picker") { pick.launch(arrayOf("image/*")) } }
                }
                MenuRow(Icons.Rounded.Collections, "Pick from fanart.tv") { act { fanart = kind } }
                if (own.orEmpty().contains(kind)) {
                    MenuRow(Icons.Rounded.Restore, "Reset") {
                        act { offMain({ runCatching { core.resetArtistPicture(name, kind) } }, onChanged) }
                    }
                }
            }
        }
    }
    fanart?.let { kind -> FanartPicker(name, kind, onChanged) { fanart = null } }
}

/** All of fanart.tv's pictures of one kind for an artist, best first: a tap keeps one. */
@Composable
private fun FanartPicker(name: String, kind: String, onChanged: () -> Unit, onDismiss: () -> Unit) {
    val choicesLoad = rememberLoaded(name, kind, cache = false) { artistFanartChoices(name, kind) }
    val choices by choicesLoad
    var saving by remember { mutableStateOf(false) }
    androidx.compose.material3.AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(PictureKinds.firstOrNull { it.first == kind }?.second ?: "Picture") },
        text = {
            when {
                choicesLoad.loading -> Text("Looking on fanart.tv…")
                choices.isNullOrEmpty() -> Text("fanart.tv has no pictures of this kind for $name. Online lookups must be on.")
                else -> androidx.compose.foundation.lazy.grid.LazyVerticalGrid(
                    columns = GridCells.Fixed(if (kind == "photo") 3 else 2),
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                    verticalArrangement = Arrangement.spacedBy(8.dp),
                    modifier = Modifier.heightIn(max = 420.dp),
                ) {
                    val list = choices.orEmpty()
                    items(list.size) { i ->
                        val c = list[i]
                        coil3.compose.AsyncImage(
                            model = c.preview,
                            contentDescription = "Picture ${i + 1}, ${c.likes} likes",
                            contentScale = if (kind == "logo") androidx.compose.ui.layout.ContentScale.Fit else androidx.compose.ui.layout.ContentScale.Crop,
                            modifier = Modifier
                                .fillMaxWidth()
                                .aspectRatio(if (kind == "photo") 1f else if (kind == "logo") 2.6f else 1.78f)
                                .clip(RoundedCornerShape(8.dp))
                                .background(MaterialTheme.colorScheme.surfaceContainerHighest)
                                .clickable(enabled = !saving) {
                                    saving = true
                                    var kept: Result<String>? = null
                                    offMain({ kept = runCatching { core.pickArtistFanart(name, kind, c.url) } }) {
                                        kept?.onSuccess { showMessage("Picture kept for $name"); onChanged() }
                                            ?.onFailure { showMessage(it.message ?: "Could not keep that picture") }
                                        onDismiss()
                                    }
                                },
                        )
                    }
                }
            }
        },
        confirmButton = { androidx.compose.material3.TextButton(onClick = onDismiss) { Text("Close") } },
    )
}

/** Work off the main thread, then `then` on it. */
private fun offMain(work: () -> Unit, then: () -> Unit = {}) {
    NeedleApp.instance.scope.launch(kotlinx.coroutines.Dispatchers.Main) {
        kotlinx.coroutines.withContext(kotlinx.coroutines.Dispatchers.IO) { work() }
        then()
    }
}
