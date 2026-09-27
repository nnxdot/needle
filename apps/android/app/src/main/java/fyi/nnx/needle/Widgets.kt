package fyi.nnx.needle

import android.content.Context
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.graphics.Canvas
import android.graphics.Paint
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.map
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.luminance
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.glance.ColorFilter
import androidx.glance.GlanceId
import androidx.glance.GlanceModifier
import androidx.glance.GlanceTheme
import androidx.glance.Image
import androidx.glance.ImageProvider
import androidx.glance.LocalSize
import androidx.glance.action.ActionParameters
import androidx.glance.action.actionParametersOf
import androidx.glance.action.actionStartActivity
import androidx.glance.action.clickable
import androidx.glance.appwidget.GlanceAppWidget
import androidx.glance.appwidget.GlanceAppWidgetReceiver
import androidx.glance.appwidget.LinearProgressIndicator
import androidx.glance.appwidget.SizeMode
import androidx.glance.appwidget.action.ActionCallback
import androidx.glance.appwidget.action.actionRunCallback
import androidx.glance.appwidget.cornerRadius
import androidx.glance.appwidget.provideContent
import androidx.glance.appwidget.updateAll
import androidx.glance.background
import androidx.glance.layout.Alignment
import androidx.glance.layout.Box
import androidx.glance.layout.Column
import androidx.glance.layout.ContentScale
import androidx.glance.layout.Row
import androidx.glance.layout.Spacer
import androidx.glance.layout.fillMaxHeight
import androidx.glance.layout.fillMaxSize
import androidx.glance.layout.fillMaxWidth
import androidx.glance.layout.height
import androidx.glance.layout.padding
import androidx.glance.layout.size
import androidx.glance.layout.width
import androidx.glance.text.FontWeight
import androidx.glance.text.Text
import androidx.glance.text.TextStyle
import androidx.glance.unit.ColorProvider
import fyi.nnx.needle.core.Song

/*
 * Home screen widgets, drawn like the app's player: the cover, blurred, fills the widget;
 * the cover itself sits on it; the play button is a pill in the cover's own colour.
 */

/** A cover, small, for a widget (widgets hold pictures in memory, so it stays small). */
private fun cover(path: String?, size: Int = 256): Bitmap? = path?.let {
    runCatching {
        val bounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }
        BitmapFactory.decodeFile(it, bounds)
        val step = maxOf(1, minOf(bounds.outWidth, bounds.outHeight) / size)
        BitmapFactory.decodeFile(it, BitmapFactory.Options().apply { inSampleSize = step })?.let { b -> fyi.nnx.needle.ui.trimBars(b) }
    }.getOrNull()
}

/**
 * The cover as a soft, dark wash: shrunk to a few pixels and grown back, which blurs it, then
 * darkened so white text reads on it.
 */
private fun wash(art: Bitmap?, dark: Float = 0.5f): Bitmap? = art?.let {
    val tiny = Bitmap.createScaledBitmap(it, 12, 12, true)
    val soft = Bitmap.createScaledBitmap(tiny, 240, 240, true)
    val out = soft.copy(Bitmap.Config.ARGB_8888, true)
    Canvas(out).drawColor(android.graphics.Color.argb((dark * 255).toInt(), 0, 0, 0))
    out
}

/** The cover's lively colour for the play button, light enough for a dark icon on it. */
private fun accent(art: Bitmap?): Color {
    val palette = art?.let { androidx.palette.graphics.Palette.from(it).generate() }
    val rgb = palette?.lightVibrantSwatch?.rgb ?: palette?.vibrantSwatch?.rgb ?: palette?.lightMutedSwatch?.rgb
        ?: return Color(0xFFE2B46C)
    val c = Color(rgb)
    // Lifted toward white until a dark icon reads on it.
    return if (c.luminance() < 0.45f) Color(c.red * 0.5f + 0.5f, c.green * 0.5f + 0.5f, c.blue * 0.5f + 0.5f) else c
}

private val White = ColorProvider(Color.White)
private val Soft = ColorProvider(Color(0xCCFFFFFF))
private val Faint = ColorProvider(Color(0x99FFFFFF))
private val Glass = ColorProvider(Color(0x26FFFFFF))
private val DarkIcon = ColorProvider(Color(0xFF1A1714))

