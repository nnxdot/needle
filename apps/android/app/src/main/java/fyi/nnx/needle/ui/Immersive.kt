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
import androidx.compose.runtime.collectAsState
import fyi.nnx.needle.NeedleApp
import androidx.compose.runtime.getValue
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.graphics.toArgb
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clipToBounds
import androidx.compose.ui.draw.drawWithContent
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.graphics.drawscope.drawIntoCanvas
import androidx.compose.ui.graphics.nativeCanvas
import androidx.compose.runtime.setValue
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.isSpecified
import androidx.compose.ui.viewinterop.AndroidView
import coil3.compose.AsyncImage
import fyi.nnx.needle.core.Song
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

/**
 * The player's immersive cover (Settings › Appearance › Immersive cover), on a phone: behind
 * all, the player's backdrop (the cover far out of focus under a veil of its colour); over it,
 * the cover itself, sharp, across the top, edge to edge, solid down to half its height and then
 * going out into the backdrop below, with no hard edge. A moving cover beside the songs
 * (cover.mp4, or an animated cover.webp or cover.gif) plays in its place, fading the same way.
 *
 * On a wide screen the player gives its size: `width` (the player's area left of its pane) and
 * `height` (down to the song's name); `sideways`, the cover fills the left of the player from
 * top to foot instead, solid to half its width and going out into the backdrop at its right.
 * `fadeEnd`: across the top, its last part going out from left to right too (under the pane).
 */
@Composable
fun ImmersiveCover(
    song: Song,
    animated: Boolean,
    swipe: SkipSwipe? = null,
    width: Dp = Dp.Unspecified,
    height: Dp = Dp.Unspecified,
    sideways: Boolean = false,
    fadeEnd: Dp = 0.dp,
) {
    val video = movingCover(song, animated)
    BoxWithConstraints(Modifier.fillMaxSize()) {
        PlayerBackdrop(song)
        // About the top 56 % of the screen, never less than its width: on a tall phone the
        // picture reaches down behind the title, where it has gone out already.
        val tall = when {
            sideways -> maxHeight
            height.isSpecified -> height.coerceIn(0.dp, maxHeight)
            else -> maxOf(maxWidth, maxHeight * 0.56f).coerceAtMost(maxHeight)
        }
        val across = if (width.isSpecified) width.coerceIn(0.dp, maxWidth) else maxWidth
        val swiped = swipe
        Box(
            Modifier
                .width(across)
                .height(tall)
                .then(if (swiped == null) Modifier else Modifier.onSizeChanged { swiped.width = it.width.toFloat().coerceAtLeast(1f) })
                // Its own layer, so the fade below takes the picture away (DstIn) rather than
                // painting a colour over it. Swiped sideways over its room in the player, the
                // picture goes with the finger.
                .graphicsLayer {
                    compositingStrategy = androidx.compose.ui.graphics.CompositingStrategy.Offscreen
                    if (swiped != null) {
                        val shift = swiped.shift
                        translationX = shift
                        alpha = 1f - (kotlin.math.abs(shift) / swiped.width).coerceIn(0f, 1f) * 0.9f
                    }
                }
                .drawWithContent {
                    drawContent()
                    drawRect(
                        if (sideways) Brush.horizontalGradient(0.5f to Color.Black, 1f to Color.Transparent)
                        else Brush.verticalGradient(0.5f to Color.Black, 1f to Color.Transparent),
                        blendMode = androidx.compose.ui.graphics.BlendMode.DstIn,
                    )
                    if (fadeEnd > 0.dp && size.width > 0f) {
                        val from = (1f - fadeEnd.toPx() / size.width).coerceIn(0f, 1f)
                        drawRect(
                            Brush.horizontalGradient(from to Color.Black, 1f to Color.Transparent),
                            blendMode = androidx.compose.ui.graphics.BlendMode.DstIn,
                        )
                    }
                }
                .clipToBounds(),
        ) {
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
            // A video fades in its own layer (see Shade.mask).
            video?.let {
                val end = if (across > 0.dp) (fadeEnd / across).coerceIn(0f, 1f) else 0f
                val shade = if (sideways) Shade(Color.Transparent, right = 0f, foot = 0f, maskRight = 0.5f) else Shade(Color.Transparent, right = 0f, foot = 0f, mask = 0.5f, maskRight = end)
                MovingCover(it, shade, Modifier.fillMaxSize())
            }
            // The status bar and the two round buttons read over a bright cover.
            Box(
                Modifier.fillMaxWidth().height(if (sideways) 160.dp else tall * 0.24f)
                    .background(Brush.verticalGradient(listOf(Color.Black.copy(alpha = 0.32f), Color.Transparent))),
            )
        }
    }
}

