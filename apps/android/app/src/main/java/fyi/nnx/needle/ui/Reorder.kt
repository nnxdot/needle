package fyi.nnx.needle.ui

import androidx.compose.foundation.gestures.detectDragGestures
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
fun rememberReorder(count: Int, move: (from: Int, to: Int) -> Unit): Reorder =
    // The latest `move` each time: the first one may hold a list that had not loaded yet.
    remember { Reorder(move) }.also { it.move = move; it.count = count }

/** The row's place while one is dragged: the dragged one follows the finger, others step aside. */
fun Modifier.reorderRow(reorder: Reorder, index: Int): Modifier =
    onSizeChanged { if (index == 0 || reorder.rowHeight <= 1f) reorder.rowHeight = it.height.toFloat() }
        .graphicsLayer {
            if (index == reorder.dragging) {
                translationY = reorder.offset
                shadowElevation = 12.dp.toPx()
                scaleX = 1.02f
                scaleY = 1.02f
            } else {
                translationY = reorder.shift(index) * reorder.rowHeight
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
            .semantics {
                customActions = listOfNotNull(
                    if (up) CustomAccessibilityAction("Move up") { reorder.move(index, index - 1); true } else null,
                    if (down) CustomAccessibilityAction("Move down") { reorder.move(index, index + 1); true } else null,
                )
            }
            .focusable(interactionSource = focus)
            .pointerInput(index) {
                detectDragGestures(
                    onDragStart = { haptics(true); reorder.dragging = index; reorder.offset = 0f },
                    onDragEnd = {
                        val from = reorder.dragging
                        val to = reorder.target
                        reorder.dragging = -1
                        reorder.offset = 0f
                        if (from >= 0 && to >= 0 && from != to) reorder.move(from, to)
                    },
                    onDragCancel = { reorder.dragging = -1; reorder.offset = 0f },
                ) { change, drag ->
                    change.consume()
                    reorder.offset += drag.y
                }
            },
        contentAlignment = Alignment.Center,
    ) {
        Icon(Icons.Rounded.DragHandle, contentDescription = "Drag to move", tint = tint, modifier = Modifier.padding(4.dp))
    }
}