/** Refreshes the widgets, when what plays changes. */
suspend fun updateWidgets(context: Context) {
    runCatching { NowPlayingWidget().updateAll(context) }
    runCatching { RecentWidget().updateAll(context) }
}

// ---------- Now playing

private class Now(val song: Song?, val playing: Boolean, val progress: Float, val art: Bitmap?, val wash: Bitmap?, val accent: Color)

class NowPlayingWidget : GlanceAppWidget() {
    override val sizeMode = SizeMode.Exact

    override suspend fun provideGlance(context: Context, id: GlanceId) {
        // What plays, followed while the widget is on screen: a new song, play or pause, or a
        // step of the progress bar (every 2%) redraws it; nothing else does.
        val shown = NeedleApp.instance.playback
            .map { p ->
                val song = p?.current
                val step = if (song != null && song.duration > 0) ((p.position / song.duration) * 50).toInt() else 0
                Triple(song, p?.playing == true, step)
            }
            .distinctUntilChanged { a, b -> a.first?.id == b.first?.id && a.second == b.second && a.third == b.third }
        provideContent {
            val (song, playing, step) = shown.collectAsState(Triple(null, false, 0)).value
            val art = androidx.compose.runtime.remember(song?.artwork) { cover(song?.artwork, 320) }
            val soft = androidx.compose.runtime.remember(art) { wash(art) }
            val color = androidx.compose.runtime.remember(art) { accent(art) }
            val now = Now(song, playing, step / 50f, art, soft, color)
            GlanceTheme {
                val size = LocalSize.current
                when {
                    size.width < 200.dp -> Small(now)
                    size.height < 250.dp -> Wide(now)
                    else -> Large(now)
                }
            }
        }
    }
}

/** The blurred cover behind everything, or Needle's charcoal. */
@Composable
private fun Backdrop(now: Now, content: @Composable () -> Unit) {
    Box(
        GlanceModifier.fillMaxSize().cornerRadius(28.dp).background(ColorProvider(Color(0xFF1B1918)))
            .clickable(actionStartActivity<MainActivity>()),
    ) {
        now.wash?.let { Image(ImageProvider(it), contentDescription = null, contentScale = ContentScale.FillBounds, modifier = GlanceModifier.fillMaxSize()) }
        content()
    }
}

@Composable
private fun CoverImage(now: Now, side: Dp, round: Dp = 18.dp) {
    Box(GlanceModifier.size(side).cornerRadius(round).background(Glass), contentAlignment = Alignment.Center) {
        if (now.art != null) {
            Image(ImageProvider(now.art), contentDescription = now.song?.title, contentScale = ContentScale.Crop, modifier = GlanceModifier.size(side).cornerRadius(round))
        } else {
            Image(ImageProvider(R.drawable.needle_logo), contentDescription = null, modifier = GlanceModifier.size(side * 0.5f).cornerRadius(10.dp))
        }
    }
}

@Composable
private fun Titles(now: Now, big: Boolean) {
    Text(
        now.song?.title ?: "Needle",
        maxLines = if (big) 2 else 1,
        style = TextStyle(color = White, fontSize = if (big) 22.sp else 17.sp, fontWeight = FontWeight.Bold),
    )
    Text(
        now.song?.artist ?: "Tap play for a shuffle of your music",
        maxLines = 1,
        style = TextStyle(color = Soft, fontSize = if (big) 15.sp else 13.sp),
    )
}

@Composable
private fun Progress(now: Now) {
    if (now.song == null) return
    LinearProgressIndicator(
        progress = now.progress,
        color = ColorProvider(now.accent),
        backgroundColor = ColorProvider(Color(0x33FFFFFF)),
        modifier = GlanceModifier.fillMaxWidth().height(4.dp).cornerRadius(2.dp),
    )
}

@Composable
private fun Controls(now: Now, play: Dp = 64.dp) {
    Row(verticalAlignment = Alignment.CenterVertically) {
        Round(R.drawable.ic_widget_previous, "Previous", actionRunCallback<Previous>())
        Spacer(GlanceModifier.width(10.dp))
        PlayPill(now, play)
        Spacer(GlanceModifier.width(10.dp))
        Round(R.drawable.ic_widget_next, "Next", actionRunCallback<Next>())
    }
}

