package fyi.nnx.needle.ui

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Close
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.lerp
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import fyi.nnx.needle.NeedleApp
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import kotlin.math.PI
import kotlin.math.cos
import kotlin.math.sin

/** How many bands the visualizer reads from the core's spectrum. */
private const val Bands = 48

/**
 * The spectrum of what plays now, read from the core about thirty times a second while shown,
 * as the core eases it (rising quickly, falling slowly, as light does). All zero while nothing
 * plays.
 */
@Composable
private fun rememberSpectrum(): FloatArray {
    var shown by remember { mutableStateOf(FloatArray(Bands)) }
    val reduce = reduceMotion()
    LaunchedEffect(reduce) {
        // Reduce motion: the light rests, a soft even glow.
        if (reduce) {
            shown = FloatArray(Bands) { 0.3f }
            return@LaunchedEffect
        }
        while (true) {
            withFrameNanos { }
            // Eased by the core already (a band rises over about 70 ms, falls over about 350 ms).
            val now = withContext(Dispatchers.Default) { runCatching { NeedleApp.instance.core.spectrum(Bands.toUInt()) }.getOrNull() }
            if (now != null && now.size == Bands) shown = FloatArray(Bands) { now[it].coerceIn(0f, 1f) }
            kotlinx.coroutines.delay(30)
        }
    }
    return shown
}

/**
 * Light that moves with the music, in the cover's colours (the player's colour scheme): a
 * soft glow in the middle that swells with the bass, and rays all round, each as long as its
 * band is loud.
 */
@Composable
fun Visualizer(modifier: Modifier = Modifier) {
    val bands = rememberSpectrum()
    val scheme = MaterialTheme.colorScheme
    val inner = scheme.primary
    val outer = scheme.tertiary
    Canvas(modifier) {
        val c = Offset(size.width / 2, size.height / 2)
        val r = minOf(size.width, size.height) / 2
        val bass = (bands.take(6).average().toFloat()).coerceIn(0f, 1f)
        val level = bands.average().toFloat().coerceIn(0f, 1f)
        // The glow: larger and brighter with the bass.
        drawCircle(
            Brush.radialGradient(
                listOf(inner.copy(alpha = 0.25f + 0.55f * bass), outer.copy(alpha = 0.12f + 0.3f * level), Color.Transparent),
                center = c,
                radius = r * (0.55f + 0.45f * bass),
            ),
            radius = r * (0.55f + 0.45f * bass),
            center = c,
        )
        // The rays: low bands at the top, round both sides, so the picture is even.
        val base = r * 0.34f
        val stroke = (2 * PI * base / (Bands * 2) * 0.55).toFloat().coerceAtLeast(2f)
        for (i in 0 until Bands * 2) {
            val band = if (i < Bands) i else Bands * 2 - 1 - i
            val v = bands[band]
            val angle = (i.toFloat() / (Bands * 2)) * 2 * PI - PI / 2
            val length = base + (r * 0.62f) * (0.06f + v)
            val start = Offset(c.x + cos(angle).toFloat() * base, c.y + sin(angle).toFloat() * base)
            val end = Offset(c.x + cos(angle).toFloat() * length, c.y + sin(angle).toFloat() * length)
            drawLine(lerp(inner, outer, band.toFloat() / Bands).copy(alpha = 0.55f + 0.45f * v), start, end, stroke, StrokeCap.Round)
        }
    }
}

/**
 * The visualizer full screen, for a party or a TV: the light on black, the song small at the
 * foot, and the screen kept on. A tap shows Close; Back closes it too.
 */
@Composable
fun FullVisualizer(onClose: () -> Unit) {
    val playbackState = NeedleApp.instance.playback.collectAsState()
    val current by remember { derivedStateOf { playbackState.value?.current } }
    val song = current
    val view = LocalView.current
    DisposableEffect(Unit) {
        view.keepScreenOn = true
        onDispose { view.keepScreenOn = false }
    }
    androidx.activity.compose.BackHandler(onBack = onClose)
    var chrome by remember { mutableStateOf(true) }
    LaunchedEffect(chrome) {
        if (chrome) {
            kotlinx.coroutines.delay(3000)
            chrome = false
        }
    }
    CoverTheme(song?.artwork) {
        Box(
            Modifier
                .fillMaxSize()
                .background(Color.Black)
                .clickable(interactionSource = remember { MutableInteractionSource() }, indication = null) { chrome = !chrome },
        ) {
            Visualizer(Modifier.fillMaxSize().padding(24.dp))
            if (song != null) {
                Column(Modifier.align(Alignment.BottomCenter).navigationBarsPadding().padding(bottom = 32.dp), horizontalAlignment = Alignment.CenterHorizontally) {
                    Text(song.title, color = Color.White, style = MaterialTheme.typography.titleLarge, fontWeight = FontWeight.SemiBold, maxLines = 1, overflow = TextOverflow.Ellipsis)
                    Text(song.artist, color = Color.White.copy(alpha = 0.7f), style = MaterialTheme.typography.bodyLarge, maxLines = 1)
                }
            }
            if (chrome) {
                IconButton(onClick = onClose, modifier = Modifier.align(Alignment.TopEnd).statusBarsPadding().padding(12.dp).firstFocus()) {
                    Icon(Icons.Rounded.Close, contentDescription = "Close the visualizer", tint = Color.White)
                }
            }
        }
    }
}
