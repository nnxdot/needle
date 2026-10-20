package fyi.nnx.needle.ui

import android.content.pm.PackageManager
import androidx.compose.foundation.IndicationNodeFactory
import androidx.compose.foundation.clickable
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.foundation.focusGroup
import androidx.compose.foundation.interaction.FocusInteraction
import androidx.compose.foundation.interaction.InteractionSource
import androidx.compose.material3.LocalContentColor
import androidx.compose.material3.ripple
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusProperties
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.drawscope.ContentDrawScope
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.input.InputMode
import androidx.compose.ui.node.CompositionLocalConsumerModifierNode
import androidx.compose.ui.node.DelegatableNode
import androidx.compose.ui.node.DelegatingNode
import androidx.compose.ui.node.DrawModifierNode
import androidx.compose.ui.node.currentValueOf
import androidx.compose.ui.node.invalidateDraw
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalInputModeManager
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch

/** Needle runs on a TV (Android TV or Google TV), where a remote is all there is. */
@Composable
fun isTv(): Boolean {
    val context = LocalContext.current
    return remember { context.packageManager.hasSystemFeature(PackageManager.FEATURE_LEANBACK) }
}

/** Someone moves about with keys (a TV's remote or a keyboard), not by touch. */
@Composable
fun usingKeys(): Boolean = LocalInputModeManager.current.inputMode == InputMode.Keyboard

/**
 * A layer over the whole app (the player, the welcome guide) for a remote: the focus moves into
 * it when it opens, or when what had the focus in it goes, and the arrows cannot leave it for
 * the buttons hidden under it. By touch it changes nothing.
 */
@Composable
fun Modifier.focusLayer(): Modifier {
    val keys = usingKeys()
    val requester = remember { FocusRequester() }
    var inside by remember { mutableStateOf(false) }
    LaunchedEffect(inside, keys) {
        if (inside || !keys) return@LaunchedEffect
        // The layer's buttons can come a moment after the layer (it loads, or it slides in).
        repeat(20) {
            delay(50)
            if (runCatching { requester.requestFocus() }.getOrDefault(false)) return@LaunchedEffect
        }
    }
    return this
        .focusRequester(requester)
        .onFocusChanged { inside = it.hasFocus }
        .focusProperties { onExit = { cancelFocusChange() } }
        .focusGroup()
}

/**
 * With a remote, this takes the focus when its page opens (the page's own Play), rather than
 * the page's first button. Only just after the page opens, not when a list scrolls it back.
 */
@Composable
fun Modifier.firstFocus(): Modifier {
    if (!usingKeys()) return this
    val requester = remember { FocusRequester() }
    val opened = LocalPageOpened.current
    LaunchedEffect(Unit) {
        // A slow TV can take a few seconds to load a page's songs, and so its Play.
        if (android.os.SystemClock.uptimeMillis() - opened > 3000) return@LaunchedEffect
        // After the frame gives the page the focus (Root.kt), so this one is the last.
        delay(500)
        runCatching { requester.requestFocus() }
    }
    return focusRequester(requester)
}

/**
 * A text box a remote can leave: up and down move on to what is above or below, where the
 * text box itself would keep them. Left and right still move along the text. A box of
 * [lines] keeps up and down for its lines where there is a keyboard, so only on a TV.
 */
@Composable
fun Modifier.remoteEscape(lines: Boolean = false): Modifier {
    if (lines && !isTv()) return this
    val focus = androidx.compose.ui.platform.LocalFocusManager.current
    return onPreviewKeyEvent {
        val direction = when (it.key) {
            Key.DirectionUp -> androidx.compose.ui.focus.FocusDirection.Up
            Key.DirectionDown -> androidx.compose.ui.focus.FocusDirection.Down
            else -> return@onPreviewKeyEvent false
        }
        if (it.type == KeyEventType.KeyDown) focus.moveFocus(direction)
        true
    }
}

