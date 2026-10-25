package fyi.nnx.needle.wear

import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.basicMarquee
import androidx.compose.foundation.focusable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Favorite
import androidx.compose.material.icons.rounded.FavoriteBorder
import androidx.compose.material.icons.rounded.MusicNote
import androidx.compose.material.icons.rounded.Pause
import androidx.compose.material.icons.rounded.PlayArrow
import androidx.compose.material.icons.rounded.SkipNext
import androidx.compose.material.icons.rounded.SkipPrevious
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.input.rotary.onRotaryScrollEvent
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.wear.compose.material3.FilledIconButton
import androidx.wear.compose.material3.Icon
import androidx.wear.compose.material3.IconButton
import androidx.wear.compose.material3.IconButtonDefaults
import androidx.wear.compose.material3.MaterialTheme
import androidx.wear.compose.material3.Text
import com.google.android.gms.wearable.DataClient
import com.google.android.gms.wearable.DataEvent
import com.google.android.gms.wearable.DataMapItem
import com.google.android.gms.wearable.Wearable
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.tasks.await
import kotlinx.coroutines.withContext

/** Where the phone puts what plays, and where the watch's presses go (as the phone's WearLink). */
const val NOW = "/needle/now"
const val DO = "/needle/do"

/** Needle's amber and the grey of second lines, as on the phone. */
private val Amber = Color(0xFFE2B46C)
private val Grey = Color(0xFFA8A29C)

/** What plays on the phone, as it last said. */
data class Now(
    val title: String,
    val artist: String,
    val playing: Boolean,
    val favorite: Boolean,
    val position: Double,
    val duration: Double,
    /** When the phone said it (ms), so the time goes on between its words. */
    val at: Long,
    val cover: Bitmap?,
)

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        // A debug build opened with `--ez demo true` shows a made-up song, for a look without a
        // paired phone.
        val debuggable = applicationInfo.flags and android.content.pm.ApplicationInfo.FLAG_DEBUGGABLE != 0
        val demo = debuggable && intent.getBooleanExtra("demo", false)
        setContent { MaterialTheme { NeedleWatch(demo) } }
    }
}

/** Reads what plays from a data item the phone put. */
private suspend fun read(client: DataClient, item: com.google.android.gms.wearable.DataItem): Now? {
    if (item.uri.path != NOW) return null
    val map = DataMapItem.fromDataItem(item).dataMap
    if (!map.containsKey("title")) return null
    val cover = map.getAsset("cover")?.let { asset ->
        runCatching {
            val fd = client.getFdForAsset(asset).await()
            withContext(Dispatchers.IO) { fd.inputStream.use { BitmapFactory.decodeStream(it) } }
        }.getOrNull()
    }
    return Now(
        map.getString("title", ""),
        map.getString("artist", ""),
        map.getBoolean("playing"),
        map.getBoolean("favorite"),
        map.getDouble("position"),
        map.getDouble("duration"),
        map.getLong("at"),
        cover,
    )
}

/**
 * Needle on the watch: the song playing on the phone with its cover, Previous, a round amber
 * Play/Pause, Next, and the heart; the time played as an amber ring round the edge; the crown
 * changes the phone's volume.
 */
