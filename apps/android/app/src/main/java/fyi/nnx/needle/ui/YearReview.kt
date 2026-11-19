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
import androidx.compose.material.icons.rounded.BarChart
import androidx.compose.material.icons.automirrored.rounded.KeyboardArrowRight
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
import androidx.compose.ui.graphics.toArgb
import androidx.compose.ui.semantics.heading
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

/** The share picture's colours: the theme's, read where the picture is asked for. */
private class CardLook(val page: Int, val ink: Int, val faint: Int, val accent: Int)

/**
 * The way into the year in review: a row in the page's own raised colour with the year ("Your
 * 2026 in music"), on History and, in December, on Home. It follows the theme as every other
 * row does. Nothing while there is no listening.
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
            .background(MaterialTheme.colorScheme.surfaceContainerHigh)
            .clickable { open(Route.YearReview(y)) }
            .padding(horizontal = 14.dp, vertical = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(14.dp),
    ) {
        Box(
            Modifier.size(44.dp).clip(RoundedCornerShape(12.dp)).background(MaterialTheme.colorScheme.primary),
            contentAlignment = Alignment.Center,
        ) {
            androidx.compose.material3.Icon(
                androidx.compose.material.icons.Icons.Rounded.BarChart,
                contentDescription = null,
                tint = MaterialTheme.colorScheme.onPrimary,
            )
        }
        Column(Modifier.weight(1f)) {
            Text("Your $y in music", style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.SemiBold, maxLines = 1, overflow = TextOverflow.Ellipsis)
            Text("Your year in review", style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant, maxLines = 1, overflow = TextOverflow.Ellipsis)
        }
        androidx.compose.material3.Icon(
            androidx.compose.material.icons.Icons.AutoMirrored.Rounded.KeyboardArrowRight,
            contentDescription = null,
            tint = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}

/** "1,284" in the phone's own way of grouping digits. */
private fun grouped(n: Long) = java.text.NumberFormat.getIntegerInstance().format(n)

