package fyi.nnx.needle.ui

import android.os.Build
import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.animateDpAsState
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.tween
import androidx.compose.animation.core.spring
import androidx.compose.animation.scaleIn
import androidx.compose.animation.scaleOut
import androidx.compose.material3.SwipeToDismissBox
import androidx.compose.material3.SwipeToDismissBoxValue
import androidx.compose.material3.rememberSwipeToDismissBoxState
import androidx.compose.runtime.DisposableEffect
import androidx.compose.ui.graphics.TransformOrigin
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.text.withStyle
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.basicMarquee
import androidx.compose.foundation.layout.offset
import androidx.compose.material.icons.rounded.KeyboardArrowDown
import kotlinx.coroutines.launch
import androidx.compose.foundation.gestures.detectHorizontalDragGestures
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.gestures.detectVerticalDragGestures
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.ui.unit.coerceAtMost
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.QueueMusic
import androidx.compose.material.icons.rounded.Lyrics
import androidx.compose.material.icons.rounded.Cast
import androidx.compose.material.icons.automirrored.rounded.VolumeUp
import androidx.compose.material.icons.automirrored.rounded.VolumeDown
import androidx.compose.material.icons.rounded.QueuePlayNext
import androidx.compose.material.icons.rounded.Forward10
import androidx.compose.material.icons.rounded.Replay10
import androidx.compose.material.icons.rounded.Search
import androidx.compose.material.icons.rounded.Computer
import androidx.compose.material.icons.rounded.Favorite
import androidx.compose.material.icons.rounded.FavoriteBorder
import androidx.compose.material.icons.rounded.Pause
import androidx.compose.material.icons.rounded.PlayArrow
import androidx.compose.material.icons.rounded.Repeat
import androidx.compose.material.icons.rounded.RepeatOne
import androidx.compose.material.icons.rounded.Shuffle
import androidx.compose.material.icons.rounded.SkipNext
import androidx.compose.material.icons.rounded.SkipPrevious
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.toShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.semantics.progressBarRangeInfo
import androidx.compose.ui.semantics.setProgress
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.blur
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.foundation.focusable
import androidx.compose.foundation.interaction.collectIsFocusedAsState
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.foundation.combinedClickable
import androidx.compose.ui.semantics.CustomAccessibilityAction
import androidx.compose.ui.semantics.customActions
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.onClick
import androidx.compose.ui.semantics.role
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.material3.LocalContentColor
import androidx.palette.graphics.Palette
import coil3.compose.AsyncImage
import coil3.imageLoader
import coil3.request.ImageRequest
import coil3.request.SuccessResult
import coil3.request.allowHardware
import coil3.toBitmap
import fyi.nnx.needle.NeedleApp
import fyi.nnx.needle.core.RepeatMode
import fyi.nnx.needle.core.Song
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

/**
 * The song playing, floating above the tabs as in Apple Music: cover, title, play or pause, and
 * next. Tap it for the full player.
 */
