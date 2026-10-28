package fyi.nnx.needle.ui

import android.content.Context
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.graphics.Canvas
import android.graphics.LinearGradient
import android.graphics.Paint
import android.graphics.RectF
import android.graphics.Shader
import android.text.Layout
import android.text.StaticLayout
import android.text.TextPaint
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.produceState
import androidx.compose.ui.graphics.toArgb
import com.google.mlkit.common.model.DownloadConditions
import com.google.mlkit.nl.languageid.LanguageIdentification
import com.google.mlkit.nl.translate.TranslateLanguage
import com.google.mlkit.nl.translate.Translation
import com.google.mlkit.nl.translate.TranslatorOptions
import fyi.nnx.needle.NeedleApp
import fyi.nnx.needle.core.Song
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.tasks.await
import kotlinx.coroutines.withContext
import java.io.File

// ---------- Lyric cards

// ---------- Lyric cards

private const val CardWidth = 1080
private const val CardCover = 300
private const val CardMargin = 84

/** The most rows of words; a longer line is written smaller. */
private const val MostRows = 12

/** How wide a letter is, in narrow letters: Japanese, Korean, and Chinese ones take two (as on the computers). */
internal fun letterWidth(c: Int): Int = when (c) {
    in 0x1100..0x115F, in 0x2E80..0xA4CF, in 0xAC00..0xD7A3, in 0xF900..0xFAFF,
    in 0xFE30..0xFE4F, in 0xFF00..0xFF60, in 0xFFE0..0xFFE6, in 0x20000..0x3FFFD -> 2
    else -> 1
}

internal fun textWidth(text: String): Int = text.codePoints().toArray().sumOf { letterWidth(it) }

/**
 * The line cut into rows at most `width` narrow letters wide: at spaces, between wide letters
 * (which need no spaces), and inside a word too long for a row (as on the computers).
 */
internal fun wrapLyric(text: String, width: Int): List<String> {
    val w = width.coerceAtLeast(2)
    // The pieces a row may break between: words, and each wide letter on its own; and whether a space comes before.
    val pieces = mutableListOf<Pair<String, Boolean>>()
    for (word in text.split(Regex("\\s+")).filter { it.isNotEmpty() }) {
        var spaced = true
        val part = StringBuilder()
        word.codePoints().forEach { c ->
            if (letterWidth(c) == 2) {
                if (part.isNotEmpty()) { pieces += part.toString() to spaced; part.clear(); spaced = false }
                pieces += String(Character.toChars(c)) to spaced
                spaced = false
            } else part.appendCodePoint(c)
        }
        if (part.isNotEmpty()) pieces += part.toString() to spaced
    }
    val rows = mutableListOf<String>()
    for ((piece, spaced) in pieces) {
        val gap = if (spaced) 1 else 0
        val last = rows.lastOrNull()
        if (last != null && textWidth(last) + gap + textWidth(piece) <= w) {
            rows[rows.size - 1] = last + (if (spaced) " " else "") + piece
        } else {
            // A word wider than a row is cut.
            val row = StringBuilder()
            piece.codePoints().forEach { c ->
                if (row.isNotEmpty() && textWidth(row.toString()) + letterWidth(c) > w) { rows += row.toString(); row.clear() }
                row.appendCodePoint(c)
            }
            rows += row.toString()
        }
    }
    return rows
}

/**
 * The words' size (px) and rows, as on the computers: as large as the line allows, from 104
 * down to 52; a line of up to 30 letters fits in 3 rows, up to 90 in 5, longer in up to 12.
 */
internal fun lyricLayout(line: String): Pair<Int, List<String>> {
    val room = (CardWidth - 2 * CardMargin).toFloat()
    val length = textWidth(line)
    val most = when {
        length <= 30 -> 3
        length <= 90 -> 5
        else -> MostRows
    }
    var size = 104
    while (true) {
        // About half the size per narrow letter, in this font.
        val rows = wrapLyric(line, (room / (size * 0.53f)).toInt())
        if (rows.size <= most || size <= 52) return size to rows.take(MostRows)
        size -= 6
    }
}

