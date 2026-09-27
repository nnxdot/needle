package fyi.nnx.needle

import android.content.Context
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.glance.GlanceId
import androidx.glance.GlanceModifier
import androidx.glance.GlanceTheme
import androidx.glance.Image
import androidx.glance.ImageProvider
import androidx.glance.action.ActionParameters
import androidx.glance.action.actionParametersOf
import androidx.glance.action.actionStartActivity
import androidx.glance.action.clickable
import androidx.glance.appwidget.GlanceAppWidget
import androidx.glance.appwidget.GlanceAppWidgetReceiver
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
import androidx.glance.layout.Row
import androidx.glance.layout.Spacer
import androidx.glance.layout.fillMaxSize
import androidx.glance.layout.fillMaxWidth
import androidx.glance.layout.padding
import androidx.glance.layout.size
import androidx.glance.layout.width
import androidx.glance.text.FontWeight
import androidx.glance.text.Text
import androidx.glance.text.TextStyle
import androidx.glance.unit.ColorProvider
import fyi.nnx.needle.core.Song

/** A cover, small, for a widget (widgets hold pictures in memory, so it stays small). */
private fun cover(path: String?, size: Int = 256): Bitmap? = path?.let {
    runCatching {
        val bounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }
        BitmapFactory.decodeFile(it, bounds)
        val step = maxOf(1, minOf(bounds.outWidth, bounds.outHeight) / size)
        BitmapFactory.decodeFile(it, BitmapFactory.Options().apply { inSampleSize = step })?.let { b -> fyi.nnx.needle.ui.trimBars(b) }
    }.getOrNull()
}

private val Ink = ColorProvider(Color(0xFFF1EFED))
private val Faint = ColorProvider(Color(0xFFA8A29C))
private val Amber = ColorProvider(Color(0xFFE2B46C))
private val Back = ColorProvider(Color(0xFF1B1918))

/** Refreshes the widgets, when what plays changes. */
suspend fun updateWidgets(context: Context) {
    runCatching { NowPlayingWidget().updateAll(context) }
    runCatching { RecentWidget().updateAll(context) }
}

// ---------- Now playing

/** The cover's darkest strong colour, for the widget's background; Needle's charcoal without one. */
private fun tint(art: Bitmap?): Color {
    val swatch = art?.let { androidx.palette.graphics.Palette.from(it).generate() }
    val rgb = swatch?.darkVibrantSwatch?.rgb ?: swatch?.darkMutedSwatch?.rgb ?: return Color(0xFF1B1918)
    // Darkened, so white text always reads on it.
    val c = Color(rgb)
    return Color(c.red * 0.55f, c.green * 0.55f, c.blue * 0.55f)
}

class NowPlayingWidget : GlanceAppWidget() {
    override val sizeMode = SizeMode.Exact

    override suspend fun provideGlance(context: Context, id: GlanceId) {
        val core = NeedleApp.instance.core
        val playback = runCatching { core.playback() }.getOrNull()
        val song = playback?.current
        val art = cover(song?.artwork, 320)
        val back = tint(art)
        provideContent {
            GlanceTheme {
                if (androidx.glance.LocalSize.current.width < 200.dp) Square(song, playback?.playing == true, art, back)
                else NowPlaying(song, playback?.playing == true, art, back)
            }
        }
    }
}

/** Small: the cover fills it, with play or pause in a corner. */
@Composable
private fun Square(song: Song?, playing: Boolean, art: Bitmap?, back: Color) {
    Box(
        GlanceModifier.fillMaxSize().background(ColorProvider(back)).cornerRadius(28.dp).clickable(actionStartActivity<MainActivity>()),
        contentAlignment = Alignment.BottomEnd,
    ) {
        if (art != null) Image(ImageProvider(art), contentDescription = song?.title, contentScale = androidx.glance.layout.ContentScale.Crop, modifier = GlanceModifier.fillMaxSize().cornerRadius(28.dp))
        else Box(GlanceModifier.fillMaxSize(), contentAlignment = Alignment.Center) { Image(ImageProvider(R.drawable.ic_widget_play), contentDescription = null, modifier = GlanceModifier.size(36.dp)) }
        Box(GlanceModifier.padding(10.dp)) { PlayButton(playing, 48) }
    }
}