/**
 * The moving cover of the song (a video beside the songs, or an animated WebP or GIF), looked
 * up off the main thread, or null: none, `animated` off (Moving covers off, or Reduce motion
 * on), or the computer's song. The last song's never plays over this one.
 */
@Composable
fun movingCover(song: Song, animated: Boolean): String? {
    val context = androidx.compose.ui.platform.LocalContext.current
    val app by NeedleApp.instance.app.collectAsState()
    val anyNetwork = app.movingCoversOnMobileData
    val path by produceState<String?>(null, song.id, animated, anyNetwork) {
        value = null
        if (animated && !song.onComputer()) {
            value = withContext(Dispatchers.IO) {
                runCatching {
                    // On mobile data (or any metered network), with "On mobile data" off: only a
                    // moving cover already on this phone, never a download.
                    if (!anyNetwork && meteredNetwork(context)) core.animatedCoverKept(song.id) else core.animatedCover(song.id)
                }.getOrNull()
            }
        }
    }
    return path
}

/** The phone is on mobile data, or another network that counts what it sends (not unmetered). */
private fun meteredNetwork(context: android.content.Context): Boolean = runCatching {
    context.getSystemService(android.net.ConnectivityManager::class.java)?.isActiveNetworkMetered ?: false
}.getOrDefault(false)

/**
 * A moving cover filling its frame (cut, not squeezed): an animated WebP or GIF drawn with
 * Compose (under any fades drawn over it), or a video in a layer of its own, whose fades are
 * drawn in that layer too ([shade]; null for none), or it would show hard edges.
 */
@Composable
fun MovingCover(path: String, shade: Shade?, modifier: Modifier = Modifier) {
    if (isAnimatedPicture(path)) AnimatedPicture(path, modifier)
    else LoopingVideo(path, shade ?: Shade(Color.Transparent, right = 0f, foot = 0f), modifier, darkTop = shade != null)
}

/**
 * A short video over and over, silent, filling its frame (cut, not squeezed): a moving cover.
 * Android's own player, on a texture, so it moves and fades with the rest of the page.
 */
/**
 * The fades over a moving cover, drawn in the video's own layer: into `tone` across its right
 * side (`right`, a share of its width) and its foot (`foot`, of its height), and the darker top
 * where the status bar and the buttons sit. `mask`: the lowest share of its height goes
 * see-through instead (the immersive cover going into the backdrop under it); `maskRight`, the
 * same across its right side (the wide player's cover, at the left).
 */
data class Shade(val tone: Color, val right: Float, val foot: Float, val mask: Float = 0f, val maskRight: Float = 0f)

@Composable
fun LoopingVideo(path: String, shade: Shade, modifier: Modifier = Modifier, darkTop: Boolean = true) {
    val holder = remember(path) { arrayOfNulls<MediaPlayer>(1) }
    androidx.compose.runtime.DisposableEffect(path) {
        onDispose {
            holder[0]?.release()
            holder[0] = null
        }
    }
    val top = if (darkTop) with(androidx.compose.ui.platform.LocalDensity.current) { 200.dp.toPx() } else 0f
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
                // A layer of its own, so the mask (Fades) takes the video away below it.
                if (shade.mask > 0f || shade.maskRight > 0f) setLayerType(android.view.View.LAYER_TYPE_HARDWARE, null)
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
        if (end > 0f) fill(
            android.graphics.LinearGradient(0f, 0f, 0f, end, intArrayOf(0x80000000.toInt(), 0x33000000, 0), floatArrayOf(0f, 0.5f, 1f), android.graphics.Shader.TileMode.CLAMP),
            0f, 0f, w, end,
        )
        if (shade.mask > 0f) {
            // Keeps what is drawn under it where the gradient is solid, and takes it away where
            // it is clear (DST_IN), in the frame's own layer.
            val from = h * (1f - shade.mask)
            paint.xfermode = android.graphics.PorterDuffXfermode(android.graphics.PorterDuff.Mode.DST_IN)
            fill(android.graphics.LinearGradient(0f, from, 0f, h, 0xFF000000.toInt(), 0, android.graphics.Shader.TileMode.CLAMP), 0f, from, w, h)
            paint.xfermode = null
        }
        if (shade.maskRight > 0f) {
            val from = w * (1f - shade.maskRight)
            paint.xfermode = android.graphics.PorterDuffXfermode(android.graphics.PorterDuff.Mode.DST_IN)
            fill(android.graphics.LinearGradient(from, 0f, w, 0f, 0xFF000000.toInt(), 0, android.graphics.Shader.TileMode.CLAMP), from, 0f, w, h)
            paint.xfermode = null
        }
    }
}

