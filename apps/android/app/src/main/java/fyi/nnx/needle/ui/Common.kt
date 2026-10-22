package fyi.nnx.needle.ui

import androidx.compose.material.icons.rounded.Dns
import android.net.Uri
import androidx.compose.foundation.background
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.KeyboardArrowRight
import androidx.compose.material.icons.automirrored.rounded.PlaylistAdd
import androidx.compose.material.icons.rounded.Album
import androidx.compose.material.icons.automirrored.rounded.QueueMusic
import androidx.compose.material.icons.rounded.Extension
import androidx.compose.material.icons.rounded.Lyrics
import androidx.compose.material.icons.rounded.Radio
import androidx.compose.material.icons.rounded.RemoveCircleOutline
import androidx.compose.material.icons.rounded.Star
import androidx.compose.material.icons.rounded.StarBorder
import kotlinx.coroutines.launch
import androidx.compose.material.icons.rounded.MoreVert
import androidx.compose.material.icons.rounded.MusicNote
import androidx.compose.material.icons.rounded.Person
import androidx.compose.material.icons.rounded.QueuePlayNext
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.State
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.lifecycle.repeatOnLifecycle
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.Modifier
import androidx.compose.foundation.layout.offset
import androidx.compose.ui.unit.sp
import androidx.compose.ui.graphics.luminance
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import coil3.compose.AsyncImage
import fyi.nnx.needle.NeedleApp
import fyi.nnx.needle.core.Album
import fyi.nnx.needle.core.Needle
import fyi.nnx.needle.core.Song
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.io.File

val core: Needle get() = NeedleApp.instance.core

/** Covers, as in Apple Music: small corners, so the picture stays the picture. */
val CoverShape = RoundedCornerShape(8.dp)
val SmallCoverShape = RoundedCornerShape(6.dp)

/** Space from the screen edge to content. */
val Edge = 20.dp

/** The last answers pages loaded, a few of them, for a page that comes back (see [rememberLoaded]). */
private object Loads {
    /** Each answer with the library's version it was read at. */
    private val kept = object : LinkedHashMap<List<Any?>, Pair<Int, fyi.nnx.needle.LoadResult<*>>>(32, 0.75f, true) {
        override fun removeEldestEntry(eldest: MutableMap.MutableEntry<List<Any?>, Pair<Int, fyi.nnx.needle.LoadResult<*>>>?) = size > 32
    }
    @Synchronized fun get(key: List<Any?>, version: Int) = kept[key]?.takeIf { it.first == version }?.second
    @Synchronized fun put(key: List<Any?>, version: Int, value: fyi.nnx.needle.LoadResult<*>) {
        // Answers from an older library are never used again: they go, and their memory with them.
        kept.values.removeAll { it.first < version }
        kept[key] = version to value
    }
}

class LoadedState<T> internal constructor(private val result: fyi.nnx.needle.LoadResult<T>) : State<T?> {
    override val value: T? get() = (result as? fyi.nnx.needle.LoadResult.Ready)?.data
    val loading: Boolean get() = result is fyi.nnx.needle.LoadResult.Loading
    val failed: Boolean get() = result is fyi.nnx.needle.LoadResult.Failed
}

