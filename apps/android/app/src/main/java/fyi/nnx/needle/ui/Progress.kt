package fyi.nnx.needle.ui

import androidx.compose.animation.core.animateDpAsState
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.LinearWavyProgressIndicator
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import fyi.nnx.needle.NeedleApp

/** The looks the seek bar can have (Settings › Appearance). */
enum class SeekStyle(val id: String, val label: String) {
    Wave("wave", "Wave"),
    Line("line", "Line"),
    Thick("thick", "Thick");

    companion object {
        fun of(id: String) = entries.firstOrNull { it.id == id } ?: Wave
    }
}

@Composable
fun seekStyle(): SeekStyle {
    val app by NeedleApp.instance.app.collectAsState()
    return SeekStyle.of(app.seekStyle)
}

/**
 * How far the song has got, in the chosen style: a wave that moves while it plays and lies
 * flat while it rests (Android 16's media controls), a fine line, or a thick bar.
 * `big` is the full player's size; the mini player's is small.
 */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun ProgressLine(
    progress: Float,
    playing: Boolean,
    color: Color,
    track: Color,
    modifier: Modifier = Modifier,
    big: Boolean = true,
    style: SeekStyle = seekStyle(),
) {
    val moving = playing && !reduceMotion()
    when (style) {
        SeekStyle.Wave -> {
            val wave by animateFloatAsState(if (moving) 1f else 0f, tween(400), label = "wave")
            LinearWavyProgressIndicator(
                progress = { progress },
                amplitude = { wave },
                color = color,
                trackColor = track,
                wavelength = if (big) 36.dp else 24.dp,
                modifier = modifier.fillMaxWidth().height(if (big) 14.dp else 6.dp),
            )
        }
        SeekStyle.Line, SeekStyle.Thick -> {
            val thick = style == SeekStyle.Thick
            val size by animateDpAsState(
                when {
                    thick && big -> 10.dp
                    thick -> 5.dp
                    big -> 4.dp
                    else -> 3.dp
                },
                label = "bar",
            )
            Bar(progress, size, color, track, modifier)
        }
    }
}

/** A flat bar: the track, and the part played over it. */
@Composable
fun Bar(progress: Float, size: Dp, color: Color, track: Color, modifier: Modifier = Modifier) {
    Box(modifier.fillMaxWidth().height(size.coerceAtLeast(14.dp)), contentAlignment = Alignment.CenterStart) {
        Box(Modifier.fillMaxWidth().height(size).clip(CircleShape).background(track))
        Box(Modifier.fillMaxWidth(progress.coerceIn(0f, 1f)).height(size).clip(CircleShape).background(color))
    }
}