@Composable
fun NeedleWatch(demo: Boolean = false) {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    var now by remember {
        mutableStateOf(if (demo) Now("Night Bus", "The Static Hours", true, true, 64.0, 162.0, System.currentTimeMillis(), null) else null)
    }
    // Null while looking for the phone; false when it is not there.
    var phone by remember { mutableStateOf<Boolean?>(null) }
    val data = remember { Wearable.getDataClient(context) }
    DisposableEffect(data) {
        val listener = DataClient.OnDataChangedListener { events ->
            events.filter { it.type == DataEvent.TYPE_CHANGED }.map { it.dataItem.freeze() }.forEach { item ->
                scope.launch { read(data, item)?.let { now = it } }
            }
            events.release()
        }
        data.addListener(listener)
        onDispose { data.removeListener(listener) }
    }
    LaunchedEffect(Unit) {
        runCatching {
            val items = data.dataItems.await()
            val found = items.firstOrNull { it.uri.path == NOW }?.freeze()
            items.release()
            found?.let { read(data, it) }?.let { now = it }
        }
        while (true) {
            phone = runCatching { Wearable.getNodeClient(context).connectedNodes.await().isNotEmpty() }.getOrDefault(false)
            delay(10_000)
        }
    }
    // Presses go to every phone the watch is connected to (there is one).
    val send: (String) -> Unit = { action ->
        scope.launch {
            runCatching {
                val messages = Wearable.getMessageClient(context)
                Wearable.getNodeClient(context).connectedNodes.await().forEach { node ->
                    messages.sendMessage(node.id, DO, action.toByteArray()).await()
                }
            }
        }
    }
    // The time played moves on between the phone's words.
    var tick by remember { mutableLongStateOf(System.currentTimeMillis()) }
    LaunchedEffect(now?.playing) {
        while (now?.playing == true) {
            tick = System.currentTimeMillis()
            delay(1000)
        }
    }
    val focus = remember { FocusRequester() }
    LaunchedEffect(Unit) { runCatching { focus.requestFocus() } }
    val n = now
    Box(
        Modifier
            .fillMaxSize()
            .background(Color.Black)
            // The crown: the phone's volume, a step at a time.
            .onRotaryScrollEvent { e ->
                send(if (e.verticalScrollPixels > 0) "louder" else "quieter")
                true
            }
            .focusRequester(focus)
            .focusable(),
        contentAlignment = Alignment.Center,
    ) {
        if (n != null && n.duration > 0) {
            val played = (n.position + if (n.playing) (tick - n.at).coerceAtLeast(0) / 1000.0 else 0.0) / n.duration
            Canvas(Modifier.fillMaxSize().padding(3.dp)) {
                val stroke = 4.dp.toPx()
                val box = Size(size.width - stroke, size.height - stroke)
                val corner = Offset(stroke / 2, stroke / 2)
                drawArc(Color.White.copy(alpha = 0.12f), -90f, 360f, false, corner, box, style = Stroke(stroke))
                drawArc(Amber, -90f, (played.coerceIn(0.0, 1.0) * 360).toFloat(), false, corner, box, style = Stroke(stroke, cap = StrokeCap.Round))
            }
        }
        when {
            n == null -> Message(if (phone == false) "Needle on your phone is not in reach" else "Nothing plays. Choose music in Needle on your phone.")
            else -> Playing(n, send)
        }
    }
}

@Composable
private fun Message(text: String) {
    Column(horizontalAlignment = Alignment.CenterHorizontally, modifier = Modifier.padding(24.dp)) {
        Icon(Icons.Rounded.MusicNote, contentDescription = null, tint = Amber, modifier = Modifier.size(28.dp))
        Text(text, color = Grey, textAlign = TextAlign.Center, fontSize = 13.sp, modifier = Modifier.padding(top = 8.dp))
    }
}

@Composable
private fun Playing(n: Now, send: (String) -> Unit) {
    Column(
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.Center,
        modifier = Modifier.fillMaxSize().padding(horizontal = 22.dp, vertical = 18.dp),
    ) {
        // The cover, a soft square; a note where there is none.
        Box(Modifier.size(44.dp).clip(RoundedCornerShape(8.dp)).background(Color(0xFF2B2927)), contentAlignment = Alignment.Center) {
            if (n.cover != null) {
                Image(n.cover.asImageBitmap(), contentDescription = null, contentScale = ContentScale.Crop, modifier = Modifier.fillMaxSize())
            } else {
                Icon(Icons.Rounded.MusicNote, contentDescription = null, tint = Grey, modifier = Modifier.size(22.dp))
            }
        }
        Text(
            n.title,
            color = Color.White,
            fontSize = 15.sp,
            fontWeight = FontWeight.Bold,
            maxLines = 1,
            modifier = Modifier.padding(top = 6.dp).basicMarquee(iterations = Int.MAX_VALUE),
        )
        Text(n.artist, color = Grey, fontSize = 12.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
        Row(
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(6.dp),
            modifier = Modifier.padding(top = 6.dp),
        ) {
            IconButton(onClick = { send("previous") }) {
                Icon(Icons.Rounded.SkipPrevious, contentDescription = "Previous", tint = Color.White)
            }
            FilledIconButton(
                onClick = { send("toggle") },
                shapes = IconButtonDefaults.shapes(shape = CircleShape),
                colors = IconButtonDefaults.filledIconButtonColors(containerColor = Amber, contentColor = Color(0xFF231A0C)),
                modifier = Modifier.size(52.dp),
            ) {
                Icon(if (n.playing) Icons.Rounded.Pause else Icons.Rounded.PlayArrow, contentDescription = if (n.playing) "Pause" else "Play", modifier = Modifier.size(28.dp))
            }
            IconButton(onClick = { send("next") }) {
                Icon(Icons.Rounded.SkipNext, contentDescription = "Next", tint = Color.White)
            }
        }
        IconButton(onClick = { send("favorite") }, modifier = Modifier.size(36.dp)) {
            Icon(
                if (n.favorite) Icons.Rounded.Favorite else Icons.Rounded.FavoriteBorder,
                contentDescription = if (n.favorite) "Take out of favorites" else "Add to favorites",
                tint = if (n.favorite) Amber else Grey,
                modifier = Modifier.size(20.dp),
            )
        }
    }
}