/** Loading, a completed value (including empty/null), and failure remain distinct. */
@Composable
fun <T> rememberLoaded(vararg keys: Any?, cache: Boolean = true, load: Needle.() -> T): LoadedState<T> {
    val version by NeedleApp.instance.libraryVersion.collectAsState()
    var attempt by remember(*keys) { mutableStateOf(0) }
    // Back on the failure card: first it tries again, then (still failing) it leaves the page.
    var retried by remember(*keys) { mutableStateOf(false) }
    val back = androidx.activity.compose.LocalOnBackPressedDispatcherOwner.current
    // A change in the library (a heart, a scan) reloads quietly: what shows stays until the new
    // answer comes. New keys (another album, another song) are other data, so they show loading.
    val wanted = keys.toList()
    // What this load gave last time (the same call in the code, the same keys): a page that
    // comes back (Back) shows it at once and reads it again quietly, so its rows are there
    // to scroll back to where they were, rather than placeholders.
    val cacheKey = listOf<Any?>(load::class.java.name) + wanted
    // Only from the library as it is now: after any change (a rename, a delete) it is read anew.
    // Not for a tile's small answer (an artist's photo), of which a grid has dozens: they
    // would push out the page's own list, which is what Back needs.
    val seed = remember(cacheKey) { if (cache) Loads.get(cacheKey, version) else null }
    val shown = remember { arrayOfNulls<List<Any?>>(1).also { if (seed != null) it[0] = wanted } }
    @Suppress("UNCHECKED_CAST")
    val result by produceState<fyi.nnx.needle.LoadResult<T>>(
        (seed as fyi.nnx.needle.LoadResult<T>?) ?: fyi.nnx.needle.LoadResult.Loading, version, attempt, *keys,
    ) {
        if (shown[0] != wanted || value !is fyi.nnx.needle.LoadResult.Ready) {
            value = (if (cache) Loads.get(cacheKey, version) as fyi.nnx.needle.LoadResult<T>? else null) ?: fyi.nnx.needle.LoadResult.Loading
        }
        shown[0] = wanted
        val asked = version
        value = withContext(Dispatchers.IO) { fyi.nnx.needle.loadAttempt { core.load() } }
        if (cache && value is fyi.nnx.needle.LoadResult.Ready) Loads.put(cacheKey, asked, value)
    }
    val failure = result as? fyi.nnx.needle.LoadResult.Failed
    if (result is fyi.nnx.needle.LoadResult.Ready) retried = false
    if (failure != null) {
        // With a remote the card takes the focus, so Retry is in reach.
        val keys = usingKeys()
        androidx.compose.ui.window.Popup(
            alignment = Alignment.BottomCenter,
            onDismissRequest = if (keys) ({
                if (retried) back?.onBackPressedDispatcher?.onBackPressed() else { retried = true; attempt++ }
            }) else null,
            properties = androidx.compose.ui.window.PopupProperties(focusable = keys),
        ) {
            androidx.compose.material3.Surface(shape = CoverShape, tonalElevation = 6.dp, modifier = Modifier.padding(Edge)) {
                Column(Modifier.padding(16.dp)) {
                    Text(failure.error.message ?: "Could not load this page", color = MaterialTheme.colorScheme.error)
                    androidx.compose.material3.TextButton(onClick = { attempt++ }) { Text("Retry") }
                }
            }
        }
    }
    return remember(result) { LoadedState(result) }
}

/** A cover's place: a file on the phone, or a link (covers from Needle on a computer). */
fun fileUri(path: String?): Uri? = path?.let {
    if (it.startsWith("http://") || it.startsWith("https://")) Uri.parse(it) else Uri.fromFile(File(it))
}

/**
 * Puts songs next, or at the end of what is to play, and says so. Off the main thread: the core
 * looks each song up, which for a long playlist takes a moment. `ids` is read there too.
 */
fun addToQueue(next: Boolean, ids: () -> List<String>) {
    NeedleApp.instance.scope.launch(Dispatchers.IO) {
        runCatching {
            val list = ids()
            if (next) core.playNext(list) else core.enqueue(list)
        }
            .onSuccess { showMessage(if (next) "Plays next" else "Added to the queue") }
            .onFailure { showMessage(it.message ?: "Could not add them") }
    }
}

/** Needle may read the music files (Android 13 and newer ask for music alone; older, for files). */
fun hasMusicAccess(context: android.content.Context): Boolean {
    if (android.os.Build.VERSION.SDK_INT >= 30 && android.os.Environment.isExternalStorageManager()) return true
    val permission = if (android.os.Build.VERSION.SDK_INT >= 33) android.Manifest.permission.READ_MEDIA_AUDIO else android.Manifest.permission.READ_EXTERNAL_STORAGE
    return androidx.core.content.ContextCompat.checkSelfPermission(context, permission) == android.content.pm.PackageManager.PERMISSION_GRANTED
}

/**
 * Whether Needle may see the pictures beside the songs (an album folder's cover.jpg). Android 12
 * and older give that with music; Android 13 and newer ask about pictures on their own.
 */
fun hasCoverAccess(context: android.content.Context): Boolean {
    if (android.os.Build.VERSION.SDK_INT < 33) return true
    if (android.os.Environment.isExternalStorageManager()) return true
    // Pictures, and videos for moving covers (cover.mp4): Android asks about both at once.
    return coverPermissions.all {
        androidx.core.content.ContextCompat.checkSelfPermission(context, it) == android.content.pm.PackageManager.PERMISSION_GRANTED
    }
}

