package fyi.nnx.needle.ui

import androidx.compose.foundation.gestures.detectHorizontalDragGestures
import androidx.compose.foundation.gestures.detectVerticalDragGestures
import androidx.compose.ui.draw.drawWithContent
import androidx.compose.foundation.interaction.collectIsFocusedAsState
import android.provider.Settings
import androidx.compose.animation.AnimatedVisibilityScope
import androidx.compose.animation.ExperimentalSharedTransitionApi
import androidx.compose.animation.SharedTransitionScope
import androidx.compose.animation.core.FastOutSlowInEasing
import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.spring
import androidx.compose.animation.core.tween
import androidx.compose.animation.core.VisibilityThreshold
import androidx.compose.foundation.background
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material.icons.automirrored.rounded.ArrowBack
import androidx.compose.runtime.Composable
import androidx.compose.runtime.ProvidableCompositionLocal
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.compositionLocalOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.lifecycle.compose.currentStateAsState
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.Modifier
import androidx.compose.ui.composed
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import fyi.nnx.needle.NeedleApp

/** Motion is off when Needle's own switch is, or when Android's "Remove animations" is on. */
@Composable
fun reduceMotion(): Boolean = LocalReduceMotion.current

/** Worked out once for the whole app ([ProvideReduceMotion]), not at every tile that asks. */
val LocalReduceMotion = androidx.compose.runtime.compositionLocalOf { false }

@Composable
fun ProvideReduceMotion(content: @Composable () -> Unit) {
    val app by NeedleApp.instance.app.collectAsState()
    val context = LocalContext.current
    // Read again each time Needle comes back on screen: the switch may have changed meanwhile.
    val lifecycle by androidx.lifecycle.compose.LocalLifecycleOwner.current.lifecycle.currentStateAsState()
    val system = remember(lifecycle.isAtLeast(androidx.lifecycle.Lifecycle.State.RESUMED)) {
        Settings.Global.getFloat(context.contentResolver, Settings.Global.ANIMATOR_DURATION_SCALE, 1f) == 0f
    }
    androidx.compose.runtime.CompositionLocalProvider(LocalReduceMotion provides (app.reduceMotion || system), content = content)
}

/** A short tap of the phone's vibration, when that is on (Settings › Appearance). */
@Composable
fun rememberHaptics(): (strong: Boolean) -> Unit {
    val haptic = LocalHapticFeedback.current
    val app by NeedleApp.instance.app.collectAsState()
    return { strong ->
        if (app.haptics) {
            haptic.performHapticFeedback(if (strong) HapticFeedbackType.LongPress else HapticFeedbackType.TextHandleMove)
        }
    }
}

/** How far a round button shrinks while pressed (covers and tiles keep 0.96). */
const val ButtonPress = 0.92f

/**
 * Shrinks a little the moment it is pressed (0.96 for covers and tiles, 0.92 for round buttons),
 * and springs back with no bounce, as covers do in Apple Music. With a remote it grows a little
 * and takes a ring while it has the focus, as on a TV's home screen.
 */
fun Modifier.pressScale(interaction: MutableInteractionSource, scale: Float = 0.96f): Modifier = composed {
    val pressed by interaction.collectIsPressedAsState()
    val focused by interaction.collectIsFocusedAsState()
    val reduce = reduceMotion()
    val value by animateFloatAsState(
        when {
            reduce -> 1f
            pressed -> scale
            focused -> 1.04f
            else -> 1f
        },
        // Down quickly, starting fast; back up (or caught halfway) on a spring with no bounce.
        if (pressed) tween(100, easing = androidx.compose.animation.core.EaseOutCubic) else spring(dampingRatio = 1f, stiffness = 600f),
        label = "press",
    )
    graphicsLayer { scaleX = value; scaleY = value }.focusRing(interaction)
}

/**
 * A ring just outside what has a remote's focus, so its own corners do not cut it: for cards,
 * and for Material's filled buttons, whose own focus only darkens them. [corner] is the
 * thing's own corner; a pill or a circle keeps its shape.
 */
