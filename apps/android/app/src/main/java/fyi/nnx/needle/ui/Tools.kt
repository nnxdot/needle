package fyi.nnx.needle.ui

import android.content.Context
import android.content.Intent
import android.net.Uri
import android.net.wifi.WifiManager
import android.os.Build
import android.os.Environment
import android.provider.MediaStore
import android.provider.Settings as AndroidSettings
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.IntentSenderRequest
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Album
import androidx.compose.material.icons.rounded.Cast
import androidx.compose.material.icons.rounded.History
import androidx.compose.material.icons.rounded.LibraryMusic
import androidx.compose.material.icons.rounded.PhoneAndroid
import androidx.compose.material.icons.rounded.PlaylistPlay
import androidx.compose.material.icons.rounded.Speaker
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.Checkbox
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.FilterChip
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.PrimaryTabRow
import androidx.compose.material3.Slider
import androidx.compose.material3.Tab
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.toArgb
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import fyi.nnx.needle.NeedleApp
import fyi.nnx.needle.core.AlbumProblem
import fyi.nnx.needle.core.DuplicateSet
import fyi.nnx.needle.core.Output
import fyi.nnx.needle.core.ReleaseChoice
import fyi.nnx.needle.core.Song
import fyi.nnx.needle.core.SoundMatch
import fyi.nnx.needle.core.TagChange
import fyi.nnx.needle.core.TagReport
import fyi.nnx.needle.core.TidyPlan
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.io.File
import java.text.DateFormat
import java.util.Date

/*
 * The library tools, as on desktop: the tag editor, Fix my library, importing from other
 * players, the theme editor, and choosing several speakers.
 */

private fun refresh() {
    NeedleApp.instance.libraryVersion.value++
}

/** Runs `work` off the main thread, then `done` with its result (or says what went wrong). */
private fun <T> work(work: () -> T, done: (T) -> Unit) {
    NeedleApp.instance.scope.launch {
        val result = withContext(Dispatchers.IO) { runCatching(work) }
        withContext(Dispatchers.Main) {
            result.onSuccess(done).onFailure { showMessage(it.message ?: "It did not work") }
        }
    }
}

// ---------- Changing music files needs "All files access"

/** Whether Needle may change music files (tags, tidying), which Android asks about once. */
fun canChangeFiles(): Boolean = Build.VERSION.SDK_INT < 30 || Environment.isExternalStorageManager()

/**
 * Asks for "All files access" when it is missing, explaining why first; then runs `then`.
 * Returned as a function to call from a button.
 */
@Composable
fun rememberFileAccess(): (then: () -> Unit) -> Unit {
    val context = LocalContext.current
    var asking by remember { mutableStateOf<(() -> Unit)?>(null) }
    asking?.let { then ->
        AlertDialog(
            onDismissRequest = { asking = null },
            title = { Text("Let Needle change your music files?") },
            text = {
                Text(
                    "To write tags and tidy files, Android needs to let Needle change files in your music folders. " +
                        "Needle keeps a copy of each file before it changes it, and checks the sound is the same after.",
                )
            },
            confirmButton = {
                TextButton(onClick = {
                    asking = null
                    context.startActivity(
                        Intent(AndroidSettings.ACTION_MANAGE_APP_ALL_FILES_ACCESS_PERMISSION, Uri.parse("package:${context.packageName}"))
                            .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK),
                    )
                    // Android asks in its own settings; the person taps the button again after.
                    showMessage("Turn on Needle, then come back and try again")
                }) { Text("Open settings") }
            },
            dismissButton = { TextButton(onClick = { asking = null }) { Text("Not now") } },
        )
    }
    return { then -> if (canChangeFiles()) then() else asking = then }
}

// ---------- Tag editor

/**
 * Changes the tags of one song or many. With many, a field shows a value only when every
 * song has it; left as it is, each song keeps its own.
 */
