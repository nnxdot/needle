package fyi.nnx.needle.ui

import android.os.Build
import android.util.LruCache
import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.core.Animatable
import androidx.compose.ui.graphics.lerp
import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.ColorScheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.blur
import androidx.compose.ui.draw.clipToBounds
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.graphics.luminance
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import androidx.palette.graphics.Palette
import coil3.compose.AsyncImage
import coil3.imageLoader
import coil3.request.ImageRequest
import coil3.request.SuccessResult
import coil3.request.allowHardware
import coil3.toBitmap
import com.materialkolor.hct.Hct
import com.materialkolor.scheme.DynamicScheme
import com.materialkolor.scheme.SchemeContent
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

/** Each cover's own colour, found once. */
private val seeds = LruCache<String, Int>(200)

/** The colour a cover is known by: its most lively one, or the one it has most of. */
suspend fun coverSeed(context: android.content.Context, path: String): Int? {
    seeds.get(path)?.let { return it }
    return withContext(Dispatchers.IO) {
        val request = ImageRequest.Builder(context).data(fileUri(path)).size(96).allowHardware(false).build()
        val bitmap = (context.imageLoader.execute(request) as? SuccessResult)?.image?.toBitmap()?.let(::trimBars)
            ?: return@withContext null
        val palette = Palette.from(bitmap).generate()
        val seed = (palette.vibrantSwatch ?: palette.dominantSwatch ?: palette.mutedSwatch)?.rgb ?: return@withContext null
        seeds.put(path, seed)
        seed
    }
}

/**
 * Material's whole set of colour roles made from one cover, as Android makes them from the
 * wallpaper: the cover's hue on the page, the buttons, the containers, and the text on them.
 */
fun coverScheme(seed: Int, dark: Boolean, black: Boolean): ColorScheme {
    val s: DynamicScheme = SchemeContent(Hct.fromInt(seed), dark, 0.0)
    fun c(argb: Int) = Color(argb)
    val base = if (dark) darkColorScheme() else lightColorScheme()
    val scheme = base.copy(
        primary = c(s.primary), onPrimary = c(s.onPrimary),
        primaryContainer = c(s.primaryContainer), onPrimaryContainer = c(s.onPrimaryContainer),
        inversePrimary = c(s.inversePrimary),
        secondary = c(s.secondary), onSecondary = c(s.onSecondary),
        secondaryContainer = c(s.secondaryContainer), onSecondaryContainer = c(s.onSecondaryContainer),
        tertiary = c(s.tertiary), onTertiary = c(s.onTertiary),
        tertiaryContainer = c(s.tertiaryContainer), onTertiaryContainer = c(s.onTertiaryContainer),
        background = c(s.background), onBackground = c(s.onBackground),
        surface = c(s.surface), onSurface = c(s.onSurface),
        surfaceVariant = c(s.surfaceVariant), onSurfaceVariant = c(s.onSurfaceVariant),
        surfaceTint = c(s.surfaceTint),
        inverseSurface = c(s.inverseSurface), inverseOnSurface = c(s.inverseOnSurface),
        outline = c(s.outline), outlineVariant = c(s.outlineVariant),
        surfaceBright = c(s.surfaceBright), surfaceDim = c(s.surfaceDim),
        surfaceContainerLowest = c(s.surfaceContainerLowest), surfaceContainerLow = c(s.surfaceContainerLow),
        surfaceContainer = c(s.surfaceContainer), surfaceContainerHigh = c(s.surfaceContainerHigh),
        surfaceContainerHighest = c(s.surfaceContainerHighest),
        primaryFixed = c(s.primaryFixed), primaryFixedDim = c(s.primaryFixedDim),
        onPrimaryFixed = c(s.onPrimaryFixed), onPrimaryFixedVariant = c(s.onPrimaryFixedVariant),
        secondaryFixed = c(s.secondaryFixed), secondaryFixedDim = c(s.secondaryFixedDim),
        onSecondaryFixed = c(s.onSecondaryFixed), onSecondaryFixedVariant = c(s.onSecondaryFixedVariant),
        tertiaryFixed = c(s.tertiaryFixed), tertiaryFixedDim = c(s.tertiaryFixedDim),
        onTertiaryFixed = c(s.onTertiaryFixed), onTertiaryFixedVariant = c(s.onTertiaryFixedVariant),
    )
    // The page itself carries the hue, a little more than Material's near-neutral surfaces, as
    // Apple Music's album pages do.
    val hct = Hct.fromInt(seed)
    val chroma = minOf(hct.chroma, 18.0)
    val page = Color(Hct.from(hct.hue, chroma, if (dark) 11.0 else 95.0).toInt())
    val tinted = scheme.copy(
        background = page,
        surface = page,
        surfaceContainerLow = Color(Hct.from(hct.hue, chroma, if (dark) 14.0 else 92.0).toInt()),
        surfaceContainer = Color(Hct.from(hct.hue, chroma, if (dark) 17.0 else 90.0).toInt()),
        surfaceContainerHigh = Color(Hct.from(hct.hue, chroma, if (dark) 21.0 else 87.0).toInt()),
        surfaceContainerHighest = Color(Hct.from(hct.hue, chroma, if (dark) 25.0 else 84.0).toInt()),
    )
    // Midnight keeps its black page; only the colours on it change.
    return if (black) tinted.copy(background = Color.Black, surface = Color.Black, surfaceContainerLowest = Color.Black) else tinted
}

