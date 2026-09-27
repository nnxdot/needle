package fyi.nnx.needle.ui

import android.graphics.Bitmap
import android.graphics.Color
import coil3.size.Size
import androidx.compose.runtime.Composable
import coil3.request.transformations
import coil3.transform.Transformation

/**
 * Covers taken from videos often come as a square with see-through or black bands around a
 * wide frame. This finds the picture inside, so it can fill the square like any cover.
 */
fun trimBars(input: Bitmap): Bitmap {
    val w = input.width
    val h = input.height
    if (w < 16 || h < 16) return input
    val step = maxOf(1, minOf(w, h) / 64)

    // A band is empty when every sample in it is see-through, or near black and flat.
    fun empty(pixels: IntArray): Boolean {
        var lo = 255
        var hi = 0
        for (p in pixels) {
            if (Color.alpha(p) < 24) continue
            val luma = (Color.red(p) * 3 + Color.green(p) * 6 + Color.blue(p)) / 10
            if (luma > 28) return false
            lo = minOf(lo, luma)
            hi = maxOf(hi, luma)
        }
        return hi - lo < 16
    }
    fun row(y: Int) = IntArray((w + step - 1) / step) { input.getPixel(minOf(it * step, w - 1), y) }
    fun column(x: Int) = IntArray((h + step - 1) / step) { input.getPixel(x, minOf(it * step, h - 1)) }

    var top = 0
    while (top < h / 2 && empty(row(top))) top += step
    var bottom = h - 1
    while (bottom > h / 2 && empty(row(bottom))) bottom -= step
    var left = 0
    while (left < w / 2 && empty(column(left))) left += step
    var right = w - 1
    while (right > w / 2 && empty(column(right))) right -= step

    val width = right - left + 1
    val height = bottom - top + 1
    // Nothing to trim, or so much that it was the picture itself (a dark cover).
    if ((top == 0 && left == 0 && bottom == h - 1 && right == w - 1) || width < w / 4 || height < h / 4) return input
    return Bitmap.createBitmap(input, left, top, width, height)
}

/** [trimBars] for Coil, so every cover it loads fills its frame. */
class TrimBars : Transformation() {
    override val cacheKey = "trim-bars-1"
    override suspend fun transform(input: Bitmap, size: Size): Bitmap = trimBars(input)
}

/** A cover to load, with any bands around the picture trimmed (see [trimBars]). */
@Composable
fun coverRequest(model: Any?): coil3.request.ImageRequest =
    coil3.request.ImageRequest.Builder(androidx.compose.ui.platform.LocalContext.current)
        .data(model)
        .transformations(TrimBars())
        .build()
