package fyi.nnx.needle.ui

import android.graphics.SurfaceTexture
import android.media.MediaPlayer
import android.view.Surface
import android.view.TextureView
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.width
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.graphics.toArgb
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clipToBounds
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.unit.dp
import androidx.compose.ui.viewinterop.AndroidView
import coil3.compose.AsyncImage
import fyi.nnx.needle.core.Song
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

/**
 * The player's cover as Apple Music shows it (Settings › Appearance › Immersive cover): across
 * the top, edge to edge, down to just above the song's name, fading into the cover's colour
 * (deep, as the player is always dark). Beside the controls on a wide screen, it fills the
 * left side instead. A moving cover beside the songs (cover.mp4) plays over it.
 */
@Composable
fun ImmersiveCover(song: Song, animated: Boolean, wide: Boolean) {
    val tone = coverColor(song.artwork)
    // The moving cover, looked up off the main thread; the last song's never plays over this one.
    val video by produceState<String?>(null, song.id, animated) {
        value = null
        if (animated && !song.onComputer()) {
            value = withContext(Dispatchers.IO) { runCatching { core.animatedCover(song.id) }.getOrNull() }
        }
    }
    BoxWithConstraints(Modifier.fillMaxSize().background(tone)) {
        val height = maxHeight
        val picture: @Composable (Modifier, Shade) -> Unit = { frame, shade ->
            Box(frame.clipToBounds()) {
                if (song.artwork != null) {
                    AsyncImage(
                        model = coverRequest(fileUri(song.artwork)),
                        contentDescription = null,
                        contentScale = ContentScale.Crop,
                        modifier = Modifier.fillMaxSize(),
                    )
                } else {
                    MadeUpCover(song.album + song.artist)
                }
                // The video draws in a layer of its own, over what Compose draws: its fades are
                // drawn in that layer too (shade), or it would show hard edges.
                video?.let { LoopingVideo(it, shade, Modifier.fillMaxSize()) }
            }
        }
        if (wide) {
            // The left side, top to foot, fading into the colour toward the controls and the foot.
            val side = minOf(maxHeight, maxWidth * 0.62f)
            Box(Modifier.width(side).fillMaxHeight()) {
                picture(Modifier.fillMaxSize(), Shade(tone, right = 0.45f, foot = 0.4f))
                Box(
                    Modifier.align(Alignment.CenterEnd).width(side * 0.45f).fillMaxHeight()
                        .background(Brush.horizontalGradient(listOf(tone.copy(alpha = 0f), tone.copy(alpha = 0.7f), tone))),
                )
                Box(
                    Modifier.align(Alignment.BottomCenter).fillMaxWidth().height(height * 0.4f)
                        .background(Brush.verticalGradient(listOf(tone.copy(alpha = 0f), tone))),
                )
            }
        } else {
            // Down to just above the name and the controls: a square cover on a tall phone
            // would stop halfway, over an empty band.
            val tall = maxOf(maxWidth, maxHeight - 400.dp)
            Box(Modifier.fillMaxWidth().height(tall)) {
                picture(Modifier.fillMaxSize(), Shade(tone, right = 0f, foot = 0.42f))
                Box(
                    Modifier.align(Alignment.BottomCenter).fillMaxWidth().height(tall * 0.42f)
                        .background(Brush.verticalGradient(listOf(tone.copy(alpha = 0f), tone.copy(alpha = 0.75f), tone))),
                )
            }
        }
        // The status bar and the top buttons read over a bright cover.
        Box(
            Modifier.fillMaxWidth().height(200.dp)
                .background(Brush.verticalGradient(listOf(Color.Black.copy(alpha = 0.5f), Color.Black.copy(alpha = 0.2f), Color.Transparent))),
        )
    }
}

/**
 * A short video over and over, silent, filling its frame (cut, not squeezed): a moving cover.
 * Android's own player, on a texture, so it moves and fades with the rest of the page.
 */
/**
 * The fades over a moving cover, drawn in the video's own layer: into `tone` across its right
 * side (`right`, a share of its width) and its foot (`foot`, of its height), and the darker top
 * where the status bar and the buttons sit.
 */
data class Shade(val tone: Color, val right: Float, val foot: Float)