/** The colour scheme of a cover, or `null` while it is found (or when there is no cover). */
@Composable
fun rememberCoverScheme(path: String?, forceDark: Boolean = false): ColorScheme? {
    val context = LocalContext.current
    val theme = MaterialTheme.colorScheme
    val dark = forceDark || theme.background.luminance() < 0.5f
    val black = theme.background == Color.Black
    var scheme by remember(path, dark, black) {
        mutableStateOf(path?.let { seeds.get(it) }?.let { coverScheme(it, dark, black) })
    }
    LaunchedEffect(path, dark, black) {
        if (path == null || scheme != null) return@LaunchedEffect
        val seed = coverSeed(context, path) ?: return@LaunchedEffect
        scheme = withContext(Dispatchers.Default) { coverScheme(seed, dark, black) }
    }
    return scheme
}

/** Everything inside takes the cover's colours: the page, the buttons, the menus. */
@Composable
fun CoverTheme(path: String?, dark: Boolean = false, content: @Composable () -> Unit) {
    val target = rememberCoverScheme(path, dark) ?: MaterialTheme.colorScheme
    MaterialTheme(colorScheme = animatedScheme(target), typography = MaterialTheme.typography, shapes = MaterialTheme.shapes, content = content)
}

/** A colour scheme that blends into the next one, rather than snapping. */
@Composable
fun animatedScheme(target: ColorScheme): ColorScheme {
    var from by remember { mutableStateOf(target) }
    var to by remember { mutableStateOf(target) }
    val t = remember { Animatable(1f) }
    LaunchedEffect(target) {
        if (target == to) return@LaunchedEffect
        from = blend(from, to, t.value)
        to = target
        t.snapTo(0f)
        t.animateTo(1f, tween(450))
    }
    return if (t.value >= 1f) to else blend(from, to, t.value)
}

