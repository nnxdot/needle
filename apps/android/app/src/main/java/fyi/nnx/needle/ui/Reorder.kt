package fyi.nnx.needle.ui

import androidx.compose.foundation.gestures.detectDragGestures
import androidx.compose.foundation.gestures.scrollBy
import androidx.compose.ui.composed
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.DragHandle
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.foundation.focusable
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.semantics.CustomAccessibilityAction
import androidx.compose.ui.semantics.customActions
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.unit.dp
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.ui.Alignment
import kotlin.math.roundToInt

/**
 * Rows put in another order by dragging their handle: the row follows the finger, the rows
 * it passes make room, and on letting go `move(from, to)` is called once.
 */
class Reorder(var move: (from: Int, to: Int) -> Unit) {
    var dragging by mutableIntStateOf(-1)
    var offset by mutableFloatStateOf(0f)
    var rowHeight by mutableFloatStateOf(1f)
    var count by mutableIntStateOf(0)

    /** When a dragged row was last let go (uptime, ms): its list's rows are in place already. */
    var droppedAt = 0L

    /** The list the rows are in, scrolled while a row is held near its top or foot. */
    var list: androidx.compose.foundation.lazy.LazyListState? = null

    /** The rows' keys in that list, to find the row held. */
    var keys: List<Any> = emptyList()

    /** The last drop: where the row landed, and how far from its new place it was let go. */
    var drops by mutableIntStateOf(0)
    var droppedTo = -1
    var droppedOffset = 0f

    /** Lets go of the row held: it goes to its new place, from where the finger left it. */
    fun drop() {
        val from = dragging
        val to = target
        droppedAt = android.os.SystemClock.uptimeMillis()
        droppedTo = to
        droppedOffset = offset - (to - from) * rowHeight
        dragging = -1
        offset = 0f
        drops++
        if (from >= 0 && to >= 0 && from != to) move(from, to)
    }


    /** Where the dragged row would land now. */
    val target: Int
        get() = if (dragging < 0) -1 else (dragging + (offset / rowHeight).roundToInt()).coerceIn(0, maxOf(0, count - 1))

    /** How far row `i` moves aside for the one being dragged, in rows. */
    fun shift(i: Int): Int {
        val t = target
        if (dragging < 0 || i == dragging) return 0
        return when {
            dragging < t && i in (dragging + 1)..t -> -1
            dragging > t && i in t until dragging -> 1
            else -> 0
        }
    }
}

@Composable
fun rememberReorder(count: Int, list: androidx.compose.foundation.lazy.LazyListState? = null, move: (from: Int, to: Int) -> Unit): Reorder =
    // The latest `move` each time: the first one may hold a list that had not loaded yet.
    remember { Reorder(move) }.also { it.move = move; it.count = count; it.list = list }

/**
 * A row of a list that changes in place (the queue, a playlist): when a song is taken out or moved
 * (its menu, a remote, TalkBack), the rows slide to their new places rather than jump. Not just
 * after a drag, whose rows stand in their new places already, and not with Reduce motion.
 */
@Composable
fun Modifier.movingRow(item: androidx.compose.foundation.lazy.LazyItemScope, reorder: Reorder?): Modifier {
    if (reduceMotion()) return this
    val dragged = reorder != null && android.os.SystemClock.uptimeMillis() - reorder.droppedAt < 400
    return with(item) { this@movingRow.animateItem(
        placementSpec = if (dragged) null else androidx.compose.animation.core.spring(
            stiffness = androidx.compose.animation.core.Spring.StiffnessMediumLow,
            visibilityThreshold = androidx.compose.ui.unit.IntOffset(1, 1),
        ),
    ) }
}

/**
 * The row's place while one is dragged: the dragged one follows the finger and lifts; the others
 * slide aside to make room for it. Let go, it slides from where it was into its new place.
 * Reduce motion: they move at once.
 */