@Composable
fun TagEditorDialog(ids: List<String>, onDone: () -> Unit) {
    var loaded by remember { mutableStateOf(false) }
    val start = remember { mutableStateMapOf<String, String>() }
    val now = remember { mutableStateMapOf<String, String>() }
    var backups by remember { mutableStateOf<List<fyi.nnx.needle.core.TagBackupInfo>>(emptyList()) }
    var identifying by remember { mutableStateOf(false) }
    var saving by remember { mutableStateOf(false) }
    val access = rememberFileAccess()
    LaunchedEffect(ids) {
        val tags = withContext(Dispatchers.IO) { ids.mapNotNull { runCatching { core.songTags(it) }.getOrNull() } }
        fun shared(f: (fyi.nnx.needle.core.SongTags) -> String) = tags.map(f).distinct().singleOrNull() ?: ""
        val fields = mapOf(
            "title" to shared { it.title },
            "artist" to shared { it.artist },
            "album" to shared { it.album },
            "albumArtist" to shared { it.albumArtist },
            "genre" to shared { it.genre },
            "year" to shared { if (it.year > 0u) it.year.toString() else "" },
            "track" to shared { if (it.trackNumber > 0u) it.trackNumber.toString() else "" },
        )
        start.putAll(fields)
        now.putAll(fields)
        if (ids.size == 1) backups = withContext(Dispatchers.IO) { runCatching { core.tagBackups(ids[0]) }.getOrDefault(emptyList()) }
        loaded = true
    }
    fun changed(key: String) = now[key] != start[key]
    fun text(key: String) = now[key]?.takeIf { changed(key) }
    fun number(key: String) = now[key]?.takeIf { changed(key) }?.let { it.trim().toUIntOrNull() ?: 0u }
    fun save() {
        val change = TagChange(
            title = text("title").takeIf { ids.size == 1 },
            artist = text("artist"),
            album = text("album"),
            albumArtist = text("albumArtist"),
            genre = text("genre"),
            year = number("year"),
            trackNumber = number("track").takeIf { ids.size == 1 },
        )
        saving = true
        work({ core.editTags(ids, change) }) { r: TagReport ->
            saving = false
            showMessage(if (r.failed.isEmpty()) "Saved the tags of ${count(r.saved.toInt(), "song")}" else "Saved ${r.saved}; ${r.failed.size} could not be changed")
            refresh()
            onDone()
        }
    }
    AlertDialog(
        onDismissRequest = onDone,
        title = { Text(if (ids.size == 1) "Edit tags" else "Edit ${ids.size} songs") },
        text = {
            if (!loaded) {
                CircularProgressIndicator()
                return@AlertDialog
            }
            Column(Modifier.verticalScroll(rememberScrollState()), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                val fields = buildList {
                    if (ids.size == 1) add("title" to "Title")
                    add("artist" to "Artist")
                    add("album" to "Album")
                    add("albumArtist" to "Album artist")
                    add("genre" to "Genre")
                    add("year" to "Year")
                    if (ids.size == 1) add("track" to "Track number")
                }
                fields.forEach { (key, label) ->
                    OutlinedTextField(
                        value = now[key] ?: "",
                        onValueChange = { now[key] = it },
                        label = { Text(label) },
                        placeholder = { if (ids.size > 1 && start[key].isNullOrEmpty()) Text("As each song has it") },
                        singleLine = true,
                        keyboardOptions = if (key == "year" || key == "track") KeyboardOptions(keyboardType = KeyboardType.Number) else KeyboardOptions.Default,
                        modifier = Modifier.fillMaxWidth(),
                    )
                }
                Text(
                    "Needle keeps a copy of each file before changing it.",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                if (ids.size == 1) {
                    TextButton(onClick = { identifying = true }) { Text("Find what it is by its sound") }
                    if (backups.isNotEmpty()) {
                        Text("Earlier versions", style = MaterialTheme.typography.titleSmall, modifier = Modifier.padding(top = 8.dp))
                        backups.take(5).forEach { b ->
                            Row(verticalAlignment = Alignment.CenterVertically) {
                                Icon(Icons.Rounded.History, contentDescription = null, tint = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.size(18.dp))
                                Text(
                                    DateFormat.getDateTimeInstance(DateFormat.MEDIUM, DateFormat.SHORT).format(Date(b.createdAt * 1000)),
                                    style = MaterialTheme.typography.bodyMedium,
                                    modifier = Modifier.weight(1f).padding(start = 8.dp),
                                )
                                TextButton(onClick = {
                                    access {
                                        work({ core.restoreTags(ids[0], b.createdAt) }) {
                                            showMessage("Put back the tags from then")
                                            refresh()
                                            onDone()
                                        }
                                    }
                                }) { Text("Put back") }
                            }
                        }
                    }
                }
            }
        },
        confirmButton = {
            TextButton(enabled = loaded && !saving && now.keys.any(::changed), onClick = { access { save() } }) {
                Text(if (saving) "Saving…" else "Save")
            }
        },
        dismissButton = { TextButton(onClick = onDone) { Text("Cancel") } },
    )
    if (identifying) IdentifyDialog(ids[0]) { identifying = false; if (it) onDone() }
}

/** What a song is, from its sound (AcoustID); a match's names can be written into it. */
@Composable
private fun IdentifyDialog(id: String, onDone: (changed: Boolean) -> Unit) {
    var key by remember { mutableStateOf("") }
    var hasKey by remember { mutableStateOf(core.hasAcoustidKey()) }
    var matches by remember { mutableStateOf<List<SoundMatch>?>(null) }
    var busy by remember { mutableStateOf(false) }
    val access = rememberFileAccess()
    val context = LocalContext.current
    LaunchedEffect(hasKey) {
        if (hasKey) {
            busy = true
            matches = withContext(Dispatchers.IO) { runCatching { core.identify(id) }.onFailure { showMessage(it.message ?: "Could not identify it") }.getOrDefault(emptyList()) }
            busy = false
        }
    }
    AlertDialog(
        onDismissRequest = { onDone(false) },
        title = { Text("Find it by its sound") },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
                when {
                    !hasKey -> {
                        Text("Needle asks AcoustID, which needs a free application key.")
                        TextButton(onClick = {
                            context.startActivity(Intent(Intent.ACTION_VIEW, Uri.parse("https://acoustid.org/new-application")).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
                        }) { Text("Get a key at acoustid.org") }
                        OutlinedTextField(key, { key = it }, label = { Text("Application key") }, singleLine = true, modifier = Modifier.fillMaxWidth())
                    }
                    busy -> Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                        CircularProgressIndicator(Modifier.size(20.dp), strokeWidth = 2.dp)
                        Text("Listening…")
                    }
                    matches.isNullOrEmpty() -> Text("No match found.")
                    else -> matches!!.take(6).forEach { m ->
                        Row(
                            Modifier.fillMaxWidth().clip(RoundedCornerShape(12.dp)).clickable {
                                access { work({ core.applyMatch(id, m) }) { showMessage("Saved"); refresh(); onDone(true) } }
                            }.padding(8.dp),
                            verticalAlignment = Alignment.CenterVertically,
                        ) {
                            Column(Modifier.weight(1f)) {
                                Text(m.title, style = MaterialTheme.typography.bodyLarge)
                                Text(listOf(m.artist, m.album).filter { it.isNotBlank() }.joinToString(" · "), style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                            }
                            Text("${m.score}%", style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.primary)
                        }
                    }
                }
            }
        },
        confirmButton = {
            if (!hasKey) TextButton(enabled = key.isNotBlank(), onClick = { runCatching { core.setAcoustidKey(key) }; hasKey = core.hasAcoustidKey() }) { Text("Save key") }
        },
        dismissButton = { TextButton(onClick = { onDone(false) }) { Text("Close") } },
    )
}