/**
 * A text box a remote passes over on its way down the page: on a TV the keyboard waits for OK,
 * where it would open (and take the remote's arrows) as soon as the box had the focus. By touch
 * it is the text box alone. [content] gets the modifier for its text box.
 */
@Composable
fun RemoteTyping(modifier: Modifier = Modifier, content: @Composable (Modifier) -> Unit) {
    if (!isTv()) {
        content(modifier.remoteEscape())
        return
    }
    var typing by remember { mutableStateOf(false) }
    val field = remember { FocusRequester() }
    LaunchedEffect(typing) { if (typing) runCatching { field.requestFocus() } }
    androidx.compose.foundation.layout.Box(modifier.clickable { typing = true }) {
        content(
            Modifier
                .focusRequester(field)
                .onFocusChanged { if (!it.isFocused) typing = false }
                .remoteEscape()
                .focusProperties { canFocus = typing },
        )
    }
}

/** What sits under a [focusLayer]: while [covered], the remote's arrows cannot come into it. */
fun Modifier.focusCovered(covered: Boolean): Modifier = this
    .focusProperties { onEnter = { if (covered) cancelFocusChange() } }
    .focusGroup()

/**
 * What has a remote's focus shows plainly: Material's own controls (buttons, pills, switches)
 * with Material's ring inside their shape and a deeper shade, and everything else with a
 * [FocusRing]. Inside each MaterialTheme, which sets its own ripple.
 */
@OptIn(androidx.compose.material3.ExperimentalMaterial3Api::class)
@Composable
fun FocusShades(content: @Composable () -> Unit) = androidx.compose.runtime.CompositionLocalProvider(
    androidx.compose.foundation.LocalIndication provides FocusRing,
    androidx.compose.material3.LocalRippleThemeConfiguration provides androidx.compose.material3.RippleDefaults.InsetFocusRingThemeConfiguration,
    androidx.compose.material3.LocalRippleConfiguration provides androidx.compose.material3.RippleConfiguration(
        rippleAlpha = androidx.compose.material.ripple.RippleAlpha(
            draggedAlpha = 0.16f,
            focusedAlpha = 0.3f,
            hoveredAlpha = 0.08f,
            pressedAlpha = 0.1f,
        ),
    ),
    content = content,
)

/**
 * Material's ripple, and a ring in the text colour round what has a remote's focus: a TV is
 * looked at from across the room, and the ripple's own faint shade is not enough there.
 */
object FocusRing : IndicationNodeFactory {
    override fun create(interactionSource: InteractionSource): DelegatableNode =
        // The ripple without Material's own focus ring: this one draws its ring instead.
        FocusRingNode(interactionSource, ripple(enableFocusIndication = false).create(interactionSource))

    override fun equals(other: Any?) = other === this
    override fun hashCode() = 7
}

private class FocusRingNode(
    private val source: InteractionSource,
    ripple: DelegatableNode,
) : DelegatingNode(), DrawModifierNode, CompositionLocalConsumerModifierNode {
    private var focused = false

    init {
        delegate(ripple)
    }

    override fun onAttach() {
        coroutineScope.launch {
            var count = 0
            source.interactions.collect {
                when (it) {
                    is FocusInteraction.Focus -> count++
                    is FocusInteraction.Unfocus -> count--
                }
                if ((count > 0) != focused) {
                    focused = count > 0
                    invalidateDraw()
                }
            }
        }
    }

    override fun ContentDrawScope.draw() {
        drawContent()
        if (!focused) return
        val width = 3.dp.toPx()
        // Material's largest corner, or a circle's: round enough to sit inside the shape that
        // clips it, near enough to follow a card's own corners.
        val radius = minOf(size.minDimension / 2, 28.dp.toPx())
        drawRoundRect(
            currentValueOf(LocalContentColor),
            topLeft = Offset(width / 2, width / 2),
            size = Size(size.width - width, size.height - width),
            cornerRadius = CornerRadius(radius),
            style = Stroke(width),
        )
    }
}
