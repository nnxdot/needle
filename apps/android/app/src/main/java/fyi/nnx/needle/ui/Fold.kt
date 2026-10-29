package fyi.nnx.needle.ui

import android.app.Activity
import android.content.Context
import android.content.ContextWrapper
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.compositionLocalOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.unit.Dp
import androidx.window.layout.FoldingFeature
import androidx.window.layout.WindowInfoTracker

/**
 * A foldable half closed (a Galaxy Z Fold, a Pixel Fold): the crease across the screen, in dp
 * from the window's top left. `tabletop` when it runs across (the phone stands like a laptop),
 * else it runs down (held like a book). Flat or closed, there is none: the screen is used whole,
 * by its width as any other.
 */
data class Hinge(val tabletop: Boolean, val left: Dp, val top: Dp, val right: Dp, val bottom: Dp)

/** The hinge of a half-closed foldable, for the root and the player (null when there is none). */
val LocalHinge = compositionLocalOf<Hinge?> { null }

private tailrec fun Context.activity(): Activity? = when (this) {
    is Activity -> this
    is ContextWrapper -> baseContext.activity()
    else -> null
}

/** The hinge now, from Jetpack WindowManager: only while the screen is half closed. */
@Composable
fun rememberHinge(): Hinge? {
    val activity = LocalContext.current.activity() ?: return null
    val layouts = remember(activity) { WindowInfoTracker.getOrCreate(activity).windowLayoutInfo(activity) }
    val info by layouts.collectAsState(initial = null)
    val fold = info?.displayFeatures?.filterIsInstance<FoldingFeature>()?.firstOrNull { it.state == FoldingFeature.State.HALF_OPENED } ?: return null
    val b = fold.bounds
    return with(LocalDensity.current) {
        Hinge(
            tabletop = fold.orientation == FoldingFeature.Orientation.HORIZONTAL,
            left = b.left.toDp(),
            top = b.top.toDp(),
            right = b.right.toDp(),
            bottom = b.bottom.toDp(),
        )
    }
}