// ---------- Fix my library

/**
 * Moves files to the phone's trash, where they stay 30 days, after Android asks the person.
 * Needs the files to be in Android's media list (music in the phone's own folders is).
 */
private fun trashRequest(context: Context, paths: List<String>): android.app.PendingIntent? {
    if (Build.VERSION.SDK_INT < 30 || paths.isEmpty()) return null
    val uris = mutableListOf<Uri>()
    paths.chunked(50).forEach { chunk ->
        val where = "${MediaStore.MediaColumns.DATA} IN (${chunk.joinToString(",") { "?" }})"
        context.contentResolver.query(MediaStore.Audio.Media.EXTERNAL_CONTENT_URI, arrayOf(MediaStore.MediaColumns._ID), where, chunk.toTypedArray(), null)?.use { c ->
            while (c.moveToNext()) uris += android.content.ContentUris.withAppendedId(MediaStore.Audio.Media.EXTERNAL_CONTENT_URI, c.getLong(0))
        }
    }
    return if (uris.isEmpty()) null else MediaStore.createTrashRequest(context.contentResolver, uris, true)
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun FixLibraryScreen() {
    var tab by rememberSaveable { mutableIntStateOf(0) }
    LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(bottom = 32.dp + LocalBottomInset.current)) {
        item { LargeTitle("Fix my library") }
        item {
            PrimaryTabRow(selectedTabIndex = tab, containerColor = MaterialTheme.colorScheme.background) {
                listOf("Duplicates", "Covers", "Albums", "Tidy").forEachIndexed { i, label ->
                    Tab(selected = tab == i, onClick = { tab = i }, text = { Text(label) })
                }
            }
        }
        item {
            Box(Modifier.padding(top = 8.dp)) {
                when (tab) {
                    0 -> Duplicates()
                    1 -> MissingCovers()
                    2 -> AlbumProblems()
                    else -> Tidy()
                }
            }
        }
    }
}

@Composable
private fun Explain(text: String) {
    Text(text, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.padding(horizontal = Edge, vertical = 8.dp))
}

@Composable
private fun Loading(text: String) {
    Row(Modifier.padding(Edge), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
        CircularProgressIndicator(Modifier.size(20.dp), strokeWidth = 2.dp)
        Text(text, color = MaterialTheme.colorScheme.onSurfaceVariant)
    }
}

@Composable
private fun Card(content: @Composable () -> Unit) {
    Column(
        Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 6.dp).clip(RoundedCornerShape(24.dp)).background(MaterialTheme.colorScheme.surfaceContainer).padding(12.dp),
        verticalArrangement = Arrangement.spacedBy(6.dp),
    ) { content() }
}