/** What Android 13 and newer ask about for covers beside the songs: pictures and videos. */
private val coverPermissions = arrayOf(android.Manifest.permission.READ_MEDIA_IMAGES, android.Manifest.permission.READ_MEDIA_VIDEO)

/**
 * Settings › Library, on Android 13 and newer until it is allowed: a way to let Needle see the
 * covers kept beside the songs. Albums whose cover is only such a picture show none without it.
 */
@Composable
fun FolderCoversAccess(modifier: Modifier = Modifier) {
    if (android.os.Build.VERSION.SDK_INT < 33) return
    val context = androidx.compose.ui.platform.LocalContext.current
    var allowed by remember { mutableStateOf(hasCoverAccess(context)) }
    // Checked again when Needle comes back (from Android's Settings): the folders are read
    // again then, so the covers show without a restart.
    val lifecycle = androidx.lifecycle.compose.LocalLifecycleOwner.current.lifecycle
    LaunchedEffect(lifecycle) {
        lifecycle.repeatOnLifecycle(androidx.lifecycle.Lifecycle.State.RESUMED) {
            val now = hasCoverAccess(context)
            if (now && !allowed) runCatching { core.rescan() }
            allowed = now
        }
    }
    var askedAt by remember { androidx.compose.runtime.mutableLongStateOf(0L) }
    val ask = androidx.activity.compose.rememberLauncherForActivityResult(androidx.activity.result.contract.ActivityResultContracts.RequestMultiplePermissions()) { _ ->
        allowed = hasCoverAccess(context)
        if (allowed) runCatching { core.rescan() }
        // An answer at once: Android no longer shows its question, so its Settings page opens.
        else if (android.os.SystemClock.uptimeMillis() - askedAt < 500) tryOpen("page in Settings for Needle") {
            context.startActivity(
                android.content.Intent(android.provider.Settings.ACTION_APPLICATION_DETAILS_SETTINGS, android.net.Uri.parse("package:${context.packageName}"))
                    .addFlags(android.content.Intent.FLAG_ACTIVITY_NEW_TASK),
            )
        }
    }
    if (allowed) return
    Column(
        modifier.fillMaxWidth().clip(CoverShape).background(MaterialTheme.colorScheme.secondaryContainer).padding(16.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Text("Covers beside your songs", style = MaterialTheme.typography.titleMedium, color = MaterialTheme.colorScheme.onSecondaryContainer)
        Text(
            "Some albums keep their cover in their folder: a picture (cover.jpg) or a short video that moves (cover.mp4). Android asks about pictures and videos apart from music, so Needle needs your yes to show them. It reads only the covers in your music folders.",
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSecondaryContainer,
        )
        androidx.compose.material3.Button(onClick = {
            askedAt = android.os.SystemClock.uptimeMillis()
            ask.launch(coverPermissions)
        }) { Text("Allow") }
    }
}

/**
 * Said where the library is empty because Needle may not read music: Android's question again,
 * or (asked too often already, when Android no longer asks) its page for Needle in Settings.
 */
@Composable
fun MusicAccessNotice(modifier: Modifier = Modifier) {
    val context = androidx.compose.ui.platform.LocalContext.current
    // Checked again each time Needle comes back to the screen (from Settings, say).
    val lifecycle = androidx.lifecycle.compose.LocalLifecycleOwner.current.lifecycle
    var allowed by remember { mutableStateOf(hasMusicAccess(context)) }
    LaunchedEffect(lifecycle) {
        lifecycle.repeatOnLifecycle(androidx.lifecycle.Lifecycle.State.RESUMED) {
            val now = hasMusicAccess(context)
            // Allowed in Android's own Settings: the folders are read now, not at the next start.
            if (now && !allowed) runCatching { core.rescan() }
            allowed = now
        }
    }
    // When Android was asked: an answer at once means it no longer shows its question (asked
    // too often), and only then does its Settings page open. A plain "Don't allow" is left be.
    var askedAt by remember { androidx.compose.runtime.mutableLongStateOf(0L) }
    val ask = androidx.activity.compose.rememberLauncherForActivityResult(androidx.activity.result.contract.ActivityResultContracts.RequestPermission()) { granted ->
        allowed = granted
        if (granted) runCatching { core.rescan() }
        else if (android.os.SystemClock.uptimeMillis() - askedAt < 500) tryOpen("page in Settings for Needle") {
            context.startActivity(
                android.content.Intent(android.provider.Settings.ACTION_APPLICATION_DETAILS_SETTINGS, android.net.Uri.parse("package:${context.packageName}"))
                    .addFlags(android.content.Intent.FLAG_ACTIVITY_NEW_TASK),
            )
        }
    }
    if (allowed) return
    Column(
        modifier.fillMaxWidth().clip(CoverShape).background(MaterialTheme.colorScheme.errorContainer).padding(16.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Text("Needle may not read your music", style = MaterialTheme.typography.titleMedium, color = MaterialTheme.colorScheme.onErrorContainer)
        Text(
            "Android asks first. Without it, the music folders look empty.",
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onErrorContainer,
        )
        androidx.compose.material3.Button(onClick = {
            askedAt = android.os.SystemClock.uptimeMillis()
            ask.launch(if (android.os.Build.VERSION.SDK_INT >= 33) android.Manifest.permission.READ_MEDIA_AUDIO else android.Manifest.permission.READ_EXTERNAL_STORAGE)
        }) { Text("Allow") }
    }
}

/**
 * Writes to the library off the main thread (a scan writing at the same time holds it a
 * moment, and the page would freeze), then reports back on the main thread: `done` with the
 * answer, or `failed`; the lists are read again either way.
 */
fun <T> libraryWrite(
    work: () -> T,
    failed: (Throwable) -> Unit = { showMessage(it.message ?: "That did not work") },
    done: (T) -> Unit = {},
) {
    NeedleApp.instance.scope.launch(Dispatchers.Main) {
        withContext(Dispatchers.IO) { runCatching(work) }.onSuccess(done).onFailure(failed)
        NeedleApp.instance.libraryChanged()
    }
}

/**
 * Keys for a list's rows that stay with each song when the list is put in another order: the
 * song's id, and how many times it came before (a playlist may hold a song twice).
 */
fun songKeys(list: List<fyi.nnx.needle.core.Song>): List<String> {
    val seen = HashMap<String, Int>()
    return list.map { s -> "${s.id}#${seen.merge(s.id, 1, Int::plus)}" }
}

/** The id of the song playing, for marking it in a list (see [NeedleApp.playingId]). */
@Composable
fun playingSongId(): String? = NeedleApp.instance.playingId.collectAsState().value

/**
 * Plays songs from `start`, then opens the player (when that is on). Off the main thread: the
 * core looks each song up, which for a whole library or a long playlist takes a moment.
 */
fun playSongs(ids: List<String>, start: Int = 0) {
    // Nothing to play (an empty playlist): the player is not opened on nothing.
    if (ids.isEmpty()) return
    NeedleApp.instance.scope.launch(Dispatchers.IO) {
        runCatching { core.play(ids, start.toUInt()) }
            .onSuccess { NeedleApp.instance.openPlayer.tryEmit(Unit) }
            .onFailure { showMessage(it.message ?: "Could not play these songs") }
    }
}

fun time(seconds: Double): String {
    val s = seconds.coerceAtLeast(0.0).toLong()
    return if (s >= 3600) "%d:%02d:%02d".format(s / 3600, s / 60 % 60, s % 60)
    else "%d:%02d".format(s / 60, s % 60)
}

fun count(n: Int, word: String) = if (n == 1) "1 $word" else "$n ${word}s"

/** "12 songs, 48 minutes", as at the foot of an Apple Music album. */
fun songsAndLength(songs: List<Song>): String {
    val minutes = Math.round(songs.sumOf { it.duration } / 60).toInt()
    val length = if (minutes >= 60) "${minutes / 60} hr ${minutes % 60} min" else count(minutes, "minute")
    return "${count(songs.size, "song")}, $length"
}

/**
 * A cover picture. Without one, a made-up cover as on desktop: a two-colour gradient and a
 * large, cropped first letter, different for every album (`seed`) and the same every time.
 */
@Composable
fun Cover(path: String?, modifier: Modifier = Modifier, shape: Shape = CoverShape, seed: String? = null) {
    Box(
        modifier
            .clip(shape)
            .background(MaterialTheme.colorScheme.surfaceContainerHigh),
        contentAlignment = Alignment.Center,
    ) {
        when {
            path != null -> AsyncImage(
                model = coverRequest(fileUri(path)),
                contentDescription = null,
                contentScale = ContentScale.Crop,
                modifier = Modifier.fillMaxSize(),
            )
            !seed.isNullOrBlank() -> MadeUpCover(seed)
            else -> Icon(
                Icons.Rounded.MusicNote,
                contentDescription = null,
                tint = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.6f),
                modifier = Modifier.fillMaxSize(0.36f),
            )
        }
    }
}

/** A playlist's picture: its own, a mosaic of four of its songs' covers, or one cover. */
@Composable
fun PlaylistCover(playlist: fyi.nnx.needle.core.Playlist, modifier: Modifier = Modifier, shape: Shape = SmallCoverShape) {
    Mosaic(playlist.mosaic, playlist.artwork, modifier, shape, seed = playlist.name)
}

/** Four covers in a square, two by two; fewer than four shows the one cover instead. */
@Composable
fun Mosaic(covers: List<String>, artwork: String?, modifier: Modifier = Modifier, shape: Shape = CoverShape, seed: String? = null) {
    if (covers.size < 4) {
        Cover(artwork ?: covers.firstOrNull(), modifier, shape, seed = seed)
        return
    }
    Column(modifier.clip(shape)) {
        covers.take(4).chunked(2).forEach { pair ->
            Row(Modifier.weight(1f)) {
                pair.forEach { c -> Cover(c, Modifier.weight(1f).fillMaxSize(), androidx.compose.ui.graphics.RectangleShape) }
            }
        }
    }
}

/** The same hash as desktop's, so an album's made-up cover matches on both. */
private fun seedHash(seed: String): Long =
    seed.lowercase().toByteArray().fold(2166136261L) { h, b -> ((h xor (b.toLong() and 0xff)) * 16777619L) and 0xffffffffL }

/** The main colour of an album's made-up cover, for tinting like a real one. */
fun seedColor(seed: String): Color = Color.hsl((seedHash(seed) % 360).toFloat(), 0.5f, 0.4f)

@Composable
internal fun MadeUpCover(seed: String) {
    val hash = seedHash(seed)
    val h1 = (hash % 360).toFloat()
    val h2 = (h1 + 21.6f + ((hash shr 9) % 12) * 3.6f) % 360f
    val dark = MaterialTheme.colorScheme.background.luminance() < 0.5f
    val (a, b) = if (dark) Color.hsl(h1, 0.42f, 0.34f) to Color.hsl(h2, 0.5f, 0.17f) else Color.hsl(h1, 0.5f, 0.8f) to Color.hsl(h2, 0.42f, 0.62f)
    val letter = seed.firstOrNull { it.isLetterOrDigit() }?.uppercase() ?: "♪"
    androidx.compose.foundation.layout.BoxWithConstraints(
        Modifier.fillMaxSize().background(Brush.linearGradient(listOf(a, b))),
    ) {
        val size = maxWidth
        if (size >= 28.dp) {
            Text(
                letter,
                color = (if (dark) Color.White else Color.Black).copy(alpha = if (dark) 0.2f else 0.16f),
                fontSize = (size.value * 0.78f).sp,
                lineHeight = (size.value * 0.9f).sp,
                fontFamily = titleFamily(0.2f),
                fontWeight = FontWeight.SemiBold,
                maxLines = 1,
                softWrap = false,
                modifier = Modifier.align(Alignment.BottomStart).offset(x = size * 0.08f, y = size * 0.2f),
            )
        }
    }
}

/** A page's large title, left-aligned under the status bar. */
@Composable
fun LargeTitle(text: String, modifier: Modifier = Modifier, action: @Composable () -> Unit = {}) {
    val back = LocalCanGoBack.current
    Column(modifier.fillMaxWidth()) {
        // Pages opened from another get a way back at the top, as Android's own apps do.
        if (back) BackButton(Modifier.padding(start = 6.dp, top = 4.dp))
    Row(
        Modifier.fillMaxWidth().padding(start = Edge, end = 8.dp, top = if (back) 0.dp else 16.dp, bottom = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(
            text,
            style = MaterialTheme.typography.headlineLarge,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.weight(1f).semantics { heading() },
        )
        action()
    }
    }
}

/** A section's title; with `onMore`, a chevron leads to the whole list. */
@OptIn(androidx.compose.material3.ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun SectionHeader(text: String, modifier: Modifier = Modifier, top: Dp = 28.dp, onMore: (() -> Unit)? = null) {
    Row(
        modifier
            .fillMaxWidth()
            .then(if (onMore != null) Modifier.clickable(onClick = onMore) else Modifier)
            .padding(start = Edge, end = 12.dp, top = top, bottom = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(text, style = MaterialTheme.typography.titleLargeEmphasized, modifier = Modifier.semantics { heading() })
        if (onMore != null) {
            Icon(
                Icons.AutoMirrored.Rounded.KeyboardArrowRight,
                contentDescription = "See all",
                tint = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
    }
}

@Composable
@OptIn(ExperimentalFoundationApi::class)
fun AlbumTile(album: Album, modifier: Modifier = Modifier, open: ((Route) -> Unit)? = null, onClick: () -> Unit) {
    val interaction = remember { MutableInteractionSource() }
    var menu by remember { mutableStateOf(false) }
    val haptics = rememberHaptics()
    if (menu) AlbumSheet(album, open) { menu = false }
    Column(
        modifier
            .pressScale(interaction)
            .combinedClickable(interactionSource = interaction, indication = null, onClick = onClick, onLongClick = { haptics(true); menu = true }),
    ) {
        Cover(album.artwork, Modifier.fillMaxWidth().aspectRatio(1f).sharedCover("album-${album.key}"), seed = album.title + album.artist)
        Text(
            album.title.ifBlank { "Unknown album" },
            style = MaterialTheme.typography.bodyMedium,
            fontWeight = FontWeight.Medium,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.padding(top = 8.dp),
        )
        Text(
            album.artist,
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
    }
}

/** A song's menu: play it next, add it to the queue, or go to its album or artist. */
@Composable
fun SongMenu(song: Song, open: ((Route) -> Unit)?, remove: (() -> Unit)? = null) {
    var shown by remember { mutableStateOf(false) }
    SongMenu(song, open, shown, remove) { shown = it }
}

/** The menu's button; the menu itself is a sheet (see [SongSheet]). Opened from outside too. */
@Composable
fun SongMenu(
    song: Song,
    open: ((Route) -> Unit)?,
    shown: Boolean,
    remove: (() -> Unit)? = null,
    onShown: (Boolean) -> Unit,
) {
    IconButton(onClick = { onShown(true) }) {
        Icon(Icons.Rounded.MoreVert, contentDescription = "More", tint = MaterialTheme.colorScheme.onSurfaceVariant)
    }
    if (shown) SongSheet(song, open, remove) { onShown(false) }
}

@OptIn(ExperimentalFoundationApi::class)
/** How close list rows sit (Settings › Appearance › Density). */
data class Rows(
    /** A song row's height, its cover, and the gap between them. */
    val height: Dp,
    val cover: Dp,
    val gap: Dp,
    /** Space above and below the words of any other row (settings, menus, lists). */
    val pad: Dp,
    /** Space between covers in grids and shelves. */
    val grid: Dp,
) {
    companion object {
        fun of(density: String) = when (density) {
            "compact" -> Rows(48.dp, 38.dp, 12.dp, 6.dp, 10.dp)
            "spacious" -> Rows(76.dp, 60.dp, 18.dp, 20.dp, 30.dp)
            else -> Rows(60.dp, 48.dp, 14.dp, 12.dp, 20.dp)
        }
    }
}

val LocalRows = androidx.compose.runtime.compositionLocalOf { Rows.of("comfortable") }

/**
 * A song in a list: its cover (or its number on an album page), title and artist, and its menu,
 * over a hairline that starts where the text does. With a [place], a row of a group instead
 * (`segment`): no hairline, and the playing song on its own tone.
 */
@Composable
fun SongRow(
    song: Song,
    playing: Boolean,
    open: ((Route) -> Unit)?,
    modifier: Modifier = Modifier,
    number: Boolean = false,
    divider: Boolean = true,
    remove: (() -> Unit)? = null,
    /** Shown at the end instead of the menu, such as a handle to drag the row by. */
    trailing: (@Composable () -> Unit)? = null,
    place: Place? = null,
    onClick: () -> Unit,
) {
    val rows = LocalRows.current
    // Choosing many songs: a tap adds the row or takes it away.
    val chosenSongs by Selection.songs.collectAsState()
    val choosing = chosenSongs.isNotEmpty()
    val chosen = chosenSongs.any { it.id == song.id }
    val lead: Dp = if (number) 36.dp else rows.cover + 4.dp
    var menu by remember { mutableStateOf(false) }
    val haptics = rememberHaptics()
    // Not the whole playback: a row would redraw at every tick of the playing position.
    val isPlaying = playing && NeedleApp.instance.isPlaying.collectAsState().value
    val colors = MaterialTheme.colorScheme
    Column(
        modifier
            .fillMaxWidth()
            .then(
                if (place == null) {
                    Modifier.background(if (chosen) colors.primaryContainer.copy(alpha = 0.45f) else Color.Transparent)
                } else {
                    Modifier.segment(place, when {
                        chosen -> colors.primaryContainer
                        playing -> colors.secondaryContainer
                        else -> groupColor()
                    })
                },
            )
            // TalkBack hears what only colour shows: the song playing, a song chosen.
            .semantics {
                if (playing) stateDescription = if (isPlaying) "Playing" else "Paused"
                if (choosing) selected = chosen
            }
            .combinedClickable(
                onClick = { if (choosing) Selection.toggle(song) else onClick() },
                onLongClick = { haptics(true); if (choosing) Selection.toggle(song) else menu = true },
            ),
    ) {
        Row(
            Modifier.fillMaxWidth().heightIn(min = rows.height).padding(start = if (place == null) Edge else 14.dp, end = 4.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(rows.gap),
        ) {
            if (number) {
                Box(Modifier.width(22.dp)) {
                    if (playing) {
                        PlayingBars(isPlaying)
                    } else {
                        Text(
                            if (song.trackNumber > 0) song.trackNumber.toString() else "–",
                            style = MaterialTheme.typography.bodyLarge,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                }
            } else {
                Box(Modifier.size(rows.cover), contentAlignment = Alignment.Center) {
                    Cover(song.artwork, Modifier.fillMaxSize(), SmallCoverShape, seed = song.album + song.artist)
                    if (playing) {
                        Box(Modifier.fillMaxSize().clip(SmallCoverShape).background(Color.Black.copy(alpha = 0.45f)))
                        PlayingBars(isPlaying, color = Color.White)
                    }
                }
            }
            Column(Modifier.weight(1f).padding(vertical = if (rows.height < 56.dp) 4.dp else 8.dp)) {
                Text(
                    song.title,
                    style = MaterialTheme.typography.bodyLarge,
                    color = if (playing) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurface,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
                if (!number) {
                    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                        // A song on a music server (Navidrome, Subsonic), not on the phone.
                        if (song.fromServer()) {
                            Icon(
                                Icons.Rounded.Dns,
                                contentDescription = "On your music server",
                                tint = MaterialTheme.colorScheme.onSurfaceVariant,
                                modifier = Modifier.size(14.dp),
                            )
                        }
                        Text(
                            song.artist,
                            style = MaterialTheme.typography.bodyMedium,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                        )
                    }
                }
            }
            when {
                // The row is the one stop that chooses; the box only shows it.
                choosing -> androidx.compose.material3.Checkbox(chosen, onCheckedChange = null, modifier = Modifier.padding(12.dp))
                trailing != null -> trailing()
                else -> SongMenu(song, open, menu, remove) { menu = it }
            }
        }
        if (divider && place == null) {
            HorizontalDivider(
                color = MaterialTheme.colorScheme.outlineVariant,
                modifier = Modifier.padding(start = Edge + lead + rows.gap),
            )
        }
    }
}

/** A row of the Library's list: an amber icon, a name, and a chevron. */
@Composable
fun NavRow(icon: androidx.compose.ui.graphics.vector.ImageVector, text: String, onClick: () -> Unit) {
    Column(Modifier.fillMaxWidth().clickable(onClick = onClick)) {
        Row(
            Modifier.fillMaxWidth().heightIn(min = LocalRows.current.height - 4.dp).padding(start = Edge, end = 12.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(18.dp),
        ) {
            Icon(icon, contentDescription = null, tint = MaterialTheme.colorScheme.primary)
            Text(text, style = MaterialTheme.typography.titleMedium, modifier = Modifier.weight(1f))
            Icon(
                Icons.AutoMirrored.Rounded.KeyboardArrowRight,
                contentDescription = null,
                tint = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        HorizontalDivider(
            color = MaterialTheme.colorScheme.outlineVariant,
            modifier = Modifier.padding(start = Edge + 24.dp + 18.dp),
        )
    }
}

@Composable
fun RoundCover(path: String?, size: Dp) {
    Cover(path, Modifier.size(size), CircleShape)
}
