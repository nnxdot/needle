package fyi.nnx.needle.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.detectDragGesturesAfterLongPress
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.QueueMusic
import androidx.compose.material.icons.rounded.Cast
import androidx.compose.material.icons.rounded.Favorite
import androidx.compose.material.icons.rounded.Lyrics
import androidx.compose.material.icons.rounded.PlayArrow
import androidx.compose.material.icons.rounded.Repeat
import androidx.compose.material.icons.rounded.Shuffle
import androidx.compose.material.icons.rounded.SkipNext
import androidx.compose.material.icons.rounded.SkipPrevious
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.boundsInRoot
import androidx.compose.ui.layout.onGloballyPositioned
import androidx.compose.ui.semantics.CustomAccessibilityAction
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.customActions
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import androidx.compose.ui.zIndex

/**
 * The player's buttons (Settings › Appearance › Player buttons, the core's `player_buttons`):
 * every one, in Needle's own order. The first [MainButtons] chosen sit in the player's main
 * row; the rest in the row under it.
 */
val AllPlayerButtons = listOf("shuffle", "previous", "play", "next", "repeat", "favorite", "lyrics", "queue", "speakers")

/** How many chosen buttons the player's main row holds; the rest go in the row under it. */
const val MainButtons = 5

/** A button's icon and name. */
fun playerButton(name: String): Pair<ImageVector, String> = when (name) {
    "shuffle" -> Icons.Rounded.Shuffle to "Shuffle"
    "previous" -> Icons.Rounded.SkipPrevious to "Previous"
    "play" -> Icons.Rounded.PlayArrow to "Play/Pause"
    "next" -> Icons.Rounded.SkipNext to "Next"
    "repeat" -> Icons.Rounded.Repeat to "Repeat"
    "favorite" -> Icons.Rounded.Favorite to "Favorite"
    "lyrics" -> Icons.Rounded.Lyrics to "Lyrics"
    "queue" -> Icons.AutoMirrored.Rounded.QueueMusic to "Queue"
    else -> Icons.Rounded.Cast to "Speakers"
}

/** The chosen buttons, known ones only, each once, Play/Pause always among them. */
fun chosenButtons(stored: List<String>): List<String> {
    val known = stored.filter { it in AllPlayerButtons }.distinct()
    return if ("play" in known) known else listOf("play") + known
}

/**
 * The editor under the Player buttons row: a small copy of the player's buttons (its main row
 * and the row under it) and, below, the buttons not in it. Held and dragged, a button moves;
 * dragged out of the player, it leaves it; dragged in, it joins it where it is let go. A tap
 * takes a button off (or puts it on, below). Play/Pause always stays.
 */
