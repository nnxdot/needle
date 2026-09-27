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
        BitmapFactory.decodeFile(it, BitmapFactory.Options().apply { inSampleSize = step })
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

class NowPlayingWidget : GlanceAppWidget() {
    override val sizeMode = SizeMode.Exact

    override suspend fun provideGlance(context: Context, id: GlanceId) {
        val core = NeedleApp.instance.core
        val playback = runCatching { core.playback() }.getOrNull()
        val song = playback?.current
        val art = cover(song?.artwork)
        provideContent { GlanceTheme { NowPlaying(song, playback?.playing == true, art) } }
    }
}

@Composable
private fun NowPlaying(song: Song?, playing: Boolean, art: Bitmap?) {
    Row(
        GlanceModifier.fillMaxSize().background(Back).cornerRadius(24.dp).padding(12.dp)
            .clickable(actionStartActivity<MainActivity>()),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Box(GlanceModifier.size(72.dp).cornerRadius(12.dp).background(ColorProvider(Color(0xFF2B2927))), contentAlignment = Alignment.Center) {
            if (art != null) Image(ImageProvider(art), contentDescription = null, modifier = GlanceModifier.size(72.dp).cornerRadius(12.dp))
            else Text("♪", style = TextStyle(color = Amber, fontSize = 28.sp))
        }
        Spacer(GlanceModifier.width(12.dp))
        Column(GlanceModifier.defaultWeight()) {
            Text(song?.title ?: "Needle", maxLines = 1, style = TextStyle(color = Ink, fontSize = 15.sp, fontWeight = FontWeight.Bold))
            Text(song?.artist ?: "Nothing playing", maxLines = 1, style = TextStyle(color = Faint, fontSize = 13.sp))
            Spacer(GlanceModifier.size(6.dp))
            Row(verticalAlignment = Alignment.CenterVertically) {
                Button(R.drawable.ic_widget_previous, "Previous", actionRunCallback<Previous>())
                Spacer(GlanceModifier.width(8.dp))
                Button(if (playing) R.drawable.ic_widget_pause else R.drawable.ic_widget_play, if (playing) "Pause" else "Play", actionRunCallback<Toggle>())
                Spacer(GlanceModifier.width(8.dp))
                Button(R.drawable.ic_widget_next, "Next", actionRunCallback<Next>())
            }
        }
    }
}

@Composable
private fun Button(icon: Int, label: String, action: androidx.glance.action.Action) {
    Box(
        GlanceModifier.size(40.dp).cornerRadius(20.dp).background(ColorProvider(Color(0xFF353230))).clickable(action),
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
                Column(GlanceModifier.fillMaxSize().background(Back).cornerRadius(24.dp).padding(12.dp)) {
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
                                if (art != null) Image(ImageProvider(art), contentDescription = album.title, modifier = GlanceModifier.size(72.dp).cornerRadius(10.dp))
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