/**
 * Year in review (NEEDLE-2.0.md): "Your 2026 in music": the hours, the plays, the songs found
 * that year, plays by month, the top artist (and the months in the top five), and the top songs
 * (up to 50). Save as a picture to share, Make a playlist ("Top songs of 2026"), Play top 50.
 * Opens from History and, in December, from Home. A page as the others are: the theme's own
 * background and colours, the accent for the one large number and the busiest month.
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
    val ink = MaterialTheme.colorScheme.onSurface
    val faint = MaterialTheme.colorScheme.onSurfaceVariant
    Box(Modifier.fillMaxSize()) {
        LazyColumn(
            Modifier.fillMaxSize(),
            contentPadding = PaddingValues(start = Edge, end = Edge, bottom = 32.dp + LocalBottomInset.current),
        ) {
            item {
                // The page's title, as LargeTitle writes one.
                val back = LocalCanGoBack.current
                Column(Modifier.statusBarsPadding()) {
                    if (back) BackButton(Modifier.padding(top = 4.dp).offset(x = (-14).dp))
                    Text(
                        shown?.let { "Your $it in music" } ?: "Your year in music",
                        style = MaterialTheme.typography.headlineMedium,
                        fontWeight = FontWeight.Bold,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                        modifier = Modifier.padding(top = if (back) 0.dp else 16.dp, bottom = 8.dp).semantics { heading() },
                    )
                }
            }
            if ((years?.size ?: 0) > 1) item {
                LazyRow(horizontalArrangement = Arrangement.spacedBy(8.dp), modifier = Modifier.padding(bottom = 12.dp)) {
                    items(years.orEmpty()) { y -> FilterChip(selected = y == shown, onClick = { chosen = y }, label = { Text(y.toString()) }, shape = RoundedCornerShape(50)) }
                }
            }
            if (years != null && years!!.isEmpty()) {
                item { Text("Play some music, and your year in music shows here.", color = faint, style = MaterialTheme.typography.bodyLarge, modifier = Modifier.padding(top = 24.dp)) }
                return@LazyColumn
            }
            if (r == null) {
                item { androidx.compose.material3.CircularProgressIndicator(Modifier.padding(top = 48.dp).size(24.dp), strokeWidth = 2.dp) }
                return@LazyColumn
            }
            item { Hours(r) }
            r.topArtist?.let { a -> item { TopArtist(a) } }
            item { Months(r) }
            if (r.topSongs.isNotEmpty()) {
                item { Section("Top songs") }
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
                        Text("${i + 1}", color = faint, style = MaterialTheme.typography.bodyMedium, modifier = Modifier.width(22.dp))
                        Cover(top.song.artwork, Modifier.size(44.dp), SmallCoverShape, seed = top.song.album + top.song.artist)
                        Column(Modifier.weight(1f)) {
                            Text(top.song.title, color = ink, style = MaterialTheme.typography.bodyLarge, fontWeight = FontWeight.Medium, maxLines = 1, overflow = TextOverflow.Ellipsis)
                            Text(top.song.artist, color = faint, style = MaterialTheme.typography.bodyMedium, maxLines = 1, overflow = TextOverflow.Ellipsis)
                        }
                        Text(grouped(top.plays.toLong()), color = faint, style = MaterialTheme.typography.bodyMedium)
                    }
                }
                if (r.topSongs.size > 10) item {
                    TextButton(onClick = { all = !all }) { Text(if (all) "Show the top 10" else "Show all ${r.topSongs.size}") }
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
    Column(Modifier.padding(top = 4.dp)) {
        Text(
            grouped(shown),
            color = MaterialTheme.colorScheme.primary,
            fontSize = 84.sp,
            lineHeight = 88.sp,
            fontFamily = titleFamily(1f),
            fontWeight = FontWeight.Bold,
        )
        Text(
            when {
                minutes -> if (shown == 1L) "minute of music" else "minutes of music"
                hours == 1L -> "hour of music"
                else -> "hours of music"
            },
            style = MaterialTheme.typography.titleLarge, fontWeight = FontWeight.Bold)
        val parts = buildList {
            if (days >= 1) add("That is ${count(days.toInt(), "whole day")}.")
            add("${grouped(r.plays.toLong())} plays.")
            if (r.newSongs > 0u) add("${count(r.newSongs.toInt(), "new song")} found.")
        }
        Text(parts.joinToString(" "), color = MaterialTheme.colorScheme.onSurfaceVariant, style = MaterialTheme.typography.bodyLarge, modifier = Modifier.padding(top = 6.dp))
    }
}

@Composable
private fun TopArtist(a: fyi.nnx.needle.core.YearTopArtist) {
    val photo by rememberLoaded(a.name, cache = false) { artistPhoto(a.name) ?: artistAlbums(a.name).firstNotNullOfOrNull { it.artwork } }
    Section("Top artist")
    Row(Modifier.fillMaxWidth().padding(top = 6.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(16.dp)) {
        Cover(photo, Modifier.size(72.dp), CircleShape, seed = a.name)
        Column(Modifier.weight(1f)) {
            Text(a.name, style = MaterialTheme.typography.titleLarge, fontWeight = FontWeight.SemiBold, maxLines = 1, overflow = TextOverflow.Ellipsis)
            Text(
                "${if (a.seconds < 3600) count((a.seconds / 60).toInt(), "minute") else count((a.seconds / 3600).toInt(), "hour")} · ${count(a.plays.toInt(), "play")}" +
                    if (a.monthsInTopFive > 1u) " · in your top five for ${a.monthsInTopFive} months" else "",
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                style = MaterialTheme.typography.bodyMedium,
            )
        }
    }
}

/** A part's title, as a shelf's on the other pages. */
@Composable
private fun Section(text: String) {
    Text(text, style = MaterialTheme.typography.titleLarge, fontWeight = FontWeight.SemiBold, modifier = Modifier.padding(top = 26.dp, bottom = 6.dp).semantics { heading() })
}

