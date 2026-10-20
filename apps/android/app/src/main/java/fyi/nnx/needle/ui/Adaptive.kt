package fyi.nnx.needle.ui

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.GridItemSpan
import androidx.compose.foundation.lazy.grid.LazyGridScope
import androidx.compose.runtime.Composable
import androidx.compose.runtime.compositionLocalOf
import androidx.compose.ui.Modifier
import androidx.compose.ui.layout.layout
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp

/** How wide the pages are: the screen less the rail of tabs. The app's frame sets it. */
val LocalPageWidth = compositionLocalOf { 400.dp }

/** A tablet or an unfolded phone: room for a cover beside its words, or two panes. */
@Composable
fun isWide(): Boolean = LocalPageWidth.current >= 560.dp

/** Room for a list beside the page it opens, as Android's own Settings on a tablet. */
@Composable
fun isTwoPane(): Boolean = LocalPageWidth.current >= 840.dp

/**
 * Columns for a grid of covers or tiles: two on any phone, as before; on a tablet, as many as
 * fit at least `min` wide.
 */
@Composable
fun gridColumns(min: Dp): GridCells =
    GridCells.Fixed(if (isWide()) (LocalPageWidth.current / min).toInt().coerceAtLeast(2) else 2)

/**
 * Lets an item run past its list's padding at each side, to the page's edges: for the rows and
 * headers of a grid whose covers keep a margin.
 */
fun Modifier.bleed(side: Dp): Modifier = layout { measurable, constraints ->
    val px = side.roundToPx()
    val placeable = measurable.measure(
        constraints.copy(
            minWidth = constraints.minWidth + px * 2,
            maxWidth = if (constraints.hasBoundedWidth) constraints.maxWidth + px * 2 else constraints.maxWidth,
        ),
    )
    layout((placeable.width - px * 2).coerceAtLeast(0), placeable.height) { placeable.place(-px, 0) }
}

/** A grid item across the whole row that runs past the grid's side padding (see [bleed]). */
internal fun LazyGridScope.edgeToEdge(side: Dp, key: Any? = null, content: @Composable () -> Unit) =
    item(key = key, span = { GridItemSpan(maxLineSpan) }) { Box(Modifier.bleed(side)) { content() } }