/** A moving cover that is a picture (an animated WebP or GIF), not a video. */
private fun isAnimatedPicture(path: String): Boolean =
    path.substringAfterLast('.', "").lowercase() in setOf("webp", "gif")

/**
 * An animated WebP or GIF, over and over, filling its frame (cut, not squeezed). Android 9 and
 * newer play it (AnimatedImageDrawable); older Android shows its first frame, still.
 */
@Composable
private fun AnimatedPicture(path: String, modifier: Modifier = Modifier) {
    val context = androidx.compose.ui.platform.LocalContext.current
    val picture by produceState<android.graphics.drawable.Drawable?>(null, path) {
        value = withContext(Dispatchers.IO) {
            runCatching {
                if (android.os.Build.VERSION.SDK_INT >= 28) {
                    android.graphics.ImageDecoder.decodeDrawable(android.graphics.ImageDecoder.createSource(java.io.File(path)))
                } else {
                    android.graphics.BitmapFactory.decodeFile(path)?.let { android.graphics.drawable.BitmapDrawable(context.resources, it) }
                }
            }.getOrNull()
        }
    }
    val drawable = picture ?: return
    // Each new frame redraws the picture, and only it.
    var frame by androidx.compose.runtime.remember(drawable) { androidx.compose.runtime.mutableIntStateOf(0) }
    // Kept here: a drawable holds its callback weakly.
    val callback = androidx.compose.runtime.remember(drawable) {
        val handler = android.os.Handler(android.os.Looper.getMainLooper())
        object : android.graphics.drawable.Drawable.Callback {
            override fun invalidateDrawable(who: android.graphics.drawable.Drawable) { frame++ }
            override fun scheduleDrawable(who: android.graphics.drawable.Drawable, what: Runnable, `when`: Long) { handler.postAtTime(what, who, `when`) }
            override fun unscheduleDrawable(who: android.graphics.drawable.Drawable, what: Runnable) { handler.removeCallbacks(what, who) }
        }
    }
    androidx.compose.runtime.DisposableEffect(drawable) {
        drawable.callback = callback
        if (android.os.Build.VERSION.SDK_INT >= 28 && drawable is android.graphics.drawable.AnimatedImageDrawable) {
            drawable.repeatCount = android.graphics.drawable.AnimatedImageDrawable.REPEAT_INFINITE
            drawable.start()
        }
        onDispose {
            if (android.os.Build.VERSION.SDK_INT >= 28 && drawable is android.graphics.drawable.AnimatedImageDrawable) drawable.stop()
            drawable.callback = null
        }
    }
    androidx.compose.foundation.Canvas(modifier.clipToBounds()) {
        frame.let { }
        val width = drawable.intrinsicWidth.coerceAtLeast(1)
        val height = drawable.intrinsicHeight.coerceAtLeast(1)
        val scale = maxOf(size.width / width, size.height / height)
        val left = (size.width - width * scale) / 2
        val top = (size.height - height * scale) / 2
        drawable.setBounds(left.toInt(), top.toInt(), (left + width * scale).toInt(), (top + height * scale).toInt())
        drawIntoCanvas { drawable.draw(it.nativeCanvas) }
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