@Composable
private fun Duplicates() {
    val context = LocalContext.current
    var sets by remember { mutableStateOf<List<DuplicateSet>?>(null) }
    var round by remember { mutableIntStateOf(0) }
    var trash by remember { mutableStateOf<List<String>>(emptyList()) }
    val trashLauncher = rememberLauncherForActivityResult(ActivityResultContracts.StartIntentSenderForResult()) {
        showMessage(if (it.resultCode == android.app.Activity.RESULT_OK) "Moved to the trash" else "The files stay")
        trash = emptyList()
    }
    LaunchedEffect(round) { sets = withContext(Dispatchers.IO) { core.duplicates() } }
    Column {
        Explain("The same song more than once. Needle keeps the best copy; the others' plays, stars, and playlist places move to it.")
        val list = sets
        when {
            list == null -> Loading("Looking for songs that are here twice…")
            list.isEmpty() -> Explain("No duplicates. Nice and tidy.")
        }
        list.orEmpty().forEach { set ->
            var keep by remember(set) { mutableIntStateOf(set.keep.toInt()) }
            Card {
                set.songs.forEachIndexed { i, song ->
                    Row(Modifier.fillMaxWidth().clip(RoundedCornerShape(12.dp)).clickable { keep = i }.padding(4.dp), verticalAlignment = Alignment.CenterVertically) {
                        Cover(song.artwork, Modifier.size(40.dp), SmallCoverShape, seed = song.album + song.artist)
                        Column(Modifier.weight(1f).padding(horizontal = 10.dp)) {
                            Text(song.title, style = MaterialTheme.typography.bodyLarge, maxLines = 1, overflow = TextOverflow.Ellipsis)
                            Text("${song.format} · ${set.paths[i].substringAfterLast('/')}", style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant, maxLines = 1, overflow = TextOverflow.Ellipsis)
                        }
                        if (i == keep) Text("Keep", style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.primary)
                    }
                }
                FilledTonalButton(onClick = {
                    val others = set.songs.filterIndexed { i, _ -> i != keep }
                    val paths = set.paths.filterIndexed { i, _ -> i != keep }
                    work({ core.mergeDuplicates(set.songs[keep].id, others.map { it.id }) }) {
                        showMessage("Kept one")
                        refresh()
                        trash = paths
                        round++
                    }
                }) { Text("Keep this one") }
            }
        }
    }
    if (trash.isNotEmpty()) {
        AlertDialog(
            onDismissRequest = { trash = emptyList() },
            title = { Text("Move the other files to the trash?") },
            text = { Text("They left your library. In the trash they stay 30 days, then go.") },
            confirmButton = {
                TextButton(onClick = {
                    val request = runCatching { trashRequest(context, trash) }.getOrNull()
                    if (request != null) trashLauncher.launch(IntentSenderRequest.Builder(request.intentSender).build())
                    else { showMessage("Android cannot move these; they stay"); trash = emptyList() }
                }) { Text("Move to trash") }
            },
            dismissButton = { TextButton(onClick = { trash = emptyList() }) { Text("Keep the files") } },
        )
    }
}