@Composable
fun LoopingVideo(path: String, shade: Shade, modifier: Modifier = Modifier) {
    val holder = remember(path) { arrayOfNulls<MediaPlayer>(1) }
    androidx.compose.runtime.DisposableEffect(path) {
        onDispose {
            holder[0]?.release()
            holder[0] = null
        }
    }
    val top = with(androidx.compose.ui.platform.LocalDensity.current) { 200.dp.toPx() }
    AndroidView(
        modifier = modifier,
        update = { frame -> (frame.getChildAt(1) as Fades).set(shade, top) },
        factory = { context ->
            val video = TextureView(context).apply {
                surfaceTextureListener = object : TextureView.SurfaceTextureListener {
                    override fun onSurfaceTextureAvailable(texture: SurfaceTexture, width: Int, height: Int) {
                        val view = this@apply
                        runCatching {
                            holder[0]?.release()
                            holder[0] = MediaPlayer().apply {
                                setDataSource(path)
                                setSurface(Surface(texture))
                                isLooping = true
                                setVolume(0f, 0f)
                                setOnVideoSizeChangedListener { _, w, h -> fill(view, w, h) }
                                setOnPreparedListener { it.start() }
                                prepareAsync()
                            }
                        }
                    }

                    override fun onSurfaceTextureSizeChanged(texture: SurfaceTexture, width: Int, height: Int) {
                        holder[0]?.let { fill(this@apply, it.videoWidth, it.videoHeight) }
                    }

                    override fun onSurfaceTextureDestroyed(texture: SurfaceTexture): Boolean {
                        holder[0]?.release()
                        holder[0] = null
                        return true
                    }

                    override fun onSurfaceTextureUpdated(texture: SurfaceTexture) {}
                }
            }
            android.widget.FrameLayout(context).apply {
                addView(video)
                addView(Fades(context))
            }
        },
    )
}

/** Draws a [Shade]'s fades, over the video, in its layer. */
private class Fades(context: android.content.Context) : android.view.View(context) {
    private var shade: Shade? = null
    private var top = 0f
    private val paint = android.graphics.Paint()

    fun set(shade: Shade, top: Float) {
        if (shade == this.shade && top == this.top) return
        this.shade = shade
        this.top = top
        invalidate()
    }

    override fun onDraw(canvas: android.graphics.Canvas) {
        val shade = shade ?: return
        val w = width.toFloat()
        val h = height.toFloat()
        val tone = shade.tone.toArgb()
        val clear = android.graphics.Color.argb(0, android.graphics.Color.red(tone), android.graphics.Color.green(tone), android.graphics.Color.blue(tone))
        val mid = android.graphics.Color.argb(190, android.graphics.Color.red(tone), android.graphics.Color.green(tone), android.graphics.Color.blue(tone))
        fun fill(shader: android.graphics.Shader, left: Float, topY: Float, right: Float, bottom: Float) {
            paint.shader = shader
            canvas.drawRect(left, topY, right, bottom, paint)
        }
        if (shade.foot > 0f) {
            val from = h * (1f - shade.foot)
            fill(android.graphics.LinearGradient(0f, from, 0f, h, intArrayOf(clear, mid, tone), floatArrayOf(0f, 0.5f, 1f), android.graphics.Shader.TileMode.CLAMP), 0f, from, w, h)
        }
        if (shade.right > 0f) {
            val from = w * (1f - shade.right)
            fill(android.graphics.LinearGradient(from, 0f, w, 0f, intArrayOf(clear, mid, tone), floatArrayOf(0f, 0.6f, 1f), android.graphics.Shader.TileMode.CLAMP), from, 0f, w, h)
        }
        val end = minOf(top, h)
        fill(
            android.graphics.LinearGradient(0f, 0f, 0f, end, intArrayOf(0x80000000.toInt(), 0x33000000, 0), floatArrayOf(0f, 0.5f, 1f), android.graphics.Shader.TileMode.CLAMP),
            0f, 0f, w, end,
        )
    }
}

/** The video scaled to fill the view and cut at its sides (or top and foot), not squeezed. */
private fun fill(view: TextureView, videoWidth: Int, videoHeight: Int) {
    if (videoWidth <= 0 || videoHeight <= 0 || view.width <= 0 || view.height <= 0) return
    val w = view.width.toFloat()
    val h = view.height.toFloat()
    val scale = maxOf(w / videoWidth, h / videoHeight)
    val matrix = android.graphics.Matrix()
    matrix.setScale(videoWidth * scale / w, videoHeight * scale / h, w / 2, h / 2)
    view.setTransform(matrix)
}
