package fyi.nnx.needle.ui

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
import androidx.compose.runtime.Composable
import androidx.compose.runtime.ProvidableCompositionLocal
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.compositionLocalOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
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
fun reduceMotion(): Boolean {
    val app by NeedleApp.instance.app.collectAsState()
    val context = LocalContext.current
    val system = remember {
        Settings.Global.getFloat(context.contentResolver, Settings.Global.ANIMATOR_DURATION_SCALE, 1f) == 0f
    }
    return app.reduceMotion || system
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

/** Shrinks a little while pressed, and springs back, as covers do in Apple Music. */
fun Modifier.pressScale(interaction: MutableInteractionSource, scale: Float = 0.96f): Modifier = composed {
    val pressed by interaction.collectIsPressedAsState()
    val reduce = reduceMotion()
    val value by animateFloatAsState(
        if (pressed && !reduce) scale else 1f,
        spring(dampingRatio = 0.6f, stiffness = 600f),
        label = "press",
    )
    graphicsLayer { scaleX = value; scaleY = value }
}

/** Three bars that bounce while the song plays and rest while it is paused. */
@Composable
fun PlayingBars(playing: Boolean, modifier: Modifier = Modifier, color: Color = MaterialTheme.colorScheme.primary, height: Dp = 14.dp) {
    val transition = rememberInfiniteTransition(label = "bars")
    val reduce = reduceMotion()
    Row(modifier.size(width = 16.dp, height = height), horizontalArrangement = Arrangement.spacedBy(2.dp), verticalAlignment = Alignment.Bottom) {
        listOf(420, 310, 520).forEachIndexed { i, period ->
            val bounce by transition.animateFloat(
                initialValue = 0.25f,
                targetValue = 1f,
                animationSpec = infiniteRepeatable(tween(period, delayMillis = i * 60, easing = FastOutSlowInEasing), RepeatMode.Reverse),
                label = "bar$i",
            )
            val level = if (playing && !reduce) bounce else listOf(0.45f, 0.8f, 0.6f)[i]
            Box(
                Modifier
                    .width(4.dp)
                    .fillMaxHeight(level)
                    .clip(RoundedCornerShape(1.dp))
                    .background(color),
            )
        }
    }
}

/** A soft sweep of light across a placeholder while its content loads. */
fun Modifier.shimmer(): Modifier = composed {
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

/** Lets this cover fly between screens that show the same one under `key`. */
@OptIn(ExperimentalSharedTransitionApi::class)
fun Modifier.sharedCover(key: String): Modifier = composed {
    val shared = LocalShared.current
    val animated = LocalAnimated.current
    if (shared == null || animated == null || reduceMotion()) return@composed this
    with(shared) {
        this@composed.sharedElement(
            rememberSharedContentState(key),
            animated,
            boundsTransform = { _, _ -> spring(dampingRatio = 0.85f, stiffness = 380f) },
        )
    }
}

/** When the page on screen opened (uptime in ms), so rows know whether to come in with it. */
val LocalPageOpened: ProvidableCompositionLocal<Long> = compositionLocalOf { 0L }

/**
 * Rows and tiles come in one after another as a page opens: up a little, and from clear. Rows
 * that scroll in later just appear, so a list never waits on its own motion.
 */
fun Modifier.entrance(index: Int): Modifier = composed {
    val opened = LocalPageOpened.current
    val reduce = reduceMotion()
    val fresh = remember { !reduce && index < 14 && android.os.SystemClock.uptimeMillis() - opened < 600 }
    if (!fresh) return@composed this
    val progress = remember { androidx.compose.animation.core.Animatable(0f) }
    androidx.compose.runtime.LaunchedEffect(Unit) {
        kotlinx.coroutines.delay(60L + index * 35L)
        progress.animateTo(1f, spring(dampingRatio = 0.85f, stiffness = 260f))
    }
    graphicsLayer {
        alpha = progress.value
        translationY = (1f - progress.value) * 28.dp.toPx()
    }
}

/** How far a list has scrolled past its first item, in pixels (large once it is gone). */
fun androidx.compose.foundation.lazy.LazyListState.headerScroll(): Float =
    if (firstVisibleItemIndex == 0) firstVisibleItemScrollOffset.toFloat() else 100_000f

fun androidx.compose.foundation.lazy.grid.LazyGridState.headerScroll(): Float =
    if (firstVisibleItemIndex == 0) firstVisibleItemScrollOffset.toFloat() else 100_000f

/** Room a full-bleed page leaves at its foot for the mini player and the tabs over it. */
val LocalBottomInset: ProvidableCompositionLocal<Dp> = compositionLocalOf { 0.dp }