fun Modifier.reorderRow(reorder: Reorder, index: Int): Modifier = composed {
    val reduce = reduceMotion()
    val holding = reorder.dragging >= 0
    // Once let go, the rows stand in their new places at once (the list has changed under them).
    val shift by androidx.compose.animation.core.animateFloatAsState(
        reorder.shift(index).toFloat(),
        if (reduce || !holding) androidx.compose.animation.core.snap() else androidx.compose.animation.core.spring(dampingRatio = 1f, stiffness = 800f),
        label = "make room",
    )
    val lift by androidx.compose.animation.core.animateFloatAsState(
        if (index == reorder.dragging) 1f else 0f,
        if (reduce) androidx.compose.animation.core.snap() else androidx.compose.animation.core.spring(dampingRatio = 1f, stiffness = 600f),
        label = "lift",
    )
    val settle = remember { androidx.compose.animation.core.Animatable(0f) }
    androidx.compose.runtime.LaunchedEffect(reorder.drops, index) {
        val fresh = android.os.SystemClock.uptimeMillis() - reorder.droppedAt < 300
        if (reduce || !fresh || index != reorder.droppedTo) {
            settle.snapTo(0f)
            return@LaunchedEffect
        }
        settle.snapTo(reorder.droppedOffset)
        settle.animateTo(0f, androidx.compose.animation.core.spring(dampingRatio = 1f, stiffness = 600f))
    }
    onSizeChanged { if (index == 0 || reorder.rowHeight <= 1f) reorder.rowHeight = it.height.toFloat() }
        .graphicsLayer {
            translationY = if (index == reorder.dragging) reorder.offset else shift * reorder.rowHeight + settle.value
            shadowElevation = 12.dp.toPx() * lift
            scaleX = 1f + 0.02f * lift
            scaleY = 1f + 0.02f * lift
        }
}

/**
 * The handle a row is dragged by. With a remote or keys it takes the focus, and up and down move
 * the row a place; TalkBack offers the same as actions.
 */
@Composable
fun DragHandle(reorder: Reorder, index: Int, tint: androidx.compose.ui.graphics.Color = MaterialTheme.colorScheme.onSurfaceVariant, haptics: (Boolean) -> Unit = rememberHaptics()) {
    val focus = remember { androidx.compose.foundation.interaction.MutableInteractionSource() }
    // The rows' keys follow their songs (songKeys), so the focus stays on the row moved.
    val up = index > 0
    val down = index < reorder.count - 1
    // Held near the list's top or foot, the list scrolls (faster the nearer the edge), so a song
    // can go anywhere in a long list. The row held is kept while its own place scrolls away.
    val density = androidx.compose.ui.platform.LocalDensity.current
    val pinnable = androidx.compose.ui.layout.LocalPinnableContainer.current
    val held = reorder.dragging == index
    androidx.compose.runtime.LaunchedEffect(held) {
        if (!held) return@LaunchedEffect
        val pin = pinnable?.pin()
        try {
            val state = reorder.list ?: kotlinx.coroutines.awaitCancellation()
            val key = reorder.keys.getOrNull(index)
            val start = state.layoutInfo.visibleItemsInfo.firstOrNull { it.key == key } ?: kotlinx.coroutines.awaitCancellation()
            val middle = start.offset + start.size / 2f
            val edge = with(density) { 56.dp.toPx() }
            val fastest = with(density) { 16.dp.toPx() }
            var scrolled = 0f
            while (true) {
                androidx.compose.runtime.withFrameNanos {}
                val info = state.layoutInfo
                val finger = middle + reorder.offset - scrolled
                val top = info.viewportStartOffset + info.beforeContentPadding + edge
                val foot = info.viewportEndOffset - info.afterContentPadding - edge
                val speed = when {
                    finger < top -> -((top - finger) / edge).coerceAtMost(1f)
                    finger > foot -> ((finger - foot) / edge).coerceAtMost(1f)
                    else -> 0f
                }
                if (speed != 0f) {
                    val used = state.scrollBy(speed * fastest)
                    scrolled += used
                    reorder.offset += used
                }
            }
        } finally {
            pin?.release()
        }
    }
    Box(
        Modifier
            .size(48.dp)
            .focusRing(focus, 24.dp)
            .onKeyEvent {
                val to = when (it.key) {
                    Key.DirectionUp -> if (up) index - 1 else return@onKeyEvent false
                    Key.DirectionDown -> if (down) index + 1 else return@onKeyEvent false
                    else -> return@onKeyEvent false
                }
                if (it.type == KeyEventType.KeyDown) reorder.move(index, to)
                true
            }
            // One stop for TalkBack: the name and the moves on the handle itself.
            .semantics(mergeDescendants = true) {
                contentDescription = "Drag to move"
                customActions = listOfNotNull(
                    if (up) CustomAccessibilityAction("Move up") { reorder.move(index, index - 1); true } else null,
                    if (down) CustomAccessibilityAction("Move down") { reorder.move(index, index + 1); true } else null,
                )
            }
            .focusable(interactionSource = focus)
            .pointerInput(index) {
                detectDragGestures(
                    onDragStart = { haptics(true); reorder.dragging = index; reorder.offset = 0f },
                    onDragEnd = { reorder.drop() },
                    onDragCancel = { reorder.dragging = -1; reorder.offset = 0f },
                ) { change, drag ->
                    change.consume()
                    reorder.offset += drag.y
                }
            },
        contentAlignment = Alignment.Center,
    ) {
        Icon(Icons.Rounded.DragHandle, contentDescription = null, tint = tint, modifier = Modifier.padding(4.dp))
    }
}