/**
 * A picture of one lyric line in the cover's colours, to share, laid out as on the computers:
 * 1080 wide and as tall as the words need (a square at least); the cover at the top, the line
 * large in the middle of the room under it, the song and Needle's mark at the foot. Made off the
 * main thread; returns the picture's file in Needle's cache.
 */
suspend fun makeLyricCard(context: Context, song: Song, line: String, colors: Pair<Int, Int>, ink: Int): File = withContext(Dispatchers.Default) {
    val cover = song.artwork?.takeIf { !it.startsWith("http") }?.let { runCatching { BitmapFactory.decodeFile(it) }.getOrNull() }
    val (size, rows) = lyricLayout(line.trim())
    val leading = size + size / 4
    val wordsTop = CardMargin + if (cover != null) CardCover + 90 else 0
    val words = rows.size * leading
    val foot = 200
    val w = CardWidth
    val h = maxOf(wordsTop + words + 110 + foot, CardWidth)
    val room = h - foot - wordsTop
    val first = wordsTop + (room - words) / 2 + size
    val bitmap = Bitmap.createBitmap(w, h, Bitmap.Config.ARGB_8888)
    val canvas = Canvas(bitmap)
    val paint = Paint(Paint.ANTI_ALIAS_FLAG)
    paint.shader = LinearGradient(0f, 0f, w * 0.4f, h.toFloat(), colors.first, colors.second, Shader.TileMode.CLAMP)
    canvas.drawRect(0f, 0f, w.toFloat(), h.toFloat(), paint)
    paint.shader = null
    if (cover != null) {
        // Cut to a square from the middle, with round corners, as Needle draws covers.
        val side = minOf(cover.width, cover.height)
        val src = android.graphics.Rect((cover.width - side) / 2, (cover.height - side) / 2, (cover.width + side) / 2, (cover.height + side) / 2)
        val box = RectF(CardMargin.toFloat(), CardMargin.toFloat(), (CardMargin + CardCover).toFloat(), (CardMargin + CardCover).toFloat())
        canvas.save()
        canvas.clipPath(android.graphics.Path().apply { addRoundRect(box, 28f, 28f, android.graphics.Path.Direction.CW) })
        canvas.drawBitmap(cover, src, box, Paint(Paint.FILTER_BITMAP_FLAG))
        canvas.restore()
    }
    // Needle's own serif for the words; a row with Japanese, Korean, or Chinese in the phone's bold.
    val serif = runCatching { androidx.core.content.res.ResourcesCompat.getFont(context, fyi.nnx.needle.R.font.fraunces72ptsoft_semibold) }.getOrNull()
    val bold = android.graphics.Typeface.create(android.graphics.Typeface.DEFAULT, android.graphics.Typeface.BOLD)
    val text = TextPaint(Paint.ANTI_ALIAS_FLAG).apply { color = ink; textSize = size.toFloat() }
    rows.forEachIndexed { i, row ->
        val wide = row.codePoints().anyMatch { letterWidth(it) == 2 }
        text.typeface = if (wide || serif == null) bold else serif
        canvas.drawText(row, CardMargin.toFloat(), (first + i * leading).toFloat(), text)
    }
    // The song, and Needle's mark, at the foot.
    val who = listOf(song.title, song.artist).filter { it.isNotBlank() }.joinToString(" · ")
    val whoPaint = TextPaint(Paint.ANTI_ALIAS_FLAG).apply {
        color = (ink and 0x00FFFFFF) or (0xD9 shl 24)
        textSize = 34f
        typeface = bold
    }
    canvas.drawText(android.text.TextUtils.ellipsize(who, whoPaint, (w - 2 * CardMargin).toFloat(), android.text.TextUtils.TruncateAt.END).toString(), CardMargin.toFloat(), (h - CardMargin - 70).toFloat(), whoPaint)
    val amber = 0xFFE2B46C.toInt()
    canvas.drawCircle((CardMargin + 11).toFloat(), (h - CardMargin - 10).toFloat(), 11f, Paint(Paint.ANTI_ALIAS_FLAG).apply { color = amber })
    canvas.drawText("Needle", (CardMargin + 34).toFloat(), (h - CardMargin).toFloat(), TextPaint(Paint.ANTI_ALIAS_FLAG).apply { color = amber; textSize = 30f; typeface = bold })
    val dir = File(context.cacheDir, "cards").apply { mkdirs() }
    val file = File(dir, "lyric-card.png")
    file.outputStream().use { bitmap.compress(Bitmap.CompressFormat.PNG, 100, it) }
    bitmap.recycle()
    file
}