@OptIn(androidx.compose.material3.ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun MiniPlayer(onOpen: () -> Unit) {
    val playback by NeedleApp.instance.playback.collectAsState()
    val p = playback ?: return
    val song = p.current ?: return
    val app by NeedleApp.instance.app.collectAsState()
    CoverTheme(if (app.miniPlayerColored) song.artwork else null) { MiniPlayerBody(p, song, onOpen, app.miniPlayerColored, app.miniPlayerLyrics) }
}

@OptIn(androidx.compose.material3.ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun MiniPlayerBody(p: fyi.nnx.needle.core.Playback, song: Song, onOpen: () -> Unit, colored: Boolean, lyrics: Boolean) {
    val haptics = rememberHaptics()
    // Swiped sideways, it skips: left for the next song, right for the one before.
    var drag by remember { mutableFloatStateOf(0f) }
    val shift by animateFloatAsState(drag, spring(dampingRatio = 0.7f, stiffness = 500f), label = "mini swipe")
    Surface(
        color = if (colored) MaterialTheme.colorScheme.primaryContainer else MaterialTheme.colorScheme.surfaceContainerHigh,
        contentColor = if (colored) MaterialTheme.colorScheme.onPrimaryContainer else MaterialTheme.colorScheme.onSurface,
        shape = RoundedCornerShape(20.dp),
        shadowElevation = 8.dp,
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = 10.dp, vertical = 8.dp)
            .graphicsLayer { translationX = shift; alpha = 1f - (kotlin.math.abs(shift) / 900f).coerceIn(0f, 0.5f) }
            .pointerInput(song.id) {
                detectHorizontalDragGestures(
                    onDragEnd = {
                        when {
                            drag < -160f -> { haptics(false); Controls.next() }
                            drag > 160f -> { haptics(false); Controls.previous() }
                        }
                        drag = 0f
                    },
                    onDragCancel = { drag = 0f },
                ) { _, dx -> drag += dx }
            }
            .clip(RoundedCornerShape(20.dp))
            .clickable(onClick = onOpen),
    ) {
        Box {
            Column {
                Row(
                    Modifier.padding(start = 8.dp, end = 4.dp, top = 8.dp, bottom = 8.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(12.dp),
                ) {
                    Cover(song.artwork, Modifier.size(44.dp).sharedCover("now-${song.id}"), SmallCoverShape, seed = song.album + song.artist)
                    Column(Modifier.weight(1f)) {
                        Text(song.title, style = MaterialTheme.typography.bodyLarge, maxLines = 1, overflow = TextOverflow.Ellipsis)
                        Text(
                            if (song.onComputer()) "On your computer · ${song.artist}" else song.artist,
                            style = MaterialTheme.typography.bodySmall,
                            color = androidx.compose.material3.LocalContentColor.current.copy(alpha = 0.75f),
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                        )
                    }
                    // A tablet's bar has room for the song before as well.
                    if (isWide()) {
                        IconButton(onClick = { Controls.previous() }) {
                            Icon(Icons.Rounded.SkipPrevious, contentDescription = "Previous", modifier = Modifier.size(30.dp))
                        }
                    }
                    // Round while paused, a softer square while it plays.
                    androidx.compose.material3.FilledIconToggleButton(
                        checked = p.playing,
                        onCheckedChange = { haptics(false); Controls.toggle() },
                        shapes = androidx.compose.material3.IconButtonDefaults.toggleableShapes(),
                        colors = androidx.compose.material3.IconButtonDefaults.filledIconToggleButtonColors(
                            containerColor = MaterialTheme.colorScheme.primary,
                            contentColor = MaterialTheme.colorScheme.onPrimary,
                            checkedContainerColor = MaterialTheme.colorScheme.primary,
                            checkedContentColor = MaterialTheme.colorScheme.onPrimary,
                        ),
                        modifier = Modifier.playPauseSemantics(p.playing) { haptics(false); Controls.toggle() },
                    ) { PlayPauseIcon(p.playing, 26.dp) }
                    IconButton(onClick = { Controls.next() }) {
                        Icon(Icons.Rounded.SkipNext, contentDescription = "Next", modifier = Modifier.size(30.dp))
                    }
                }
                if (lyrics) MiniLyrics(song, p.position, Modifier.padding(start = 16.dp, end = 16.dp, bottom = 16.dp))
            }
            // Progress along the foot: a wave while it plays, flat while it rests (Android 16).
            val progress = if (song.duration > 0) (p.position / song.duration).toFloat().coerceIn(0f, 1f) else 0f
            ProgressLine(
                progress,
                p.playing,
                MaterialTheme.colorScheme.primary,
                androidx.compose.material3.LocalContentColor.current.copy(alpha = 0.15f),
                Modifier.align(Alignment.BottomCenter).padding(horizontal = 14.dp).height(6.dp),
                big = false,
            )
        }
    }
}

/**
 * The line of the lyrics playing, under the mini player's song (Settings › Appearance › Lyrics):
 * on a tablet the bar is wide, and the words can be read from across the room. A song without
 * timed lyrics shows nothing here.
 */
@Composable
private fun MiniLyrics(song: Song, position: Double, modifier: Modifier = Modifier) {
    // Not rememberLoaded: a song without lyrics should not offer Retry over the page.
    val lyrics by androidx.compose.runtime.produceState<fyi.nnx.needle.core.Lyrics?>(null, song.id) {
        value = withContext(Dispatchers.IO) {
            runCatching {
                if (song.onComputer()) {
                    core.pcLyrics(fyi.nnx.needle.core.PcSong(song.id.removePrefix(PcPrefix), song.title, song.artist, song.album, song.duration, song.format, song.artwork))
                } else core.lyrics(song.id)
            }.getOrNull()
        }
    }
    val lines = lyrics?.lines.orEmpty()
    if (lines.isEmpty()) return
    val now = lines.indexOfLast { it.time <= position + 0.2 }
    val line = lines.getOrNull(now)?.text?.ifBlank { null } ?: "♪"
    val reduce = reduceMotion()
    AnimatedContent(
        line,
        transitionSpec = {
            if (reduce) fadeIn(tween(150)) togetherWith fadeOut(tween(150))
            else (fadeIn(tween(250)) + androidx.compose.animation.slideInVertically(tween(250)) { it / 2 }) togetherWith fadeOut(tween(150))
        },
        label = "mini lyric",
        modifier = modifier.fillMaxWidth(),
    ) { text ->
        Text(
            text,
            style = MaterialTheme.typography.titleMedium,
            fontWeight = FontWeight.Bold,
            textAlign = androidx.compose.ui.text.style.TextAlign.Center,
            maxLines = 2,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.fillMaxWidth(),
        )
    }
}

/** The cover's strongest colour, where the background cannot be a blurred cover. */
@Composable
internal fun coverColor(path: String?): Color {
    val context = LocalContext.current
    val plain = Color(0xFF3A2E20)
    var color by remember { mutableStateOf(plain) }
    LaunchedEffect(path) {
        // A song without a cover goes back to the plain colour, not the last song's.
        val found = path?.let { coverSeed(context, it) }
        if (found == null) {
            color = plain
            return@LaunchedEffect
        }
        // Deep enough for white on it.
        color = Color(com.materialkolor.hct.Hct.fromInt(found).let { if (it.tone > 32) it.withTone(32.0) else it }.toInt())
    }
    return animateColorAsState(color, tween(400), label = "cover colour").value
}

/** Behind the player: the cover itself, blurred and darkened, as in Apple Music. */
@Composable
private fun PlayerBackdrop(song: Song) {
    val glow = coverColor(song.artwork)
    Box(Modifier.fillMaxSize().background(Color(0xFF0F0E0D))) {
        val app by NeedleApp.instance.app.collectAsState()
        if (app.coverBackdrop && Build.VERSION.SDK_INT >= 31 && song.artwork != null) {
            CoverBackdrop(song.artwork, Modifier.fillMaxSize(), dim = 0.3f)
        } else {
            Box(Modifier.fillMaxSize().background(Brush.verticalGradient(listOf(glow, Color(0xFF0F0E0D)))))
        }
        // Darker at the foot, so the controls always read.
        Box(
            Modifier.fillMaxSize().background(
                Brush.verticalGradient(0.5f to Color.Transparent, 1f to Color.Black.copy(alpha = 0.55f)),
            ),
        )
    }
}

private enum class Panel { Cover, Lyrics, UpNext }

@Composable
fun FullPlayer(onClose: () -> Unit, open: (Route) -> Unit) {
    val playback by NeedleApp.instance.playback.collectAsState()
    val p = playback ?: return
    val song = p.current ?: return
    var panel by rememberSaveable { mutableStateOf(Panel.Cover) }
    val haptics = rememberHaptics()
    // Read again when the library changes, so stars given from the song's menu show here.
    val version by NeedleApp.instance.libraryVersion.collectAsState()
    var favorite by remember(song.id, version) { mutableStateOf(runCatching { core.isFavorite(song.id) }.getOrDefault(song.rating >= 4)) }
    val hearts = androidx.compose.runtime.rememberCoroutineScope()
    val toggleFavorite: () -> Unit = {
        haptics(false)
        // Off the main thread: it writes to the library, which waits while a scan writes too.
        hearts.launch {
            withContext(kotlinx.coroutines.Dispatchers.IO) { runCatching { core.toggleFavorite(song.id) } }
                .onSuccess { favorite = it; showMessage(if (it) "Added to favorites" else "Taken out of favorites") }
                .onFailure { showMessage("Could not change favorites: ${it.message}") }
            NeedleApp.instance.libraryVersion.value++
        }
    }
    // The A-B loop: the first tap marks A, the second B, the third lets go.
    var loopStart by remember(song.id) { mutableStateOf<Double?>(null) }
    // Read again after each change of the loop too, not only as the song moves (it may be paused).
    var loopChanged by remember { androidx.compose.runtime.mutableIntStateOf(0) }
    val looping = remember(p.position, song.id, loopChanged) { core.loopRange() }
    // Pulled down far enough, the player closes.
    var pull by remember { mutableFloatStateOf(0f) }
    var searching by remember { mutableStateOf(false) }
    var dragging by remember { mutableStateOf(false) }
    // Let go short of closing, the player springs back up; closed, it is ready again at the top.
    val shown by animateFloatAsState(if (dragging) pull else 0f, if (dragging) androidx.compose.animation.core.snap() else spring(dampingRatio = 0.8f, stiffness = 400f), label = "pull")
    val closer = androidx.compose.runtime.rememberCoroutineScope()
    androidx.compose.runtime.DisposableEffect(Unit) {
        playerShowing.value = true
        onDispose { playerShowing.value = false }
    }

    CoverTheme(song.artwork, dark = true) {
    Box(
        Modifier
            .fillMaxSize()
            .graphicsLayer { translationY = (if (dragging) pull else shown).coerceAtLeast(0f) }
            .clickable(enabled = false) {}
            .pointerInput(Unit) {
                detectVerticalDragGestures(
                    onDragStart = { dragging = true },
                    onDragEnd = {
                        if (pull > 220f) {
                            onClose()
                            // Kept where it was while it slides away, then back at the top.
                            closer.launch { kotlinx.coroutines.delay(500); dragging = false; pull = 0f }
                        } else {
                            dragging = false
                            pull = 0f
                        }
                    },
                    onDragCancel = { dragging = false; pull = 0f },
                ) { _, dy -> pull += dy }
            },
    ) {
        PlayerBackdrop(song)
        CompositionLocalProvider(LocalContentColor provides Color.White) {
            BoxWithConstraints(Modifier.fillMaxSize().statusBarsPadding().navigationBarsPadding().padding(horizontal = 28.dp)) {
            // A tablet (or a phone) on its side: the cover, the lyrics, or what is up next at the
            // left, and the song and its controls at the right, rather than one tall column.
            val wide = maxWidth >= 600.dp && maxWidth > maxHeight
            val roomy = maxHeight >= 560.dp
            val side = (maxWidth * 0.46f).coerceAtMost(520.dp)
            // The cover, or in its place the lyrics or what is up next.
            val stage: @Composable () -> Unit = {
                when (panel) {
                    Panel.Cover -> PlayerCover(song, p.playing, haptics)
                    Panel.Lyrics -> LyricsPanel(song, p.position) { open(Route.Timing(song.id)) }
                    Panel.UpNext -> UpNextPanel(p.queueVersion)
                }
            }
            // A name too long for the width slides slowly along, as in Apple Music; the heart
            // sits at its end.
            val name: @Composable () -> Unit = {
                Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
                    Column(Modifier.weight(1f)) {
                        Text(
                            song.title,
                            style = MaterialTheme.typography.headlineMedium,
                            maxLines = 1,
                            modifier = Modifier.basicMarquee(iterations = Int.MAX_VALUE, initialDelayMillis = 2000, repeatDelayMillis = 3000),
                        )
                        Text(
                            song.artist,
                            style = MaterialTheme.typography.titleMedium,
                            color = Color.White.copy(alpha = 0.7f),
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                            modifier = Modifier.clip(RoundedCornerShape(6.dp)).clickable(enabled = !song.onComputer()) { open(Route.Artist(song.artist)) },
                        )
                        if (song.onComputer()) OnComputerChip(Modifier.padding(top = 8.dp))
                    }
                    if (!song.onComputer()) FavoriteButton(favorite, toggleFavorite, Modifier.offset(x = 12.dp), large = true)
                }
            }
            val controls: @Composable () -> Unit = {
                SeekBar(position = p.position, duration = song.duration, playing = p.playing)
                PlayerControls(p.playing, p.position, song.duration, haptics)
                val app by NeedleApp.instance.app.collectAsState()
                if (song.onComputer() || app.volumeSlider) VolumeRow(song.onComputer())
                PlayerToolbar(
                    panel = panel,
                    onPanel = { panel = if (panel == it) Panel.Cover else it },
                    repeat = p.repeat,
                    looping = looping.isNotEmpty(),
                    loopStarted = loopStart != null,
                    onLoop = {
                        when {
                            // The loop is the phone's player's own; the computer's has none.
                            song.onComputer() -> showMessage("The A-B loop works for songs playing on this phone")
                            looping.isNotEmpty() -> { core.setLoop(null, null); loopStart = null; loopChanged++; showMessage("Loop off") }
                            loopStart == null -> { loopStart = p.position; showMessage("Loop from ${time(p.position)}: tap again where it ends") }
                            else -> {
                                // B may be before A (sought back): the loop is between them either way.
                                val a = minOf(loopStart!!, p.position)
                                val b = maxOf(loopStart!!, p.position)
                                if (b - a < 0.5) {
                                    showMessage("Too short to loop: tap again further on")
                                } else {
                                    core.setLoop(a, b)
                                    loopChanged++
                                    showMessage("Looping ${time(a)} to ${time(b)}")
                                    loopStart = null
                                }
                            }
                        }
                    },
                )
            }
            Column(Modifier.fillMaxSize()) {
                // The top row: close at the left, the handle to pull the player down by in the
                // middle, and the song's heart and menu at the right, clear of its name.
                Box(Modifier.fillMaxWidth().padding(top = 4.dp)) {
                    IconButton(onClick = onClose, modifier = Modifier.align(Alignment.CenterStart).offset(x = (-12).dp)) {
                        Icon(Icons.Rounded.KeyboardArrowDown, contentDescription = "Close the player", modifier = Modifier.size(30.dp))
                    }
                    Box(
                        Modifier
                            .align(Alignment.Center)
                            .size(width = 40.dp, height = 5.dp)
                            .clip(CircleShape)
                            .background(Color.White.copy(alpha = 0.45f))
                            .clickable(onClick = onClose),
                    )
                    Row(Modifier.align(Alignment.CenterEnd).offset(x = 12.dp)) {
                        IconButton(onClick = { searching = true }) {
                            Icon(Icons.Rounded.Search, contentDescription = "Add songs", tint = Color.White.copy(alpha = 0.8f))
                        }
                        // Beside the song's name instead, wherever its name shows.
                        if (!song.onComputer() && !wide && panel != Panel.Cover) FavoriteButton(favorite, toggleFavorite)
                        if (!song.onComputer()) SongMenu(song, open)
                    }
                }
                if (searching) PlayerSearch(song.onComputer()) { searching = false }
                if (wide) {
                    Row(Modifier.weight(1f).fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(48.dp)) {
                        Box(Modifier.weight(1f).fillMaxHeight().padding(vertical = 12.dp), contentAlignment = Alignment.Center) { stage() }
                        // Scrolls when a phone on its side is too short for all of it.
                        Box(Modifier.width(side).fillMaxHeight(), contentAlignment = Alignment.Center) {
                            Column(Modifier.fillMaxWidth().verticalScroll(rememberScrollState())) {
                                // The cover, small, while the lyrics or the queue have its place.
                                if (panel != Panel.Cover && roomy) {
                                    Cover(song.artwork, Modifier.padding(bottom = 20.dp).size(132.dp), CoverShape, seed = song.album + song.artist)
                                }
                                name()
                                controls()
                            }
                        }
                    }
                } else {
                    // On a tablet standing up, no wider than a large phone's player.
                    Column(Modifier.weight(1f).align(Alignment.CenterHorizontally).widthIn(max = 640.dp).fillMaxWidth()) {
                        if (panel != Panel.Cover) SmallNowPlaying(song)
                        Box(Modifier.weight(1f).fillMaxWidth(), contentAlignment = Alignment.Center) { stage() }
                        if (panel == Panel.Cover) name()
                        controls()
                    }
                }
            }
            }
        }
    }
    }
}

/** The heart: in favorites or not. */
@Composable
private fun FavoriteButton(favorite: Boolean, onClick: () -> Unit, modifier: Modifier = Modifier, large: Boolean = false) {
    IconButton(onClick = onClick, modifier = modifier) {
        Icon(
            if (favorite) Icons.Rounded.Favorite else Icons.Rounded.FavoriteBorder,
            contentDescription = if (favorite) "Take out of favorites" else "Add to favorites",
            tint = if (favorite) MaterialTheme.colorScheme.primary else Color.White.copy(alpha = 0.8f),
            modifier = Modifier.size(if (large) 28.dp else 24.dp),
        )
    }
}

/**
 * Previous, play or pause, and next, as Android 16's media controls: one large button in the
 * cover's colour, round while paused and a softer square while playing, between two wide
 * buttons. Each squeezes a little as it is pressed.
 */
@OptIn(androidx.compose.material3.ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun PlayerControls(playing: Boolean, position: Double, duration: Double, haptics: (Boolean) -> Unit) {
    val app by NeedleApp.instance.app.collectAsState()
    val jump = app.sideButtons == "jump"
    val back = { haptics(false); if (jump) Controls.seek((position - 10).coerceAtLeast(0.0)) else Controls.previous() }
    val ahead = { haptics(false); if (jump) Controls.seek((position + 10).coerceAtMost(duration)) else Controls.next() }
    val toggle = { haptics(false); Controls.toggle() }
    val backIcon = if (jump) Icons.Rounded.Replay10 else Icons.Rounded.SkipPrevious
    val aheadIcon = if (jump) Icons.Rounded.Forward10 else Icons.Rounded.SkipNext
    val backLabel = if (jump) "Back 10 seconds" else "Previous"
    val aheadLabel = if (jump) "Ahead 10 seconds" else "Next"
    val middle = remember { androidx.compose.foundation.interaction.MutableInteractionSource() }
    Row(
        Modifier.fillMaxWidth().padding(top = 14.dp, bottom = 18.dp),
        horizontalArrangement = Arrangement.spacedBy(if (app.controlStyle == "minimal") 36.dp else 14.dp, Alignment.CenterHorizontally),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        when (app.controlStyle) {
            // Round: three circles, the middle one large and in the cover's colour.
            "round" -> {
                val side = androidx.compose.material3.IconButtonDefaults.filledTonalIconButtonColors(containerColor = Color.White.copy(alpha = 0.14f), contentColor = Color.White)
                androidx.compose.material3.FilledTonalIconButton(onClick = back, colors = side, shape = CircleShape, modifier = Modifier.size(60.dp)) {
                    Icon(backIcon, contentDescription = backLabel, modifier = Modifier.size(28.dp))
                }
                androidx.compose.material3.FilledIconButton(
                    onClick = toggle,
                    shape = CircleShape,
                    colors = androidx.compose.material3.IconButtonDefaults.filledIconButtonColors(containerColor = MaterialTheme.colorScheme.primary, contentColor = MaterialTheme.colorScheme.onPrimary),
                    interactionSource = middle,
                    modifier = Modifier.size(80.dp),
                ) { PlayPauseIcon(playing, 38.dp) }
                androidx.compose.material3.FilledTonalIconButton(onClick = ahead, colors = side, shape = CircleShape, modifier = Modifier.size(60.dp)) {
                    Icon(aheadIcon, contentDescription = aheadLabel, modifier = Modifier.size(28.dp))
                }
            }
            // Minimal: the icons alone, large and white, as in Apple Music.
            "minimal" -> {
                IconButton(onClick = back, modifier = Modifier.size(64.dp)) { Icon(backIcon, contentDescription = backLabel, modifier = Modifier.size(44.dp)) }
                IconButton(onClick = toggle, interactionSource = middle, modifier = Modifier.size(80.dp)) { PlayPauseIcon(playing, 64.dp) }
                IconButton(onClick = ahead, modifier = Modifier.size(64.dp)) { Icon(aheadIcon, contentDescription = aheadLabel, modifier = Modifier.size(44.dp)) }
            }
            // Expressive (Android 16's media controls): one large button in the cover's colour,
            // round while paused and a softer square while playing, between two wide ones.
            // Each squeezes a little as it is pressed.
            else -> {
                val side = androidx.compose.material3.IconButtonDefaults.filledTonalIconButtonColors(containerColor = Color.White.copy(alpha = 0.14f), contentColor = Color.White)
                androidx.compose.material3.FilledTonalIconButton(
                    onClick = back,
                    shapes = androidx.compose.material3.IconButtonDefaults.shapes(),
                    colors = side,
                    modifier = Modifier.size(width = 64.dp, height = 56.dp),
                ) { Icon(backIcon, contentDescription = backLabel, modifier = Modifier.size(30.dp)) }
                androidx.compose.material3.FilledIconToggleButton(
                    checked = playing,
                    onCheckedChange = { toggle() },
                    shapes = androidx.compose.material3.IconButtonDefaults.toggleableShapes(),
                    colors = androidx.compose.material3.IconButtonDefaults.filledIconToggleButtonColors(
                        containerColor = MaterialTheme.colorScheme.primary,
                        contentColor = MaterialTheme.colorScheme.onPrimary,
                        checkedContainerColor = MaterialTheme.colorScheme.primary,
                        checkedContentColor = MaterialTheme.colorScheme.onPrimary,
                    ),
                    interactionSource = middle,
                    // A button that plays or pauses, not a box to tick: TalkBack says which it does.
                    modifier = Modifier.size(width = 88.dp, height = 72.dp).playPauseSemantics(playing) { toggle() },
                ) { PlayPauseIcon(playing, 36.dp) }
                androidx.compose.material3.FilledTonalIconButton(
                    onClick = ahead,
                    shapes = androidx.compose.material3.IconButtonDefaults.shapes(),
                    colors = side,
                    modifier = Modifier.size(width = 64.dp, height = 56.dp),
                ) { Icon(aheadIcon, contentDescription = aheadLabel, modifier = Modifier.size(30.dp)) }
            }
        }
    }
}

/**
 * The volume: the computer's while playing there, otherwise the phone's own music volume
 * (the same one its buttons change).
 */
@Composable
private fun VolumeRow(computer: Boolean) {
    val context = androidx.compose.ui.platform.LocalContext.current
    val audio = remember { context.getSystemService(android.content.Context.AUDIO_SERVICE) as android.media.AudioManager }
    val stream = android.media.AudioManager.STREAM_MUSIC
    val playback by NeedleApp.instance.playback.collectAsState()
    var held by remember { mutableStateOf<Float?>(null) }
    // The phone's volume can change by its buttons, so it is read again every half second.
    var phone by remember { mutableFloatStateOf(audio.getStreamVolume(stream).toFloat() / audio.getStreamMaxVolume(stream)) }
    LaunchedEffect(computer) {
        while (!computer) {
            phone = audio.getStreamVolume(stream).toFloat() / audio.getStreamMaxVolume(stream)
            kotlinx.coroutines.delay(500)
        }
    }
    val value = held ?: if (computer) playback?.volume ?: 1f else phone
    Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.fillMaxWidth().padding(bottom = 10.dp)) {
        Icon(Icons.AutoMirrored.Rounded.VolumeDown, contentDescription = null, tint = Color.White.copy(alpha = 0.7f))
        androidx.compose.material3.Slider(
            modifier = Modifier.weight(1f).padding(horizontal = 8.dp).named("Volume"),
            value = value,
            onValueChange = { v ->
                held = v
                if (computer) Controls.volume(v, final = false)
                else audio.setStreamVolume(stream, (v * audio.getStreamMaxVolume(stream)).toInt(), 0)
            },
            onValueChangeFinished = {
                val v = held
                if (computer && v != null) Controls.volume(v, final = true)
                if (!computer && v != null) phone = v
                held = null
            },
            colors = androidx.compose.material3.SliderDefaults.colors(
                thumbColor = Color.White,
                activeTrackColor = Color.White,
                inactiveTrackColor = Color.White.copy(alpha = 0.25f),
            ),
        )
        Icon(Icons.AutoMirrored.Rounded.VolumeUp, contentDescription = null, tint = Color.White.copy(alpha = 0.7f))
    }
}

/**
 * Search from the player, to add songs to what plays: the computer's music while playing there,
 * the phone's own otherwise. A song can play now, next, or at the end.
 */
@OptIn(androidx.compose.material3.ExperimentalMaterial3Api::class)
@Composable
private fun PlayerSearch(computer: Boolean, onDismiss: () -> Unit) {
    var query by remember { mutableStateOf("") }
    var found by remember { mutableStateOf<List<Song>>(emptyList()) }
    LaunchedEffect(query) {
        kotlinx.coroutines.delay(250)
        found = if (query.isBlank()) emptyList() else kotlinx.coroutines.withContext(kotlinx.coroutines.Dispatchers.IO) {
            runCatching {
                if (computer) core.pcSearch(query).map { it.asSong() } else core.search(query)
            }.getOrDefault(emptyList())
        }
    }
    fun act(song: Song, how: String) {
        val id = song.id.removePrefix(PcPrefix)
        NeedleApp.instance.scope.launch(kotlinx.coroutines.Dispatchers.IO) {
            runCatching {
                when {
                    computer && how == "now" -> core.pcPlay(listOf(id))
                    computer && how == "next" -> core.pcPlayNext(listOf(id))
                    computer -> core.pcEnqueue(listOf(id))
                    how == "now" -> core.play(listOf(id), 0u)
                    how == "next" -> core.playNext(listOf(id))
                    else -> core.enqueue(listOf(id))
                }
            }.onSuccess {
                showMessage(
                    when (how) {
                        "now" -> "Playing ${song.title}"
                        "next" -> "${song.title} plays next"
                        else -> "Added ${song.title} to the queue"
                    },
                )
            }.onFailure { showMessage(it.message ?: "It did not work") }
        }
    }
    androidx.compose.material3.ModalBottomSheet(onDismissRequest = onDismiss, containerColor = MaterialTheme.colorScheme.surfaceContainerLow) {
        Column(Modifier.fillMaxWidth().fillMaxHeight(0.85f).padding(horizontal = 16.dp)) {
            Text(
                if (computer) "Add from your computer" else "Add songs",
                style = MaterialTheme.typography.headlineSmall,
                modifier = Modifier.padding(start = 4.dp, bottom = 12.dp),
            )
            androidx.compose.material3.TextField(
                value = query,
                onValueChange = { query = it },
                placeholder = { Text("Songs, albums, artists") },
                leadingIcon = { Icon(Icons.Rounded.Search, contentDescription = null) },
                singleLine = true,
                shape = RoundedCornerShape(28.dp),
                colors = androidx.compose.material3.TextFieldDefaults.colors(
                    focusedIndicatorColor = Color.Transparent,
                    unfocusedIndicatorColor = Color.Transparent,
                    focusedContainerColor = MaterialTheme.colorScheme.surfaceContainerHigh,
                    unfocusedContainerColor = MaterialTheme.colorScheme.surfaceContainerHigh,
                ),
                modifier = Modifier.remoteEscape().fillMaxWidth(),
            )
            LazyColumn(Modifier.fillMaxSize().padding(top = 8.dp)) {
                itemsIndexed(found, key = { i, s -> "$i-${s.id}" }) { _, song ->
                    Row(
                        Modifier.fillMaxWidth().clip(RoundedCornerShape(12.dp)).clickable { act(song, "now") }.padding(vertical = 6.dp),
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(12.dp),
                    ) {
                        Cover(song.artwork, Modifier.size(48.dp), SmallCoverShape)
                        Column(Modifier.weight(1f)) {
                            Text(song.title, style = MaterialTheme.typography.bodyLarge, maxLines = 1, overflow = TextOverflow.Ellipsis)
                            Text(song.artist, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant, maxLines = 1, overflow = TextOverflow.Ellipsis)
                        }
                        IconButton(onClick = { act(song, "next") }) { Icon(Icons.Rounded.QueuePlayNext, contentDescription = "Play next") }
                        IconButton(onClick = { act(song, "end") }) { Icon(Icons.AutoMirrored.Rounded.QueueMusic, contentDescription = "Add to the queue") }
                    }
                }
            }
        }
    }
}

/**
 * Lyrics, shuffle, repeat, the A-B loop, Play on, and Up next: six buttons of one kind, evenly
 * spaced on a soft pill (Material 3 Expressive's floating toolbar). A button that is on fills.
 */
@Composable
private fun PlayerToolbar(
    panel: Panel,
    onPanel: (Panel) -> Unit,
    repeat: RepeatMode,
    looping: Boolean,
    loopStarted: Boolean,
    onLoop: () -> Unit,
) {
    Row(
        Modifier
            .fillMaxWidth()
            .padding(bottom = 12.dp)
            .clip(CircleShape)
            .background(Color.White.copy(alpha = 0.1f))
            .padding(horizontal = 6.dp, vertical = 4.dp),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically,
    ) {
        RoundToggle(panel == Panel.Lyrics, { onPanel(Panel.Lyrics) }, if (panel == Panel.Lyrics) "Showing" else "Hidden") { Icon(Icons.Rounded.Lyrics, contentDescription = "Lyrics") }
        RoundToggle(false, { Controls.shuffle(); showMessage("Shuffled what is up next") }) { Icon(Icons.Rounded.Shuffle, contentDescription = "Shuffle what is up next") }
        RoundToggle(repeat != RepeatMode.OFF, {
            Controls.setRepeat(
                when (repeat) {
                    RepeatMode.OFF -> RepeatMode.ALL
                    RepeatMode.ALL -> RepeatMode.ONE
                    RepeatMode.ONE -> RepeatMode.OFF
                },
            )
        }, when (repeat) {
            RepeatMode.OFF -> "Off"
            RepeatMode.ALL -> "All songs"
            RepeatMode.ONE -> "This song"
        }) { Icon(if (repeat == RepeatMode.ONE) Icons.Rounded.RepeatOne else Icons.Rounded.Repeat, contentDescription = "Repeat") }
        RoundToggle(looping || loopStarted, onLoop, if (looping) "Looping" else if (loopStarted) "Start marked" else "Off") {
            Text(
                if (loopStarted) "A…" else "A-B",
                style = MaterialTheme.typography.labelLarge,
                modifier = Modifier.semantics { contentDescription = "A-B loop" },
            )
        }
        RoundToggle(false, { Ui.sheet.value = Sheet.PlayOn }) { Icon(Icons.Rounded.Cast, contentDescription = "Play on") }
        RoundToggle(panel == Panel.UpNext, { onPanel(Panel.UpNext) }, if (panel == Panel.UpNext) "Showing" else "Hidden") { Icon(Icons.AutoMirrored.Rounded.QueueMusic, contentDescription = "Up next") }
    }
}

/**
 * An icon button that shows a soft circle when on, as Apple Music's lyrics and queue buttons.
 * `state` is what TalkBack says it is set to; actions (Shuffle, Play on) have none.
 */
@Composable
private fun RoundToggle(on: Boolean, onClick: () -> Unit, state: String? = null, icon: @Composable () -> Unit) {
    val fill by animateColorAsState(if (on) Color.White.copy(alpha = 0.2f) else Color.Transparent, label = "toggle")
    IconButton(onClick = onClick, modifier = Modifier.clip(CircleShape).background(fill).semantics { if (state != null) stateDescription = state }) {
        CompositionLocalProvider(LocalContentColor provides if (on) Color.White else Color.White.copy(alpha = 0.7f)) {
            icon()
        }
    }
}

/**
 * The playing cover, in the shape chosen in Settings › Appearance: a rounded square; a record
 * that turns while the song plays; or one of Material's shapes, slowly turning.
 */
@OptIn(androidx.compose.material3.ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun PlayerCover(song: Song, playing: Boolean, haptics: (Boolean) -> Unit) {
    val app by NeedleApp.instance.app.collectAsState()
    // Swiped sideways (when that is on), the cover goes with the finger; far enough, it slides
    // off and the next song's (or the last one's) slides in from the other side.
    val slide = remember { androidx.compose.animation.core.Animatable(0f) }
    var width by remember { mutableFloatStateOf(1f) }
    // Which way the last swipe went, until the song it asked for shows: 1 next, -1 back.
    var swiped by remember { androidx.compose.runtime.mutableIntStateOf(0) }
    val swipes = androidx.compose.runtime.rememberCoroutineScope()
    LaunchedEffect(song.id) {
        val from = swiped
        swiped = 0
        if (from == 0) slide.snapTo(0f) else {
            slide.snapTo(from * width)
            slide.animateTo(0f, spring(dampingRatio = 0.85f, stiffness = 400f))
        }
    }
    val scale by animateFloatAsState(if (playing) 1f else 0.82f, tween(350), label = "cover size")
    val reduce = reduceMotion()
    val still = reduce || app.coverShape == "square"
    // Turning only while it plays and is not square: a turn that stands still takes no frames.
    // Read only where the layer draws, so the turning redraws the cover, not the whole player.
    val turn: androidx.compose.runtime.State<Float> = if (!still && playing) {
        androidx.compose.animation.core.rememberInfiniteTransition(label = "spin").animateFloat(
            0f, 360f,
            androidx.compose.animation.core.infiniteRepeatable(tween(if (app.coverShape == "round") 12_000 else 40_000, easing = androidx.compose.animation.core.LinearEasing)),
            label = "turn",
        )
    } else remember { mutableFloatStateOf(0f) }
    // The turn holds where it was last drawn while paused, and goes on from there.
    val drawn = remember { floatArrayOf(0f) }
    var held by remember { mutableFloatStateOf(0f) }
    var offset by remember { mutableFloatStateOf(0f) }
    LaunchedEffect(playing) { if (!playing) held = drawn[0] else offset = held - turn.value }
    val shape = when (app.coverShape) {
        "round" -> CircleShape
        "shape" -> androidx.compose.material3.MaterialShapes.Cookie12Sided.toShape()
        else -> RoundedCornerShape(12.dp)
    }
    val depth by animateFloatAsState(if (playing) 1f else 0.4f, tween(350), label = "shadow")
    BoxWithConstraints(
        Modifier
            .onSizeChanged { width = it.width.toFloat().coerceAtLeast(1f) }
            .graphicsLayer {
                translationX = slide.value
                alpha = 1f - (kotlin.math.abs(slide.value) / width * 0.7f).coerceIn(0f, 0.7f)
            }
            .then(
                if (!app.swipeCover) Modifier else Modifier.pointerInput(song.id) {
                    detectHorizontalDragGestures(
                        onDragEnd = {
                            val moved = slide.value
                            swipes.launch {
                                if (kotlin.math.abs(moved) < width * 0.22f) {
                                    slide.animateTo(0f, spring(dampingRatio = 0.7f, stiffness = 500f))
                                    return@launch
                                }
                                val way = if (moved < 0) 1 else -1
                                haptics(false)
                                slide.animateTo(-way * width * 1.1f, tween(160))
                                swiped = way
                                if (way > 0) Controls.next() else Controls.previous()
                                // No other song came (the end of the list, or the song began
                                // again): the cover comes back.
                                kotlinx.coroutines.delay(1500)
                                if (swiped == way) {
                                    swiped = 0
                                    slide.animateTo(0f, spring(dampingRatio = 0.85f, stiffness = 400f))
                                }
                            }
                        },
                        onDragCancel = { swipes.launch { slide.animateTo(0f) } },
                    ) { change, dx ->
                        change.consume()
                        swipes.launch { slide.snapTo(slide.value + dx) }
                    }
                },
            ),
        contentAlignment = Alignment.Center,
    ) {
        // As large as fits whichever way the room is shorter: across on a phone standing up,
        // down on a tablet on its side.
        val side = minOf(maxWidth, maxHeight)
        // The shadow: a soft dark glow behind the cover, drawn once. A true shadow of a turning
        // or many-sided shape is worked out again every frame, and freezes slower phones.
        Box(
            Modifier
                .size(side)
                .graphicsLayer { scaleX = scale * 0.96f; scaleY = scale * 0.96f; translationY = 18.dp.toPx() * depth; alpha = depth }
                .background(
                    Brush.radialGradient(0.55f to Color.Black.copy(alpha = 0.55f), 1f to Color.Transparent),
                ),
        )
        Cover(
            song.artwork,
            Modifier
                .size(side)
                .sharedCover("now-${song.id}")
                .graphicsLayer {
                scaleX = scale
                scaleY = scale
                rotationZ = (if (still) 0f else if (playing) turn.value + offset else held).also { drawn[0] = it }
            },
            shape,
            seed = song.album + song.artist,
        )
        // A record's label hole.
        if (app.coverShape == "round") {
            Box(Modifier.size(34.dp).graphicsLayer { scaleX = scale; scaleY = scale }.clip(CircleShape).background(Color.Black.copy(alpha = 0.55f)))
            Box(Modifier.size(10.dp).clip(CircleShape).background(Color(0xFF0F0E0D)))
        }
    }
}

/** Says the song plays on the computer, with the computer's icon. */
@Composable
private fun OnComputerChip(modifier: Modifier = Modifier) {
    Row(
        modifier.clip(CircleShape).background(Color.White.copy(alpha = 0.14f)).padding(horizontal = 10.dp, vertical = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        Icon(androidx.compose.material.icons.Icons.Rounded.Computer, contentDescription = null, modifier = Modifier.size(16.dp))
        Text("Playing on your computer", style = MaterialTheme.typography.labelMedium)
    }
}

/** The song, small, above the lyrics or the queue. */
@Composable
private fun SmallNowPlaying(song: Song) {
    Row(
        Modifier.fillMaxWidth().padding(vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Cover(song.artwork, Modifier.size(56.dp), SmallCoverShape)
        Column(Modifier.weight(1f)) {
            Text(song.title, style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.Bold, maxLines = 1, overflow = TextOverflow.Ellipsis)
            Text(song.artist, style = MaterialTheme.typography.bodyMedium, color = Color.White.copy(alpha = 0.7f), maxLines = 1, overflow = TextOverflow.Ellipsis)
        }
    }
}

/**
 * A thin bar that thickens while held, as in Apple Music. The time shown follows the finger;
 * the song moves only when it lets go.
 */
@Composable
private fun SeekBar(position: Double, duration: Double, playing: Boolean) {
    var held by remember { mutableStateOf<Float?>(null) }
    val length = duration.coerceAtLeast(1.0).toFloat()
    val shown = held ?: (position.toFloat() / length).coerceIn(0f, 1f)
    // With a remote or keys: the bar takes the focus, and left and right go back or on ten seconds.
    val focus = remember { androidx.compose.foundation.interaction.MutableInteractionSource() }
    val focused by focus.collectIsFocusedAsState()
    val thickness by animateDpAsState(if (held != null || focused) 10.dp else 5.dp, label = "seek thickness")
    Column(Modifier.padding(top = 16.dp)) {
        Box(
            Modifier
                .fillMaxWidth()
                .height(28.dp)
                .onKeyEvent {
                    val step = when (it.key) {
                        Key.DirectionLeft -> -10.0
                        Key.DirectionRight -> 10.0
                        else -> return@onKeyEvent false
                    }
                    if (it.type == KeyEventType.KeyDown) Controls.seek((position + step).coerceIn(0.0, length.toDouble()))
                    true
                }
                .focusable(interactionSource = focus)
                .semantics {
                    contentDescription = "Seek"
                    // Where the song is, and a way for TalkBack to move it.
                    progressBarRangeInfo = androidx.compose.ui.semantics.ProgressBarRangeInfo(shown, 0f..1f)
                    setProgress { to -> Controls.seek(to.coerceIn(0f, 1f) * length.toDouble()); true }
                }
                .pointerInput(length) {
                    detectTapGestures { Controls.seek((it.x / size.width).coerceIn(0f, 1f) * length.toDouble()) }
                }
                .pointerInput(length) {
                    detectHorizontalDragGestures(
                        onDragStart = { held = (it.x / size.width).coerceIn(0f, 1f) },
                        onDragEnd = { held?.let { Controls.seek(it * length.toDouble()) }; held = null },
                        onDragCancel = { held = null },
                    ) { change, _ -> held = (change.position.x / size.width).coerceIn(0f, 1f) }
                },
            contentAlignment = Alignment.CenterStart,
        ) {
            if (held == null && !focused) {
                ProgressLine(shown, playing, Color.White, Color.White.copy(alpha = 0.25f))
            } else {
                Box(Modifier.fillMaxWidth().height(thickness).clip(CircleShape).background(Color.White.copy(alpha = 0.25f)))
                Box(Modifier.fillMaxWidth(shown).height(thickness).clip(CircleShape).background(Color.White))
            }
        }
        Row(Modifier.fillMaxWidth()) {
            Text(time((shown * length).toDouble()), style = MaterialTheme.typography.labelMedium, color = Color.White.copy(alpha = 0.6f))
            Spacer(Modifier.weight(1f))
            // A song whose length is not known has no time left to show.
            if (duration > 0) Text("-" + time(length.toDouble() - shown * length), style = MaterialTheme.typography.labelMedium, color = Color.White.copy(alpha = 0.6f))
        }
    }
}

@Composable
private fun LyricsPanel(song: Song, position: Double, time: () -> Unit) {
    // Looked up again after NetEase is added.
    var retry by remember(song.id) { androidx.compose.runtime.mutableIntStateOf(0) }
    var askingNetEase by remember(song.id) { mutableStateOf(false) }
    val lyricsLoad = rememberLoaded(song.id, retry) {
        if (song.onComputer()) {
            pcLyrics(fyi.nnx.needle.core.PcSong(song.id.removePrefix(PcPrefix), song.title, song.artist, song.album, song.duration, song.format, song.artwork))
        } else lyrics(song.id)
    }
    val lyrics by lyricsLoad
    val lines = lyrics?.lines.orEmpty()
    val state = rememberLazyListState()
    val now = lines.indexOfLast { it.time <= position + 0.2 }
    val app by NeedleApp.instance.app.collectAsState()
    LaunchedEffect(now, app.liveLyrics) {
        if (app.liveLyrics && now >= 0) state.animateScrollToItem((now - 1).coerceAtLeast(0))
    }
    // Keeps the screen on while the lyrics show, when that is on.
    val view = LocalView.current
    DisposableEffect(app.keepScreenOnLyrics) {
        view.keepScreenOn = app.keepScreenOnLyrics
        onDispose { view.keepScreenOn = false }
    }
    when {
        lyricsLoad.loading -> Text("Loading lyrics…", color = Color.White.copy(alpha = 0.6f))
        lyricsLoad.failed -> Unit // The load failure offers Retry above this page.
        lyrics == null || (lines.isEmpty() && lyrics?.plain.isNullOrBlank()) -> Column(horizontalAlignment = Alignment.CenterHorizontally) {
            Text(
                if (lyrics?.instrumental == true) "Instrumental" else "No lyrics for this song",
                style = MaterialTheme.typography.titleMedium,
                color = Color.White.copy(alpha = 0.6f),
            )
            if (lyrics?.instrumental != true) {
                // NetEase has timed lyrics for a great many songs, K-pop and J-pop above all.
                androidx.compose.material3.TextButton(
                    enabled = !askingNetEase,
                    onClick = {
                        askingNetEase = true
                        NeedleApp.instance.scope.launch {
                            kotlinx.coroutines.withContext(kotlinx.coroutines.Dispatchers.IO) { runCatching { core.addNetease(song.id.removePrefix(PcPrefix)) } }
                            showMessage("Looking on NetEase…")
                            kotlinx.coroutines.delay(1500)
                            retry++
                            askingNetEase = false
                        }
                    },
                ) { Text(if (askingNetEase) "Looking…" else "Also look on NetEase", color = Color.White) }
                if (!song.onComputer()) androidx.compose.material3.TextButton(onClick = time) { Text("Add and time them", color = Color.White) }
            }
        }
        lines.isEmpty() -> LazyColumn(Modifier.fillMaxSize()) {
            item { Text(lyrics!!.plain, style = MaterialTheme.typography.titleLarge, fontWeight = FontWeight.Bold) }
            if (!song.onComputer()) item { androidx.compose.material3.TextButton(onClick = time) { Text("Time these lyrics", color = Color.White) } }
        }
        else -> LazyColumn(Modifier.fillMaxSize(), state = state) {
            itemsIndexed(lines) { i, line ->
                val lit = !app.liveLyrics || i == now
                val alpha by animateFloatAsState(if (lit) 1f else if (i < now) 0.25f else 0.4f, tween(300), label = "line")
                val grow by animateFloatAsState(if (i == now && app.liveLyrics) 1f else 0.94f, spring(dampingRatio = 0.7f, stiffness = 300f), label = "line size")
                // Karaoke: the words sung so far are lit, the rest wait.
                val sung = if (i == now && line.words.isNotEmpty() && app.liveLyrics) {
                    androidx.compose.ui.text.buildAnnotatedString {
                        line.words.forEach { word ->
                            withStyle(androidx.compose.ui.text.SpanStyle(color = if (word.time <= position) Color.White else Color.White.copy(alpha = 0.4f))) {
                                append(word.text)
                            }
                        }
                    }
                } else null
                Text(
                    sung ?: androidx.compose.ui.text.AnnotatedString(line.text.ifBlank { "♪" }),
                    style = MaterialTheme.typography.headlineMedium,
                    fontWeight = FontWeight.Bold,
                    color = Color.White.copy(alpha = alpha),
                    modifier = Modifier
                        .fillMaxWidth()
                        .graphicsLayer { scaleX = grow; scaleY = grow; transformOrigin = TransformOrigin(0f, 0.5f) }
                        .clip(RoundedCornerShape(8.dp))
                        .clickable { Controls.seek(line.time) }
                        .padding(vertical = 12.dp),
                )
            }
        }
    }
}

@OptIn(androidx.compose.foundation.ExperimentalFoundationApi::class)
@Composable
private fun UpNextPanel(version: ULong) {
    val songsLoad = rememberLoaded(version) { Controls.upNext() }
    val songs by songsLoad
    val list = songs.orEmpty()
    val onComputer = list.firstOrNull()?.onComputer() == true
    // Up next on this phone can be put in order by dragging a song's handle.
    val reorder = rememberReorder(list.size) { from, to -> core.moveUpNext(from.toUInt(), to.toUInt()) }
    Column(Modifier.fillMaxHeight()) {
        Text("Up next", style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.Bold, modifier = Modifier.padding(vertical = 8.dp))
        if (songs != null && list.isEmpty()) {
            Text("Nothing up next", color = Color.White.copy(alpha = 0.6f))
            return@Column
        }
        LazyColumn(Modifier.fillMaxSize()) {
            itemsIndexed(list, key = { i, s -> "$i-${s.id}" }) { i, song ->
                // Swiped away, a song leaves the queue.
                val dismiss = rememberSwipeToDismissBoxState()
                LaunchedEffect(dismiss.currentValue) {
                    if (dismiss.currentValue != SwipeToDismissBoxValue.Settled && !song.onComputer()) core.removeUpNext(i.toUInt())
                }
                SwipeToDismissBox(
                    modifier = if (onComputer) Modifier else Modifier.reorderRow(reorder, i),
                    state = dismiss,
                    // The computer's queue cannot be changed from here, so its rows do not swipe.
                    enableDismissFromStartToEnd = !onComputer,
                    enableDismissFromEndToStart = !onComputer,
                    backgroundContent = {
                        // Only while a row is being swiped; the rows themselves are see-through.
                        if (dismiss.dismissDirection != SwipeToDismissBoxValue.Settled) Box(
                            Modifier.fillMaxSize().clip(RoundedCornerShape(8.dp)).background(Color.White.copy(alpha = 0.12f)).padding(horizontal = 16.dp),
                            contentAlignment = Alignment.CenterEnd,
                        ) { Text("Remove", color = Color.White.copy(alpha = 0.8f)) }
                    },
                ) {
                    // Held (or OK held on a remote), a menu moves the song or takes it out: the
                    // swipe and the handle are for touch alone. TalkBack offers the same.
                    var menu by remember { mutableStateOf(false) }
                    val move: (Int) -> Unit = { to -> core.moveUpNext(i.toUInt(), to.toUInt()) }
                    Box {
                    Row(
                        Modifier
                            .fillMaxWidth()
                            .clip(RoundedCornerShape(8.dp))
                            .combinedClickable(onClick = { Controls.jump(i, song) }, onLongClick = { if (!onComputer) menu = true })
                            .semantics {
                                if (!onComputer) customActions = listOfNotNull(
                                    if (i > 0) CustomAccessibilityAction("Move up") { move(i - 1); true } else null,
                                    if (i < list.size - 1) CustomAccessibilityAction("Move down") { move(i + 1); true } else null,
                                    CustomAccessibilityAction("Remove") { core.removeUpNext(i.toUInt()); true },
                                )
                            }
                            .padding(vertical = 6.dp),
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(12.dp),
                    ) {
                        Cover(song.artwork, Modifier.size(44.dp), SmallCoverShape, seed = song.album + song.artist)
                        Column(Modifier.weight(1f)) {
                            Text(song.title, style = MaterialTheme.typography.bodyLarge, maxLines = 1, overflow = TextOverflow.Ellipsis)
                            Text(song.artist, style = MaterialTheme.typography.bodyMedium, color = Color.White.copy(alpha = 0.6f), maxLines = 1, overflow = TextOverflow.Ellipsis)
                        }
                        if (!onComputer) DragHandle(reorder, i, tint = Color.White.copy(alpha = 0.7f))
                    }
                    androidx.compose.material3.DropdownMenu(menu, { menu = false }, shape = RoundedCornerShape(20.dp)) {
                        if (i > 0) androidx.compose.material3.DropdownMenuItem(text = { Text("Move up") }, onClick = { menu = false; move(i - 1) })
                        if (i < list.size - 1) androidx.compose.material3.DropdownMenuItem(text = { Text("Move down") }, onClick = { menu = false; move(i + 1) })
                        androidx.compose.material3.DropdownMenuItem(text = { Text("Remove") }, onClick = { menu = false; core.removeUpNext(i.toUInt()) })
                    }
                    }
                }
            }
        }
    }
}

/** Play and pause as one button for TalkBack: "Play" or "Pause", not a checked box. */
private fun Modifier.playPauseSemantics(playing: Boolean, toggle: () -> Unit) = clearAndSetSemantics {
    role = androidx.compose.ui.semantics.Role.Button
    contentDescription = if (playing) "Pause" else "Play"
    onClick { toggle(); true }
}

/** Play and pause, turning into each other. */
@Composable
private fun PlayPauseIcon(playing: Boolean, size: Dp) {
    AnimatedContent(
        playing,
        transitionSpec = { (scaleIn(tween(180), 0.6f) + fadeIn(tween(180))) togetherWith (scaleOut(tween(140), 0.6f) + fadeOut(tween(140))) },
        label = "play pause",
    ) { on ->
        Icon(
            if (on) Icons.Rounded.Pause else Icons.Rounded.PlayArrow,
            contentDescription = if (on) "Pause" else "Play",
            modifier = Modifier.size(size),
        )
    }
}