/** Plays by month: twelve bars, the busiest lit. */
@Composable
private fun Months(r: YearInReview) {
    val most = (r.months.maxOfOrNull { it.plays } ?: 0u).toFloat().coerceAtLeast(1f)
    val busiest = r.months.maxByOrNull { it.plays }?.month
    val names = java.text.DateFormatSymbols.getInstance().shortMonths
    val accent = MaterialTheme.colorScheme.primary
    val bar = MaterialTheme.colorScheme.onSurface.copy(alpha = 0.14f)
    val faint = MaterialTheme.colorScheme.onSurfaceVariant
    Column {
        Section("Plays by month")
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
                        .background(if (m.month == busiest && m.plays > 0u) accent else bar),
                )
            }
        }
        Row(Modifier.fillMaxWidth().padding(top = 4.dp).clearAndSetSemantics {}, horizontalArrangement = Arrangement.spacedBy(5.dp)) {
            r.months.forEach { m ->
                Text(names[m.month.toInt() - 1].take(1), color = faint, style = MaterialTheme.typography.labelSmall, modifier = Modifier.weight(1f), textAlign = androidx.compose.ui.text.style.TextAlign.Center)
            }
        }
    }
}

@Composable
private fun Actions(r: YearInReview, context: Context) {
    val scheme = MaterialTheme.colorScheme
    val look = CardLook(scheme.background.toArgb(), scheme.onSurface.toArgb(), scheme.onSurfaceVariant.toArgb(), scheme.primary.toArgb())
    Column(Modifier.fillMaxWidth().padding(top = 28.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
        Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            Button(
                onClick = { shareYear(context, r, look) },
                modifier = Modifier.weight(1f).height(52.dp),
            ) { Text("Share", fontWeight = FontWeight.SemiBold) }
            FilledTonalButton(
                enabled = r.topSongs.isNotEmpty(),
                onClick = { playSongs(r.topSongs.map { it.song.id }) },
                modifier = Modifier.weight(1f).height(52.dp),
            ) { Text(if (r.topSongs.size >= 50) "Play top 50" else "Play top songs", fontWeight = FontWeight.SemiBold) }
        }
        TextButton(enabled = r.topSongs.isNotEmpty(), onClick = {
            libraryWrite({ core.saveYearPlaylist(r.year) }) { showMessage("Saved as “Top songs of ${r.year}”") }
        }) { Text("Make a playlist") }
    }
}

/** The year as a picture (1080 × 1350), to Android's share sheet, as lyric cards are. */
private fun shareYear(context: Context, r: YearInReview, look: CardLook) {
    NeedleApp.instance.scope.launch(Dispatchers.Main) {
        val file = runCatching { withContext(Dispatchers.Default) { yearCard(context, r, look) } }
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

private fun yearCard(context: Context, r: YearInReview, look: CardLook): File {
    val w = 1080
    val h = 1350
    val bitmap = Bitmap.createBitmap(w, h, Bitmap.Config.ARGB_8888)
    val canvas = Canvas(bitmap)
    val paint = Paint(Paint.ANTI_ALIAS_FLAG)
    paint.color = look.page
    canvas.drawRect(0f, 0f, w.toFloat(), h.toFloat(), paint)
    val margin = 96f
    val bold = android.graphics.Typeface.create(android.graphics.Typeface.DEFAULT, android.graphics.Typeface.BOLD)
    fun text(size: Float, color: Int, typeface: android.graphics.Typeface? = null) = TextPaint(Paint.ANTI_ALIAS_FLAG).apply {
        textSize = size
        this.color = color
        if (typeface != null) this.typeface = typeface
    }
    val amber = look.accent
    val white = look.ink
    val faint = look.faint
    var y = margin + 40f
    canvas.drawText("Your ${r.year} in music", margin, y, text(44f, white, bold))
    y += 210f
    // Under an hour: minutes, as the page says it.
    val hours = (r.seconds / 3600).toLong()
    val number = if (hours < 1) (r.seconds / 60).toLong() else hours
    canvas.drawText(grouped(number), margin, y, text(200f, amber, bold))
    y += 70f
    canvas.drawText(if (hours < 1) (if (number == 1L) "minute of music" else "minutes of music") else if (hours == 1L) "hour of music" else "hours of music", margin, y, text(54f, white, bold))
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
