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
class Reorder(val move: (from: Int, to: Int) -> Unit) {
    var dragging by mutableIntStateOf(-1)
    var offset by mutableFloatStateOf(0f)
    var rowHeight by mutableFloatStateOf(1f)
    var count by mutableIntStateOf(0)

    /** Where the dragged row would land now. */
    val target: Int
        get() = if (dragging < 0) -1 else (dragging + (offset / rowHeight).roundToInt()).coerceIn(0, count - 1)

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
    remember { Reorder(move) }.also { it.count = count }

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

/** The handle a row is dragged by. */
@Composable
fun DragHandle(reorder: Reorder, index: Int, tint: androidx.compose.ui.graphics.Color = MaterialTheme.colorScheme.onSurfaceVariant, haptics: (Boolean) -> Unit = rememberHaptics()) {
    Box(
        Modifier
            .size(48.dp)
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
