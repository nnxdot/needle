package fyi.nnx.needle.ui

import android.content.Context
import android.graphics.Bitmap
import android.graphics.Canvas
import android.graphics.LinearGradient
import android.graphics.Paint
import android.graphics.Shader
import android.text.TextPaint
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.FilterChip
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import fyi.nnx.needle.NeedleApp
import fyi.nnx.needle.core.YearInReview
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.io.File

private val Ink = Color.White
private val Faint = Color.White.copy(alpha = 0.7f)
private val Amber = Color(0xFFE2B46C)

/**
 * The way into the year in review: a warm card with the year ("Your 2026 in music"), on History
 * and, in December, on Home. Nothing while there is no listening.
 */
@Composable
fun YearReviewRow(open: (Route) -> Unit, december: Boolean = false) {
    val yearLoad = rememberLoaded(december) { if (december) homeReviewYear() else listeningYears().firstOrNull()?.let { reviewYear() } }
    val year by yearLoad
    val y = year ?: return
    Row(
        Modifier
            .fillMaxWidth()
            .padding(horizontal = 16.dp, vertical = 8.dp)
            .clip(RoundedCornerShape(16.dp))
            .background(Brush.horizontalGradient(listOf(Color(0xFF8A5422), Color(0xFF3A2A40))))
            .clickable { open(Route.YearReview(y)) }
            .padding(horizontal = 20.dp, vertical = 16.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(Modifier.weight(1f)) {
            Text("YOUR $y IN MUSIC", color = Amber, style = MaterialTheme.typography.labelMedium, fontFamily = FontFamily.Monospace, letterSpacing = 2.sp)
            Text("Your year in review", color = Ink, style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.Bold)
        }
        Text("›", color = Ink, style = MaterialTheme.typography.headlineSmall)
    }
}

/** "1,284" in the phone's own way of grouping digits. */
private fun grouped(n: Long) = java.text.NumberFormat.getIntegerInstance().format(n)

/**
 * Year in review (NEEDLE-2.0.md): "Your 2026 in music": the hours, the plays, the songs found
 * that year, plays by month, the top artist (and the months in the top five), and the top songs
 * (up to 50). Save as a picture to share, Make a playlist ("Top songs of 2026"), Play top 50.
 * Opens from History and, in December, from Home.
 */
@Composable
fun YearReviewScreen(year: Int?, open: (Route) -> Unit) {
    val context = LocalContext.current
    val yearsLoad = rememberLoaded { listeningYears() }
    val years by yearsLoad
    val firstLoad = rememberLoaded { reviewYear() }
    val first by firstLoad
    var chosen by rememberSaveable { mutableStateOf(year) }
    val shown = chosen ?: first
    val reviewLoad = rememberLoaded(shown) { shown?.let { yearInReview(it) } }
    val review by reviewLoad
    var all by rememberSaveable { mutableStateOf(false) }
    val r = review
    Box(
        Modifier
            .fillMaxSize()
            .background(Brush.verticalGradient(listOf(Color(0xFF8A5422), Color(0xFF3A2A40), Color(0xFF1A1420)))),
    ) {
        LazyColumn(
            Modifier.fillMaxSize(),
            contentPadding = PaddingValues(start = 28.dp, end = 28.dp, bottom = 32.dp + LocalBottomInset.current),
        ) {
            item {
                Column(Modifier.statusBarsPadding()) {
                    if (LocalCanGoBack.current) BackButton(Modifier.padding(top = 4.dp).offset(x = (-20).dp))
                }
            }
            if ((years?.size ?: 0) > 1) item {
                LazyRow(horizontalArrangement = Arrangement.spacedBy(8.dp), modifier = Modifier.padding(bottom = 12.dp)) {
                    items(years.orEmpty()) { y -> FilterChip(selected = y == shown, onClick = { chosen = y }, label = { Text(y.toString()) }, shape = RoundedCornerShape(50)) }
                }
            }
            if (years != null && years!!.isEmpty()) {
                item { Text("Play some music, and your year in music shows here.", color = Faint, style = MaterialTheme.typography.bodyLarge, modifier = Modifier.padding(top = 24.dp)) }
                return@LazyColumn
            }
            if (r == null) {
                item { androidx.compose.material3.CircularProgressIndicator(color = Amber, modifier = Modifier.padding(top = 48.dp)) }
                return@LazyColumn
            }
            item { Hours(r) }
            r.topArtist?.let { a -> item { TopArtist(a) } }
            item { Months(r) }
            if (r.topSongs.isNotEmpty()) {
                item { Text("Top songs", style = MaterialTheme.typography.titleMedium, color = Faint, modifier = Modifier.padding(top = 28.dp, bottom = 4.dp)) }
                val shownSongs = if (all) r.topSongs else r.topSongs.take(10)
                itemsIndexed(shownSongs, key = { _, s -> s.song.id }) { i, top ->
                    Row(
                        Modifier
                            .fillMaxWidth()
                            .clip(RoundedCornerShape(12.dp))
                            .clickable { playSongs(r.topSongs.map { it.song.id }, i) }
                            .padding(vertical = 8.dp),
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(14.dp),
                    ) {
                        Text("${i + 1}", color = Faint, style = MaterialTheme.typography.bodyMedium, modifier = Modifier.width(22.dp))
                        Cover(top.song.artwork, Modifier.size(44.dp), SmallCoverShape, seed = top.song.album + top.song.artist)
                        Column(Modifier.weight(1f)) {
                            Text(top.song.title, color = Ink, style = MaterialTheme.typography.bodyLarge, fontWeight = FontWeight.SemiBold, maxLines = 1, overflow = TextOverflow.Ellipsis)
                            Text(top.song.artist, color = Faint, style = MaterialTheme.typography.bodyMedium, maxLines = 1, overflow = TextOverflow.Ellipsis)
                        }
                        Text(grouped(top.plays.toLong()), color = Faint, style = MaterialTheme.typography.bodyMedium)
                    }
                }
                if (r.topSongs.size > 10) item {
                    TextButton(onClick = { all = !all }) { Text(if (all) "Show the top 10" else "Show all ${r.topSongs.size}", color = Amber) }
                }
            }
            item { Actions(r, context) }
        }
    }
}

@Composable
private fun Hours(r: YearInReview) {
    val hours = (r.seconds / 3600).toLong()
    val days = (r.seconds / 86400).toLong()
    // Under an hour: minutes, not "0 hours".
    val minutes = hours < 1
    val shown = if (minutes) (r.seconds / 60).toLong() else hours
    Column(Modifier.padding(top = 8.dp)) {
        Text("YOUR ${r.year} IN MUSIC", color = Amber, style = MaterialTheme.typography.labelLarge, fontFamily = FontFamily.Monospace, letterSpacing = 2.sp)
        Text(
            grouped(shown),
            color = Ink,
            fontSize = 84.sp,
            lineHeight = 88.sp,
            fontFamily = titleFamily(1f),
            fontWeight = FontWeight.Bold,
            modifier = Modifier.padding(top = 12.dp),
        )
        Text(
            when {
                minutes -> if (shown == 1L) "minute of music" else "minutes of music"
                hours == 1L -> "hour of music"
                else -> "hours of music"
            },
            color = Ink, style = MaterialTheme.typography.titleLarge, fontWeight = FontWeight.Bold)
        val parts = buildList {
            if (days >= 1) add("That is ${count(days.toInt(), "whole day")}.")
            add("${grouped(r.plays.toLong())} plays.")
            if (r.newSongs > 0u) add("${count(r.newSongs.toInt(), "new song")} found.")
        }
        Text(parts.joinToString(" "), color = Faint, style = MaterialTheme.typography.bodyLarge, modifier = Modifier.padding(top = 6.dp))
    }
}

@Composable
private fun TopArtist(a: fyi.nnx.needle.core.YearTopArtist) {
    val photo by rememberLoaded(a.name, cache = false) { artistPhoto(a.name) ?: artistAlbums(a.name).firstNotNullOfOrNull { it.artwork } }
    Row(Modifier.fillMaxWidth().padding(top = 28.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(16.dp)) {
        Cover(photo, Modifier.size(84.dp), CircleShape, seed = a.name)
        Column(Modifier.weight(1f)) {
            Text("Top artist", color = Faint, style = MaterialTheme.typography.bodyMedium)
            Text(a.name, color = Ink, style = MaterialTheme.typography.headlineSmall, fontWeight = FontWeight.Bold, maxLines = 1, overflow = TextOverflow.Ellipsis)
            Text(
                "${if (a.seconds < 3600) count((a.seconds / 60).toInt(), "minute") else count((a.seconds / 3600).toInt(), "hour")} · ${count(a.plays.toInt(), "play")}" +
                    if (a.monthsInTopFive > 1u) " · in your top five for ${a.monthsInTopFive} months" else "",
                color = Faint,
                style = MaterialTheme.typography.bodyMedium,
            )
        }
    }
}

/** Plays by month: twelve bars, the busiest lit. */
@Composable
private fun Months(r: YearInReview) {
    val most = (r.months.maxOfOrNull { it.plays } ?: 0u).toFloat().coerceAtLeast(1f)
    val busiest = r.months.maxByOrNull { it.plays }?.month
    val names = java.text.DateFormatSymbols.getInstance().shortMonths
    Column(Modifier.padding(top = 28.dp)) {
        Text("Plays by month", color = Faint, style = MaterialTheme.typography.titleMedium)
        Row(
            Modifier
                .fillMaxWidth()
                .height(96.dp)
                .padding(top = 12.dp)
                .semantics { contentDescription = busiest?.let { "Most plays in ${names[it.toInt() - 1]}" } ?: "No plays" },
            horizontalArrangement = Arrangement.spacedBy(5.dp),
            verticalAlignment = Alignment.Bottom,
        ) {
            r.months.forEach { m ->
                Box(
                    Modifier
                        .weight(1f)
                        .fillMaxHeight((m.plays.toFloat() / most).coerceIn(0.04f, 1f))
                        .clip(RoundedCornerShape(4.dp))
                        .background(if (m.month == busiest && m.plays > 0u) Amber else Color.White.copy(alpha = 0.35f)),
                )
            }
        }
        Row(Modifier.fillMaxWidth().padding(top = 4.dp).clearAndSetSemantics {}, horizontalArrangement = Arrangement.spacedBy(5.dp)) {
            r.months.forEach { m ->
                Text(names[m.month.toInt() - 1].take(1), color = Faint, style = MaterialTheme.typography.labelSmall, modifier = Modifier.weight(1f), textAlign = androidx.compose.ui.text.style.TextAlign.Center)
            }
        }
    }
}

@Composable
private fun Actions(r: YearInReview, context: Context) {
    Column(Modifier.fillMaxWidth().padding(top = 28.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
        Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            Button(
                onClick = { shareYear(context, r) },
                colors = ButtonDefaults.buttonColors(containerColor = Amber, contentColor = Color(0xFF1B1816)),
                modifier = Modifier.weight(1f).height(52.dp),
            ) { Text("Share", fontWeight = FontWeight.SemiBold) }
            FilledTonalButton(
                enabled = r.topSongs.isNotEmpty(),
                onClick = { playSongs(r.topSongs.map { it.song.id }) },
                colors = ButtonDefaults.filledTonalButtonColors(containerColor = Color.White.copy(alpha = 0.14f), contentColor = Ink),
                modifier = Modifier.weight(1f).height(52.dp),
            ) { Text(if (r.topSongs.size >= 50) "Play top 50" else "Play top songs", fontWeight = FontWeight.SemiBold) }
        }
        TextButton(enabled = r.topSongs.isNotEmpty(), onClick = {
            libraryWrite({ core.saveYearPlaylist(r.year) }) { showMessage("Saved as “Top songs of ${r.year}”") }
        }) { Text("Make a playlist", color = Amber) }
    }
}

/** The year as a picture (1080 × 1350), to Android's share sheet, as lyric cards are. */
private fun shareYear(context: Context, r: YearInReview) {
    NeedleApp.instance.scope.launch(Dispatchers.Main) {
        val file = runCatching { withContext(Dispatchers.Default) { yearCard(context, r) } }
            .getOrElse { showMessage("Could not make the picture"); return@launch }
        val uri = androidx.core.content.FileProvider.getUriForFile(context, "${context.packageName}.files", file)
        val send = android.content.Intent(android.content.Intent.ACTION_SEND)
            .setType("image/png")
            .putExtra(android.content.Intent.EXTRA_STREAM, uri)
            .putExtra(android.content.Intent.EXTRA_TEXT, "My ${r.year} in music")
            .addFlags(android.content.Intent.FLAG_GRANT_READ_URI_PERMISSION)
        tryOpen("app to share with") {
            context.startActivity(android.content.Intent.createChooser(send, "Share your year").addFlags(android.content.Intent.FLAG_ACTIVITY_NEW_TASK))
        }
    }
}

private fun yearCard(context: Context, r: YearInReview): File {
    val w = 1080
    val h = 1350
    val bitmap = Bitmap.createBitmap(w, h, Bitmap.Config.ARGB_8888)
    val canvas = Canvas(bitmap)
    val paint = Paint(Paint.ANTI_ALIAS_FLAG)
    paint.shader = LinearGradient(0f, 0f, 0f, h.toFloat(), intArrayOf(0xFF8A5422.toInt(), 0xFF3A2A40.toInt(), 0xFF1A1420.toInt()), null, Shader.TileMode.CLAMP)
    canvas.drawRect(0f, 0f, w.toFloat(), h.toFloat(), paint)
    val margin = 96f
    val bold = android.graphics.Typeface.create(android.graphics.Typeface.DEFAULT, android.graphics.Typeface.BOLD)
    fun text(size: Float, color: Int, typeface: android.graphics.Typeface? = null) = TextPaint(Paint.ANTI_ALIAS_FLAG).apply {
        textSize = size
        this.color = color
        if (typeface != null) this.typeface = typeface
    }
    val amber = 0xFFE2B46C.toInt()
    val white = 0xFFFFFFFF.toInt()
    val faint = 0xB3FFFFFF.toInt()
    var y = margin + 40f
    canvas.drawText("YOUR ${r.year} IN MUSIC", margin, y, text(38f, amber, android.graphics.Typeface.MONOSPACE))
    y += 210f
    canvas.drawText(grouped((r.seconds / 3600).toLong()), margin, y, text(200f, white, bold))
    y += 70f
    canvas.drawText("hours of music", margin, y, text(54f, white, bold))
    y += 60f
    canvas.drawText("${grouped(r.plays.toLong())} plays · ${grouped(r.newSongs.toLong())} new songs", margin, y, text(40f, faint))
    r.topArtist?.let { a ->
        y += 120f
        canvas.drawText("Top artist", margin, y, text(36f, faint))
        y += 64f
        canvas.drawText(android.text.TextUtils.ellipsize(a.name, text(60f, white, bold), w - margin * 2, android.text.TextUtils.TruncateAt.END).toString(), margin, y, text(60f, white, bold))
    }
    y += 110f
    canvas.drawText("Top songs", margin, y, text(36f, faint))
    r.topSongs.take(5).forEachIndexed { i, s ->
        y += 66f
        val line = "${i + 1}  ${s.song.title} · ${s.song.artist}"
        canvas.drawText(android.text.TextUtils.ellipsize(line, text(42f, white), w - margin * 2, android.text.TextUtils.TruncateAt.END).toString(), margin, y, text(42f, white))
    }
    canvas.drawText("Needle", margin, h - margin, text(40f, amber, bold))
    val dir = File(context.cacheDir, "cards").apply { mkdirs() }
    val file = File(dir, "year-${r.year}.png")
    file.outputStream().use { bitmap.compress(Bitmap.CompressFormat.PNG, 100, it) }
    bitmap.recycle()
    return file
}
