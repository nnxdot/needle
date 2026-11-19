package fyi.nnx.needle.ui

import android.os.Build
import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.animateDpAsState
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.tween
import androidx.compose.animation.core.spring
import androidx.compose.animation.core.snap
import androidx.compose.foundation.interaction.collectIsDraggedAsState
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
import androidx.compose.foundation.layout.offset
import androidx.compose.material.icons.rounded.KeyboardArrowDown
import kotlinx.coroutines.launch
import dev.chrisbanes.haze.hazeSource
import androidx.compose.material.icons.rounded.MoreHoriz
import androidx.compose.material.icons.rounded.Add
import androidx.compose.material.icons.rounded.Person
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.graphics.drawscope.scale as scaleBy
import androidx.compose.ui.graphics.vector.ImageVector
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
import androidx.compose.foundation.border
import androidx.compose.material.icons.rounded.Close
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.heading
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
import androidx.compose.material.icons.rounded.IosShare
import androidx.compose.material.icons.rounded.Translate
import androidx.compose.material.icons.rounded.GraphicEq
import androidx.compose.material.icons.rounded.Fullscreen
import androidx.compose.material.icons.rounded.Album
import androidx.compose.material.icons.automirrored.rounded.PlaylistAdd
import androidx.compose.material.icons.rounded.Extension
import androidx.compose.material.icons.rounded.Radio
import androidx.compose.material.icons.rounded.Speed
import androidx.compose.ui.layout.positionInWindow
import androidx.compose.ui.layout.positionInParent
import androidx.compose.ui.layout.onGloballyPositioned
import androidx.compose.material.icons.rounded.Bedtime
import androidx.compose.material.icons.outlined.Bedtime
import androidx.compose.material.icons.rounded.Image
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
import androidx.compose.foundation.layout.RowScope
import androidx.compose.runtime.derivedStateOf
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.wrapContentHeight
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.layout.only
import androidx.compose.ui.unit.sp
import androidx.compose.foundation.layout.displayCutout
import androidx.compose.foundation.layout.navigationBars
import androidx.compose.foundation.layout.statusBars
import androidx.compose.foundation.layout.union
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
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.hoverable
import androidx.compose.foundation.interaction.collectIsHoveredAsState
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
fun MiniPlayer(onOpen: () -> Unit, docked: Boolean = false) {
    // The position is read only by what shows it (AtPosition), not by the whole bar.
    val playbackState = NeedleApp.instance.playback.collectAsState()
    val current by remember { derivedStateOf { playbackState.value?.current } }
    val playing by remember { derivedStateOf { playbackState.value?.playing == true } }
    // The same object while the song is the same: what takes it can then skip a redraw. The last
    // song stays while a new list of songs has none yet (a moment), so the bar does not blink.
    var last by remember { mutableStateOf<Song?>(null) }
    val song = remember(current) { current } ?: last ?: return
    androidx.compose.runtime.SideEffect { if (current != null) last = current }
    val app by NeedleApp.instance.app.collectAsState()
    CoverTheme(if (app.miniPlayerColored) song.artwork else null) { MiniPlayerBody(playing, song, onOpen, app.miniPlayerColored, app.miniPlayerLyrics, docked) }
}