/** Makes a lyric card and opens Android's share sheet with it. */
fun shareLyricCard(context: Context, song: Song, line: String, colors: Pair<androidx.compose.ui.graphics.Color, androidx.compose.ui.graphics.Color>, ink: androidx.compose.ui.graphics.Color) {
    NeedleApp.instance.scope.launch(Dispatchers.Main) {
        val file = runCatching { makeLyricCard(context, song, line, colors.first.toArgb() to colors.second.toArgb(), ink.toArgb()) }
            .getOrElse { showMessage("Could not make the card"); return@launch }
        val uri = androidx.core.content.FileProvider.getUriForFile(context, "${context.packageName}.files", file)
        val send = android.content.Intent(android.content.Intent.ACTION_SEND)
            .setType("image/png")
            .putExtra(android.content.Intent.EXTRA_STREAM, uri)
            .putExtra(android.content.Intent.EXTRA_TEXT, "${song.title} · ${song.artist}")
            .addFlags(android.content.Intent.FLAG_GRANT_READ_URI_PERMISSION)
        tryOpen("app to share with") {
            context.startActivity(android.content.Intent.createChooser(send, "Share the lyric").addFlags(android.content.Intent.FLAG_ACTIVITY_NEW_TASK))
        }
    }
}

// ---------- Translated lyrics

/** What a translation of a song's lyrics came to. */
sealed interface Translated {
    data object Working : Translated
    data class Lines(val lines: List<String>) : Translated
    data class Failed(val why: String) : Translated
}

/** Lines translated before, by song, while Needle runs. */
private val translations = HashMap<String, List<String>>()

/**
 * The song's lyric lines in the phone's own language, translated on the phone (ML Kit): the
 * lyrics' language is found, that language's model downloads once (on Wi-Fi), and each line is
 * translated. Nothing while `on` is false.
 */
@Composable
fun rememberTranslation(song: Song, lines: List<String>, on: Boolean): Translated? {
    val state by produceState<Translated?>(null, song.id, lines, on) {
        if (!on || lines.isEmpty()) { value = null; return@produceState }
        translations[song.id]?.takeIf { it.size == lines.size }?.let { value = Translated.Lines(it); return@produceState }
        value = Translated.Working
        value = runCatching {
            val text = lines.filter { it.isNotBlank() }.joinToString("\n")
            val found = LanguageIdentification.getClient().identifyLanguage(text).await()
            val from = TranslateLanguage.fromLanguageTag(found) ?: return@runCatching Translated.Failed("Needle could not tell the lyrics' language")
            val to = TranslateLanguage.fromLanguageTag(java.util.Locale.getDefault().language) ?: TranslateLanguage.ENGLISH
            if (from == to) return@runCatching Translated.Failed("These lyrics are in your own language")
            val translator = Translation.getClient(TranslatorOptions.Builder().setSourceLanguage(from).setTargetLanguage(to).build())
            try {
                translator.downloadModelIfNeeded(DownloadConditions.Builder().requireWifi().build()).await()
                val out = lines.map { if (it.isBlank()) "" else translator.translate(it).await() }
                translations[song.id] = out
                Translated.Lines(out)
            } finally {
                translator.close()
            }
        }.getOrElse { Translated.Failed(it.message ?: "The translation could not be made. It needs Wi-Fi the first time.") }
    }
    return state
}