@Composable
fun PlayerButtonsEditor(buttons: List<String>, onChange: (List<String>) -> Unit) {
    val chosen = chosenButtons(buttons)
    val unused = AllPlayerButtons.filter { it !in chosen }
    // Where each button and the player sit on screen, for a drag's drop.
    val places = remember { mutableStateMapOf<String, Rect>() }
    var player by remember { mutableStateOf(Rect.Zero) }
    var dragging by remember { mutableStateOf<String?>(null) }
    var moved by remember { mutableStateOf(Offset.Zero) }
    val scheme = MaterialTheme.colorScheme

    fun remove(name: String) {
        if (name == "play") showMessage("Play/Pause always stays") else onChange(chosen - name)
    }
    fun add(name: String, at: Int = chosen.size) {
        val rest = chosen - name
        onChange(rest.take(at.coerceIn(0, rest.size)) + name + rest.drop(at.coerceIn(0, rest.size)))
    }
    // Let go at `point`: in the player, the button goes before the first one it is left of
    // (row by row); outside it, it leaves.
    fun drop(name: String, point: Offset) {
        if (!player.inflate(24f).contains(point)) {
            if (name in chosen) remove(name)
            return
        }
        val others = chosen - name
        val index = others.indexOfFirst { other ->
            val r = places[other] ?: return@indexOfFirst false
            point.y < r.top || (point.y <= r.bottom && point.x < r.center.x)
        }.let { if (it < 0) others.size else it }
        add(name, index)
    }

    @Composable
    fun Button(name: String, inPlayer: Boolean) {
        val (icon, label) = playerButton(name)
        val play = name == "play"
        val lift = dragging == name
        Box(
            Modifier
                .zIndex(if (lift) 1f else 0f)
                .onGloballyPositioned { places[name] = it.boundsInRoot() }
                .graphicsLayer {
                    if (lift) {
                        translationX = moved.x
                        translationY = moved.y
                        scaleX = 1.15f
                        scaleY = 1.15f
                    }
                }
                .size(if (play) 44.dp else 38.dp)
                .clip(CircleShape)
                .background(if (play) scheme.primary else if (inPlayer) Color.White.copy(alpha = 0.12f) else scheme.surfaceContainerHighest)
                .then(if (inPlayer) Modifier else Modifier.border(0.5.dp, scheme.outlineVariant, CircleShape))
                .pointerInput(name, inPlayer) {
                    detectDragGesturesAfterLongPress(
                        onDragStart = { dragging = name; moved = Offset.Zero },
                        onDragEnd = {
                            val start = places[name]?.center ?: Offset.Zero
                            drop(name, start + moved)
                            dragging = null
                            moved = Offset.Zero
                        },
                        onDragCancel = { dragging = null; moved = Offset.Zero },
                    ) { change, amount -> change.consume(); moved += amount }
                }
                .clickable(onClickLabel = if (inPlayer) "Take off the player" else "Put on the player") {
                    if (inPlayer) remove(name) else add(name)
                }
                .semantics {
                    contentDescription = label
                    val i = chosen.indexOf(name)
                    customActions = buildList {
                        if (inPlayer && i > 0) add(CustomAccessibilityAction("Move earlier") { onChange(chosen.toMutableList().apply { add(i - 1, removeAt(i)) }); true })
                        if (inPlayer && i in 0 until chosen.size - 1) add(CustomAccessibilityAction("Move later") { onChange(chosen.toMutableList().apply { add(i + 1, removeAt(i)) }); true })
                    }
                },
            contentAlignment = Alignment.Center,
        ) {
            Icon(icon, contentDescription = null, tint = if (play) scheme.onPrimary else if (inPlayer) Color.White else scheme.onSurface, modifier = Modifier.size(if (play) 24.dp else 20.dp))
        }
    }

    // The player, small: dark as the real one, its main row and the row under it.
    Column(
        Modifier
            .fillMaxWidth()
            .onGloballyPositioned { player = it.boundsInRoot() }
            .clip(RoundedCornerShape(16.dp))
            .background(Color(0xFF1B1918))
            .padding(horizontal = 12.dp, vertical = 10.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Row(Modifier.fillMaxWidth().heightIn(min = 46.dp), horizontalArrangement = Arrangement.SpaceEvenly, verticalAlignment = Alignment.CenterVertically) {
            chosen.take(MainButtons).forEach { Button(it, inPlayer = true) }
        }
        if (chosen.size > MainButtons) {
            Row(
                Modifier.fillMaxWidth().clip(CircleShape).background(Color.White.copy(alpha = 0.06f)).padding(4.dp),
                horizontalArrangement = Arrangement.SpaceEvenly,
                verticalAlignment = Alignment.CenterVertically,
            ) { chosen.drop(MainButtons).forEach { Button(it, inPlayer = true) } }
        }
    }
    Text(
        if (unused.isEmpty()) "Every button is on the player." else "Not on the player",
        style = MaterialTheme.typography.labelMedium,
        color = scheme.onSurfaceVariant,
        modifier = Modifier.padding(top = 4.dp),
    )
    if (unused.isNotEmpty()) {
        Row(horizontalArrangement = Arrangement.spacedBy(10.dp), verticalAlignment = Alignment.CenterVertically) {
            unused.forEach { Button(it, inPlayer = false) }
        }
    }
    Text(
        "Hold a button to drag it. A tap takes it off, or puts it on.",
        style = MaterialTheme.typography.bodySmall,
        color = scheme.onSurfaceVariant,
    )
}