@Composable
private fun MissingCovers() {
    var albums by remember { mutableStateOf<List<Song>?>(null) }
    var busy by remember { mutableStateOf(false) }
    var round by remember { mutableIntStateOf(0) }
    LaunchedEffect(round) { albums = withContext(Dispatchers.IO) { core.albumsWithoutCovers() } }
    Column {
        Explain("Albums with no cover. Needle can look for them on the Cover Art Archive, with each album's name and artist.")
        val list = albums
        when {
            list == null -> Loading("Looking…")
            list.isEmpty() -> Explain("Every album has a cover.")
            else -> Button(
                enabled = !busy,
                onClick = {
                    busy = true
                    work({ core.findCovers(list.map { it.id }) }) { found ->
                        busy = false
                        showMessage("Found ${count(found.toInt(), "cover")}")
                        refresh()
                        round++
                    }
                },
                modifier = Modifier.padding(horizontal = Edge),
            ) { Text(if (busy) "Looking online…" else "Find ${count(list.size, "cover")} online") }
        }
        list.orEmpty().forEach { song ->
            Row(Modifier.fillMaxWidth().padding(horizontal = Edge, vertical = 6.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                Cover(null, Modifier.size(44.dp), SmallCoverShape, seed = song.album + song.artist)
                Column {
                    Text(song.album, style = MaterialTheme.typography.bodyLarge)
                    Text(song.artist, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                }
            }
        }
    }
}

@Composable
private fun AlbumProblems() {
    var problems by remember { mutableStateOf<List<AlbumProblem>?>(null) }
    var looking by remember { mutableStateOf<AlbumProblem?>(null) }
    var round by remember { mutableIntStateOf(0) }
    LaunchedEffect(round) { problems = withContext(Dispatchers.IO) { core.albumProblems() } }
    Column {
        Explain("Albums missing a year or track numbers, or with mixed artists. MusicBrainz can fill them in.")
        val list = problems
        when {
            list == null -> Loading("Checking your albums…")
            list.isEmpty() -> Explain("Every album looks right.")
        }
        list.orEmpty().forEach { p ->
            Card {
                Text(p.album.ifBlank { "Unknown album" }, style = MaterialTheme.typography.titleMedium)
                Text("${p.artist} · ${count(p.songIds.size, "song")}", style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                p.problems.forEach { Text("• $it", style = MaterialTheme.typography.bodyMedium) }
                FilledTonalButton(onClick = { looking = p }) { Text("Look it up on MusicBrainz") }
            }
        }
    }
    looking?.let { p -> ReleaseDialog(p) { changed -> looking = null; if (changed) round++ } }
}

@Composable
private fun ReleaseDialog(album: AlbumProblem, onDone: (Boolean) -> Unit) {
    var choices by remember { mutableStateOf<List<ReleaseChoice>?>(null) }
    val access = rememberFileAccess()
    LaunchedEffect(album) {
        choices = withContext(Dispatchers.IO) {
            runCatching { core.findReleases(album.songIds) }.onFailure { showMessage(it.message ?: "MusicBrainz did not answer") }.getOrDefault(emptyList())
        }
    }
    AlertDialog(
        onDismissRequest = { onDone(false) },
        title = { Text(album.album) },
        text = {
            Column(Modifier.verticalScroll(rememberScrollState()), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                val list = choices
                when {
                    list == null -> Loading("Asking MusicBrainz…")
                    list.isEmpty() -> Text("MusicBrainz knows no album by that name.")
                }
                list.orEmpty().forEach { r ->
                    Row(
                        Modifier.fillMaxWidth().clip(RoundedCornerShape(12.dp)).clickable {
                            access {
                                work({ core.applyRelease(r.id) }) { report ->
                                    showMessage(if (report.failed.isEmpty()) "Tagged ${count(report.saved.toInt(), "song")}" else "Tagged ${report.saved}; ${report.failed.size} could not be changed")
                                    refresh()
                                    onDone(true)
                                }
                            }
                        }.padding(8.dp),
                    ) {
                        Column(Modifier.weight(1f)) {
                            Text(r.title, style = MaterialTheme.typography.bodyLarge)
                            Text(
                                listOf(r.artist, r.year.takeIf { it > 0u }?.toString(), r.country, r.format).filter { !it.isNullOrBlank() }.joinToString(" · "),
                                style = MaterialTheme.typography.bodyMedium,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                        }
                        Text("${r.matched}/${album.songIds.size}", style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.primary)
                    }
                }
            }
        },
        confirmButton = {},
        dismissButton = { TextButton(onClick = { onDone(false) }) { Text("Close") } },
    )
}

@Composable
private fun Tidy() {
    val patterns = remember { core.tidyPatterns() }
    var pattern by rememberSaveable { mutableStateOf(patterns.first()) }
    var plan by remember { mutableStateOf<TidyPlan?>(null) }
    var busy by remember { mutableStateOf(false) }
    var canUndo by remember { mutableStateOf(core.canUndoTidy()) }
    val access = rememberFileAccess()
    LaunchedEffect(pattern) { plan = null; plan = withContext(Dispatchers.IO) { core.planTidy(pattern) } }
    Column {
        Explain("Puts files in folders by artist and album, inside your music folders. It can be undone.")
        LazyRow(contentPadding = PaddingValues(horizontal = 16.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            items(patterns) { p -> FilterChip(selected = p == pattern, onClick = { pattern = p }, label = { Text(p.replace("{", "").replace("}", "")) }) }
        }
        val p = plan
        if (p == null) Loading("Working it out…") else Card {
            Text(if (p.moves == 0u) "Everything is where it should be." else "${count(p.moves.toInt(), "file")} to move", style = MaterialTheme.typography.titleMedium)
            Text("${p.inPlace} already in place${if (p.skipped > 0u) ", ${p.skipped} left alone" else ""}", style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
            p.examples.forEach { Text(it, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant, maxLines = 2, overflow = TextOverflow.Ellipsis) }
            if (p.moves > 0u) Button(enabled = !busy, onClick = {
                access {
                    busy = true
                    work({ core.tidy() }) { moved ->
                        busy = false
                        showMessage("Moved ${count(moved.toInt(), "file")}")
                        canUndo = core.canUndoTidy()
                        refresh()
                        pattern = pattern
                    }
                }
            }) { Text(if (busy) "Moving…" else "Move the files") }
        }
        if (canUndo) TextButton(onClick = {
            access { work({ core.undoTidy() }) { back -> showMessage("Put back ${count(back.toInt(), "file")}"); canUndo = core.canUndoTidy(); refresh() } }
        }, modifier = Modifier.padding(horizontal = 8.dp)) { Text("Undo the last tidy") }
    }
}

// ---------- Import

/** Brings in plays, stars, and playlists from other players and services. */
@Composable
fun ImportScreen() {
    val context = LocalContext.current
    var busy by remember { mutableStateOf(false) }
    var result by remember { mutableStateOf<String?>(null) }
    var history by remember { mutableStateOf<String?>(null) }
    fun run(kind: String, uri: Uri?) {
        val path = uri?.let { copyToCache(context, it) } ?: return
        busy = true
        work({ core.importFile(kind, path).also { File(path).delete() } }) { busy = false; result = it; refresh() }
    }
    val itunes = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument()) { run("itunes", it) }
    val spotify = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument()) { run("spotify", it) }
    val playlist = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument()) { run("playlist", it) }
    LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(bottom = 32.dp + LocalBottomInset.current)) {
        item { LargeTitle("Import") }
        item { Explain("Bring in your plays, stars, and playlists from another player. Songs are matched to the music you already have.") }
        item {
            ImportCard(Icons.Rounded.LibraryMusic, "iTunes, Apple Music, or MusicBee", "Their library XML file: plays, stars, and playlists") { itunes.launch(arrayOf("*/*")) }
            ImportCard(Icons.Rounded.Album, "Spotify", "Your data download from Spotify (the zip): plays and playlists") { spotify.launch(arrayOf("*/*")) }
            ImportCard(Icons.Rounded.PlaylistPlay, "A playlist file", "An .m3u or .m3u8 playlist") { playlist.launch(arrayOf("*/*")) }
            ImportCard(Icons.Rounded.History, "Last.fm or ListenBrainz", "Your listening history, by your user name") { history = "listenbrainz" }
        }
        if (busy) item { Loading("Bringing it in…") }
        result?.let { item { Card { Text(it, style = MaterialTheme.typography.bodyLarge) } } }
    }
    history?.let { first ->
        var service by remember { mutableStateOf(first) }
        var user by remember { mutableStateOf("") }
        var key by remember { mutableStateOf("") }
        AlertDialog(
            onDismissRequest = { history = null },
            title = { Text("Listening history") },
            text = {
                Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
                    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        FilterChip(service == "listenbrainz", { service = "listenbrainz" }, label = { Text("ListenBrainz") })
                        FilterChip(service == "lastfm", { service = "lastfm" }, label = { Text("Last.fm") })
                    }
                    OutlinedTextField(user, { user = it }, label = { Text("User name") }, singleLine = true, modifier = Modifier.fillMaxWidth())
                    if (service == "lastfm") OutlinedTextField(key, { key = it }, label = { Text("Last.fm API key") }, supportingText = { Text("From last.fm/api") }, singleLine = true, modifier = Modifier.fillMaxWidth())
                }
            },
            confirmButton = {
                TextButton(enabled = user.isNotBlank() && (service != "lastfm" || key.isNotBlank()), onClick = {
                    history = null
                    busy = true
                    work({ core.importHistory(service, user.trim(), key.trim()) }) { busy = false; result = it; refresh() }
                }) { Text("Bring it in") }
            },
            dismissButton = { TextButton(onClick = { history = null }) { Text("Cancel") } },
        )
    }
}