/** Play or pause: a pill in the cover's colour, as the app's player button. */
@Composable
private fun PlayPill(now: Now, width: Dp, height: Dp = 48.dp) {
    Box(
        GlanceModifier.size(width, height).cornerRadius(height / 2).background(ColorProvider(now.accent)).clickable(actionRunCallback<Toggle>()),
        contentAlignment = Alignment.Center,
    ) {
        Image(
            ImageProvider(if (now.playing) R.drawable.ic_widget_pause else R.drawable.ic_widget_play),
            contentDescription = if (now.playing) "Pause" else "Play",
            colorFilter = ColorFilter.tint(DarkIcon),
            modifier = GlanceModifier.size(height * 0.5f),
        )
    }
}

@Composable
private fun Round(icon: Int, label: String, action: androidx.glance.action.Action) {
    Box(GlanceModifier.size(42.dp).cornerRadius(21.dp).background(Glass).clickable(action), contentAlignment = Alignment.Center) {
        Image(ImageProvider(icon), contentDescription = label, colorFilter = ColorFilter.tint(White), modifier = GlanceModifier.size(22.dp))
    }
}

/** The Needle mark and name, small, in a corner. */
@Composable
private fun Mark() {
    Row(verticalAlignment = Alignment.CenterVertically) {
        Image(ImageProvider(R.drawable.needle_logo), contentDescription = null, modifier = GlanceModifier.size(18.dp).cornerRadius(5.dp))
        Spacer(GlanceModifier.width(6.dp))
        Text("Needle", style = TextStyle(color = Faint, fontSize = 12.sp, fontWeight = FontWeight.Medium))
    }
}

/** Small (2×2): the cover fills it, the play pill in a corner, progress along the foot. */
@Composable
private fun Small(now: Now) {
    Backdrop(now) {
        if (now.art != null) {
            Image(ImageProvider(now.art), contentDescription = now.song?.title, contentScale = ContentScale.Crop, modifier = GlanceModifier.fillMaxSize().cornerRadius(28.dp))
        }
        Column(GlanceModifier.fillMaxSize().padding(12.dp), verticalAlignment = Alignment.Bottom, horizontalAlignment = Alignment.End) {
            PlayPill(now, 56.dp, 44.dp)
        }
    }
}

/** Wide and short (4×2): the cover at full height, the song, progress, and the buttons. */
@Composable
private fun Wide(now: Now) {
    val size = LocalSize.current
    val side = minOf(size.height - 28.dp, size.width * 0.4f).coerceAtLeast(64.dp)
    Backdrop(now) {
        Row(GlanceModifier.fillMaxSize().padding(14.dp), verticalAlignment = Alignment.CenterVertically) {
            CoverImage(now, side)
            Spacer(GlanceModifier.width(14.dp))
            Column(GlanceModifier.defaultWeight().fillMaxHeight(), verticalAlignment = Alignment.CenterVertically) {
                Titles(now, big = false)
                Spacer(GlanceModifier.height(10.dp))
                Progress(now)
                Spacer(GlanceModifier.height(10.dp))
                Controls(now)
            }
        }
    }
}

/** Large (4×3 and up): the cover and the mark at the top, the song big, then the controls. */
@Composable
private fun Large(now: Now) {
    Backdrop(now) {
        Column(GlanceModifier.fillMaxSize().padding(18.dp)) {
            Row(GlanceModifier.fillMaxWidth(), verticalAlignment = Alignment.Top) {
                CoverImage(now, 96.dp, 20.dp)
                Spacer(GlanceModifier.defaultWeight())
                Mark()
            }
            Spacer(GlanceModifier.defaultWeight())
            Titles(now, big = true)
            Spacer(GlanceModifier.height(12.dp))
            Progress(now)
            Spacer(GlanceModifier.height(12.dp))
            Row(GlanceModifier.fillMaxWidth(), horizontalAlignment = Alignment.CenterHorizontally) { Controls(now, 88.dp) }
        }
    }
}

class Toggle : ActionCallback {
    override suspend fun onAction(context: Context, glanceId: GlanceId, parameters: ActionParameters) {
        val core = NeedleApp.instance.core
        // Nothing to go on with: shuffle the whole library.
        if (core.playback().current == null) core.play(core.songs().shuffled().take(200).map { it.id }, 0u) else core.toggle()
        updateWidgets(context)
    }
}

