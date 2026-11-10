package fyi.nnx.needle.ui

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp

/** The dock's corners. */
val DockShape = RoundedCornerShape(26.dp)

/**
 * A phone's dock (DESIGN.md): the mini player and the tabs together, one small bar of frosted
 * glass floating over the foot of the page. Without a song, the tabs alone.
 */
@Composable
fun Dock(
    tab: Tab,
    choose: (Tab) -> Unit,
    showPlayer: Boolean,
    onOpenPlayer: () -> Unit,
    animated: androidx.compose.animation.AnimatedVisibilityScope?,
    modifier: Modifier = Modifier,
) {
    // With a song in it, the glass takes the cover's colour from the left (Glass.kt).
    val cover = playingGlassCover()
    Column(modifier.fillMaxWidth().glass(DockShape, cover = if (showPlayer) cover else null)) {
        // Reduce motion: the mini player fades in and out; the dock does not grow or shrink.
        val reduce = reduceMotion()
        androidx.compose.animation.AnimatedVisibility(
            showPlayer,
            enter = if (reduce) androidx.compose.animation.fadeIn(androidx.compose.animation.core.tween(150)) else androidx.compose.animation.fadeIn() + androidx.compose.animation.expandVertically(),
            exit = if (reduce) androidx.compose.animation.fadeOut(androidx.compose.animation.core.tween(150)) else androidx.compose.animation.fadeOut() + androidx.compose.animation.shrinkVertically(),
        ) {
            CompositionLocalProvider(LocalAnimated provides (animated ?: this)) {
                MiniPlayer(onOpen = onOpenPlayer, docked = true)
            }
        }
        DockTabs(tab, choose)
    }
}

/**
 * The dock's tabs: an icon and a small name each, the open one in the accent on a soft light that
 * slides from tab to tab (with Reduce motion, it moves at once).
 */
@Composable
private fun DockTabs(tab: Tab, choose: (Tab) -> Unit) {
    val reduce = reduceMotion()
    // Read only where it is drawn: the slide does not compose the tabs again.
    val at = androidx.compose.animation.core.animateFloatAsState(
        Tab.entries.indexOf(tab).toFloat(),
        if (reduce) androidx.compose.animation.core.snap() else androidx.compose.animation.core.spring(dampingRatio = 0.85f, stiffness = 500f),
        label = "tab light",
    )
    val light = MaterialTheme.colorScheme.onSurface.copy(alpha = 0.08f)
    Row(
        Modifier
            .fillMaxWidth()
            .height(56.dp)
            .padding(horizontal = 6.dp)
            .drawBehind {
                val w = size.width / Tab.entries.size
                val h = 48.dp.toPx()
                val x = if (layoutDirection == androidx.compose.ui.unit.LayoutDirection.Rtl) size.width - (at.value + 1) * w else at.value * w
                drawRoundRect(
                    light,
                    topLeft = androidx.compose.ui.geometry.Offset(x, (size.height - h) / 2),
                    size = androidx.compose.ui.geometry.Size(w, h),
                    cornerRadius = androidx.compose.ui.geometry.CornerRadius(16.dp.toPx()),
                )
            },
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Tab.entries.forEach { t ->
            val chosen = t == tab
            val ink = if (chosen) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurfaceVariant
            Column(
                Modifier
                    .weight(1f)
                    .height(48.dp)
                    .clip(RoundedCornerShape(16.dp))
                    .semantics { selected = chosen }
                    .clickable(role = Role.Tab) { choose(t) },
                horizontalAlignment = Alignment.CenterHorizontally,
                verticalArrangement = Arrangement.Center,
            ) {
                // The open tab's icon filled, the others outlined, as on the side bar.
                Icon(if (chosen) t.icon else t.outline, contentDescription = null, tint = ink, modifier = Modifier.size(24.dp))
                Text(
                    t.label,
                    style = MaterialTheme.typography.labelSmall,
                    fontWeight = if (chosen) FontWeight.SemiBold else FontWeight.Medium,
                    color = ink,
                )
            }
        }
    }
}

/**
 * A tablet's (and a TV's) player: the mini player alone, floating over the foot of the page
 * in the same glass, at most 820 dp wide, as the Mac's player bar.
 */
@Composable
fun FloatingPlayer(onOpenPlayer: () -> Unit, modifier: Modifier = Modifier) {
    Box(modifier.fillMaxWidth(), contentAlignment = Alignment.Center) {
        Box(Modifier.widthIn(max = 820.dp).fillMaxWidth().glass(DockShape, cover = playingGlassCover())) {
            MiniPlayer(onOpen = onOpenPlayer, docked = true)
        }
    }
}