@Composable
private fun ImportCard(icon: androidx.compose.ui.graphics.vector.ImageVector, title: String, detail: String, onClick: () -> Unit) {
    Row(
        Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 6.dp).clip(RoundedCornerShape(24.dp)).background(MaterialTheme.colorScheme.surfaceContainer).clickable(onClick = onClick).padding(16.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        ShapedIcon(icon, 44)
        Column(Modifier.weight(1f)) {
            Text(title, style = MaterialTheme.typography.titleMedium)
            Text(detail, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
    }
}

// ---------- Theme editor

/** The colours a theme can change, as on desktop, with names people know. */
val ThemeSlots = listOf(
    "page" to "Page",
    "sidebar" to "Bars",
    "card" to "Cards",
    "card_hover" to "Raised cards",
    "text" to "Text",
    "text_muted" to "Quiet text",
    "accent" to "Accent",
    "accent_text" to "Text on the accent",
    "border" to "Lines",
    "border_soft" to "Soft lines",
    "danger" to "Warnings",
)

private fun hexOf(color: Color) = "#%06X".format(color.toArgb() and 0xFFFFFF)
private fun colorOf(hex: String?): Color? = hex?.removePrefix("#")?.takeIf { it.length == 6 }?.toLongOrNull(16)?.let { Color(0xFF000000 or it) }

/** Makes or changes a theme of your own; it is saved as a theme file, as on desktop. */
@Composable
fun ThemeEditorScreen(id: String?, back: () -> Unit) {
    val existing = remember(id) { id?.let { i -> core.themes().firstOrNull { it.id == i } } }
    var name by remember { mutableStateOf(existing?.name ?: "My theme") }
    var base by remember { mutableStateOf(existing?.base ?: "dark") }
    val colors = remember { mutableStateMapOf<String, String>().apply { existing?.colors?.let { putAll(it) } } }
    var picking by remember { mutableStateOf<String?>(null) }
    val scheme = MaterialTheme.colorScheme
    val defaults = mapOf(
        "page" to scheme.background, "sidebar" to scheme.surfaceContainerLow, "card" to scheme.surfaceContainer,
        "card_hover" to scheme.surfaceContainerHigh, "text" to scheme.onSurface, "text_muted" to scheme.onSurfaceVariant,
        "accent" to scheme.primary, "accent_text" to scheme.onPrimary, "border" to scheme.outline,
        "border_soft" to scheme.outlineVariant, "danger" to scheme.error,
    )
    LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(bottom = 32.dp + LocalBottomInset.current)) {
        item { LargeTitle(if (existing == null) "New theme" else "Edit theme") }
        item {
            Column(Modifier.padding(horizontal = Edge), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                OutlinedTextField(name, { name = it }, label = { Text("Name") }, singleLine = true, modifier = Modifier.fillMaxWidth())
                Text("Starts from", style = MaterialTheme.typography.titleSmall)
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    listOf("dark" to "Night", "midnight" to "Midnight", "light" to "Day").forEach { (b, label) ->
                        FilterChip(base == b, { base = b }, label = { Text(label) })
                    }
                }
                Text("Colours", style = MaterialTheme.typography.titleSmall)
            }
        }
        items(ThemeSlots) { (slot, label) ->
            val color = colorOf(colors[slot])
            Row(
                Modifier.fillMaxWidth().clickable { picking = slot }.padding(horizontal = Edge, vertical = 10.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(16.dp),
            ) {
                Box(
                    Modifier.size(36.dp).clip(CircleShape).background(color ?: defaults[slot] ?: Color.Gray)
                        .border(1.dp, MaterialTheme.colorScheme.outlineVariant, CircleShape),
                )
                Column(Modifier.weight(1f)) {
                    Text(label, style = MaterialTheme.typography.bodyLarge)
                    Text(colors[slot] ?: "As the look it starts from", style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                }
                if (color != null) TextButton(onClick = { colors.remove(slot) }) { Text("Reset") }
            }
        }
        item {
            Row(Modifier.padding(Edge), horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                Button(onClick = {
                    work({ core.saveTheme(id, name, base, colors.toMap()) }) { saved ->
                        NeedleApp.instance.updateApp { it.copy(theme = saved) }
                        showMessage("Saved $name")
                        back()
                    }
                }) { Text("Save and use it") }
                if (existing != null) TextButton(onClick = {
                    work({ core.deleteTheme(existing.id) }) {
                        NeedleApp.instance.updateApp { it.copy(theme = "night") }
                        showMessage("Deleted ${existing.name}")
                        back()
                    }
                }) { Text("Delete", color = MaterialTheme.colorScheme.error) }
            }
        }
    }
    picking?.let { slot ->
        ColorDialog(colorOf(colors[slot]) ?: defaults[slot] ?: Color.Gray, { picking = null }) { colors[slot] = hexOf(it); picking = null }
    }
}

/** A colour, chosen by hue, colourfulness, and lightness, or typed as #rrggbb. */
@Composable
private fun ColorDialog(start: Color, onDismiss: () -> Unit, onPick: (Color) -> Unit) {
    val hsl = remember {
        FloatArray(3).also { androidx.core.graphics.ColorUtils.colorToHSL(start.toArgb(), it) }
    }
    var h by remember { mutableStateOf(hsl[0]) }
    var s by remember { mutableStateOf(hsl[1]) }
    var l by remember { mutableStateOf(hsl[2]) }
    val color = Color.hsl(h, s, l)
    var typed by remember(color) { mutableStateOf(hexOf(color)) }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("Colour") },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
                Box(Modifier.fillMaxWidth().heightIn(min = 56.dp).clip(RoundedCornerShape(16.dp)).background(color))
                Text("Hue"); Slider(h, { h = it }, valueRange = 0f..359f)
                Text("Colourfulness"); Slider(s, { s = it })
                Text("Lightness"); Slider(l, { l = it })
                OutlinedTextField(typed, { t ->
                    typed = t
                    colorOf(t.trim())?.let { c ->
                        val out = FloatArray(3)
                        androidx.core.graphics.ColorUtils.colorToHSL(c.toArgb(), out)
                        h = out[0]; s = out[1]; l = out[2]
                    }
                }, singleLine = true, label = { Text("Hex") }, modifier = Modifier.fillMaxWidth())
            }
        },
        confirmButton = { TextButton(onClick = { onPick(color) }) { Text("Use it") } },
        dismissButton = { TextButton(onClick = onDismiss) { Text("Cancel") } },
    )
}

