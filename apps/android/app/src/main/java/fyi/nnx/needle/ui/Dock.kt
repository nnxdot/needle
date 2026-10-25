package fyi.nnx.needle.ui

import android.os.Build
import androidx.compose.foundation.background
import androidx.compose.foundation.border
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
import androidx.compose.runtime.compositionLocalOf
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.graphics.luminance
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import dev.chrisbanes.haze.HazeInput
import dev.chrisbanes.haze.HazeState
import dev.chrisbanes.haze.blur.HazeBlurStyle
import dev.chrisbanes.haze.blur.HazeColorEffect
import dev.chrisbanes.haze.blur.hazeBlur

/**
 * What the floating glass blurs: the pages under it (`hazeSource` in Root.kt). Null where
 * nothing is captured; the glass is then a solid fill.
 */
val LocalHaze = compositionLocalOf<HazeState?> { null }

/** Real blur behind glass needs Android 12 (RenderEffect); before it, a solid fill. */
private val canBlur = Build.VERSION.SDK_INT >= Build.VERSION_CODES.S

/**
 * Frosted glass, the Needle look's one floating surface (the dock, a tablet's player and pane):
 * the page behind it blurred and tinted with the page's own colour, a thin light line around it,
 * and a soft shadow. Before Android 12, the page's raised colour, solid.
 */
@Composable
fun Modifier.glass(shape: Shape): Modifier {
    val scheme = MaterialTheme.colorScheme
    val dark = scheme.background.luminance() < 0.5f
    val haze = LocalHaze.current
    val edge = if (dark) Color.White.copy(alpha = 0.09f) else Color.Black.copy(alpha = 0.07f)
    return this
        .shadow(18.dp, shape, ambientColor = Color.Black.copy(alpha = 0.35f), spotColor = Color.Black.copy(alpha = 0.35f))
        .clip(shape)
        .then(
            if (canBlur && haze != null) {
                val tint = scheme.surfaceContainer.copy(alpha = if (dark) 0.62f else 0.7f)
                Modifier.hazeBlur(
                    HazeInput.Sources(haze),
                    HazeBlurStyle {
                        blurRadius(28.dp)
                        noiseFactor(0.06f)
                        backgroundColor(scheme.background)
                        colorEffects(listOf(HazeColorEffect.tint(tint)))
                    },
                )
            } else {
                Modifier.background(scheme.surfaceContainerHigh)
            },
        )
        .border(0.5.dp, edge, shape)
}

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
    Column(modifier.fillMaxWidth().glass(DockShape)) {
        androidx.compose.animation.AnimatedVisibility(showPlayer) {
            CompositionLocalProvider(LocalAnimated provides (animated ?: this)) {
                MiniPlayer(onOpen = onOpenPlayer, docked = true)
            }
        }
        DockTabs(tab, choose)
    }
}

/** The dock's tabs: an icon and a small name each, the open one in the accent. */
@Composable
private fun DockTabs(tab: Tab, choose: (Tab) -> Unit) {
    Row(Modifier.fillMaxWidth().height(56.dp).padding(horizontal = 6.dp), verticalAlignment = Alignment.CenterVertically) {
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
        Box(Modifier.widthIn(max = 820.dp).fillMaxWidth().glass(DockShape)) {
            MiniPlayer(onOpen = onOpenPlayer, docked = true)
        }
    }
}