/** Wide: the cover at the left, the song, and the buttons, on the cover's own colour. */
@Composable
private fun NowPlaying(song: Song?, playing: Boolean, art: Bitmap?, back: Color) {
    // The cover fills the widget's height (and at most 45% of its width).
    val size = androidx.glance.LocalSize.current
    val side = minOf(size.height - 24.dp, size.width * 0.45f).coerceAtLeast(72.dp)
    Row(
        GlanceModifier.fillMaxSize().background(ColorProvider(back)).cornerRadius(28.dp).padding(12.dp)
            .clickable(actionStartActivity<MainActivity>()),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Box(GlanceModifier.size(side).cornerRadius(20.dp).background(ColorProvider(Color(0x33FFFFFF))), contentAlignment = Alignment.Center) {
            if (art != null) Image(ImageProvider(art), contentDescription = null, contentScale = androidx.glance.layout.ContentScale.Crop, modifier = GlanceModifier.size(side).cornerRadius(20.dp))
            else Image(ImageProvider(R.drawable.ic_widget_play), contentDescription = null, modifier = GlanceModifier.size(32.dp))
        }
        Spacer(GlanceModifier.width(14.dp))
        Column(GlanceModifier.defaultWeight()) {
            Text(song?.title ?: "Needle", maxLines = 2, style = TextStyle(color = ColorProvider(Color.White), fontSize = 18.sp, fontWeight = FontWeight.Bold))
            Text(song?.artist ?: "Tap play for a shuffle of your music", maxLines = 1, style = TextStyle(color = ColorProvider(Color(0xCCFFFFFF)), fontSize = 14.sp))
            Spacer(GlanceModifier.size(14.dp))
            Row(verticalAlignment = Alignment.CenterVertically) {
                Button(R.drawable.ic_widget_previous, "Previous", actionRunCallback<Previous>())
                Spacer(GlanceModifier.width(10.dp))
                PlayButton(playing, 52)
                Spacer(GlanceModifier.width(10.dp))
                Button(R.drawable.ic_widget_next, "Next", actionRunCallback<Next>())
            }
        }
    }
}

/** Play or pause, in Needle's amber, round. */
@Composable
private fun PlayButton(playing: Boolean, size: Int) {
    Box(
        GlanceModifier.size(size.dp).cornerRadius((size / 2).dp).background(Amber).clickable(actionRunCallback<Toggle>()),
        contentAlignment = Alignment.Center,
    ) {
        Image(
            ImageProvider(if (playing) R.drawable.ic_widget_pause_dark else R.drawable.ic_widget_play_dark),
            contentDescription = if (playing) "Pause" else "Play",
            modifier = GlanceModifier.size((size * 0.5).dp),
        )
    }
}

@Composable
private fun Button(icon: Int, label: String, action: androidx.glance.action.Action) {
    Box(
        GlanceModifier.size(40.dp).cornerRadius(20.dp).background(ColorProvider(Color(0x26FFFFFF))).clickable(action),
        contentAlignment = Alignment.Center,
    ) { Image(ImageProvider(icon), contentDescription = label, modifier = GlanceModifier.size(22.dp)) }
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
        provideContent {
            GlanceTheme {
                Column(GlanceModifier.fillMaxSize().background(Back).cornerRadius(28.dp).padding(14.dp)) {
                    Text("Recently added", style = TextStyle(color = Ink, fontSize = 14.sp, fontWeight = FontWeight.Bold))
                    Spacer(GlanceModifier.size(8.dp))
                    if (albums.isEmpty()) {
                        Text("Your new albums show here.", style = TextStyle(color = Faint, fontSize = 13.sp))
                    }
                    Row(GlanceModifier.fillMaxWidth()) {
                        albums.forEachIndexed { i, album ->
                            if (i > 0) Spacer(GlanceModifier.width(8.dp))
                            Box(
                                GlanceModifier.defaultWeight().cornerRadius(10.dp).background(ColorProvider(Color(0xFF2B2927)))
                                    .clickable(actionRunCallback<PlayAlbum>(actionParametersOf(AlbumKey to album.key))),
                                contentAlignment = Alignment.Center,
                            ) {
                                val art = arts[i]
                                if (art != null) Image(ImageProvider(art), contentDescription = album.title, contentScale = androidx.glance.layout.ContentScale.Crop, modifier = GlanceModifier.size(76.dp).cornerRadius(14.dp))
                                else Text(album.title.take(12), maxLines = 2, style = TextStyle(color = Faint, fontSize = 11.sp), modifier = GlanceModifier.size(72.dp).padding(6.dp))
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