// ---------- Speakers

/** Where the music plays: this phone, one speaker, or several at once, each lined up in time. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SpeakersSheet(context: Context) {
    var all by remember { mutableStateOf<List<Output>?>(null) }
    val chosen = remember { mutableStateMapOf<String, Boolean>() }
    val delays = remember { mutableStateMapOf<String, Int>() }
    var phone by remember { mutableStateOf(true) }
    val scope = rememberCoroutineScope()
    LaunchedEffect(Unit) {
        // Finding speakers needs the phone to listen to the network's announcements.
        val wifi = context.applicationContext.getSystemService(Context.WIFI_SERVICE) as WifiManager
        val lock = wifi.createMulticastLock("needle-speakers").apply { setReferenceCounted(false); acquire() }
        try {
            val found = withContext(Dispatchers.IO) { runCatching { core.speakers() }.getOrNull() }
            if (found != null) {
                found.chosen.forEach { chosen[it] = true }
                delays.putAll(found.delays)
                phone = found.thisPhone
                all = found.all
            } else all = emptyList()
        } finally {
            lock.release()
        }
    }
    fun apply() {
        val ids = chosen.filterValues { it }.keys.toList()
        val group = ids.size + (if (phone) 1 else 0) > 1
        scope.launch(Dispatchers.IO) {
            runCatching { core.setSpeakers(ids, phone || ids.isEmpty(), if (group) delays.toMap() else emptyMap()) }
                .onFailure { showMessage(it.message ?: "Could not play there") }
        }
        showMessage(
            when {
                ids.isEmpty() -> "Playing on this phone"
                group -> "Playing on ${ids.size + (if (phone) 1 else 0)} at once"
                else -> "Playing on ${all?.firstOrNull { it.id == ids[0] }?.name ?: "the speaker"}"
            },
        )
        Ui.sheet.value = null
    }
    ModalBottomSheet(onDismissRequest = { Ui.sheet.value = null }) {
        Column(Modifier.navigationBarsPadding().padding(bottom = 16.dp)) {
            Text("Play on", style = MaterialTheme.typography.titleLarge, modifier = Modifier.padding(horizontal = Edge, vertical = 8.dp))
            Text("Choose one, or several to play on all at once.", style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.padding(horizontal = Edge))
            SpeakerRow(Icons.Rounded.PhoneAndroid, "This phone", null, phone) { phone = it }
            val list = all
            if (list == null) Loading("Looking for speakers on your network…")
            list.orEmpty().forEach { o ->
                SpeakerRow(if (o.kind == "Chromecast") Icons.Rounded.Cast else Icons.Rounded.Speaker, o.name, o.kind, chosen[o.id] == true) { chosen[o.id] = it }
            }
            if (list != null && list.isEmpty()) Explain("No speakers found. Chromecast, DLNA, and AirPlay speakers on the same Wi-Fi show here.")
            val playing = chosen.filterValues { it }.keys.toList() + (if (phone) listOf("") else emptyList())
            if (playing.size > 1) {
                HorizontalDivider(Modifier.padding(vertical = 8.dp))
                Text("Line them up", style = MaterialTheme.typography.titleSmall, modifier = Modifier.padding(horizontal = Edge))
                Explain("Hold back any that plays early, until they sound as one.")
                playing.forEach { id ->
                    val label = if (id.isEmpty()) "This phone" else list?.firstOrNull { it.id == id }?.name ?: "Speaker"
                    val ms = delays[id] ?: 0
                    Column(Modifier.padding(horizontal = Edge)) {
                        Row { Text(label, Modifier.weight(1f)); Text("$ms ms", color = MaterialTheme.colorScheme.primary) }
                        Slider(ms.toFloat(), { delays[id] = (it / 10).toInt() * 10 }, valueRange = 0f..1000f)
                    }
                }
            }
            Button(onClick = ::apply, modifier = Modifier.padding(horizontal = Edge, vertical = 8.dp).fillMaxWidth()) { Text("Done") }
        }
    }
}

@Composable
private fun SpeakerRow(icon: androidx.compose.ui.graphics.vector.ImageVector, name: String, kind: String?, on: Boolean, onChange: (Boolean) -> Unit) {
    Row(
        Modifier.fillMaxWidth().clickable { onChange(!on) }.padding(horizontal = Edge, vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        Icon(icon, contentDescription = null, tint = if (on) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurfaceVariant)
        Column(Modifier.weight(1f)) {
            Text(name, style = MaterialTheme.typography.bodyLarge)
            if (kind != null) Text(kind, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
        Checkbox(on, onChange)
    }
}