@OptIn(androidx.compose.material3.ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun MiniPlayerBody(playing: Boolean, song: Song, onOpen: () -> Unit, colored: Boolean, lyrics: Boolean, docked: Boolean) {
    val haptics = rememberHaptics()
    // Swiped sideways, it skips: left for the next song, right for the one before. Thrown, it
    // goes on that way, and the new song comes in from the other side.
    val swipe = rememberSkipSwipe(haptics)
    LaunchedEffect(song.id) { swipe.arrived() }
    // In the dock (DESIGN.md) it sits on the dock's glass, which gives it its fill and shadow;
    // its Play/Pause and progress keep the cover's colours.
    Surface(
        color = when {
            docked -> Color.Transparent
            colored -> MaterialTheme.colorScheme.primaryContainer
            else -> MaterialTheme.colorScheme.surfaceContainerHigh
        },
        contentColor = if (colored && !docked) MaterialTheme.colorScheme.onPrimaryContainer else MaterialTheme.colorScheme.onSurface,
        // In the dock, its corners follow the dock's (26 dp, 4 dp inside it).
        shape = RoundedCornerShape(if (docked) 22.dp else 20.dp),
        shadowElevation = if (docked) 0.dp else 8.dp,
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = if (docked) 4.dp else 10.dp, vertical = if (docked) 4.dp else 8.dp)
            .onSizeChanged { swipe.width = it.width.toFloat().coerceAtLeast(1f) }
            // Read only where it is drawn (graphicsLayer): the bar is not redrawn at each move.
            .graphicsLayer {
                val shift = swipe.shift
                translationX = shift
                alpha = 1f - (kotlin.math.abs(shift) / swipe.width).coerceIn(0f, 1f) * 0.9f
            }
            .skipSwipe(swipe)
            .clip(RoundedCornerShape(if (docked) 22.dp else 20.dp))
            .clickable(onClickLabel = "Open the player", onClick = onOpen),
    ) {
        // The line being sung (Settings › Appearance › Lyrics in the mini player): on a wide
        // bar, in the room between the song and the buttons; on a narrow one, in one line under.
        val read = if (lyrics) miniLyricLines(song) else emptyList()
        val lines = read.orEmpty()
        // While the next song's lyrics are read, its line keeps the room the last one had ("♪"):
        // the dock does not shrink and grow again at each new song, nor the page under it move.
        var hadLines by remember { mutableStateOf(false) }
        androidx.compose.runtime.SideEffect { if (read != null) hadLines = read.isNotEmpty() }
        val showsLine = read?.isNotEmpty() ?: hadLines
        val wide = isWide()
        val inBar = wide && showsLine
        val under = !wide && showsLine
        val info: @Composable (Modifier) -> Unit = { modifier ->
            Row(modifier, verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
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
            }
        }
        val buttons: @Composable () -> Unit = {
            // A tablet's bar has room for the song before as well.
            if (wide) {
                val press = remember { androidx.compose.foundation.interaction.MutableInteractionSource() }
                IconButton(onClick = { Controls.previous() }, interactionSource = press, modifier = Modifier.pressScale(press, ButtonPress)) {
                    Icon(Icons.Rounded.SkipPrevious, contentDescription = "Previous", modifier = Modifier.size(30.dp))
                }
            }
            // A round amber Play/Pause, as on the Mac.
            val playPress = remember { androidx.compose.foundation.interaction.MutableInteractionSource() }
            androidx.compose.material3.FilledIconButton(
                onClick = { haptics(false); Controls.toggle() },
                shape = CircleShape,
                colors = androidx.compose.material3.IconButtonDefaults.filledIconButtonColors(
                    containerColor = MaterialTheme.colorScheme.primary,
                    contentColor = MaterialTheme.colorScheme.onPrimary,
                ),
                interactionSource = playPress,
                modifier = Modifier.pressScale(playPress, ButtonPress).playPauseSemantics(playing) { haptics(false); Controls.toggle() },
            ) { PlayPauseIcon(playing, 26.dp) }
            val nextPress = remember { androidx.compose.foundation.interaction.MutableInteractionSource() }
            IconButton(onClick = { Controls.next() }, interactionSource = nextPress, modifier = Modifier.pressScale(nextPress, ButtonPress)) {
                Icon(Icons.Rounded.SkipNext, contentDescription = "Next", modifier = Modifier.size(30.dp))
            }
            // A tablet's floating player opens the side pane: the lyrics, or what is up next.
            if (docked && wide) {
                val pane by Ui.pane.collectAsState()
                SideKind.entries.forEach { kind ->
                    val on = pane == kind
                    IconButton(onClick = { Ui.pane.value = if (on) null else kind }, modifier = Modifier.semantics { selected = on }) {
                        Icon(
                            if (kind == SideKind.Lyrics) Icons.Rounded.Lyrics else Icons.AutoMirrored.Rounded.QueueMusic,
                            contentDescription = if (kind == SideKind.Lyrics) "Lyrics" else "Up next",
                            tint = if (on) MaterialTheme.colorScheme.primary else LocalContentColor.current,
                        )
                    }
                }
            }
        }
        Box {
            Column {
                Row(
                    Modifier.padding(start = 8.dp, end = 4.dp, top = 8.dp, bottom = if (under) 0.dp else 8.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(12.dp),
                ) {
                    if (inBar) {
                        // The buttons take the room they need (a floating player's five never
                        // squeezed); the song and the line share the rest, the line in its middle.
                        info(Modifier.weight(1f))
                        Box(Modifier.weight(1.2f), contentAlignment = Alignment.Center) {
                            AtPosition { at -> MiniLyrics(lines, at, big = true) }
                        }
                        Row(
                            horizontalArrangement = Arrangement.spacedBy(12.dp, Alignment.End),
                            verticalAlignment = Alignment.CenterVertically,
                        ) { buttons() }
                    } else {
                        info(Modifier.weight(1f))
                        buttons()
                    }
                }
                // Close under the song, and clear of the progress wave at the foot.
                if (under) AtPosition { at -> MiniLyrics(lines, at, big = false, Modifier.padding(start = 16.dp, end = 16.dp, bottom = 8.dp)) }
            }
            // Progress along the foot: a wave while it plays, flat while it rests (Android 16).
            val line = MaterialTheme.colorScheme.primary
            val track = androidx.compose.material3.LocalContentColor.current.copy(alpha = 0.15f)
            val at = Modifier.align(Alignment.BottomCenter).padding(horizontal = 14.dp).height(6.dp)
            AtPosition { position ->
                val progress = if (song.duration > 0) (position / song.duration).toFloat().coerceIn(0f, 1f) else 0f
                ProgressLine(progress, playing, line, track, at, big = false)
            }
        }
    }
}

/**
 * The timed lines of the mini player's song, for its line of lyrics (Settings › Appearance ›
 * Lyrics): none for a song without timed lyrics, and then nothing shows. Null while they are
 * read.
 */
@Composable
private fun miniLyricLines(song: Song): List<fyi.nnx.needle.core.LyricLine>? {
    // Not rememberLoaded: a song without lyrics should not offer Retry over the page.
    // Read again when the library changes too (lyrics just timed or saved).
    val version by NeedleApp.instance.libraryVersion.collectAsState()
    val lines by androidx.compose.runtime.produceState<List<fyi.nnx.needle.core.LyricLine>?>(null, song.id, version) {
        value = withContext(Dispatchers.IO) {
            runCatching {
                if (song.onComputer()) {
                    core.pcLyrics(fyi.nnx.needle.core.PcSong(song.id.removePrefix(PcPrefix), song.title, song.artist, song.album, song.duration, song.format, song.artwork))
                } else core.lyrics(song.id)
            }.getOrNull()?.lines.orEmpty()
        }
    }
    return lines
}

/**
 * The line of the lyrics playing, in one line, cut with … when long: in the middle of a
 * tablet's mini player ([big]), where it can be read from across the room, or under a phone's.
 */
@Composable
private fun MiniLyrics(lines: List<fyi.nnx.needle.core.LyricLine>, position: Double, big: Boolean, modifier: Modifier = Modifier, start: Boolean = false) {
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
            style = if (big) MaterialTheme.typography.titleMedium else MaterialTheme.typography.bodyMedium,
            fontWeight = FontWeight.Bold,
            textAlign = if (start) androidx.compose.ui.text.style.TextAlign.Start else androidx.compose.ui.text.style.TextAlign.Center,
            maxLines = 1,
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

/**
 * Behind the player (DESIGN.md › The player): the cover far out of focus under a veil of its own
 * colour, a little darker toward the edges; calm, not bright. Where it cannot blur (Android 11
 * and older, or "Cover behind the player" off), the cover's colour, flat.
 */
@Composable
internal fun PlayerBackdrop(song: Song, reading: Boolean = false) {
    // The cover's colour only here, where it shows: it blends at each song change.
    val tone = coverColor(song.artwork)
    Box(Modifier.fillMaxSize().background(tone)) {
        val app by NeedleApp.instance.app.collectAsState()
        if (app.coverBackdrop && Build.VERSION.SDK_INT >= 31 && song.artwork != null) {
            CoverBackdrop(song.artwork, Modifier.fillMaxSize(), dim = 0.14f, fade = false, player = true)
            // The veil: the cover's colour, more of it toward the foot, where the words are.
            Box(Modifier.fillMaxSize().background(Brush.verticalGradient(listOf(tone.copy(alpha = 0.34f), tone.copy(alpha = 0.66f)))))
        }
        // Darker toward the edges: a wide oval, lightest a little above the middle.
        Box(
            Modifier.fillMaxSize().drawBehind {
                // Nothing to shade while it has no size (a gradient of no radius cannot be drawn).
                if (size.width < 1f || size.height < 1f) return@drawBehind
                val ry = size.height * 0.9f
                val rx = size.width * 1.2f
                val center = androidx.compose.ui.geometry.Offset(size.width / 2, size.height * 0.36f)
                val edge = Brush.radialGradient(0.3f to Color.Transparent, 1f to Color(0xFF080A0E).copy(alpha = 0.38f), center = center, radius = ry)
                // A round gradient, pulled out sideways into the oval.
                val sx = rx / ry
                scaleBy(sx, 1f, pivot = center) {
                    drawRect(edge, topLeft = androidx.compose.ui.geometry.Offset(center.x - size.width / (2 * sx), 0f), size = androidx.compose.ui.geometry.Size(size.width / sx, size.height))
                }
            },
        )
        // Darker while the lyrics show: their small white words sit over the bright middle.
        val shade by animateFloatAsState(if (reading) 0.3f else 0f, label = "reading shade")
        Box(Modifier.fillMaxSize().graphicsLayer { alpha = shade }.background(Color.Black))
    }
}

private enum class Panel { Cover, Lyrics, UpNext, Visualizer }

/** The player's glass sheets (DESIGN.md › The player): More, and Up next on a phone. */
private enum class PlayerSheet { More, UpNext }

/**
 * The player's layouts: a phone standing up (Tall), a short window standing up (Short), a phone
 * on its side (Sideways), a tablet or an open foldable (Wide), and a foldable standing as a
 * laptop (Tabletop).
 */
private enum class PlayerLayout { Tall, Short, Sideways, Wide, Tabletop }

/**
 * What lies behind the player: the backdrop alone, or on it the immersive cover: across the top
 * (a phone), across the top of a wide player's area (the phone's layout there), or down its left
 * (side by side).
 */
private enum class Behind { Backdrop, Top, AreaTop, Left }

/**
 * The wide player's side pane: open or closed, and its tab (Up next or Lyrics), kept on the phone
 * as the listener left them (not in Settings), through a turn, a fold, and a new start. Open on
 * Up next the first time.
 */
private object PaneChoice {
    private val prefs get() = NeedleApp.instance.getSharedPreferences("player", android.content.Context.MODE_PRIVATE)
    val open = kotlinx.coroutines.flow.MutableStateFlow(prefs.getBoolean("pane_open", true))
    val tab = kotlinx.coroutines.flow.MutableStateFlow(if (prefs.getString("pane_tab", null) == SideKind.Lyrics.name) SideKind.Lyrics else SideKind.Queue)

    fun set(open: Boolean, tab: SideKind) {
        this.open.value = open
        this.tab.value = tab
        prefs.edit().putBoolean("pane_open", open).putString("pane_tab", tab.name).apply()
    }
}

/** A wide player's parts start this far under the status bar: under the round glass buttons. */
private val WideTop = 62.dp

/**
 * The player in a short window on its side (a phone turned sideways, about 410 dp tall): the song
 * and its controls a little smaller, so they all fit at the right without scrolling.
 */
private val LocalCompactPlayer = androidx.compose.runtime.compositionLocalOf { false }

/** The player's glass (Glass.kt): a faint white over its blurred cover; where it cannot blur, a little more. */
private val PlayerGlassFill = Color.White.copy(alpha = 0.09f)
private val PlayerGlassSolid = Color.White.copy(alpha = 0.13f)

/** The buttons of the main row; the rest of the chosen ones (heart, lyrics, Up next, Play on) have their own places. */
private val MainRowButtons = MainRowPlayerButtons.toSet()

/**
 * The least room the song's column needs beside the cover on a wide screen: the five main
 * buttons with the widest Play (the Wide and Trio styles), and a little air.
 */
private val BesideRoom = 344.dp

@Composable
fun FullPlayer(onClose: () -> Unit, open: (Route) -> Unit, onThrown: () -> Unit = onClose) {
    // Only what changes with the song or a press is read here. The position is read where it
    // shows (AtPosition), so the whole player is not redrawn five times a second as it plays.
    val playbackState = NeedleApp.instance.playback.collectAsState()
    val current by remember { derivedStateOf { playbackState.value?.current } }
    val playing by remember { derivedStateOf { playbackState.value?.playing == true } }
    val repeat by remember { derivedStateOf { playbackState.value?.repeat ?: fyi.nnx.needle.core.RepeatMode.OFF } }
    val queueVersion by remember { derivedStateOf { playbackState.value?.queueVersion ?: 0uL } }
    val upNextCount by remember { derivedStateOf { playbackState.value?.upNext?.toInt() ?: 0 } }
    val networkOutput by remember { derivedStateOf { playbackState.value?.networkOutput == true } }
    // Why the song stopped, such as a music server that stopped answering: shown until it plays again.
    val stopped by remember { derivedStateOf { playbackState.value?.error?.takeIf { playbackState.value?.playing != true } } }
    // The same object while the song is the same: what takes it can then skip a redraw.
    val song = remember(current) { current } ?: return
    val position = { playbackState.value?.position ?: 0.0 }
    var panel by rememberSaveable { mutableStateOf(Panel.Cover) }
    val haptics = rememberHaptics()
    // Read again when the library changes, so stars given from the song's menu show here.
    val version by NeedleApp.instance.libraryVersion.collectAsState()
    // Kept through a change in the library (it would go back to the song's old rating for a
    // moment), and read again then.
    var favorite by remember(song.id) { mutableStateOf(song.rating >= 4) }
    var hearting by remember { mutableStateOf(false) }
    // The library's own answer, read off the main thread.
    LaunchedEffect(song.id, version) {
        withContext(kotlinx.coroutines.Dispatchers.IO) { runCatching { core.isFavorite(song.id) }.getOrNull() }?.let { favorite = it }
    }
    // Settings › Appearance › Player › Keep the screen on: while the player shows, lyrics or not.
    val keepOn by NeedleApp.instance.app.collectAsState()
    KeepScreenOn("player", keepOn.keepScreenOnPlayer)
    val toggleFavorite: () -> Unit = {
        // One at a time: two taps at once would each read the old state, and the second undo nothing.
        if (!hearting) {
            hearting = true
            haptics(false)
            // The heart changes at once; the library's answer follows (or puts it back).
            val was = favorite
            favorite = !was
            // Off the main thread: it writes to the library, which waits while a scan writes too.
            // On the app's scope: the player may close before the library answers.
            NeedleApp.instance.scope.launch(kotlinx.coroutines.Dispatchers.Main) {
                withContext(kotlinx.coroutines.Dispatchers.IO) { runCatching { core.toggleFavorite(song.id) } }
                    .onSuccess { favorite = it; showMessage(if (it) "Added to favorites" else "Taken out of favorites") }
                    .onFailure { favorite = was; showMessage("Could not change favorites: ${it.message}") }
                hearting = false
                NeedleApp.instance.libraryChanged()
            }
        }
    }
    // The A-B loop: the first tap marks A, the second B, the third lets go.
    var loopStart by remember(song.id) { mutableStateOf<Double?>(null) }
    // Read for each song and after each change of the loop: a moment after, as the player
    // takes the change in on its own thread. The computer's player has no loop.
    var loopChanged by remember { androidx.compose.runtime.mutableIntStateOf(0) }
    val looping by androidx.compose.runtime.produceState(emptyList<Double>(), song.id, loopChanged) {
        if (song.onComputer()) {
            value = emptyList()
            return@produceState
        }
        if (loopChanged > 0) kotlinx.coroutines.delay(80)
        value = withContext(kotlinx.coroutines.Dispatchers.IO) { runCatching { core.loopRange() }.getOrDefault(emptyList()) }
    }
    val onLoop: () -> Unit = {
        when {
            // The loop is the phone's player's own; the computer's has none.
            song.onComputer() -> showMessage("The A-B loop works for songs playing on this phone")
            looping.isNotEmpty() -> { core.setLoop(null, null); loopStart = null; loopChanged++; showMessage("Loop off") }
            loopStart == null -> { val at = position(); loopStart = at; showMessage("Loop from ${time(at)}: choose A-B loop again where it ends") }
            else -> {
                // B may be before A (sought back): the loop is between them either way.
                val a = minOf(loopStart!!, position())
                val b = maxOf(loopStart!!, position())
                if (b - a < 0.5) {
                    showMessage("Too short to loop: choose it again further on")
                } else {
                    core.setLoop(a, b)
                    loopChanged++
                    showMessage("Looping ${time(a)} to ${time(b)}")
                    loopStart = null
                }
            }
        }
    }
    val loopText = when {
        looping.size >= 2 -> "${time(looping[0])} to ${time(looping[1])}"
        loopStart != null -> "From ${time(loopStart!!)}"
        else -> "Off"
    }
    // The sleep timer, lit in the pill while it runs. The computer's player has none.
    val sleeping = if (song.onComputer()) null else rememberSleepTimer()
    val openSleep: () -> Unit = {
        if (song.onComputer()) showMessage("The sleep timer works for songs playing on this phone") else Ui.sheet.value = Sheet.SleepTimer
    }
    // The timed lines of the song, for the lyric line under the title. None for a song without
    // timed lyrics: the line goes, and the parts close up. While the next song's are read, the
    // line keeps the room the last one had, so nothing moves twice.
    val timedLines = miniLyricLines(song)
    var hadLines by remember { mutableStateOf(false) }
    androidx.compose.runtime.SideEffect { if (timedLines != null) hadLines = timedLines.isNotEmpty() }
    val hasLine = timedLines?.isNotEmpty() ?: hadLines
    // The Lyrics look shows its song small at the top, while it has words to show.
    var bigHas by remember(song.id) { mutableStateOf(false) }
    var sheet by remember { mutableStateOf<PlayerSheet?>(null) }
    // A wide screen's pane (Up next, Lyrics): open or closed, and its tab, kept on the phone.
    val paneTab by PaneChoice.tab.collectAsState()
    val paneOpen by PaneChoice.open.collectAsState()
    // Pulled down far enough, or thrown down, the player goes on down at the finger's speed and
    // closes. Pulled up, it stretches a little (a rubber band).
    var pull by remember { mutableFloatStateOf(0f) }
    var searching by remember { mutableStateOf(false) }
    var dragging by remember { mutableStateOf(false) }
    // Let go short of closing, the player springs back up.
    // Read only where it is drawn (graphicsLayer): the player is not redrawn at each move.
    val shown = remember { androidx.compose.animation.core.Animatable(0f) }
    var tall by remember { mutableFloatStateOf(1f) }
    val closer = androidx.compose.runtime.rememberCoroutineScope()
    val density = androidx.compose.ui.platform.LocalDensity.current
    val stillPlayer = reduceMotion()
    val stretch = with(density) { 96.dp.toPx() }
    val pulled: (Float) -> Float = { if (it < 0f) rubberBand(it, stretch) else it }
    val letGo: (Float) -> Unit = { velocity ->
        val from = pulled(pull)
        val down = with(density) { flickWay(from, velocity, tall) } > 0
        closer.launch {
            // Taken over where the finger left it, in the same frame: no flash back to the top.
            shown.snapTo(from)
            dragging = false
            pull = 0f
            when {
                // Reduce motion: the player fades (Root.kt).
                down && stillPlayer -> onClose()
                down -> {
                    var gone = false
                    shown.animateTo(tall, flickSpring(), initialVelocity = velocity.coerceAtLeast(0f)) {
                        // All but off the screen: the rest of the way is not worth waiting for.
                        if (!gone && value >= tall * 0.97f) {
                            gone = true
                            onThrown()
                        }
                    }
                    if (!gone) onThrown()
                    // Still here (a foldable held as a book keeps the player at its right): back up.
                    kotlinx.coroutines.delay(300)
                    shown.animateTo(0f, snapBackSpring())
                }
                stillPlayer -> shown.snapTo(0f)
                else -> shown.animateTo(0f, snapBackSpring(), initialVelocity = velocity)
            }
        }
    }
    androidx.compose.runtime.DisposableEffect(Unit) {
        playerShowing.value = true
        onDispose { playerShowing.value = false }
    }

    CoverTheme(song.artwork, dark = true) {
    // What the player's own glass blurs: its backdrop (and the immersive cover on it).
    val playerHaze = dev.chrisbanes.haze.rememberHazeState()
    BoxWithConstraints(
        Modifier
            .fillMaxSize()
            .onSizeChanged { tall = it.height.toFloat().coerceAtLeast(1f) }
            .graphicsLayer { translationY = if (dragging) pulled(pull) else shown.value }
            .clickable(enabled = false) {}
            .pointerInput(Unit) {
                detectFlick(
                    horizontal = false,
                    // Caught on its way back: it goes on from where it is.
                    onStart = {
                        pull = shown.value
                        dragging = true
                        closer.launch { shown.stop() }
                    },
                    onCancel = { letGo(0f) },
                    onEnd = letGo,
                ) { _, dy -> pull += dy }
            },
    ) {
        val look by NeedleApp.instance.app.collectAsState()
        // The big player's look (NEEDLE-2.0.md): Cover, Record, Lyrics, or Visualizer, in the
        // cover's place; chosen in More.
        val choices by NeedleApp.instance.more.collectAsState()
        val bigLook = choices.bigLook.takeIf { it in BigLooks } ?: "cover"
        val hinge = LocalHinge.current
        // Read from the whole player (a foldable held as a book gives it the right half only).
        val layout = when {
            hinge?.tabletop == true -> PlayerLayout.Tabletop
            maxWidth > maxHeight && maxWidth >= 600.dp && maxHeight >= 480.dp -> PlayerLayout.Wide
            maxWidth > maxHeight && maxHeight < 540.dp -> PlayerLayout.Sideways
            maxHeight < 540.dp -> PlayerLayout.Short
            else -> PlayerLayout.Tall
        }
        val wideWidth = maxWidth
        val wideHeight = maxHeight
        // Opened out (a foldable unfolded) while the lyrics showed: they go on in the pane.
        LaunchedEffect(layout) {
            if (layout == PlayerLayout.Wide && panel == Panel.Lyrics) {
                panel = Panel.Cover
                PaneChoice.set(true, SideKind.Lyrics)
            }
        }
        // Settings › Appearance › Immersive cover. On a phone: the cover sharp at the top, edge
        // to edge, going out into the backdrop. On a wide screen: the cover across the top of
        // the player's area, or down its left (see Behind).
        val immersive = look.immersiveCover && panel == Panel.Cover && bigLook == "cover" && layout == PlayerLayout.Tall
        val wideImmersive = look.immersiveCover && layout == PlayerLayout.Wide
        val still = reduceMotion()
        // A wide screen's pane, in each view: open or closed as the listener left it.
        val paneShown = paneOpen
        val paneWidth = (wideWidth * 0.27f).coerceIn(260.dp, 380.dp)
        // Lyrics or Up next asked for (the pill's icons, or the round buttons where there is no
        // pill): the pane opens on it, changes its tab, or closes (a press on the lit one).
        val showPane: (SideKind) -> Unit = { kind ->
            if (paneOpen && paneTab == kind) PaneChoice.set(false, kind) else PaneChoice.set(true, kind)
        }
        // The player's area on a wide screen: the window less the pane, with the room round it,
        // while the pane shows. Nothing of the player sits under the pane.
        val sides = androidx.compose.foundation.layout.WindowInsets.navigationBars.union(androidx.compose.foundation.layout.WindowInsets.displayCutout)
        val leftInset = with(density) { sides.getLeft(density, androidx.compose.ui.unit.LayoutDirection.Ltr).toDp() }
        val rightInset = with(density) { sides.getRight(density, androidx.compose.ui.unit.LayoutDirection.Ltr).toDp() }
        val areaWidth = if (paneShown) (wideWidth - rightInset - 20.dp - paneWidth - 20.dp).coerceAtLeast(0.dp) else wideWidth
        // Taller than wide, or almost square (an unfolded phone or a tablet on its side beside
        // its pane): the phone's layout in the area. Clearly wider (1.1 of its height, as on the
        // iPad): the cover at the left and the song beside it. Under 600 dp no cover leaves the
        // song's column the room its buttons need (BesideRoom).
        val tallArea = areaWidth < maxHeight * 1.1f || areaWidth < 600.dp
        // The immersive cover itself (not another look) on a wide screen, behind the player.
        val pictureOn = wideImmersive && panel == Panel.Cover && bigLook == "cover"
        // Side by side, it fills the left from top to foot: as wide as the player is tall (a
        // square), but between 40 and 58 % of the area (cut at its sides or top to fill).
        // Never so wide that the song's column has less than BesideRoom (a TV beside its pane).
        val pictureWidth = maxHeight.coerceIn(areaWidth * 0.4f, areaWidth * 0.58f)
            .coerceAtMost((areaWidth - 76.dp - BesideRoom).coerceAtLeast(180.dp))
        // The phone's layout in the area: the cover across its top, down to the song's name: the
        // status bar, the round buttons' row (WideTop), and the room above the name, measured in
        // its own column (so a pull down, which moves the whole player, changes nothing).
        var roomFoot by remember { mutableFloatStateOf(-1f) }
        val statusTop = with(density) { androidx.compose.foundation.layout.WindowInsets.statusBars.getTop(density).toDp() }
        val pictureTall = if (roomFoot < 0f) maxOf(areaWidth, maxHeight * 0.56f) else statusTop + WideTop + with(density) { roomFoot.toDp() }.coerceAtLeast(120.dp)
        val behind = when {
            immersive -> Behind.Top
            pictureOn && tallArea -> Behind.AreaTop
            pictureOn -> Behind.Left
            else -> Behind.Backdrop
        }
        // The immersive cover follows a sideways swipe over its room, as the cover does.
        val coverSwipe = rememberSkipSwipe(haptics)
        LaunchedEffect(song.id) { coverSwipe.arrived() }
        Box(Modifier.fillMaxSize().hazeSource(playerHaze)) {
            androidx.compose.animation.Crossfade(behind, label = "immersive cover") { lies ->
                val moving = look.animatedCovers && !still
                when (lies) {
                    Behind.Top -> ImmersiveCover(song, animated = moving, swipe = coverSwipe)
                    // On under the pane (its glass blurs it) and fading out there, with no hard edge.
                    Behind.AreaTop -> ImmersiveCover(song, animated = moving, swipe = coverSwipe, width = areaWidth + 20.dp + 96.dp, height = pictureTall, fadeEnd = 116.dp)
                    Behind.Left -> ImmersiveCover(song, animated = moving, swipe = coverSwipe, width = pictureWidth, sideways = true)
                    Behind.Backdrop -> PlayerBackdrop(song, reading = panel != Panel.Cover)
                }
            }
        }
        val glassTint = glassCover(song.artwork)
        CompositionLocalProvider(LocalContentColor provides Color.White, LocalHaze provides playerHaze) {
            // What comes and goes with the view (the small song over the lyrics, the name under
            // the cover, the lyric line): it opens or closes its room, so the rest moves instead
            // of jumping.
            val comes = if (still) fadeIn(tween(150)) else fadeIn(tween(200)) + androidx.compose.animation.expandVertically(spring(dampingRatio = 1f, stiffness = 400f))
            val goes = if (still) fadeOut(tween(120)) else fadeOut(tween(120)) + androidx.compose.animation.shrinkVertically(spring(dampingRatio = 1f, stiffness = 400f))
            val lyricsLook = bigLook == "lyrics" && panel == Panel.Cover
            // The song small at the top: while the lyrics take the cover's place.
            val showsHead = panel == Panel.Lyrics || (lyricsLook && bigHas)
            // The lyric line: not while the lyrics show anyway.
            val showsLine = hasLine && panel != Panel.Lyrics && !lyricsLook
            val openLyrics: () -> Unit = {
                if (layout == PlayerLayout.Wide) PaneChoice.set(true, SideKind.Lyrics) else panel = Panel.Lyrics
            }
            // The two round glass buttons at the top (close, the menu), and between them what a
            // look or a layout adds (full screen; on a wide card, Lyrics and Up next).
            val topBar: @Composable (@Composable () -> Unit) -> Unit = { extra ->
                Row(
                    Modifier.fillMaxWidth().padding(start = 14.dp, end = 14.dp, top = 4.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(4.dp),
                ) {
                    GlassRound(Icons.Rounded.KeyboardArrowDown, "Close the player", glassTint, onClick = onClose)
                    Spacer(Modifier.weight(1f))
                    extra()
                    if (bigLook == "visualizer" && panel == Panel.Cover) GlassRound(Icons.Rounded.Fullscreen, "Full screen", glassTint) { Ui.fullVisualizer.value = true }
                    GlassRound(Icons.Rounded.MoreHoriz, "More", glassTint, on = sheet == PlayerSheet.More) { sheet = PlayerSheet.More }
                }
            }
            // The cover, or in its place the record, the lyrics, or the light: one fades into the other.
            val stage: @Composable (Alignment) -> Unit = { align ->
                AnimatedContent(
                    panel,
                    Modifier.fillMaxSize(),
                    transitionSpec = { (fadeIn(tween(200)) togetherWith fadeOut(tween(120))).using(null) },
                    contentAlignment = align,
                    label = "panel",
                ) { shownPanel ->
                    when (shownPanel) {
                        // Immersive, the cover is behind: here only the room for it, which still
                        // skips when swiped sideways.
                        Panel.Cover -> when {
                            immersive || pictureOn -> SwipeSpace(look.swipeCover, coverSwipe)
                            bigLook == "record" -> PlayerCover(song, playing, haptics, record = true)
                            bigLook == "lyrics" -> BigLyrics(song, position, { open(Route.Timing(song.id)) }, { bigHas = it }) { PlayerCover(song, playing, haptics) }
                            // The bars stand on the foot of the room, over the title.
                            bigLook == "visualizer" -> Visualizer(Modifier.fillMaxSize().padding(top = 24.dp, bottom = 8.dp, start = 6.dp, end = 6.dp), bars = true)
                            else -> PlayerCover(song, playing, haptics)
                        }
                        Panel.Lyrics -> LyricsPanel(song, position) { open(Route.Timing(song.id)) }
                        Panel.UpNext -> UpNextPanel(queueVersion)
                        // Light that moves with the music; full screen from its corner.
                        Panel.Visualizer -> Box(Modifier.fillMaxSize()) {
                            Visualizer(Modifier.fillMaxSize())
                            IconButton(onClick = { Ui.fullVisualizer.value = true }, modifier = Modifier.align(Alignment.BottomEnd)) {
                                Icon(Icons.Rounded.Fullscreen, contentDescription = "Full screen", tint = Color.White.copy(alpha = 0.8f))
                            }
                        }
                    }
                }
            }
            val title: @Composable (TitleSize) -> Unit = { size ->
                SongTitle(song, favorite.takeIf { !song.onComputer() }, toggleFavorite, size) { open(Route.Artist(song.artist)) }
            }
            val head: @Composable () -> Unit = {
                PlayerHead(song, favorite.takeIf { !song.onComputer() }, toggleFavorite)
            }
            val line: @Composable (Boolean, Modifier) -> Unit = { start, modifier ->
                LyricLine(timedLines.orEmpty(), start, openLyrics, modifier)
            }
            val seek: @Composable () -> Unit = {
                val shape = rememberShape(song)
                // The format between the times (Settings › Appearance › Player › Song format).
                val format = if (look.formatChip) song.quality.ifEmpty { song.format } else ""
                AtPosition { at -> SeekBar(position = at, duration = song.duration, playing = playing, heights = shape, format = format) }
                androidx.compose.animation.AnimatedVisibility(stopped != null) {
                    Text(
                        stopped.orEmpty(),
                        style = MaterialTheme.typography.bodyMedium,
                        color = Color.White.copy(alpha = 0.85f),
                        textAlign = androidx.compose.ui.text.style.TextAlign.Center,
                        modifier = Modifier.fillMaxWidth().padding(top = 8.dp).semantics { liveRegion = androidx.compose.ui.semantics.LiveRegionMode.Polite },
                    )
                }
            }
            val buttons: @Composable (Dp) -> Unit = { play ->
                val names = chosenButtons(choices.playerButtons).filter { it in MainRowButtons }
                MainButtons(names, playing, position, song.duration, haptics, repeat, play, glassTint)
                if (song.onComputer() || look.volumeSlider) VolumeRow(song.onComputer())
            }
            // On a wide screen, Lyrics and Up next show in the pane.
            val wideNow = layout == PlayerLayout.Wide
            val pill: @Composable (Modifier) -> Unit = { modifier ->
                ToolsPill(
                    lyrics = if (wideNow) paneShown && paneTab == SideKind.Lyrics else panel == Panel.Lyrics,
                    upNext = if (wideNow) paneShown && paneTab == SideKind.Queue else sheet == PlayerSheet.UpNext,
                    playOn = song.onComputer() || networkOutput,
                    sleep = sleeping != null,
                    more = sheet == PlayerSheet.More || looping.isNotEmpty() || loopStart != null,
                    cover = glassTint,
                    onLyrics = { if (wideNow) showPane(SideKind.Lyrics) else panel = if (panel == Panel.Lyrics) Panel.Cover else Panel.Lyrics },
                    onUpNext = { if (wideNow) showPane(SideKind.Queue) else sheet = PlayerSheet.UpNext },
                    onPlayOn = { Ui.sheet.value = Sheet.PlayOn },
                    onSleep = openSleep,
                    onMore = { sheet = PlayerSheet.More },
                    modifier = modifier,
                )
            }
            // The phone's layout, in its whole area: the cover (or what takes its place) as large as
            // fits above the song, and under it the song and its controls, no wider than 560 dp
            // and centred (a tablet standing up, an unfolded phone beside its pane). `measure`:
            // where the room above the song ends, for the immersive cover over a wide area.
            val tallParts: @Composable (Modifier, Boolean) -> Unit = { modifier, measure ->
                Column(modifier.padding(horizontal = 24.dp), horizontalAlignment = Alignment.CenterHorizontally) {
                    androidx.compose.animation.AnimatedVisibility(showsHead, Modifier.widthIn(max = 560.dp), enter = comes, exit = goes) { head() }
                    Box(
                        Modifier
                            .weight(1f)
                            .fillMaxWidth()
                            .then(if (measure) Modifier.onGloballyPositioned { roomFoot = it.positionInParent().y + it.size.height } else Modifier)
                            .padding(top = 6.dp, bottom = 14.dp),
                    ) { stage(Alignment.TopCenter) }
                    // Where the song and its controls are taller than the room (the largest
                    // text in a small window), they scroll: the pill of tools is never cut off.
                    val partsScroll = rememberScrollState()
                    Column(Modifier.widthIn(max = 560.dp).fillMaxWidth().verticalScroll(partsScroll, enabled = partsScroll.maxValue > 0)) {
                        androidx.compose.animation.AnimatedVisibility(!showsHead, enter = comes, exit = goes) { title(TitleSize.Phone) }
                        androidx.compose.animation.AnimatedVisibility(showsLine, enter = comes, exit = goes) { line(false, Modifier.padding(top = 10.dp)) }
                        Spacer(Modifier.height(6.dp))
                        seek()
                        buttons(76.dp)
                        pill(Modifier.padding(top = 14.dp, bottom = 14.dp))
                    }
                }
            }
            // Clear of a camera cut-out at the side too, on a phone turned sideways.
            Box(Modifier.fillMaxSize().statusBarsPadding().navigationBarsPadding().windowInsetsPadding(androidx.compose.foundation.layout.WindowInsets.displayCutout.only(androidx.compose.foundation.layout.WindowInsetsSides.Horizontal))) {
                when (layout) {
                    PlayerLayout.Tall -> Column(Modifier.fillMaxSize()) {
                        topBar {}
                        tallParts(Modifier.weight(1f).fillMaxWidth(), false)
                    }
                    PlayerLayout.Short -> Column(Modifier.fillMaxSize()) {
                        topBar {}
                        // A short window standing up (a phone's split screen): no room for the
                        // cover, so the song and its controls, scrolled where even they do not fit.
                        CompositionLocalProvider(LocalCompactPlayer provides true) {
                            Column(
                                Modifier.weight(1f).align(Alignment.CenterHorizontally).widthIn(max = 640.dp).fillMaxWidth().verticalScroll(rememberScrollState()).padding(horizontal = 24.dp),
                                verticalArrangement = Arrangement.Center,
                            ) {
                                if (panel != Panel.Cover) Box(Modifier.fillMaxWidth().height(240.dp)) { stage(Alignment.Center) }
                                title(TitleSize.Compact)
                                if (showsLine) line(false, Modifier.padding(top = 6.dp))
                                seek()
                                buttons(64.dp)
                                pill(Modifier.padding(top = 8.dp, bottom = 12.dp))
                            }
                        }
                    }
                    PlayerLayout.Sideways -> Column(Modifier.fillMaxSize()) {
                        topBar {}
                        // A phone on its side: the cover (or what takes its place) at the left,
                        // the song and its controls at the right, scrolled if they do not fit.
                        Row(Modifier.weight(1f).fillMaxWidth().padding(horizontal = 24.dp), horizontalArrangement = Arrangement.spacedBy(36.dp)) {
                            Box(Modifier.weight(1f).fillMaxHeight().padding(bottom = 12.dp), contentAlignment = Alignment.Center) { stage(Alignment.Center) }
                            Box(Modifier.width((wideWidth * 0.48f).coerceAtMost(520.dp)).fillMaxHeight(), contentAlignment = Alignment.Center) {
                                CompositionLocalProvider(LocalCompactPlayer provides true) {
                                    Column(Modifier.fillMaxWidth().verticalScroll(rememberScrollState())) {
                                        title(TitleSize.Compact)
                                        if (showsLine) line(false, Modifier.padding(top = 4.dp))
                                        seek()
                                        buttons(60.dp)
                                        pill(Modifier.padding(top = 6.dp, bottom = 10.dp))
                                    }
                                }
                            }
                        }
                    }
                    PlayerLayout.Tabletop -> Column(Modifier.fillMaxSize()) {
                        topBar {}
                        // A foldable standing as a laptop: the cover (or the lyrics, or the light)
                        // on the upper half, the song and its controls on the lower, none on the crease.
                        val crease = hinge!!
                        var top by remember { mutableStateOf(0.dp) }
                        Column(
                            Modifier
                                .weight(1f)
                                .fillMaxWidth()
                                .onGloballyPositioned { top = with(density) { it.positionInWindow().y.toDp() } },
                        ) {
                            Box(
                                Modifier.fillMaxWidth().height((crease.top - top - 12.dp).coerceAtLeast(80.dp)),
                                contentAlignment = Alignment.Center,
                            ) { stage(Alignment.Center) }
                            Spacer(Modifier.height(crease.bottom - crease.top + 24.dp))
                            CompositionLocalProvider(LocalCompactPlayer provides true) {
                                Column(
                                    Modifier.weight(1f).align(Alignment.CenterHorizontally).widthIn(max = 640.dp).fillMaxWidth().verticalScroll(rememberScrollState()).padding(horizontal = 24.dp),
                                    verticalArrangement = Arrangement.Center,
                                ) {
                                    title(TitleSize.Compact)
                                    if (showsLine) line(false, Modifier.padding(top = 4.dp))
                                    seek()
                                    buttons(60.dp)
                                    pill(Modifier.padding(top = 6.dp, bottom = 10.dp))
                                }
                            }
                        }
                    }
                    PlayerLayout.Wide -> Box(Modifier.fillMaxSize()) {
                        // The player's area: the window less the pane and the room round it. It
                        // narrows and widens with the pane as it slides.
                        val paneRoom by animateDpAsState(
                            if (paneShown) paneWidth + 40.dp else 0.dp,
                            if (still) snap() else spring(dampingRatio = 1f, stiffness = 400f),
                            label = "pane room",
                        )
                        Box(Modifier.fillMaxSize().padding(end = paneRoom)) {
                            // The round buttons on the area, as on a phone: close at its left, the
                            // menu at its right; where the area has no pill of tools (side by side),
                            // Lyrics and Up next too, which open, change, and close the pane.
                            topBar {
                                if (!tallArea) {
                                    val lyricsOn = paneShown && paneTab == SideKind.Lyrics
                                    val queueOn = paneShown && paneTab == SideKind.Queue
                                    GlassRound(Icons.Rounded.Lyrics, "Lyrics", glassTint, on = lyricsOn, state = if (lyricsOn) "Showing" else "Hidden") { showPane(SideKind.Lyrics) }
                                    GlassRound(Icons.AutoMirrored.Rounded.QueueMusic, "Up next", glassTint, on = queueOn, state = if (queueOn) "Showing" else "Hidden") { showPane(SideKind.Queue) }
                                }
                            }
                            // The pane opening or closing can turn the area from wide to tall or
                            // back: the one layout fades into the other (as the views do) while the
                            // pane slides; the song and the player's choices stay.
                            AnimatedContent(
                                tallArea,
                                Modifier.fillMaxSize().padding(top = WideTop, bottom = 20.dp, end = 20.dp),
                                transitionSpec = { (fadeIn(tween(200)) togetherWith fadeOut(tween(120))).using(null) },
                                label = "wide layout",
                            ) { tallNow ->
                                if (tallNow) {
                                    // Taller than wide: the phone's layout at the area's width.
                                    tallParts(Modifier.fillMaxSize(), pictureOn)
                                } else {
                                    // Side by side. The card: as large as 62 % of the player's height
                                    // and 46 % of its area allow, centred from top to foot; it grows
                                    // and shrinks with the pane. The immersive cover lies behind (see
                                    // Behind.Left): here only its room, which skips when swiped.
                                    val cardSide by animateDpAsState(
                                        minOf(wideHeight * 0.62f, areaWidth * 0.46f, (areaWidth - 124.dp - BesideRoom).coerceAtLeast(120.dp)),
                                        if (still) snap() else spring(dampingRatio = 1f, stiffness = 400f),
                                        label = "card side",
                                    )
                                    val left = if (pictureOn) (pictureWidth - leftInset).coerceAtLeast(0.dp) else cardSide + 48.dp
                                    Row(Modifier.fillMaxSize()) {
                                        Box(Modifier.width(left).fillMaxHeight().padding(start = if (pictureOn) 0.dp else 48.dp), contentAlignment = Alignment.Center) {
                                            if (pictureOn) stage(Alignment.Center)
                                            else Box(Modifier.width(cardSide).fillMaxHeight()) { stage(Alignment.Center) }
                                        }
                                        // The song and its controls, one group centred in the rest, no
                                        // wider than 560 dp; clear of the speaker's pill at the foot.
                                        Box(
                                            Modifier
                                                .weight(1f)
                                                .fillMaxHeight()
                                                .padding(start = 32.dp, end = 24.dp, bottom = if (!wideImmersive) 56.dp else 0.dp),
                                            contentAlignment = Alignment.Center,
                                        ) {
                                            Column(Modifier.widthIn(max = 560.dp).fillMaxWidth().verticalScroll(rememberScrollState())) {
                                                title(TitleSize.Wide)
                                                androidx.compose.animation.AnimatedVisibility(showsLine, enter = comes, exit = goes) { line(true, Modifier.padding(top = 14.dp)) }
                                                Spacer(Modifier.height(10.dp))
                                                seek()
                                                buttons(76.dp)
                                            }
                                        }
                                    }
                                }
                            }
                            // The speaker in use, at the foot of the area on the right (the card
                            // layout side by side; the phone's layout has Play on in its pill).
                            if (!wideImmersive && !tallArea) {
                                SpeakerPill(speakerInUse(song, networkOutput), glassTint, Modifier.align(Alignment.BottomEnd).padding(end = 24.dp, bottom = 20.dp))
                            }
                        }
                        // The pane: as tall as the window, from the round buttons' top to the foot.
                        androidx.compose.animation.AnimatedVisibility(
                            paneShown,
                            Modifier.align(Alignment.TopEnd),
                            enter = if (still) fadeIn(tween(150)) else fadeIn(tween(200)) + androidx.compose.animation.expandHorizontally(spring(dampingRatio = 1f, stiffness = 400f)),
                            exit = if (still) fadeOut(tween(120)) else fadeOut(tween(120)) + androidx.compose.animation.shrinkHorizontally(spring(dampingRatio = 1f, stiffness = 400f)),
                        ) {
                            PlayerPane(
                                paneTab,
                                { PaneChoice.set(true, it) },
                                { PaneChoice.set(false, paneTab) },
                                { searching = true },
                                song,
                                position,
                                queueVersion,
                                glassTint,
                                open,
                                Modifier.padding(top = 7.dp, end = 20.dp, bottom = 20.dp).width(paneWidth).fillMaxHeight(),
                            )
                        }
                    }
                }
                if (searching) PlayerSearch(song.onComputer()) { searching = false }
                when (sheet) {
                    PlayerSheet.More -> PlayerMoreSheet(
                        song = song,
                        open = open,
                        bigLook = bigLook,
                        onLook = { next ->
                            panel = Panel.Cover
                            NeedleApp.instance.updateMore { it.copy(bigLook = next) }
                        },
                        sleepText = sleeping?.let { "${kotlin.math.ceil(it.remaining / 60).toInt().coerceAtLeast(0)} min left" } ?: "Off",
                        loopText = loopText,
                        speaker = speakerInUse(song, networkOutput),
                        // The pill of tools is on every layout but a wide one side by side; on a
                        // wide one Up next is the pane, which has no Add songs.
                        tools = layout != PlayerLayout.Wide || tallArea,
                        // A wide window (not a phone on its side): two short columns.
                        twoColumns = wideWidth >= 600.dp && wideHeight >= 480.dp,
                        onSleep = openSleep,
                        onLoop = onLoop,
                        onFull = { Ui.fullVisualizer.value = true },
                        onPlayOn = { Ui.sheet.value = Sheet.PlayOn },
                        onDismiss = { sheet = null },
                    )
                    PlayerSheet.UpNext -> UpNextSheet(
                        version = queueVersion,
                        count = upNextCount,
                        art = song.artwork,
                        onAddSongs = { searching = true },
                        onDismiss = { sheet = null },
                    )
                    null -> {}
                }
            }
        }
    }
    }
}

/**
 * A tablet's (or a TV's) side pane (DESIGN.md): the lyrics or what is up next, in a rounded pane
 * floating over the page's right side, dark on the blurred cover as the player is.
 */
@Composable
fun SidePane(kind: SideKind, open: (Route) -> Unit, modifier: Modifier = Modifier) {
    val playbackState = NeedleApp.instance.playback.collectAsState()
    val current by remember { derivedStateOf { playbackState.value?.current } }
    val queueVersion by remember { derivedStateOf { playbackState.value?.queueVersion ?: 0uL } }
    val song = remember(current) { current } ?: return
    val position = { playbackState.value?.position ?: 0.0 }
    val shape = RoundedCornerShape(22.dp)
    // Glass over the page where it can blur it (Glass.kt); elsewhere the player's backdrop, solid.
    val glassy = LocalHaze.current != null && blurAllowed(large = true)
    CoverTheme(song.artwork) {
        Box(
            modifier
                .width(320.dp)
                .fillMaxHeight()
                .then(
                    if (glassy) {
                        Modifier.coverGlass(shape, coverColor(song.artwork))
                    } else {
                        Modifier
                            .shadow(18.dp, shape, ambientColor = Color.Black.copy(alpha = 0.4f), spotColor = Color.Black.copy(alpha = 0.4f))
                            .clip(shape)
                            .edgeLight(shape, dark = true)
                    },
                ),
        ) {
            if (!glassy) PlayerBackdrop(song, reading = true)
            CompositionLocalProvider(LocalContentColor provides Color.White) {
                Column(Modifier.fillMaxSize().padding(start = 20.dp, end = 8.dp, top = 8.dp, bottom = 12.dp)) {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Text(
                            if (kind == SideKind.Lyrics) "Lyrics" else "Up next",
                            style = MaterialTheme.typography.titleMedium,
                            fontWeight = FontWeight.Bold,
                            modifier = Modifier.weight(1f).semantics { heading() },
                        )
                        IconButton(onClick = { Ui.pane.value = null }) { Icon(Icons.Rounded.Close, contentDescription = "Close") }
                    }
                    Box(Modifier.weight(1f).padding(end = 12.dp)) {
                        when (kind) {
                            SideKind.Lyrics -> LyricsPanel(song, position, compact = true) { open(Route.Timing(song.id)) }
                            SideKind.Queue -> UpNextPanel(queueVersion, titled = false)
                        }
                    }
                }
            }
        }
    }
}

/** How large the song's name is: a phone's player, a short or sideways one, a wide one's, or a wide card's (centred). */
private enum class TitleSize { Phone, Compact, Wide, Centered }

/**
 * The song's name, bold and large, and under it the artist (a tap goes to the artist); the
 * heart at the right, in the accent. A name too long for the width ends with …, as the app's
 * other names do: it never slides along, which hid its first letters while it moved (TalkBack
 * reads it whole).
 */
@Composable
private fun SongTitle(song: Song, favorite: Boolean?, toggleFavorite: () -> Unit, size: TitleSize, onArtist: () -> Unit) {
    val centered = size == TitleSize.Centered
    val titleStyle = when (size) {
        TitleSize.Phone -> MaterialTheme.typography.headlineMedium.copy(fontSize = 30.sp, lineHeight = 36.sp)
        TitleSize.Compact -> MaterialTheme.typography.headlineSmall
        TitleSize.Wide -> MaterialTheme.typography.displaySmall.copy(fontSize = 44.sp, lineHeight = 50.sp)
        TitleSize.Centered -> MaterialTheme.typography.headlineSmall
    }
    val artistStyle = when (size) {
        TitleSize.Wide -> MaterialTheme.typography.titleLarge
        TitleSize.Centered, TitleSize.Compact -> MaterialTheme.typography.bodyLarge
        TitleSize.Phone -> MaterialTheme.typography.titleMedium.copy(fontSize = 16.5.sp)
    }
    val heart: @Composable () -> Unit = {
        if (favorite != null) {
            IconButton(onClick = toggleFavorite, modifier = Modifier.semantics { stateDescription = if (favorite) "In favorites" else "Not in favorites" }) {
                Icon(
                    if (favorite) Icons.Rounded.Favorite else Icons.Rounded.FavoriteBorder,
                    contentDescription = "Favorite",
                    tint = MaterialTheme.colorScheme.primary,
                    modifier = Modifier.size(if (size == TitleSize.Phone || size == TitleSize.Wide) 28.dp else 24.dp),
                )
            }
        } else if (centered) Spacer(Modifier.size(48.dp))
    }
    Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
        // The centred name keeps its middle: the heart's room on its left too.
        if (centered) Spacer(Modifier.size(48.dp))
        Column(Modifier.weight(1f), horizontalAlignment = if (centered) Alignment.CenterHorizontally else Alignment.Start) {
            Text(
                song.title,
                style = titleStyle,
                fontWeight = FontWeight.Bold,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier.semantics { heading() },
            )
            Text(
                song.artist,
                style = artistStyle,
                color = Color.White.copy(alpha = 0.72f),
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
                textAlign = if (centered) androidx.compose.ui.text.style.TextAlign.Center else null,
                // A full finger's height to tap, and a button to TalkBack.
                modifier = Modifier
                    .then(if (song.onComputer()) Modifier else Modifier.heightIn(min = if (LocalCompactPlayer.current) 36.dp else 40.dp))
                    .clip(RoundedCornerShape(6.dp))
                    .clickable(enabled = !song.onComputer(), role = androidx.compose.ui.semantics.Role.Button, onClickLabel = "Go to artist", onClick = onArtist)
                    .wrapContentHeight(Alignment.CenterVertically),
            )
            if (song.onComputer()) OnComputerChip(Modifier.padding(top = 8.dp))
        }
        // Close to the edge, as the drawing has it: the heart's own room is its padding.
        Box(Modifier.offset(x = if (centered) 0.dp else 8.dp)) { heart() }
    }
}

/** The song, small, above the lyrics: its cover, name, and artist, and the heart. */
@Composable
private fun PlayerHead(song: Song, favorite: Boolean?, toggleFavorite: () -> Unit) {
    Row(
        Modifier.fillMaxWidth().padding(top = 4.dp, bottom = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Cover(
            song.artwork,
            Modifier.size(48.dp).shadow(10.dp, RoundedCornerShape(10.dp), ambientColor = Color.Black.copy(alpha = 0.3f), spotColor = Color.Black.copy(alpha = 0.3f)),
            RoundedCornerShape(10.dp),
            seed = song.album + song.artist,
        )
        Column(Modifier.weight(1f)) {
            Text(song.title, style = MaterialTheme.typography.titleMedium.copy(fontSize = 17.sp), fontWeight = FontWeight.Bold, maxLines = 1, overflow = TextOverflow.Ellipsis)
            Text(song.artist, style = MaterialTheme.typography.bodyMedium, color = Color.White.copy(alpha = 0.72f), maxLines = 1, overflow = TextOverflow.Ellipsis)
        }
        if (favorite != null) {
            IconButton(onClick = toggleFavorite, modifier = Modifier.offset(x = 8.dp).semantics { stateDescription = if (favorite) "In favorites" else "Not in favorites" }) {
                Icon(if (favorite) Icons.Rounded.Favorite else Icons.Rounded.FavoriteBorder, contentDescription = "Favorite", tint = MaterialTheme.colorScheme.primary)
            }
        }
    }
}

/**
 * The line of the lyrics sung now, bold, moving with the song, as the mini player has it
 * (centred; at the left beside a wide screen's cover, `start`). A tap opens the lyrics.
 */
@Composable
private fun LyricLine(lines: List<fyi.nnx.needle.core.LyricLine>, start: Boolean, onOpen: () -> Unit, modifier: Modifier = Modifier) {
    Box(
        modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(8.dp))
            .clickable(role = androidx.compose.ui.semantics.Role.Button, onClickLabel = "Open the lyrics", onClick = onOpen)
            .padding(vertical = 4.dp),
    ) {
        AtPosition { at -> MiniLyrics(lines, at, big = true, start = start) }
    }
}

/**
 * The styles of the player's main buttons (Settings › Appearance › Player › Buttons). Only Play
 * changes: Accent (a circle in the accent, the first), Glass (a round button of the player's
 * glass), Wide (a wide pill in the accent), Ring (a thin ring in the accent), Soft square (a
 * square with soft corners in the accent), Trio (Play and its two neighbours in one glass pill).
 */
internal val ButtonStyles = listOf("accent", "glass", "wide", "ring", "square", "trio")

/**
 * The style chosen. The older "expressive", "round", and "minimal" (and anything else) are
 * Accent, the look the player has had since its new design.
 */
internal fun buttonStyle(saved: String): String = saved.takeIf { it in ButtonStyles } ?: "accent"

/**
 * The main buttons, in the user's order (Settings › Appearance › Player buttons): plain icons
 * with no circles, Shuffle and Repeat in the accent with a small dot under them while on; Play
 * in the chosen style ([ButtonStyles]), its sizes scaled from this layout's Play (`play`, 76 on a
 * phone). `cover`: the glass's colour, for Glass and Trio.
 */
@OptIn(androidx.compose.material3.ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun MainButtons(
    names: List<String>,
    playing: Boolean,
    position: () -> Double,
    duration: Double,
    haptics: (Boolean) -> Unit,
    repeat: RepeatMode,
    play: Dp,
    cover: Color,
) {
    val app by NeedleApp.instance.app.collectAsState()
    val jump = app.sideButtons == "jump"
    val compact = LocalCompactPlayer.current
    val back = { haptics(false); if (jump) Controls.seek((position() - 10).coerceAtLeast(0.0)) else Controls.previous() }
    val ahead = { haptics(false); if (jump) Controls.seek((position() + 10).let { if (duration > 0) it.coerceAtMost(duration) else it }) else Controls.next() }
    val toggle = { haptics(false); Controls.toggle() }
    val playPress = remember { androidx.compose.foundation.interaction.MutableInteractionSource() }
    val cell = if (compact) 48.dp else 54.dp
    val style = buttonStyle(app.controlStyle)
    // Wide: the others a little closer, so the row fits beside the wide Play.
    val smallCell = if (style == "wide") 48.dp else cell
    val skipCell = if (style == "wide") maxOf(48.dp, cell - 4.dp) else cell
    val item: @Composable (String) -> Unit = { name ->
        when (name) {
            "previous" -> PlainButton(if (jump) Icons.Rounded.Replay10 else Icons.Rounded.SkipPrevious, if (jump) "Back 10 seconds" else "Previous", skipCell, if (compact) 32.dp else 38.dp, onClick = back)
            "next" -> PlainButton(if (jump) Icons.Rounded.Forward10 else Icons.Rounded.SkipNext, if (jump) "Ahead 10 seconds" else "Next", skipCell, if (compact) 32.dp else 38.dp, onClick = ahead)
            "play" -> PlayButton(style, play, cover, playing, playPress) { toggle() }
            "shuffle" -> {
                // Shuffle stays on: a toggle, lit while on; otherwise it shuffles what is up next once.
                val stays = NeedleApp.instance.more.collectAsState().value.shuffleStays && !onComputer()
                val playbackState = NeedleApp.instance.playback.collectAsState()
                val on by remember { derivedStateOf { playbackState.value?.shuffle == true } }
                if (stays) {
                    PlainButton(Icons.Rounded.Shuffle, "Shuffle", smallCell, 24.dp, on = on, state = if (on) "On" else "Off", small = true) {
                        Controls.shuffle(); showMessage(if (on) "Shuffle is off" else "Shuffle is on")
                    }
                } else {
                    PlainButton(Icons.Rounded.Shuffle, "Shuffle what is up next", smallCell, 24.dp, small = true) { Controls.shuffle(); showMessage("Shuffled what is up next") }
                }
            }
            "repeat" -> PlainButton(
                if (repeat == RepeatMode.ONE) Icons.Rounded.RepeatOne else Icons.Rounded.Repeat,
                "Repeat",
                smallCell,
                24.dp,
                on = repeat != RepeatMode.OFF,
                state = when (repeat) {
                    RepeatMode.OFF -> "Off"
                    RepeatMode.ALL -> "All songs"
                    RepeatMode.ONE -> "This song"
                },
                small = true,
            ) {
                Controls.setRepeat(
                    when (repeat) {
                        RepeatMode.OFF -> RepeatMode.ALL
                        RepeatMode.ALL -> RepeatMode.ONE
                        RepeatMode.ONE -> RepeatMode.OFF
                    },
                )
            }
        }
    }
    // Trio: Play and the button at each side of it, in the user's order, whatever they are.
    val at = names.indexOf("play")
    val trio = if (style == "trio" && at >= 0) names.subList(maxOf(0, at - 1), minOf(names.size, at + 2)) else emptyList()
    Row(
        Modifier.fillMaxWidth().padding(top = if (compact) 2.dp else 8.dp, bottom = if (compact) 2.dp else 6.dp),
        horizontalArrangement = if (names.size >= 4) Arrangement.SpaceBetween else Arrangement.spacedBy(28.dp, Alignment.CenterHorizontally),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        if (trio.isEmpty()) {
            names.forEach { item(it) }
        } else {
            val first = names.indexOf(trio.first())
            names.take(first).forEach { item(it) }
            TrioPill(play, cover) {
                trio.forEach { name -> if (name == "play") TrioPlay(play, playing, playPress) { toggle() } else item(name) }
            }
            names.drop(first + trio.size).forEach { item(it) }
        }
    }
}

/**
 * Play in its style (see [ButtonStyles]; Trio's is [TrioPlay]), `play` its size on this layout
 * (76 on a phone). It presses as the player's other buttons do, its focus ring follows its
 * shape, and TalkBack hears Play or Pause.
 */
@OptIn(androidx.compose.material3.ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun PlayButton(style: String, play: Dp, cover: Color, playing: Boolean, press: androidx.compose.foundation.interaction.MutableInteractionSource, toggle: () -> Unit) {
    val accent = MaterialTheme.colorScheme.primary
    val onAccent = MaterialTheme.colorScheme.onPrimary
    val shade = Color.Black.copy(alpha = 0.28f)
    when (style) {
        // A round button of the player's glass, as the two at its top; a white icon.
        "glass" -> Box(
            Modifier
                .size(play)
                .pressScale(press, ButtonPress, corner = play / 2)
                .glass(CircleShape, cover = cover, shadow = 12.dp, solid = PlayerGlassSolid, fill = PlayerGlassFill)
                .clickable(interactionSource = press, indication = null, role = androidx.compose.ui.semantics.Role.Button, onClick = toggle)
                .playPauseSemantics(playing) { toggle() },
            contentAlignment = Alignment.Center,
        ) {
            CompositionLocalProvider(LocalContentColor provides Color.White) { PlayPauseIcon(playing, play * 0.47f) }
        }
        // A wide pill (116 x 68 on a phone), a soft square (corner 26), or a thin ring (2.5).
        "wide", "square", "ring" -> {
            val k = play / 76.dp
            val width = if (style == "wide") play * (116f / 76f) else play
            val height = if (style == "wide") play * (68f / 76f) else play
            val corner = if (style == "square") 26.dp * k else height / 2
            val shape = RoundedCornerShape(corner)
            val filled = style != "ring"
            Box(
                Modifier
                    .size(width, height)
                    .pressScale(press, ButtonPress, corner = corner)
                    .then(if (filled) Modifier.shadow(10.dp, shape, ambientColor = shade, spotColor = shade) else Modifier)
                    .clip(shape)
                    .then(if (filled) Modifier.background(accent) else Modifier.border(2.5.dp * k, accent, shape))
                    .clickable(interactionSource = press, indication = androidx.compose.material3.ripple(), role = androidx.compose.ui.semantics.Role.Button, onClick = toggle)
                    .playPauseSemantics(playing) { toggle() },
                contentAlignment = Alignment.Center,
            ) {
                CompositionLocalProvider(LocalContentColor provides if (filled) onAccent else accent) { PlayPauseIcon(playing, play * 0.47f) }
            }
        }
        // Accent: a circle filled with the accent, its icon dark.
        else -> androidx.compose.material3.FilledIconButton(
            onClick = toggle,
            shape = CircleShape,
            colors = androidx.compose.material3.IconButtonDefaults.filledIconButtonColors(containerColor = accent, contentColor = onAccent),
            interactionSource = press,
            modifier = Modifier
                .size(play)
                .pressScale(press, ButtonPress)
                .shadow(10.dp, CircleShape, ambientColor = shade, spotColor = shade)
                .playPauseSemantics(playing) { toggle() },
        ) { PlayPauseIcon(playing, play * 0.47f) }
    }
}

/** Trio's one pill of the player's glass (68 high on a phone, the dock's shape), round Play and its neighbours. */
@Composable
private fun TrioPill(play: Dp, cover: Color, content: @Composable RowScope.() -> Unit) {
    val k = play / 76.dp
    Row(
        Modifier
            .height((68.dp * k).coerceAtLeast(52.dp))
            .glass(CircleShape, cover = cover, shadow = 14.dp, solid = PlayerGlassSolid, fill = PlayerGlassFill)
            .padding(horizontal = 8.dp * k),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(6.dp * k),
        content = content,
    )
}

/** Trio's Play: a small pill in the accent (68 x 54 on a phone) with no shadow, in the glass pill. */
@Composable
private fun TrioPlay(play: Dp, playing: Boolean, press: androidx.compose.foundation.interaction.MutableInteractionSource, toggle: () -> Unit) {
    val k = play / 76.dp
    val height = (54.dp * k).coerceAtLeast(44.dp)
    Box(
        Modifier
            .size(68.dp * k, height)
            .pressScale(press, ButtonPress, corner = height / 2)
            .clip(CircleShape)
            .background(MaterialTheme.colorScheme.primary)
            .clickable(interactionSource = press, indication = androidx.compose.material3.ripple(), role = androidx.compose.ui.semantics.Role.Button, onClick = toggle)
            .playPauseSemantics(playing) { toggle() },
        contentAlignment = Alignment.Center,
    ) {
        CompositionLocalProvider(LocalContentColor provides MaterialTheme.colorScheme.onPrimary) { PlayPauseIcon(playing, 30.dp * k) }
    }
}

/**
 * A plain icon button of the player, no circle: `small` ones (Shuffle, Repeat) a little faint,
 * and in the accent with a small dot under them while `on`. It presses as the player's other
 * buttons do, and takes a ring with a remote's focus.
 */
@Composable
private fun PlainButton(icon: ImageVector, label: String, cell: Dp, iconSize: Dp, on: Boolean = false, state: String? = null, small: Boolean = false, onClick: () -> Unit) {
    val press = remember { androidx.compose.foundation.interaction.MutableInteractionSource() }
    val tint by animateColorAsState(
        when {
            on -> MaterialTheme.colorScheme.primary
            small -> Color.White.copy(alpha = 0.66f)
            else -> Color.White
        },
        label = "button tint",
    )
    Box(
        Modifier
            .size(cell)
            .pressScale(press, ButtonPress, corner = cell / 2)
            .clip(CircleShape)
            .clickable(interactionSource = press, indication = androidx.compose.material3.ripple(), role = androidx.compose.ui.semantics.Role.Button, onClick = onClick)
            .semantics {
                contentDescription = label
                if (state != null) stateDescription = state
            },
        contentAlignment = Alignment.Center,
    ) {
        Icon(icon, contentDescription = null, tint = tint, modifier = Modifier.size(iconSize))
        if (on) Box(Modifier.align(Alignment.BottomCenter).padding(bottom = 5.dp).size(4.dp).clip(CircleShape).background(MaterialTheme.colorScheme.primary))
    }
}

/**
 * A round glass button floating on the player (close, the menu, full screen; on a wide card,
 * Lyrics and Up next): Needle's glass, 42 dp, in a finger's 48. `on`: open or showing, in the accent.
 */
@Composable
private fun GlassRound(icon: ImageVector, label: String, cover: Color, on: Boolean = false, state: String? = null, onClick: () -> Unit) {
    val press = remember { androidx.compose.foundation.interaction.MutableInteractionSource() }
    Box(
        Modifier
            .size(48.dp)
            .clickable(interactionSource = press, indication = null, role = androidx.compose.ui.semantics.Role.Button, onClick = onClick)
            .semantics {
                contentDescription = label
                if (state != null) stateDescription = state
            },
        contentAlignment = Alignment.Center,
    ) {
        Box(
            Modifier
                .size(42.dp)
                .pressScale(press, ButtonPress, corner = 21.dp)
                .glass(CircleShape, cover = cover, shadow = 10.dp, solid = PlayerGlassSolid, fill = PlayerGlassFill),
            contentAlignment = Alignment.Center,
        ) {
            Icon(icon, contentDescription = null, tint = if (on) MaterialTheme.colorScheme.primary else Color.White, modifier = Modifier.size(24.dp))
        }
    }
}

/**
 * The tools: one glass pill in the dock's shape, five icons and no names (Lyrics, Up next, Play
 * on, Sleep timer, More). One that is on, or open, is in the accent.
 */
@Composable
private fun ToolsPill(
    lyrics: Boolean,
    upNext: Boolean,
    playOn: Boolean,
    sleep: Boolean,
    more: Boolean,
    cover: Color,
    onLyrics: () -> Unit,
    onUpNext: () -> Unit,
    onPlayOn: () -> Unit,
    onSleep: () -> Unit,
    onMore: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val compact = LocalCompactPlayer.current
    val shape = RoundedCornerShape(29.dp)
    Row(
        modifier
            .fillMaxWidth()
            .height(if (compact) 52.dp else 58.dp)
            .glass(shape, cover = cover, shadow = 14.dp, solid = PlayerGlassSolid, fill = PlayerGlassFill),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        PillCell(Icons.Rounded.Lyrics, "Lyrics", lyrics, if (lyrics) "Showing" else "Hidden", onLyrics)
        PillCell(Icons.AutoMirrored.Rounded.QueueMusic, "Up next", upNext, null, onUpNext)
        PillCell(Icons.Rounded.Cast, "Play on", playOn, null, onPlayOn)
        PillCell(if (sleep) Icons.Rounded.Bedtime else Icons.Outlined.Bedtime, "Sleep timer", sleep, if (sleep) "On" else "Off", onSleep)
        PillCell(Icons.Rounded.MoreHoriz, "More", more, null, onMore)
    }
}

@Composable
private fun RowScope.PillCell(icon: ImageVector, label: String, on: Boolean, state: String?, onClick: () -> Unit) {
    val press = remember { androidx.compose.foundation.interaction.MutableInteractionSource() }
    val tint by animateColorAsState(if (on) MaterialTheme.colorScheme.primary else Color.White, label = "tool tint")
    // Each its share of the pill, so on a narrow screen the last is not the one squeezed.
    Box(
        Modifier
            .weight(1f)
            .fillMaxHeight()
            .clickable(interactionSource = press, indication = null, role = androidx.compose.ui.semantics.Role.Button, onClick = onClick)
            .semantics {
                contentDescription = label
                if (state != null) stateDescription = state
            },
        contentAlignment = Alignment.Center,
    ) {
        Box(Modifier.size(44.dp).pressScale(press, ButtonPress, corner = 22.dp), contentAlignment = Alignment.Center) {
            Icon(icon, contentDescription = null, tint = tint, modifier = Modifier.size(24.dp))
        }
    }
}

/**
 * A wide screen's pane beside the player (the side pane the app has today): one glass pane with
 * two tabs, Up next and Lyrics.
 */
@Composable
private fun PlayerPane(
    tab: SideKind,
    onTab: (SideKind) -> Unit,
    onClose: () -> Unit,
    onAddSongs: () -> Unit,
    song: Song,
    position: () -> Double,
    queueVersion: ULong,
    cover: Color,
    open: (Route) -> Unit,
    modifier: Modifier = Modifier,
) {
    val tabs = listOf(SideKind.Queue, SideKind.Lyrics)
    Column(
        modifier
            .glass(RoundedCornerShape(24.dp), cover = cover, large = true, shadow = 16.dp, solid = PlayerGlassSolid, fill = PlayerGlassFill)
            .padding(12.dp),
    ) {
        // Its own top line: the two tabs, and a small plain close at the right.
        Row(verticalAlignment = Alignment.CenterVertically) {
            Segments(listOf("Up next", "Lyrics"), tabs.indexOf(tab), 34.dp, Modifier.weight(1f)) { onTab(tabs[it]) }
            Box(
                Modifier
                    .padding(start = 4.dp)
                    .size(40.dp)
                    .clip(CircleShape)
                    .clickable(role = androidx.compose.ui.semantics.Role.Button, onClickLabel = null, onClick = onClose)
                    .semantics { contentDescription = "Close the side pane" },
                contentAlignment = Alignment.Center,
            ) {
                Icon(Icons.Rounded.Close, contentDescription = null, tint = Color.White.copy(alpha = 0.75f), modifier = Modifier.size(20.dp))
            }
        }
        Box(Modifier.weight(1f).fillMaxWidth().padding(start = 4.dp, end = 4.dp, top = 8.dp)) {
            when (tab) {
                SideKind.Lyrics -> LyricsPanel(song, position, compact = true) { open(Route.Timing(song.id)) }
                // The same small line as the phone's Up next: Shuffle and Add songs.
                SideKind.Queue -> UpNextPanel(queueVersion, titled = false) { UpNextActions(Modifier.padding(top = 2.dp, bottom = 8.dp), onAddSongs) }
            }
        }
    }
}

/** A row of choices as one pill, the chosen one filled in the accent (the pane's tabs, More's looks). */
@Composable
private fun Segments(options: List<String>, selected: Int, height: Dp, modifier: Modifier = Modifier, onSelect: (Int) -> Unit) {
    Row(
        modifier
            .fillMaxWidth()
            .clip(CircleShape)
            .background(Color.Black.copy(alpha = 0.2f))
            .padding(3.dp)
            .selectableGroup(),
    ) {
        options.forEachIndexed { i, label ->
            val on = i == selected
            val fill by animateColorAsState(if (on) MaterialTheme.colorScheme.primary else Color.Transparent, label = "segment")
            Box(
                Modifier
                    .weight(1f)
                    .heightIn(min = height)
                    .clip(CircleShape)
                    .background(fill)
                    .selectable(selected = on, role = androidx.compose.ui.semantics.Role.Tab) { onSelect(i) },
                contentAlignment = Alignment.Center,
            ) {
                Text(
                    label,
                    style = MaterialTheme.typography.labelLarge,
                    color = if (on) MaterialTheme.colorScheme.onPrimary else Color.White.copy(alpha = 0.7f),
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                    modifier = Modifier.padding(horizontal = 6.dp),
                )
            }
        }
    }
}

/** The speaker in use, on a wide card's glass pill at the foot; a tap chooses another (Play on). */
@Composable
private fun SpeakerPill(name: String, cover: Color, modifier: Modifier = Modifier) {
    val press = remember { androidx.compose.foundation.interaction.MutableInteractionSource() }
    Row(
        modifier
            .heightIn(min = 46.dp)
            .pressScale(press, ButtonPress, corner = 23.dp)
            .glass(CircleShape, cover = cover, shadow = 12.dp, solid = PlayerGlassSolid, fill = PlayerGlassFill)
            .clickable(interactionSource = press, indication = androidx.compose.material3.ripple(), role = androidx.compose.ui.semantics.Role.Button, onClickLabel = "Choose where it plays") { Ui.sheet.value = Sheet.PlayOn }
            .semantics(mergeDescendants = true) { contentDescription = "Play on: $name" }
            .padding(horizontal = 18.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        Icon(Icons.Rounded.Cast, contentDescription = null, modifier = Modifier.size(20.dp))
        Text(name, style = MaterialTheme.typography.labelLarge, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.widthIn(max = 220.dp))
    }
}

/**
 * Where the music plays now, in a few words: the computer, the speakers of a group by name, a
 * speaker on the network, or this device.
 */
@Composable
private fun speakerInUse(song: Song, network: Boolean): String {
    val names by androidx.compose.runtime.produceState(emptyList<String>(), network) {
        value = if (network) withContext(Dispatchers.IO) { runCatching { core.groupMembers().map { it.name }.filter { it.isNotBlank() } }.getOrDefault(emptyList()) } else emptyList()
    }
    return when {
        song.onComputer() -> "Your computer"
        !network -> "This device"
        names.size == 1 -> names[0]
        names.size > 1 -> "${names.size} speakers"
        // One speaker on the network: the core does not say its name without looking for speakers.
        else -> "A speaker"
    }
}

/**
 * More (the pill's last icon, and the menu at the top), a glass sheet in one short page:
 * - the song: its cover, its name, and its stars under it;
 * - four pills, two by two: Playlist, Radio, the album's name, the artist's name;
 * - the player: the look (Cover, Record, Lyrics, Visualizer) as one row of choices, the A-B loop,
 *   Speed and pitch, Full screen; and where the layout has no pill of tools (`tools` off), the
 *   Sleep timer and Play on too (Add songs is in Up next, on the phone's sheet and in the pane);
 * - under a thin line, the rare ones: Edit tags (or for a server's song, Keep on this phone and
 *   Save to my music), Time the lyrics, and the commands of plugins.
 * A song playing on the computer: its cover and name only, then the player's rows.
 * On a wide window (`twoColumns`): two short columns, at most 720 dp across in all: the song at
 * the left, the player at the right.
 * Play next, Add to queue, and Select are not here: they stay in the menu of a song in a list.
 */
@OptIn(androidx.compose.material3.ExperimentalMaterial3Api::class)
@Composable
private fun PlayerMoreSheet(
    song: Song,
    open: (Route) -> Unit,
    bigLook: String,
    onLook: (String) -> Unit,
    sleepText: String,
    loopText: String,
    speaker: String,
    tools: Boolean,
    twoColumns: Boolean,
    onSleep: () -> Unit,
    onLoop: () -> Unit,
    onFull: () -> Unit,
    onPlayOn: () -> Unit,
    onDismiss: () -> Unit,
) {
    val state = androidx.compose.material3.rememberModalBottomSheetState(skipPartiallyExpanded = true)
    val scope = androidx.compose.runtime.rememberCoroutineScope()
    // Once only: rows still take taps while the sheet slides away. What opens another sheet or
    // window (`then`) waits until this one has gone.
    var acted by remember { mutableStateOf(false) }
    fun act(then: Boolean = false, work: () -> Unit) {
        if (acted) return
        acted = true
        if (!then) work()
        scope.launch { state.hide() }.invokeOnCompletion { onDismiss(); if (then) work() }
    }
    val mine = !song.onComputer()
    // The speed, when the player is not the computer's: read once, off the main thread.
    val speed by androidx.compose.runtime.produceState<String?>(null, song.id) {
        if (mine) value = withContext(Dispatchers.IO) { runCatching { core.speedAndPitch() }.getOrNull() }?.takeIf { it.available }?.let { speedText(it.speed) }
    }
    // The song: its cover, its name and stars; the four pills; under a thin line, the rare ones.
    val songPart: @Composable () -> Unit = {
        Row(
            Modifier.fillMaxWidth().padding(start = 10.dp, end = 10.dp, top = 6.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(14.dp),
        ) {
            Cover(
                song.artwork,
                Modifier.size(56.dp).shadow(10.dp, RoundedCornerShape(10.dp), ambientColor = Color.Black.copy(alpha = 0.3f), spotColor = Color.Black.copy(alpha = 0.3f)),
                RoundedCornerShape(10.dp),
                seed = song.album + song.artist,
            )
            Column(Modifier.weight(1f)) {
                Text(
                    song.title,
                    style = MaterialTheme.typography.titleMedium.copy(fontSize = 19.sp, lineHeight = 24.sp),
                    fontWeight = FontWeight.Bold,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                    modifier = Modifier.semantics { heading() },
                )
                // The stars' first touch room starts under the name's first letter.
                if (mine) SongStars(song, Modifier.offset(x = (-12).dp), size = 24.dp, arrangement = Arrangement.Start)
            }
        }
        if (mine) {
            // Four pills, two by two: what is done with the song, and where it goes.
            Column(Modifier.fillMaxWidth().padding(start = 6.dp, end = 6.dp, top = 16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    MorePill(Icons.AutoMirrored.Rounded.PlaylistAdd, "Playlist", "Add to a playlist", Modifier.weight(1f)) {
                        act(then = true) { Ui.sheet.value = Sheet.AddToPlaylist(listOf(song.id)) }
                    }
                    MorePill(Icons.Rounded.Radio, "Radio", "Start radio", Modifier.weight(1f)) { act { startRadioFrom(song) } }
                }
                Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    val album = song.album.ifBlank { "Unknown album" }
                    val artist = song.artist.ifBlank { "Unknown artist" }
                    MorePill(Icons.Rounded.Album, album, "Go to the album, $album", Modifier.weight(1f)) { act { open(Route.AlbumOf(song.album, song.id)) } }
                    MorePill(Icons.Rounded.Person, artist, "Go to the artist, $artist", Modifier.weight(1f)) { act { open(Route.Artist(song.artist)) } }
                }
            }
        }
    }
    val rarePart: @Composable () -> Unit = {
        if (mine) {
            Box(Modifier.fillMaxWidth().padding(start = 10.dp, end = 10.dp, top = 8.dp, bottom = 4.dp).height(1.dp).background(MaterialTheme.colorScheme.onSurface.copy(alpha = 0.13f)))
            SongFileRows(song, { then, work -> act(then, work) }) { icon, label, click -> MoreRow({ Icon(icon, contentDescription = null) }, label, onClick = click) }
            MoreRow({ Icon(Icons.Rounded.Lyrics, contentDescription = null) }, "Time the lyrics") { act { open(Route.Timing(song.id)) } }
            songCommands().forEach { (plugin, command) ->
                MoreRow({ Icon(Icons.Rounded.Extension, contentDescription = null) }, command.title) { act(then = true) { core.runPluginCommand(plugin, command.id, listOf(song.id)) } }
            }
        }
    }
    // The player: the look, then plain rows.
    val playerPart: @Composable (Dp) -> Unit = { top ->
        Text("Look", style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.padding(start = 10.dp, top = top))
        Segments(BigLooks.map { lookName(it) }, BigLooks.indexOf(bigLook), 40.dp, Modifier.padding(top = 6.dp, bottom = 4.dp)) { onLook(BigLooks[it]) }
        MoreRow({ Text("A-B", style = MaterialTheme.typography.labelLarge, fontWeight = FontWeight.Bold, maxLines = 1, softWrap = false) }, "A-B loop", loopText) { act(work = onLoop) }
        if (mine) MoreRow({ Icon(Icons.Rounded.Speed, contentDescription = null) }, "Speed and pitch", speed) { act(then = true) { Ui.sheet.value = Sheet.SpeedPitch } }
        MoreRow({ Icon(Icons.Rounded.Fullscreen, contentDescription = null) }, "Full screen") { act(work = onFull) }
        // Nothing is lost: where the pill of tools is not on the screen, its rows stay here.
        if (!tools) {
            MoreRow({ Icon(Icons.Rounded.Bedtime, contentDescription = null) }, "Sleep timer", sleepText) { act(then = true, work = onSleep) }
            MoreRow({ Icon(Icons.Rounded.Cast, contentDescription = null) }, "Play on", speaker) { act(then = true, work = onPlayOn) }
        }
    }
    GlassSheet(onDismissRequest = onDismiss, sheetState = state, art = song.artwork, maxWidth = if (twoColumns) 720.dp else androidx.compose.material3.BottomSheetDefaults.SheetMaxWidth) {
        // Scrolled where it is taller than the screen (a phone on its side, a TV, large text).
        Column(Modifier.verticalScroll(rememberScrollState()).navigationBarsPadding().padding(horizontal = 16.dp).padding(bottom = 16.dp)) {
            if (twoColumns) {
                // Two columns that start at the same height, 24 dp apart, no line between.
                Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(24.dp)) {
                    Column(Modifier.weight(1f)) {
                        songPart()
                        rarePart()
                    }
                    Column(Modifier.weight(1f)) { playerPart(6.dp) }
                }
            } else {
                songPart()
                playerPart(16.dp)
                rarePart()
            }
        }
    }
}

/** The speed for More's row: "1.0×", "1.25×". */
private fun speedText(speed: Float): String {
    val tenths = kotlin.math.round(speed * 10f)
    return if (kotlin.math.abs(speed * 10f - tenths) < 0.01f) "%.1f×".format(java.util.Locale.ROOT, speed) else "%.2f×".format(java.util.Locale.ROOT, speed)
}

/**
 * One of More's four pills: a full pill on a faint flat fill (the words' colour at 10 %, no
 * glass), an icon and one line of words that ends with …. `job`: what a screen reader says.
 */
@Composable
private fun MorePill(icon: ImageVector, label: String, job: String, modifier: Modifier = Modifier, onClick: () -> Unit) {
    val press = remember { androidx.compose.foundation.interaction.MutableInteractionSource() }
    val words = MaterialTheme.colorScheme.onSurface
    Row(
        modifier
            .height(46.dp)
            .pressScale(press, ButtonPress, corner = 23.dp)
            .clip(CircleShape)
            .background(words.copy(alpha = 0.1f))
            .clickable(interactionSource = press, indication = androidx.compose.material3.ripple(), role = androidx.compose.ui.semantics.Role.Button, onClick = onClick)
            .clearAndSetSemantics { contentDescription = job }
            .padding(start = 14.dp, end = 16.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        Icon(icon, contentDescription = null, tint = words.copy(alpha = 0.86f), modifier = Modifier.size(20.dp))
        Text(label, style = MaterialTheme.typography.labelLarge.copy(fontSize = 14.5.sp), color = words, maxLines = 1, overflow = TextOverflow.Ellipsis)
    }
}

/** A plain row of More: an icon, the words, and at the right what it is set to. */
@Composable
private fun MoreRow(icon: @Composable () -> Unit, label: String, value: String? = null, onClick: () -> Unit) {
    Row(
        Modifier
            .fillMaxWidth()
            .heightIn(min = 54.dp)
            .clip(RowShape)
            .clickable(role = androidx.compose.ui.semantics.Role.Button, onClick = onClick)
            .semantics(mergeDescendants = true) { if (value != null) stateDescription = value }
            .padding(horizontal = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(18.dp),
    ) {
        Box(Modifier.size(28.dp), contentAlignment = Alignment.Center) {
            CompositionLocalProvider(LocalContentColor provides MaterialTheme.colorScheme.onSurface.copy(alpha = 0.7f)) { icon() }
        }
        Text(label, style = MaterialTheme.typography.bodyLarge, modifier = Modifier.weight(1f))
        if (value != null) {
            Text(value, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.widthIn(max = 180.dp).clearAndSetSemantics {})
        }
    }
}

/**
 * Up next on a phone, as a glass sheet: its name, how many songs and how long, Shuffle as a pill
 * in the accent and Add songs as a plain one, then the songs (moved by their handle, taken out
 * by a swipe).
 */
@OptIn(androidx.compose.material3.ExperimentalMaterial3Api::class)
@Composable
private fun UpNextSheet(version: ULong, count: Int, art: String?, onAddSongs: () -> Unit, onDismiss: () -> Unit) {
    val state = androidx.compose.material3.rememberModalBottomSheetState(skipPartiallyExpanded = true)
    val scope = androidx.compose.runtime.rememberCoroutineScope()
    var acted by remember { mutableStateOf(false) }
    GlassSheet(onDismissRequest = onDismiss, sheetState = state, art = art) {
        Column(Modifier.fillMaxWidth().fillMaxHeight(0.85f).padding(horizontal = 20.dp)) {
            UpNextPanel(version, titled = false) { list ->
                Text("Up next", style = MaterialTheme.typography.headlineSmall, fontWeight = FontWeight.Bold, modifier = Modifier.padding(start = 4.dp).semantics { heading() })
                val songs = maxOf(count, list.size)
                val words = buildString {
                    append(if (songs == 1) "1 song" else "$songs songs")
                    // The length only when every song is read (the first 200 are).
                    if (songs > 0 && songs <= list.size) {
                        val minutes = kotlin.math.round(list.sumOf { it.duration.coerceAtLeast(0.0) } / 60).toInt()
                        append(" · ")
                        append(if (minutes >= 60) "${minutes / 60} h ${minutes % 60} min" else if (minutes == 1) "1 minute" else "$minutes minutes")
                    }
                }
                Text(words, style = MaterialTheme.typography.bodyMedium, color = Color.White.copy(alpha = 0.66f), modifier = Modifier.padding(start = 4.dp, top = 2.dp))
                UpNextActions(Modifier.padding(top = 14.dp, bottom = 8.dp)) {
                    if (!acted) {
                        acted = true
                        scope.launch { state.hide() }.invokeOnCompletion { onDismiss(); onAddSongs() }
                    }
                }
            }
        }
    }
}

/**
 * Up next's small line (the phone's sheet, the wide player's pane): Shuffle, a pill in the accent,
 * and Add songs, a plain one. On a narrow pane the second goes under the first.
 */
@OptIn(androidx.compose.foundation.layout.ExperimentalLayoutApi::class)
@Composable
private fun UpNextActions(modifier: Modifier = Modifier, onAddSongs: () -> Unit) {
    androidx.compose.foundation.layout.FlowRow(modifier, horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        val stays = NeedleApp.instance.more.collectAsState().value.shuffleStays && !onComputer()
        SheetPill(Icons.Rounded.Shuffle, "Shuffle", filled = true) {
            Controls.shuffle()
            if (!stays) showMessage("Shuffled what is up next")
        }
        SheetPill(Icons.Rounded.Add, "Add songs", onClick = onAddSongs)
    }
}

/** A pill in a sheet: filled in the accent, or a plain one of faint white. */
@Composable
private fun SheetPill(icon: ImageVector, label: String, filled: Boolean = false, onClick: () -> Unit) {
    val press = remember { androidx.compose.foundation.interaction.MutableInteractionSource() }
    Row(
        Modifier
            .heightIn(min = 40.dp)
            .pressScale(press, ButtonPress, corner = 20.dp)
            .clip(CircleShape)
            .background(if (filled) MaterialTheme.colorScheme.primary else Color.White.copy(alpha = 0.12f))
            .clickable(interactionSource = press, indication = androidx.compose.material3.ripple(), role = androidx.compose.ui.semantics.Role.Button, onClick = onClick)
            .padding(start = 14.dp, end = 18.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        val ink = if (filled) MaterialTheme.colorScheme.onPrimary else Color.White
        Icon(icon, contentDescription = null, tint = ink, modifier = Modifier.size(20.dp))
        Text(label, style = MaterialTheme.typography.labelLarge, color = ink)
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
    // Only the computer's volume, not the whole playback, which changes at every tick.
    val playbackState = NeedleApp.instance.playback.collectAsState()
    val pcVolume by remember { androidx.compose.runtime.derivedStateOf { playbackState.value?.volume ?: 1f } }
    var held by remember { mutableStateOf<Float?>(null) }
    // The phone's volume can change by its buttons, so it is read again every half second.
    var phone by remember { mutableFloatStateOf(audio.getStreamVolume(stream).toFloat() / audio.getStreamMaxVolume(stream)) }
    LaunchedEffect(computer) {
        while (!computer) {
            phone = audio.getStreamVolume(stream).toFloat() / audio.getStreamMaxVolume(stream)
            kotlinx.coroutines.delay(500)
        }
    }
    val value = held ?: if (computer) pcVolume else phone
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
                    how == "now" -> { core.play(listOf(id), 0u); startedFrom.value = null }
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
    GlassSheet(onDismissRequest = onDismiss) {
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
                        IconButton(onClick = { act(song, "next") }) { Icon(Icons.Rounded.QueuePlayNext, contentDescription = "Play ${song.title} next") }
                        IconButton(onClick = { act(song, "end") }) { Icon(Icons.AutoMirrored.Rounded.QueueMusic, contentDescription = "Add ${song.title} to the queue") }
                    }
                }
            }
        }
    }
}

/**
 * The playing cover, in the shape chosen in Settings › Appearance: a rounded square; a record
 * that turns while the song plays; or one of Material's shapes, slowly turning.
 */
@OptIn(androidx.compose.material3.ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun PlayerCover(song: Song, playing: Boolean, haptics: (Boolean) -> Unit, record: Boolean = false) {
    val app by NeedleApp.instance.app.collectAsState()
    // The Record look: the round cover's turning record, whatever the cover shape chosen.
    val coverShape = if (record) "round" else app.coverShape
    // Swiped sideways (when that is on), the cover goes with the finger; far enough or thrown,
    // it goes on off and the next song's (or the last one's) slides in from the other side.
    val swipe = rememberSkipSwipe(haptics)
    LaunchedEffect(song.id) { swipe.arrived() }
    val reduce = reduceMotion()
    // Paused, the cover rests a little smaller with less shadow: a spring, so play and pause
    // tapped quickly go on from where it is. Reduce motion: at once.
    val scale by animateFloatAsState(if (playing) 1f else 0.82f, if (reduce) snap() else spring(dampingRatio = 0.8f, stiffness = 380f), label = "cover size")
    val still = reduce || coverShape == "square"
    // Turning only while it plays and is not square: a turn that stands still takes no frames.
    // Read only where the layer draws, so the turning redraws the cover, not the whole player.
    val turn: androidx.compose.runtime.State<Float> = if (!still && playing) {
        androidx.compose.animation.core.rememberInfiniteTransition(label = "spin").animateFloat(
            0f, 360f,
            androidx.compose.animation.core.infiniteRepeatable(tween(if (coverShape == "round") 12_000 else 40_000, easing = androidx.compose.animation.core.LinearEasing)),
            label = "turn",
        )
    } else remember { mutableFloatStateOf(0f) }
    // The turn holds where it was last drawn while paused, and goes on from there: the angle is
    // taken as playing starts or stops, before that frame draws (a new turn starts at 0).
    val drawn = remember { floatArrayOf(0f) }
    val from = remember(playing, still) { drawn[0] }
    val shape = when (coverShape) {
        "round" -> CircleShape
        else -> RoundedCornerShape(12.dp)
    }
    val depth by animateFloatAsState(if (playing) 1f else 0.4f, if (reduce) snap() else spring(dampingRatio = 0.8f, stiffness = 380f), label = "shadow")
    BoxWithConstraints(
        Modifier
            .onSizeChanged { swipe.width = it.width.toFloat().coerceAtLeast(1f) }
            .graphicsLayer {
                val shift = swipe.shift
                translationX = shift
                alpha = 1f - (kotlin.math.abs(shift) / swipe.width * 0.7f).coerceIn(0f, 0.7f)
            }
            .then(if (!app.swipeCover) Modifier else Modifier.skipSwipe(swipe)),
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
        // The Record look: a black record, still, and on it the cover as its label, turning.
        if (record) Disc(Modifier.size(side).graphicsLayer { scaleX = scale; scaleY = scale })
        val face = if (record) side * 0.38f else side
        Cover(
            song.artwork,
            Modifier
                .size(face)
                .sharedCover("now-${song.id}")
                .graphicsLayer {
                scaleX = scale
                scaleY = scale
                rotationZ = (if (still) 0f else if (playing) turn.value + from else from).also { drawn[0] = it }
            },
            shape,
            seed = song.album + song.artist,
        )
        // The moving cover (Settings › Appearance › Moving covers), over the still one in the
        // same place, shape, size, and turn: here, in the immersive cover, and on the album's
        // page (HeaderCover), never in lists.
        movingCover(song, app.animatedCovers && !reduce)?.let { path ->
            MovingCover(
                path,
                shade = null,
                modifier = Modifier
                    .size(face)
                    .graphicsLayer {
                        scaleX = scale
                        scaleY = scale
                        rotationZ = if (still) 0f else if (playing) turn.value + from else from
                        this.shape = shape
                        clip = true
                    },
            )
        }
        // A record's label hole: on the Record look's label, a small hole with a light ring.
        if (record) {
            Box(Modifier.size(face).graphicsLayer { scaleX = scale; scaleY = scale }.border(3.dp, Color.Black.copy(alpha = 0.4f), CircleShape))
            Box(Modifier.size(13.dp).graphicsLayer { scaleX = scale; scaleY = scale }.clip(CircleShape).background(Color.White.copy(alpha = 0.55f)))
            Box(Modifier.size(9.dp).graphicsLayer { scaleX = scale; scaleY = scale }.clip(CircleShape).background(Color(0xFF0A0A0B)))
        } else if (coverShape == "round") {
            Box(Modifier.size(34.dp).graphicsLayer { scaleX = scale; scaleY = scale }.clip(CircleShape).background(Color.Black.copy(alpha = 0.55f)))
            Box(Modifier.size(10.dp).clip(CircleShape).background(Color(0xFF0F0E0D)))
        }
    }
}

/** The big player's looks, in the order the Look button steps through them (`model::BIG_LOOKS`). */
private val BigLooks = listOf("cover", "record", "lyrics", "visualizer")

private fun lookName(look: String) = when (look) {
    "record" -> "Record"
    "lyrics" -> "Lyrics"
    "visualizer" -> "Visualizer"
    else -> "Cover"
}

/**
 * The Record look's record: black, with fine grooves from the label out to its rim and a soft
 * sheen that stays where the light is (only the label turns).
 */
@Composable
private fun Disc(modifier: Modifier) {
    androidx.compose.foundation.Canvas(modifier) {
        val r = size.minDimension / 2
        drawCircle(Color(0xFF0A0A0B), radius = r, center = center)
        val groove = Color(0xFF1D1D20)
        val step = 2.6.dp.toPx()
        var k = r * 0.98f
        while (k > r * 0.4f) {
            drawCircle(groove, radius = k, center = center, style = androidx.compose.ui.graphics.drawscope.Stroke(width = 1.3.dp.toPx()))
            k -= step
        }
        drawCircle(
            Brush.sweepGradient(
                0f to Color.Transparent,
                0.07f to Color.Transparent,
                0.12f to Color.White.copy(alpha = 0.14f),
                0.19f to Color.Transparent,
                0.5f to Color.Transparent,
                0.58f to Color.White.copy(alpha = 0.1f),
                0.66f to Color.Transparent,
                1f to Color.Transparent,
                center = center,
            ),
            radius = r,
            center = center,
        )
        drawCircle(Color.White.copy(alpha = 0.1f), radius = r - 0.5.dp.toPx(), center = center, style = androidx.compose.ui.graphics.drawscope.Stroke(width = 1.dp.toPx()))
    }
}

/**
 * The Lyrics look: only the words, very large and centred, lit as sung (word by word where
 * timed), the translation under each line when Translate is on; a song with none shows `none`.
 */
@Composable
private fun BigLyrics(song: Song, position: () -> Double, time: () -> Unit, onHas: (Boolean) -> Unit, none: @Composable () -> Unit) {
    val load = rememberLoaded(song.id) { if (song.onComputer()) null else lyrics(song.id) }
    val found by load
    val has = found?.let { it.lines.isNotEmpty() || it.plain.isNotBlank() } == true
    // The player shows the song small at the top only while the words take the cover's place.
    val showsWords = !load.loading && (has || song.onComputer())
    androidx.compose.runtime.SideEffect { onHas(showsWords) }
    when {
        load.loading -> Unit
        has || song.onComputer() -> LyricsPanel(song, position, big = true, time = time)
        else -> none()
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

/**
 * A thin bar that thickens while held, as in Apple Music. The time shown follows the finger;
 * the song moves only when it lets go.
 */
@Composable
private fun SeekBar(position: Double, duration: Double, playing: Boolean, heights: List<Float> = emptyList(), format: String = "") {
    var held by remember { mutableStateOf<Float?>(null) }
    val length = duration.coerceAtLeast(1.0).toFloat()
    // Where it was just sent: shown until the player is there (a moment later, at its next
    // report), so the bar does not jump back to the old place first.
    var sought by remember { mutableStateOf<Float?>(null) }
    LaunchedEffect(sought) {
        if (sought != null) {
            kotlinx.coroutines.delay(1000)
            sought = null
        }
    }
    val at = (position.toFloat() / length).coerceIn(0f, 1f)
    val shown = held ?: sought?.takeIf { kotlin.math.abs(at - it) * length >= 1.5f } ?: at
    fun seekTo(fraction: Float) {
        sought = fraction.coerceIn(0f, 1f)
        Controls.seek(sought!! * length.toDouble())
    }
    // With a remote or keys: the bar takes the focus, and left and right go back or on ten seconds.
    val focus = remember { androidx.compose.foundation.interaction.MutableInteractionSource() }
    val focused by focus.collectIsFocusedAsState()
    val thickness by animateDpAsState(if (held != null || focused) 10.dp else 5.dp, label = "seek thickness")
    Column {
        Box(
            Modifier
                .fillMaxWidth()
                // A full finger's height to take hold of; the line itself stays thin.
                .height(48.dp)
                .onKeyEvent {
                    val step = when (it.key) {
                        Key.DirectionLeft -> -10.0
                        Key.DirectionRight -> 10.0
                        else -> return@onKeyEvent false
                    }
                    if (it.type == KeyEventType.KeyDown) seekTo(shown + (step / length).toFloat())
                    true
                }
                .focusable(interactionSource = focus)
                .semantics {
                    contentDescription = "Seek"
                    // Where the song is, and a way for TalkBack to move it.
                    progressBarRangeInfo = androidx.compose.ui.semantics.ProgressBarRangeInfo(shown, 0f..1f)
                    // The time, not a percent.
                    stateDescription = if (duration > 0) "${time((shown * length).toDouble())} of ${time(duration)}" else time((shown * length).toDouble())
                    setProgress { to -> seekTo(to); true }
                }
                .pointerInput(length) {
                    detectTapGestures { seekTo(it.x / size.width) }
                }
                .pointerInput(length) {
                    detectHorizontalDragGestures(
                        onDragStart = { held = (it.x / size.width).coerceIn(0f, 1f) },
                        onDragEnd = { held?.let { seekTo(it) }; held = null },
                        onDragCancel = { held = null },
                    ) { change, _ -> held = (change.position.x / size.width).coerceIn(0f, 1f) }
                },
            contentAlignment = Alignment.CenterStart,
        ) {
            // What has played is in the accent.
            val accent = MaterialTheme.colorScheme.primary
            val style = seekStyle()
            if (held == null && !focused || heights.isNotEmpty()) {
                ProgressLine(shown, playing, accent, Color.White.copy(alpha = 0.24f), heights = heights)
                // A line (thin or thick) has a small white handle where the song is.
                if (style == SeekStyle.Line || style == SeekStyle.Thick) {
                    androidx.compose.foundation.Canvas(Modifier.fillMaxWidth().height(20.dp)) {
                        val w = 6.dp.toPx()
                        val x = (shown * size.width - w / 2).coerceIn(0f, (size.width - w).coerceAtLeast(0f))
                        drawRoundRect(Color.White, topLeft = androidx.compose.ui.geometry.Offset(x, 0f), size = androidx.compose.ui.geometry.Size(w, size.height), cornerRadius = androidx.compose.ui.geometry.CornerRadius(w / 2))
                    }
                }
            } else {
                Box(Modifier.fillMaxWidth().height(thickness).clip(CircleShape).background(Color.White.copy(alpha = 0.24f)))
                Box(Modifier.fillMaxWidth(shown).height(thickness).clip(CircleShape).background(accent))
            }
        }
        // The two times, and between them the song's format ("FLAC 24/48") in the same faint
        // letters. Close under the bar, which they belong to.
        Row(
            Modifier.fillMaxWidth().offset(y = (-6).dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            val faint = Color.White.copy(alpha = 0.62f)
            Text(time((shown * length).toDouble()), style = MaterialTheme.typography.labelMedium, color = faint)
            Spacer(Modifier.weight(1f))
            if (format.isNotBlank()) {
                Text(format, style = MaterialTheme.typography.labelMedium, color = faint, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.padding(horizontal = 8.dp).weight(1f, fill = false))
                Spacer(Modifier.weight(1f))
            }
            // A song whose length is not known has no time left to show.
            if (duration > 0) Text("-" + time(length.toDouble() - shown * length), style = MaterialTheme.typography.labelMedium, color = faint)
        }
    }
}

@Composable
private fun LyricsPanel(song: Song, position: () -> Double, compact: Boolean = false, big: Boolean = false, time: () -> Unit) {
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
    // The line sung now, changing only when the line does: the lines are not all redrawn at
    // every tick of the position. Only the line sung, word by word, reads the position itself.
    val now by remember(lines) { androidx.compose.runtime.derivedStateOf { lines.indexOfLast { it.time <= position() + 0.2 } } }
    val app by NeedleApp.instance.app.collectAsState()
    // Scrolled by hand, the lyrics stay where the reader put them for 3 seconds after the finger
    // lets go, then go back to the line sung.
    val dragged by state.interactionSource.collectIsDraggedAsState()
    var reading by remember { mutableStateOf(false) }
    LaunchedEffect(dragged) {
        if (dragged) reading = true
        else if (reading) {
            kotlinx.coroutines.delay(3000)
            reading = false
        }
    }
    LaunchedEffect(now, app.liveLyrics, reading) {
        if (app.liveLyrics && now >= 0 && !reading) state.animateScrollToItem((now - 1).coerceAtLeast(0))
    }
    val stillLines = reduceMotion()
    // Keeps the screen on while the lyrics show, when that is on.
    KeepScreenOn("lyrics", app.keepScreenOnLyrics)
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
        else -> Column(Modifier.fillMaxSize()) {
          // Each line in the phone's own language under it, translated on the phone.
          var translate by androidx.compose.runtime.saveable.rememberSaveable(song.id) { mutableStateOf(false) }
          val translated = rememberTranslation(song, lines.map { it.text }, translate)
          var card by remember(song.id) { mutableStateOf<String?>(null) }
          val context = androidx.compose.ui.platform.LocalContext.current
          val scheme = MaterialTheme.colorScheme
          card?.let { text ->
              ActionSheet(song.artwork, song.title, song.artist, { card = null }) { act ->
                  Text("“$text”", style = MaterialTheme.typography.bodyLarge, modifier = Modifier.padding(horizontal = 8.dp, vertical = 8.dp))
                  MenuGroup {
                      MenuRow(Icons.Rounded.IosShare, "Share as a lyric card") {
                          // The cover's colour, deepening to near black; the words white, so they always read.
                          act.after { shareLyricCard(context, song, text, androidx.compose.ui.graphics.lerp(scheme.primary, Color.Black, 0.35f) to androidx.compose.ui.graphics.lerp(scheme.primary, Color.Black, 0.88f), Color.White) }
                      }
                  }
              }
          }
          Row(verticalAlignment = Alignment.CenterVertically) {
              androidx.compose.material3.TextButton(
                  onClick = { translate = !translate },
                  colors = androidx.compose.material3.ButtonDefaults.textButtonColors(contentColor = if (translate) MaterialTheme.colorScheme.primary else Color.White.copy(alpha = 0.75f)),
              ) {
                  Icon(Icons.Rounded.Translate, contentDescription = null, modifier = Modifier.size(18.dp))
                  Spacer(Modifier.width(6.dp))
                  Text(if (translate) "Translated" else "Translate")
              }
              when (val t = translated) {
                  Translated.Working -> Text("Translating…", style = MaterialTheme.typography.labelMedium, color = Color.White.copy(alpha = 0.6f))
                  is Translated.Failed -> Text(t.why, style = MaterialTheme.typography.labelMedium, color = Color.White.copy(alpha = 0.6f), maxLines = 2)
                  else -> {}
              }
          }
          val under = (translated as? Translated.Lines)?.lines
          LazyColumn(Modifier.fillMaxSize(), state = state) {
            itemsIndexed(lines) { i, line ->
                val lit = !app.liveLyrics || i == now
                val alpha by animateFloatAsState(if (lit) 1f else if (i < now) 0.25f else 0.4f, tween(300), label = "line")
                val grow by animateFloatAsState(if (i == now && app.liveLyrics) 1f else 0.94f, if (stillLines) snap() else spring(dampingRatio = 0.7f, stiffness = 300f), label = "line size")
                // Karaoke: the words sung so far are lit, the rest wait.
                val sung = if (i == now && line.words.isNotEmpty() && app.liveLyrics) {
                    val at = position()
                    androidx.compose.ui.text.buildAnnotatedString {
                        line.words.forEach { word ->
                            withStyle(androidx.compose.ui.text.SpanStyle(color = if (word.time <= at) Color.White else Color.White.copy(alpha = 0.4f))) {
                                append(word.text)
                            }
                        }
                    }
                } else null
                Text(
                    sung ?: androidx.compose.ui.text.AnnotatedString(line.text.ifBlank { "♪" }),
                    style = when {
                        big -> MaterialTheme.typography.displaySmall
                        compact -> MaterialTheme.typography.titleLarge
                        else -> MaterialTheme.typography.headlineMedium
                    },
                    fontWeight = FontWeight.Bold,
                    color = Color.White.copy(alpha = alpha),
                    textAlign = if (big) androidx.compose.ui.text.style.TextAlign.Center else null,
                    modifier = Modifier
                        .fillMaxWidth()
                        .graphicsLayer { scaleX = grow; scaleY = grow; transformOrigin = TransformOrigin(if (big) 0.5f else 0f, 0.5f) }
                        .clip(RoundedCornerShape(8.dp))
                        .combinedClickable(
                            onClickLabel = "Play from this line",
                            onLongClickLabel = "Share as a lyric card",
                            onLongClick = { if (line.text.isNotBlank()) card = line.text },
                        ) { Controls.seek(line.time) }
                        .semantics { if (i == now) stateDescription = "Sung now" }
                        .padding(top = 12.dp, bottom = if (under?.getOrNull(i).isNullOrBlank()) 12.dp else 2.dp),
                )
                under?.getOrNull(i)?.takeIf { it.isNotBlank() }?.let { t ->
                    Text(
                        t,
                        style = if (compact) MaterialTheme.typography.bodyLarge else if (big) MaterialTheme.typography.titleLarge else MaterialTheme.typography.titleMedium,
                        color = Color.White.copy(alpha = alpha * 0.7f),
                        textAlign = if (big) androidx.compose.ui.text.style.TextAlign.Center else null,
                        modifier = Modifier.fillMaxWidth().padding(bottom = 12.dp),
                    )
                }
            }
          }
        }
    }
}

@OptIn(androidx.compose.foundation.ExperimentalFoundationApi::class)
@Composable
private fun UpNextPanel(version: ULong, titled: Boolean = true, top: (@Composable (List<Song>) -> Unit)? = null) {
    // Read again at each change of the queue, with the old list kept on screen until the new
    // one is in: no blank moment, and the list keeps its place as songs are moved or taken out.
    var songs by remember { mutableStateOf<List<Song>?>(null) }
    LaunchedEffect(version) {
        // The first 200: a whole library queued would be thousands of songs read at each change.
        withContext(Dispatchers.IO) { runCatching { Controls.upNextFirst(200) }.getOrNull() }?.let { songs = it }
    }
    val list = songs.orEmpty()
    val onComputer = list.firstOrNull()?.onComputer() == true
    // A change shows at once, before the player has taken it in and the list is read again:
    // a song dropped in its new place stays there, rather than going back for a moment.
    val move: (Int, Int) -> Unit = { from, to ->
        songs = songs?.toMutableList()?.apply { if (from in indices && to in indices) add(to, removeAt(from)) }
        core.moveUpNext(from.toUInt(), to.toUInt())
    }
    val remove: (Int) -> Unit = { i ->
        songs = songs?.toMutableList()?.apply { if (i in indices) removeAt(i) }
        core.removeUpNext(i.toUInt())
    }
    // Up next on this phone can be put in order by dragging a song's handle.
    val upNextList = androidx.compose.foundation.lazy.rememberLazyListState()
    val reorder = rememberReorder(list.size, upNextList) { from, to -> move(from, to) }
    Column(Modifier.fillMaxHeight()) {
        if (titled) Text("Up next", style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.Bold, modifier = Modifier.padding(vertical = 8.dp))
        // A sheet's heading (Up next on a phone): what is up next, and its buttons.
        top?.invoke(list)
        if (songs != null && list.isEmpty()) {
            Text("Nothing up next", color = Color.White.copy(alpha = 0.6f))
            return@Column
        }
        LazyColumn(Modifier.fillMaxSize(), state = upNextList) {
            val keys = songKeys(list)
            reorder.keys = keys
            itemsIndexed(list, key = { i, _ -> keys[i] }) { i, song ->
                // Swiped away, a song leaves the queue.
                // Fresh each time the queue's length changes: a row's key can pass to another
                // copy of the same song after one is taken out, and must not bring it swiped.
                val dismiss = androidx.compose.runtime.key(list.size) { rememberSwipeToDismissBoxState() }
                LaunchedEffect(dismiss.currentValue) {
                    if (dismiss.currentValue != SwipeToDismissBoxValue.Settled && !song.onComputer()) remove(i)
                }
                SwipeToDismissBox(
                    // The rows slide when one goes or moves (Reorder.kt).
                    modifier = Modifier.movingRow(this, reorder).then(if (onComputer) Modifier else Modifier.reorderRow(reorder, i)),
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
                    val moveTo: (Int) -> Unit = { to -> move(i, to) }
                    // A soft highlight under a mouse's pointer, as the computer's queue has.
                    val pointer = remember { androidx.compose.foundation.interaction.MutableInteractionSource() }
                    val hovered by pointer.collectIsHoveredAsState()
                    Box {
                    Row(
                        Modifier
                            .fillMaxWidth()
                            .clip(RoundedCornerShape(8.dp))
                            .background(if (hovered) Color.White.copy(alpha = 0.08f) else Color.Transparent)
                            .hoverable(pointer)
                            .combinedClickable(onClick = { Controls.jump(i, song) }, onLongClick = { if (!onComputer) menu = true })
                            .semantics {
                                if (!onComputer) customActions = listOfNotNull(
                                    if (i > 0) CustomAccessibilityAction("Move up") { moveTo(i - 1); true } else null,
                                    if (i < list.size - 1) CustomAccessibilityAction("Move down") { moveTo(i + 1); true } else null,
                                    CustomAccessibilityAction("Remove") { remove(i); true },
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
                    GlassMenu(menu, { menu = false }) {
                        if (i > 0) androidx.compose.material3.DropdownMenuItem(text = { Text("Move up") }, onClick = { menu = false; moveTo(i - 1) })
                        if (i < list.size - 1) androidx.compose.material3.DropdownMenuItem(text = { Text("Move down") }, onClick = { menu = false; moveTo(i + 1) })
                        androidx.compose.material3.DropdownMenuItem(text = { Text("Remove") }, onClick = { menu = false; remove(i) })
                    }
                    }
                }
            }
        }
    }
}

/** Play and pause as one button for TalkBack: "Play" or "Pause", not a checked box. */
internal fun Modifier.playPauseSemantics(playing: Boolean, toggle: () -> Unit) = clearAndSetSemantics {
    role = androidx.compose.ui.semantics.Role.Button
    contentDescription = if (playing) "Pause" else "Play"
    onClick { toggle(); true }
}

/** Play and pause, turning into each other (Reduce motion: a fade alone). */
@Composable
internal fun PlayPauseIcon(playing: Boolean, size: Dp) {
    val reduce = reduceMotion()
    AnimatedContent(
        playing,
        transitionSpec = {
            if (reduce) fadeIn(tween(150)) togetherWith fadeOut(tween(120))
            else (scaleIn(tween(180), 0.6f) + fadeIn(tween(180))) togetherWith (scaleOut(tween(140), 0.6f) + fadeOut(tween(140)))
        },
        label = "play pause",
    ) { on ->
        Icon(
            if (on) Icons.Rounded.Pause else Icons.Rounded.PlayArrow,
            contentDescription = if (on) "Pause" else "Play",
            modifier = Modifier.size(size),
        )
    }
}

/**
 * The playing position, read here alone: only what shows it (the seek bar, the progress line,
 * the lyrics) is redrawn at each tick, not the whole player around it.
 */
@Composable
private fun AtPosition(content: @Composable (Double) -> Unit) {
    val playback by NeedleApp.instance.playback.collectAsState()
    content(playback?.position ?: 0.0)
}

/**
 * The immersive cover's room in the player: swiped sideways, it skips, as the cover does; the
 * cover behind (ImmersiveCover) moves with the finger.
 */
@Composable
private fun SwipeSpace(on: Boolean, swipe: SkipSwipe) {
    Box(Modifier.fillMaxSize().then(if (on) Modifier.skipSwipe(swipe) else Modifier))
}

/**
 * Something swiped sideways to skip songs: the mini player, the player's cover, the immersive
 * cover. It follows the finger 1:1; let go far enough or fast enough (Motion.kt `flickWay`), it
 * goes on that way at the finger's speed, the song skips, and the new song's comes in from the
 * other side. At the end of the list it springs back instead.
 */
@androidx.compose.runtime.Stable
class SkipSwipe(private val scope: kotlinx.coroutines.CoroutineScope) {
    val slide = androidx.compose.animation.core.Animatable(0f)
    /** The whole way across, in pixels. */
    var width by mutableFloatStateOf(1f)
    private var dragging by mutableStateOf(false)
    private var drag by mutableFloatStateOf(0f)
    /** Which way the last swipe skipped, until its song shows: 1 next, -1 back. */
    private var swiped by androidx.compose.runtime.mutableIntStateOf(0)
    var density: androidx.compose.ui.unit.Density = androidx.compose.ui.unit.Density(1f)
    var reduce = false
    var haptics: (Boolean) -> Unit = {}

    /** Where it is drawn now (read in a graphicsLayer, so only the drawing follows it). */
    val shift: Float get() = if (dragging) drag else slide.value

    fun start() {
        // Caught on its way: it goes on from where it is.
        drag = slide.value
        dragging = true
        scope.launch { slide.stop() }
    }

    fun move(dx: Float) {
        drag += dx
    }

    fun release(velocity: Float, cancelled: Boolean = false) {
        val from = drag
        // Positive is to the right: the song before.
        val way = if (cancelled) 0 else with(density) { flickWay(from, velocity, width) }
        scope.launch {
            // Taken over where the finger left it, before it is drawn from the animation.
            slide.snapTo(from)
            dragging = false
            val skip = -way
            if (way == 0 || skipBringsSong(skip) == false) {
                if (reduce) slide.snapTo(0f) else slide.animateTo(0f, snapBackSpring(), initialVelocity = velocity)
                return@launch
            }
            haptics(false)
            swiped = skip
            if (reduce) {
                skipNow(skip)
                slide.snapTo(0f)
                return@launch
            }
            var sent = false
            slide.animateTo(way * width * 1.1f, flickSpring(), initialVelocity = velocity) {
                if (!sent && kotlin.math.abs(value) >= width * 0.9f) {
                    sent = true
                    skipNow(skip)
                }
            }
            if (!sent) skipNow(skip)
            // No other song came (the song before may only start this one again): it comes back.
            kotlinx.coroutines.delay(if (onComputer()) 1500 else 400)
            if (swiped == skip) {
                swiped = 0
                slide.animateTo(0f, flickSpring())
            }
        }
    }

    private fun skipNow(skip: Int) {
        if (skip > 0) Controls.next() else Controls.previous()
    }

    /** The song changed: after a swipe, the new one comes in from the side opposite the throw. */
    suspend fun arrived() {
        val from = swiped
        swiped = 0
        if (from == 0 || reduce) slide.snapTo(0f) else {
            slide.snapTo(from * width)
            slide.animateTo(0f, flickSpring())
        }
    }
}

@Composable
private fun rememberSkipSwipe(haptics: (Boolean) -> Unit): SkipSwipe {
    val scope = androidx.compose.runtime.rememberCoroutineScope()
    val swipe = remember { SkipSwipe(scope) }
    val density = androidx.compose.ui.platform.LocalDensity.current
    val reduce = reduceMotion()
    androidx.compose.runtime.SideEffect {
        swipe.density = density
        swipe.reduce = reduce
        swipe.haptics = haptics
    }
    return swipe
}

/** Swiping sideways moves [swipe] (1:1) and, let go, skips or springs back. */
private fun Modifier.skipSwipe(swipe: SkipSwipe): Modifier = pointerInput(swipe) {
    detectFlick(
        horizontal = true,
        onStart = { swipe.start() },
        onCancel = { swipe.release(0f, cancelled = true) },
        onEnd = { swipe.release(it) },
    ) { change, dx ->
        change.consume()
        swipe.move(dx)
    }
}

/** Who keeps the screen on now (each lyrics, player, visualizer shown): on while one does. */
private val keepingOn = mutableSetOf<Any>()

/**
 * Keeps the screen on while this shows and [on] holds. Several can ask at once (the player and
 * its lyrics): one going away does not turn the screen off under another.
 */
@Composable
fun KeepScreenOn(who: String, on: Boolean) {
    val view = LocalView.current
    // This one's own token: two of a kind can show at once (the lyrics in the side pane and in
    // the player, while one slides away), and one going must not turn the screen off for the
    // other. `who` names it for reading only.
    val token = remember(who) { Any() }
    DisposableEffect(on, token) {
        if (on) keepingOn += token else keepingOn -= token
        view.keepScreenOn = keepingOn.isNotEmpty()
        onDispose {
            keepingOn -= token
            view.keepScreenOn = keepingOn.isNotEmpty()
        }
    }
}