fun Modifier.focusRing(interaction: androidx.compose.foundation.interaction.InteractionSource, corner: Dp = 20.dp): Modifier = composed {
    val focused by interaction.collectIsFocusedAsState()
    val ring = MaterialTheme.colorScheme.onSurface
    drawWithContent {
        drawContent()
        if (!focused) return@drawWithContent
        val out = 5.dp.toPx()
        drawRoundRect(
            ring,
            topLeft = androidx.compose.ui.geometry.Offset(-out, -out),
            size = androidx.compose.ui.geometry.Size(size.width + out * 2, size.height + out * 2),
            cornerRadius = androidx.compose.ui.geometry.CornerRadius(minOf(corner.toPx(), size.minDimension / 2) + out),
            style = androidx.compose.ui.graphics.drawscope.Stroke(3.dp.toPx()),
        )
    }
}

/** Three bars that bounce while the song plays and rest while it is paused. */
@Composable
fun PlayingBars(playing: Boolean, modifier: Modifier = Modifier, color: Color = MaterialTheme.colorScheme.primary, height: Dp = 14.dp) {
    val reduce = reduceMotion()
    // Bouncing only while it plays: paused, nothing asks for frames.
    val moving = playing && !reduce
    val transition = if (moving) rememberInfiniteTransition(label = "bars") else null
    Row(modifier.size(width = 16.dp, height = height), horizontalArrangement = Arrangement.spacedBy(2.dp), verticalAlignment = Alignment.Bottom) {
        listOf(420, 310, 520).forEachIndexed { i, period ->
            // Read where it is drawn: the bars move at every frame, the row is not redrawn.
            val level: () -> Float = if (transition != null) {
                val bounce = transition.animateFloat(
                    initialValue = 0.25f,
                    targetValue = 1f,
                    animationSpec = infiniteRepeatable(tween(period, delayMillis = i * 60, easing = FastOutSlowInEasing), RepeatMode.Reverse),
                    label = "bar$i",
                )
                val read: () -> Float = { bounce.value }
                read
            } else {
                val rest = listOf(0.45f, 0.8f, 0.6f)[i]
                val read: () -> Float = { rest }
                read
            }
            Box(
                Modifier
                    .width(4.dp)
                    .fillMaxHeight()
                    .drawBehind {
                        val h = size.height * level()
                        drawRoundRect(
                            color,
                            topLeft = androidx.compose.ui.geometry.Offset(0f, size.height - h),
                            size = androidx.compose.ui.geometry.Size(size.width, h),
                            cornerRadius = androidx.compose.ui.geometry.CornerRadius(1.dp.toPx()),
                        )
                    },
            )
        }
    }
}

/** A soft sweep of light across a placeholder while its content loads; still with Reduce motion. */
fun Modifier.shimmer(): Modifier = composed {
    if (reduceMotion()) return@composed background(MaterialTheme.colorScheme.surfaceContainerHigh)
    val transition = rememberInfiniteTransition(label = "shimmer")
    val x by transition.animateFloat(
        initialValue = -1f,
        targetValue = 2f,
        animationSpec = infiniteRepeatable(tween(1200, easing = LinearEasing)),
        label = "sweep",
    )
    val base = MaterialTheme.colorScheme.surfaceContainerHigh
    val light = MaterialTheme.colorScheme.surfaceContainerHighest
    background(
        Brush.linearGradient(
            listOf(base, light, base),
            start = androidx.compose.ui.geometry.Offset(x * 600f, 0f),
            end = androidx.compose.ui.geometry.Offset(x * 600f + 600f, 300f),
        ),
    )
}

/** A placeholder album while a grid loads. */
@Composable
fun AlbumPlaceholder(modifier: Modifier = Modifier) {
    Column(modifier) {
        Box(Modifier.fillMaxWidth().aspectRatio(1f).clip(CoverShape).shimmer())
        Box(Modifier.padding(top = 10.dp).fillMaxWidth(0.7f).height(12.dp).clip(RoundedCornerShape(4.dp)).shimmer())
        Box(Modifier.padding(top = 6.dp).fillMaxWidth(0.45f).height(12.dp).clip(RoundedCornerShape(4.dp)).shimmer())
    }
}

