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

/**
 * A picture of one lyric line in the cover's colours, to share: the line large, the cover
 * small with the song and artist under it, on a gradient of the song's colours. Made off the
 * main thread; returns the picture's file in Needle's cache.
 */
suspend fun makeLyricCard(context: Context, song: Song, line: String, colors: Pair<Int, Int>, ink: Int): File = withContext(Dispatchers.Default) {
    val w = 1080
    val h = 1350
    val bitmap = Bitmap.createBitmap(w, h, Bitmap.Config.ARGB_8888)
    val canvas = Canvas(bitmap)
    val paint = Paint(Paint.ANTI_ALIAS_FLAG)
    paint.shader = LinearGradient(0f, 0f, w.toFloat(), h.toFloat(), colors.first, colors.second, Shader.TileMode.CLAMP)
    canvas.drawRect(0f, 0f, w.toFloat(), h.toFloat(), paint)
    paint.shader = null
    val margin = 96f
    // The line, large and bold, wrapped to the card.
    val text = TextPaint(Paint.ANTI_ALIAS_FLAG).apply {
        color = ink
        textSize = when {
            line.length > 120 -> 56f
            line.length > 60 -> 68f
            else -> 84f
        }
        typeface = android.graphics.Typeface.create(android.graphics.Typeface.DEFAULT, android.graphics.Typeface.BOLD)
    }
    val layout = StaticLayout.Builder.obtain(line, 0, line.length, text, (w - margin * 2).toInt())
        .setAlignment(Layout.Alignment.ALIGN_NORMAL)
        .setLineSpacing(0f, 1.12f)
        .build()
    canvas.save()
    canvas.translate(margin, (h * 0.42f - layout.height / 2f).coerceAtLeast(margin))
    layout.draw(canvas)
    canvas.restore()
    // The cover, small, with the song and its artist.
    val side = 132f
    val top = h - margin - side
    val cover = song.artwork?.takeIf { !it.startsWith("http") }?.let { runCatching { BitmapFactory.decodeFile(it) }.getOrNull() }
    if (cover != null) {
        val path = android.graphics.Path().apply { addRoundRect(RectF(margin, top, margin + side, top + side), 14f, 14f, android.graphics.Path.Direction.CW) }
        canvas.save()
        canvas.clipPath(path)
        canvas.drawBitmap(cover, null, RectF(margin, top, margin + side, top + side), Paint(Paint.FILTER_BITMAP_FLAG))
        canvas.restore()
    }
    val left = if (cover != null) margin + side + 32f else margin
    val title = TextPaint(Paint.ANTI_ALIAS_FLAG).apply {
        color = ink
        textSize = 42f
        typeface = android.graphics.Typeface.create(android.graphics.Typeface.DEFAULT, android.graphics.Typeface.BOLD)
    }
    val sub = TextPaint(Paint.ANTI_ALIAS_FLAG).apply {
        color = (ink and 0x00FFFFFF) or (0xB0 shl 24)
        textSize = 36f
    }
    val room = w - margin - left
    canvas.drawText(android.text.TextUtils.ellipsize(song.title, title, room, android.text.TextUtils.TruncateAt.END).toString(), left, top + 56f, title)
    canvas.drawText(android.text.TextUtils.ellipsize(song.artist, sub, room, android.text.TextUtils.TruncateAt.END).toString(), left, top + 108f, sub)
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
