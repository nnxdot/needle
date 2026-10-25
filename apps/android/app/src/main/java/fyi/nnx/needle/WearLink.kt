package fyi.nnx.needle

import android.content.Context
import android.media.AudioManager
import com.google.android.gms.wearable.Asset
import com.google.android.gms.wearable.MessageEvent
import com.google.android.gms.wearable.PutDataMapRequest
import com.google.android.gms.wearable.Wearable
import com.google.android.gms.wearable.WearableListenerService
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.launch
import kotlinx.coroutines.tasks.await
import kotlinx.coroutines.withContext

/**
 * Needle on a Wear OS watch (the `wear` app): the phone tells the watch what plays (a data item
 * at [NOW], with a small cover), and does what the watch asks (messages at [DO]). Phones without
 * Google's Wear OS services skip it quietly.
 */
object WearLink {
    const val NOW = "/needle/now"
    const val DO = "/needle/do"

    /** No Wear OS services on this phone: stop trying. */
    @Volatile private var absent = false

    /** Follows what plays and tells a watch, when the song, playing, or favorite changes. */
    fun start(context: Context) {
        val app = context.applicationContext
        val data = Wearable.getDataClient(app)
        NeedleApp.instance.scope.launch {
            NeedleApp.instance.playback
                .map { p -> p?.current?.let { s -> Triple(s, p.playing, p.position) } }
                // Each change of song, of playing, or a jump in the time (a seek), not every tick.
                .distinctUntilChanged { a, b ->
                    a?.first?.id == b?.first?.id && a?.second == b?.second && a?.first?.rating == b?.first?.rating &&
                        (a == null || b == null || kotlin.math.abs(b.third - a.third) < 3.0)
                }
                .collect { now ->
                    if (absent || now == null) return@collect
                    val (song, playing, position) = now
                    val cover = withContext(Dispatchers.IO) { song.artwork?.takeIf { !it.startsWith("http") }?.let(::smallCover) }
                    val request = PutDataMapRequest.create(NOW).apply {
                        dataMap.putString("title", song.title)
                        dataMap.putString("artist", song.artist)
                        dataMap.putBoolean("playing", playing)
                        dataMap.putBoolean("favorite", song.rating.toInt() >= 4)
                        dataMap.putDouble("position", position)
                        dataMap.putDouble("duration", song.duration)
                        dataMap.putLong("at", System.currentTimeMillis())
                        cover?.let { dataMap.putAsset("cover", Asset.createFromBytes(it)) }
                    }.asPutDataRequest().setUrgent()
                    runCatching { data.putDataItem(request).await() }.onFailure { e ->
                        if ((e as? com.google.android.gms.common.api.ApiException)?.statusCode == com.google.android.gms.common.api.CommonStatusCodes.API_NOT_CONNECTED) absent = true
                    }
                }
        }
    }

    /** A cover about 200 pixels across, as a small JPEG for a watch's screen. */
    private fun smallCover(path: String): ByteArray? = runCatching {
        val bounds = android.graphics.BitmapFactory.Options().apply { inJustDecodeBounds = true }
        android.graphics.BitmapFactory.decodeFile(path, bounds)
        var sample = 1
        while (minOf(bounds.outWidth, bounds.outHeight) / (sample * 2) >= 200) sample *= 2
        val bitmap = android.graphics.BitmapFactory.decodeFile(path, android.graphics.BitmapFactory.Options().apply { inSampleSize = sample })
            ?: return@runCatching null
        java.io.ByteArrayOutputStream().also { bitmap.compress(android.graphics.Bitmap.CompressFormat.JPEG, 85, it) }.toByteArray()
    }.getOrNull()

    /** Does what the watch asks: play or pause, the next or last song, the heart, the volume. */
    fun act(context: Context, action: String) {
        val playback = NeedleApp.instance.playback.value
        when (action) {
            "toggle" -> fyi.nnx.needle.ui.Controls.toggle()
            "next" -> fyi.nnx.needle.ui.Controls.next()
            "previous" -> fyi.nnx.needle.ui.Controls.previous()
            "favorite" -> playback?.current?.let { song ->
                NeedleApp.instance.scope.launch(Dispatchers.IO) {
                    runCatching { NeedleApp.instance.core.toggleFavorite(song.id) }
                    NeedleApp.instance.libraryChanged()
                    NeedleApp.instance.wake()
                }
            }
            "louder", "quieter" -> context.getSystemService(AudioManager::class.java)?.adjustStreamVolume(
                AudioManager.STREAM_MUSIC,
                if (action == "louder") AudioManager.ADJUST_RAISE else AudioManager.ADJUST_LOWER,
                0,
            )
        }
    }
}

/** Takes the watch's presses (messages at [WearLink.DO]), even while Needle is in the background. */
class NeedleWearService : WearableListenerService() {
    override fun onMessageReceived(event: MessageEvent) {
        if (event.path != WearLink.DO) return
        val action = String(event.data)
        android.os.Handler(android.os.Looper.getMainLooper()).post { WearLink.act(this, action) }
    }
}