class Next : ActionCallback {
    override suspend fun onAction(context: Context, glanceId: GlanceId, parameters: ActionParameters) {
        NeedleApp.instance.core.next()
        updateWidgets(context)
    }
}

class Previous : ActionCallback {
    override suspend fun onAction(context: Context, glanceId: GlanceId, parameters: ActionParameters) {
        NeedleApp.instance.core.previous()
        updateWidgets(context)
    }
}

class NowPlayingReceiver : GlanceAppWidgetReceiver() {
    override val glanceAppWidget: GlanceAppWidget = NowPlayingWidget()
}

// ---------- Recently added: tap an album to play it

private val AlbumKey = ActionParameters.Key<String>("album")

class RecentWidget : GlanceAppWidget() {
    override val sizeMode = SizeMode.Exact

    override suspend fun provideGlance(context: Context, id: GlanceId) {
        val albums = runCatching { NeedleApp.instance.core.home().added.take(4) }.getOrDefault(emptyList())
        val arts = albums.map { cover(it.artwork, 192) }
        val back = wash(arts.firstOrNull { it != null }, 0.6f)
        provideContent {
            GlanceTheme {
                val size = LocalSize.current
                // As many covers as fit, each with its name under it.
                val count = ((size.width - 28.dp) / 84.dp).toInt().coerceIn(1, 4)
                val side = ((size.width - 28.dp - 10.dp * (count - 1)) / count).coerceAtMost(size.height - 76.dp).coerceAtLeast(48.dp)
                Box(GlanceModifier.fillMaxSize().cornerRadius(28.dp).background(ColorProvider(Color(0xFF1B1918)))) {
                    back?.let { Image(ImageProvider(it), contentDescription = null, contentScale = ContentScale.FillBounds, modifier = GlanceModifier.fillMaxSize()) }
                    Column(GlanceModifier.fillMaxSize().padding(14.dp)) {
                        Row(GlanceModifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
                            Text("Recently added", style = TextStyle(color = White, fontSize = 16.sp, fontWeight = FontWeight.Bold), modifier = GlanceModifier.defaultWeight())
                            Image(ImageProvider(R.drawable.needle_logo), contentDescription = null, modifier = GlanceModifier.size(18.dp).cornerRadius(5.dp))
                        }
                        Spacer(GlanceModifier.height(10.dp))
                        if (albums.isEmpty()) Text("Your new albums show here.", style = TextStyle(color = Soft, fontSize = 13.sp))
                        Row(GlanceModifier.fillMaxWidth()) {
                            albums.take(count).forEachIndexed { i, album ->
                                if (i > 0) Spacer(GlanceModifier.width(10.dp))
                                Column(GlanceModifier.width(side).clickable(actionRunCallback<PlayAlbum>(actionParametersOf(AlbumKey to album.key)))) {
                                    Box(GlanceModifier.size(side).cornerRadius(16.dp).background(Glass), contentAlignment = Alignment.Center) {
                                        val art = arts[i]
                                        if (art != null) Image(ImageProvider(art), contentDescription = album.title, contentScale = ContentScale.Crop, modifier = GlanceModifier.size(side).cornerRadius(16.dp))
                                        else Image(ImageProvider(R.drawable.needle_logo), contentDescription = null, modifier = GlanceModifier.size(side * 0.4f))
                                    }
                                    Spacer(GlanceModifier.height(6.dp))
                                    Text(album.title.ifBlank { "Unknown album" }, maxLines = 1, style = TextStyle(color = White, fontSize = 12.sp, fontWeight = FontWeight.Medium))
                                    Text(album.artist, maxLines = 1, style = TextStyle(color = Faint, fontSize = 11.sp))
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

class PlayAlbum : ActionCallback {
    override suspend fun onAction(context: Context, glanceId: GlanceId, parameters: ActionParameters) {
        val key = parameters[AlbumKey] ?: return
        val core = NeedleApp.instance.core
        core.play(core.albumSongs(key).map { it.id }, 0u)
        updateWidgets(context)
    }
}

class RecentReceiver : GlanceAppWidgetReceiver() {
    override val glanceAppWidget: GlanceAppWidget = RecentWidget()
}