/** A placeholder song row while a list loads. */
@Composable
fun SongPlaceholder() {
    Row(
        Modifier.fillMaxWidth().padding(horizontal = Edge, vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(14.dp),
    ) {
        Box(Modifier.size(48.dp).clip(SmallCoverShape).shimmer())
        Column(Modifier.weight(1f)) {
            Box(Modifier.fillMaxWidth(0.6f).height(12.dp).clip(RoundedCornerShape(4.dp)).shimmer())
            Box(Modifier.padding(top = 8.dp).fillMaxWidth(0.35f).height(10.dp).clip(RoundedCornerShape(4.dp)).shimmer())
        }
    }
}

// ---------- Shared elements: a cover moves from where it was to where it goes.

@OptIn(ExperimentalSharedTransitionApi::class)
val LocalShared: ProvidableCompositionLocal<SharedTransitionScope?> = compositionLocalOf { null }
val LocalAnimated: ProvidableCompositionLocal<AnimatedVisibilityScope?> = compositionLocalOf { null }

/**
 * Lets this cover fly between screens that show the same one under `key`: only while its own
 * page (or player) comes or goes. At rest a cover is a plain cover, with nothing for Compose to
 * keep in its top layer. A cover that kept its key at rest could stay there (seen rarely on a
 * phone: over Home's shelves and the dock, its own place empty), whether from a flight cut short,
 * a back swipe let go, or a shelf that changed order or was read again mid-flight.
 */
@OptIn(ExperimentalSharedTransitionApi::class)
fun Modifier.sharedCover(key: String): Modifier = composed {
    val shared = LocalShared.current
    val animated = LocalAnimated.current
    if (shared == null || animated == null || reduceMotion()) return@composed this
    val moving = animated.transition.currentState != animated.transition.targetState
    if (!moving) return@composed this
    val scope = LocalSharedScope.current
    val clip = LocalCoverClip.current
    with(shared) {
        if (clip != null) {
            // A page's cover lands with its page (300 ms, as the slide in Root.kt): a spring went
            // on settling to a hundredth of a pixel after the page, the tile it aims for moving
            // with the page restarting it, so the cover sat in the top layer about a second
            // after a back swipe. Drawn in the pages' room only, under the side bar and the dock.
            this@composed.sharedElement(
                rememberSharedContentState("$scope/$key"),
                animated,
                boundsTransform = { _, _ -> tween(300) },
                clipInOverlayDuringTransition = clip,
            )
        } else {
            // The player's cover, from the mini player in the dock: over everything, as it goes.
            this@composed.sharedElement(
                rememberSharedContentState("$scope/$key"),
                animated,
                boundsTransform = { _, _ -> spring(dampingRatio = 0.85f, stiffness = 380f, visibilityThreshold = androidx.compose.ui.geometry.Rect.VisibilityThreshold) },
            )
        }
    }
}

/**
 * Where a page's flying cover may be drawn (it flies in Compose's top layer, over everything):
 * the pages' room, less the bars at its foot (the dock, the mini player, the bar of chosen
 * songs), so the side bar and the dock stay over it, as over a cover at rest. Root.kt tells it
 * where those are; it reads them as it draws, so nothing is composed again for it.
 */
@OptIn(ExperimentalSharedTransitionApi::class)
class CoverClip : SharedTransitionScope.OverlayClip {
    /** The layout the covers fly in (its origin is the top layer's). */
    var layout: androidx.compose.ui.layout.LayoutCoordinates? = null
    var pages: androidx.compose.ui.layout.LayoutCoordinates? = null
    var foot: androidx.compose.ui.layout.LayoutCoordinates? = null

    override fun getClipPath(
        sharedContentState: SharedTransitionScope.SharedContentState,
        bounds: androidx.compose.ui.geometry.Rect,
        layoutDirection: androidx.compose.ui.unit.LayoutDirection,
        density: androidx.compose.ui.unit.Density,
    ): androidx.compose.ui.graphics.Path? {
        val layout = layout?.takeIf { it.isAttached } ?: return null
        val pages = pages?.takeIf { it.isAttached } ?: return null
        val room = androidx.compose.ui.graphics.Path().apply { addRect(layout.localBoundingBoxOf(pages)) }
        val bars = foot?.takeIf { it.isAttached }?.let { layout.localBoundingBoxOf(it) }
        if (bars == null || bars.isEmpty) return room
        val corner = with(density) { 26.dp.toPx() }
        val under = androidx.compose.ui.graphics.Path().apply {
            addRoundRect(androidx.compose.ui.geometry.RoundRect(bars, androidx.compose.ui.geometry.CornerRadius(corner)))
        }
        return androidx.compose.ui.graphics.Path.combine(androidx.compose.ui.graphics.PathOperation.Difference, room, under)
    }
}

/** Given inside the pages (Root.kt): their covers fly clipped to it, and with their page. */
val LocalCoverClip: ProvidableCompositionLocal<CoverClip?> = compositionLocalOf { null }

/**
 * The tile tapped last (its own id): only its cover flies to the page it opens, and back. An
 * album shown twice on one page (two shelves of Home) would otherwise put two covers under one
 * key, and Compose then draws them above everything, the dock too, leaving their places empty.
 */
private val tappedTile = androidx.compose.runtime.mutableStateOf<String?>(null)

/** A tile's own id for [tileCover], kept while its page is kept. */
@Composable
fun rememberTileId(): String = androidx.compose.runtime.saveable.rememberSaveable { java.util.UUID.randomUUID().toString() }

/** This tile was tapped: its cover is the one that flies. */
fun tapTile(id: String) {
    tappedTile.value = id
}

/** The cover of tile `id`: it flies under `key` only while it is the tile tapped last. */
fun Modifier.tileCover(id: String, key: String): Modifier = composed {
    if (tappedTile.value == id) Modifier.sharedCover(key) else Modifier
}

/** When the page on screen was last drawn anew (uptime in ms): a tab switch and Back count too. */
val LocalPageOpened: ProvidableCompositionLocal<Long> = compositionLocalOf { 0L }

/**
 * When the page on screen was opened from another (uptime in ms; 0 if it never was): only
 * then do its rows come in. A tab switched to, or a page gone back to, shows its rows at once.
 */
val LocalPagePushed: ProvidableCompositionLocal<Long> = compositionLocalOf { 0L }

/**
 * Rows and tiles come in one after another as a page opens: up a little, and from clear. Rows
 * that scroll in later just appear, so a list never waits on its own motion. Only the first few
 * wait their turn, so the last lands about a quarter of a second after the first.
 */
fun Modifier.entrance(index: Int): Modifier = composed {
    val pushed = LocalPagePushed.current
    val reduce = reduceMotion()
    val fresh = remember { !reduce && pushed > 0L && index < 14 && android.os.SystemClock.uptimeMillis() - pushed < 600 }
    if (!fresh) return@composed this
    val progress = remember { androidx.compose.animation.core.Animatable(0f) }
    androidx.compose.runtime.LaunchedEffect(Unit) {
        kotlinx.coroutines.delay(30L * minOf(index, 8))
        progress.animateTo(1f, spring(dampingRatio = 1f, stiffness = 400f))
    }
    graphicsLayer {
        alpha = progress.value
        translationY = (1f - progress.value) * 16.dp.toPx()
    }
}

/** How far a list has scrolled past its first item, in pixels (large once it is gone). */
fun androidx.compose.foundation.lazy.LazyListState.headerScroll(): Float =
    if (firstVisibleItemIndex == 0) firstVisibleItemScrollOffset.toFloat() else 100_000f

fun androidx.compose.foundation.lazy.grid.LazyGridState.headerScroll(): Float =
    if (firstVisibleItemIndex == 0) firstVisibleItemScrollOffset.toFloat() else 100_000f

/** Room a full-bleed page leaves at its foot for the mini player and the tabs over it. */
val LocalBottomInset: ProvidableCompositionLocal<Dp> = compositionLocalOf { 0.dp }

/** Which tab a page is in, so covers fly only between pages of the same tab. */
val LocalSharedScope: ProvidableCompositionLocal<String> = compositionLocalOf { "" }

/** Whether the page on screen has one to go back to. */
val LocalCanGoBack: ProvidableCompositionLocal<Boolean> = compositionLocalOf { false }

/** Back, as Android's back gesture does it. */
@Composable
fun BackButton(modifier: Modifier = Modifier) {
    val back = androidx.activity.compose.LocalOnBackPressedDispatcherOwner.current
    androidx.compose.material3.IconButton(onClick = { back?.onBackPressedDispatcher?.onBackPressed() }, modifier = modifier) {
        androidx.compose.material3.Icon(androidx.compose.material.icons.Icons.AutoMirrored.Rounded.ArrowBack, contentDescription = "Back")
    }
}

// ---------- Drags that close or skip: one way to end them, so the big player, the mini player,
// and the covers feel the same (DESIGN.md, Motion).

/** A drag past this part of its travel goes on when let go… */
const val FlickDistance = 0.25f

/** …and so does a throw faster than this, however short. */
val FlickSpeed = 900.dp

/** What a thrown thing rides on after the finger lets go: started at the finger's speed, no bounce. */
fun <T> flickSpring(): androidx.compose.animation.core.SpringSpec<T> = spring(dampingRatio = 1f, stiffness = 380f)

/** Back to rest when a drag was not far or fast enough: quick, no bounce. */
fun <T> snapBackSpring(): androidx.compose.animation.core.SpringSpec<T> = spring(dampingRatio = 1f, stiffness = 500f)

/**
 * Which way a drag let go goes on: 1 towards positive offsets, -1 towards negative, 0 back to
 * rest. A throw decides by its speed; a slow drag by how far it went ([travel] is the whole way).
 */
fun androidx.compose.ui.unit.Density.flickWay(offset: Float, velocity: Float, travel: Float): Int {
    val speed = FlickSpeed.toPx()
    return when {
        velocity > speed -> 1
        velocity < -speed -> -1
        kotlin.math.abs(velocity) > speed / 3 && kotlin.math.sign(velocity) != kotlin.math.sign(offset) -> 0
        offset > travel * FlickDistance -> 1
        offset < -travel * FlickDistance -> -1
        else -> 0
    }
}

/**
 * Past an end, a drag moves less and less the further it goes (a rubber band), never stopping
 * at a wall. [over] is how far past the end the finger is; [limit] about how far it may stretch.
 */
fun rubberBand(over: Float, limit: Float): Float {
    if (limit <= 0f) return 0f
    val sign = kotlin.math.sign(over)
    val x = kotlin.math.abs(over)
    return sign * (1f - 1f / (x * 0.55f / limit + 1f)) * limit
}

/**
 * A sideways ([horizontal]) or up-and-down drag that tells its speed when let go, in pixels a
 * second. [onDrag] gets each move of the finger, 1:1.
 */
suspend fun androidx.compose.ui.input.pointer.PointerInputScope.detectFlick(
    horizontal: Boolean,
    onStart: () -> Unit = {},
    onCancel: () -> Unit,
    onEnd: (velocity: Float) -> Unit,
    onDrag: (change: androidx.compose.ui.input.pointer.PointerInputChange, delta: Float) -> Unit,
) {
    // The finger's own path, summed from its moves: what is dragged moves under it, so its
    // position in the dragged thing would hardly change.
    val tracker = androidx.compose.ui.input.pointer.util.VelocityTracker()
    var total = 0f
    val start = {
        tracker.resetTracking()
        total = 0f
        onStart()
    }
    val move = { change: androidx.compose.ui.input.pointer.PointerInputChange, delta: Float ->
        total += delta
        tracker.addPosition(change.uptimeMillis, androidx.compose.ui.geometry.Offset(total, 0f))
        onDrag(change, delta)
    }
    val end = { onEnd(tracker.calculateVelocity().x) }
    if (horizontal) {
        detectHorizontalDragGestures(
            onDragStart = { start() },
            onDragEnd = end,
            onDragCancel = onCancel,
            onHorizontalDrag = move,
        )
    } else {
        detectVerticalDragGestures(
            onDragStart = { start() },
            onDragEnd = end,
            onDragCancel = onCancel,
            onVerticalDrag = move,
        )
    }
}

/**
 * Whether skipping that way ([way] 1 next, -1 back) will bring another song, as far as the app
 * can tell: false at the end of the list with nothing to follow, null when it cannot tell (the
 * song before may only start the song again; the computer's player is not read here).
 */
suspend fun skipBringsSong(way: Int): Boolean? {
    if (way < 0 || onComputer()) return null
    val now = fyi.nnx.needle.NeedleApp.instance.playback.value ?: return null
    if (now.upNext > 0u || now.repeat != fyi.nnx.needle.core.RepeatMode.OFF) return true
    // Nothing up next: only "When the queue runs out" can bring more.
    val autoplay = kotlinx.coroutines.withContext(kotlinx.coroutines.Dispatchers.IO) {
        runCatching { core.playbackSettings().autoplayQuery }.getOrNull()
    } ?: return null
    return autoplay.isNotBlank()
}
