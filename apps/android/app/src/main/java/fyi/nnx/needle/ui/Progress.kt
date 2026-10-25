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
    Thick("thick", "Thick"),
    /** The song's own loud and quiet shape (its waveform, measured once by the core). */
    Shape("shape", "Shape");

    companion object {
        fun of(id: String) = entries.firstOrNull { it.id == id } ?: Shape
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
    /** The song's shape (0 to 1 each), for [SeekStyle.Shape]; empty while it is measured. */
    heights: List<Float> = emptyList(),
) {
    val moving = playing && !reduceMotion()
    when (style) {
        // The mini player is too small for the shape: a fine line there.
        SeekStyle.Shape -> if (big) Shape(progress, heights, color, track, modifier) else Bar(progress, 3.dp, color, track, modifier)
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

/**
 * The song's shape as the seek bar: thin rounded bars, loud parts tall and quiet ones short,
 * the part played in `color`. Until the shape is known, a fine line.
 */
@Composable
fun Shape(progress: Float, heights: List<Float>, color: Color, track: Color, modifier: Modifier = Modifier) {
    if (heights.isEmpty()) {
        Bar(progress, 4.dp, color, track, modifier)
        return
    }
    androidx.compose.foundation.Canvas(modifier.fillMaxWidth().height(36.dp)) {
        val step = size.width / heights.size
        val bar = (step * 0.62f).coerceAtLeast(1f)
        val played = progress.coerceIn(0f, 1f) * size.width
        heights.forEachIndexed { i, h ->
            val x = i * step + step / 2
            val tall = (0.12f + h.coerceIn(0f, 1f) * 0.88f) * size.height
            drawLine(
                if (x <= played) color else track,
                androidx.compose.ui.geometry.Offset(x, (size.height - tall) / 2),
                androidx.compose.ui.geometry.Offset(x, (size.height + tall) / 2),
                strokeWidth = bar,
                cap = androidx.compose.ui.graphics.StrokeCap.Round,
            )
        }
    }
}

/**
 * The shape of `song` for the seek bar, `count` parts: measured by the core the first time (a
 * moment, off the main thread) and kept. None for songs on a server or on the computer.
 */
@Composable
fun rememberShape(song: fyi.nnx.needle.core.Song, count: Int = 90): List<Float> {
    val wanted = seekStyle() == SeekStyle.Shape && !song.fromServer() && !song.onComputer()
    val shape by androidx.compose.runtime.produceState(emptyList<Float>(), song.id, wanted) {
        value = emptyList()
        if (wanted) {
            value = kotlinx.coroutines.withContext(kotlinx.coroutines.Dispatchers.IO) {
                runCatching { NeedleApp.instance.core.waveform(song.id, count.toUInt()) }.getOrDefault(emptyList())
            }
        }
    }
    return shape
}

/** A flat bar: the track, and the part played over it. */
@Composable
fun Bar(progress: Float, size: Dp, color: Color, track: Color, modifier: Modifier = Modifier) {
    Box(modifier.fillMaxWidth().height(size.coerceAtLeast(14.dp)), contentAlignment = Alignment.CenterStart) {
        Box(Modifier.fillMaxWidth().height(size).clip(CircleShape).background(track))
        Box(Modifier.fillMaxWidth(progress.coerceIn(0f, 1f)).height(size).clip(CircleShape).background(color))
    }
}