private fun blend(a: ColorScheme, b: ColorScheme, t: Float): ColorScheme = if (t >= 1f) b else b.copy(
        primary = lerp(a.primary, b.primary, t),
        onPrimary = lerp(a.onPrimary, b.onPrimary, t),
        primaryContainer = lerp(a.primaryContainer, b.primaryContainer, t),
        onPrimaryContainer = lerp(a.onPrimaryContainer, b.onPrimaryContainer, t),
        inversePrimary = lerp(a.inversePrimary, b.inversePrimary, t),
        secondary = lerp(a.secondary, b.secondary, t),
        onSecondary = lerp(a.onSecondary, b.onSecondary, t),
        secondaryContainer = lerp(a.secondaryContainer, b.secondaryContainer, t),
        onSecondaryContainer = lerp(a.onSecondaryContainer, b.onSecondaryContainer, t),
        tertiary = lerp(a.tertiary, b.tertiary, t),
        onTertiary = lerp(a.onTertiary, b.onTertiary, t),
        tertiaryContainer = lerp(a.tertiaryContainer, b.tertiaryContainer, t),
        onTertiaryContainer = lerp(a.onTertiaryContainer, b.onTertiaryContainer, t),
        background = lerp(a.background, b.background, t),
        onBackground = lerp(a.onBackground, b.onBackground, t),
        surface = lerp(a.surface, b.surface, t),
        onSurface = lerp(a.onSurface, b.onSurface, t),
        surfaceVariant = lerp(a.surfaceVariant, b.surfaceVariant, t),
        onSurfaceVariant = lerp(a.onSurfaceVariant, b.onSurfaceVariant, t),
        surfaceTint = lerp(a.surfaceTint, b.surfaceTint, t),
        inverseSurface = lerp(a.inverseSurface, b.inverseSurface, t),
        inverseOnSurface = lerp(a.inverseOnSurface, b.inverseOnSurface, t),
        outline = lerp(a.outline, b.outline, t),
        outlineVariant = lerp(a.outlineVariant, b.outlineVariant, t),
        surfaceBright = lerp(a.surfaceBright, b.surfaceBright, t),
        surfaceDim = lerp(a.surfaceDim, b.surfaceDim, t),
        surfaceContainerLowest = lerp(a.surfaceContainerLowest, b.surfaceContainerLowest, t),
        surfaceContainerLow = lerp(a.surfaceContainerLow, b.surfaceContainerLow, t),
        surfaceContainer = lerp(a.surfaceContainer, b.surfaceContainer, t),
        surfaceContainerHigh = lerp(a.surfaceContainerHigh, b.surfaceContainerHigh, t),
        surfaceContainerHighest = lerp(a.surfaceContainerHighest, b.surfaceContainerHighest, t),
)

/**
 * The cover, huge and blurred, drifting slowly behind a page, as Apple Music's moving
 * backgrounds do. Android 11 and older cannot blur, so they get the cover's colour instead.
 */
@Composable
fun CoverBackdrop(path: String?, modifier: Modifier = Modifier, dim: Float = 0.35f) {
    val page = MaterialTheme.colorScheme.background
    val reduce = reduceMotion()
    Box(modifier.clipToBounds().background(page)) {
        if (path != null && Build.VERSION.SDK_INT >= 31) {
            val drift = rememberInfiniteTransition(label = "drift")
            val turn by drift.animateFloat(0f, 360f, infiniteRepeatable(tween(90_000, easing = LinearEasing)), label = "turn")
            AnimatedContent(path, transitionSpec = { fadeIn(tween(600)) togetherWith fadeOut(tween(600)) }, label = "backdrop") { art ->
                AsyncImage(
                    model = coverRequest(fileUri(art)),
                    contentDescription = null,
                    contentScale = ContentScale.Crop,
                    modifier = Modifier
                        .fillMaxSize()
                        .graphicsLayer {
                            val r = if (reduce) 0f else turn
                            rotationZ = r
                            scaleX = 1.9f
                            scaleY = 1.9f
                            translationX = if (reduce) 0f else kotlin.math.sin(Math.toRadians(r * 2.0)).toFloat() * size.width * 0.08f
                        }
                        .blur(70.dp),
                )
            }
            Box(Modifier.fillMaxSize().background(page.copy(alpha = dim)))
        } else {
            Box(Modifier.fillMaxSize().background(MaterialTheme.colorScheme.primaryContainer.copy(alpha = 0.6f)))
        }
        // Fades into the page at the foot.
        Box(Modifier.fillMaxSize().background(Brush.verticalGradient(0.35f to Color.Transparent, 1f to page)))
    }
}
