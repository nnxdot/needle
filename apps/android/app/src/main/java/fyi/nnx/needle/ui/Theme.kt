package fyi.nnx.needle.ui

import android.os.Build
import androidx.compose.material3.ColorScheme
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.MaterialExpressiveTheme
import androidx.compose.material3.MotionScheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.dynamicDarkColorScheme
import androidx.compose.material3.dynamicLightColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.luminance
import androidx.compose.ui.graphics.toArgb
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.ui.platform.LocalContext
import fyi.nnx.needle.NeedleApp
import fyi.nnx.needle.core.ThemeInfo

/**
 * Needle's Night palette with its amber accent. The page is a deeper charcoal than the desktop
 * window so covers carry the colour, as in Apple Music; amber marks what is playing, links, and
 * the main actions only.
 */
val NeedleColors = darkColorScheme(
    primary = Color(0xFFE2B46C),
    onPrimary = Color(0xFF231A0C),
    primaryContainer = Color(0xFF4A3920),
    onPrimaryContainer = Color(0xFFF6DDB4),
    secondary = Color(0xFFD4BFA0),
    onSecondary = Color(0xFF2A2116),
    secondaryContainer = Color(0xFF2E2A26),
    onSecondaryContainer = Color(0xFFE2B46C),
    tertiary = Color(0xFFB9C7FF),
    background = Color(0xFF141312),
    onBackground = Color(0xFFF1EFED),
    surface = Color(0xFF141312),
    onSurface = Color(0xFFF1EFED),
    surfaceVariant = Color(0xFF2B2927),
    onSurfaceVariant = Color(0xFFA8A29C),
    surfaceContainerLowest = Color(0xFF0F0E0D),
    surfaceContainerLow = Color(0xFF1B1918),
    surfaceContainer = Color(0xFF211F1E),
    surfaceContainerHigh = Color(0xFF2B2927),
    surfaceContainerHighest = Color(0xFF353230),
    outline = Color(0xFF6B6560),
    outlineVariant = Color(0xFF2E2B29),
)

/** Midnight: true black, for OLED screens, with the same amber. */
val MidnightColors = NeedleColors.copy(
    background = Color.Black,
    surface = Color.Black,
    surfaceContainerLowest = Color.Black,
    surfaceContainerLow = Color(0xFF0B0B0B),
    surfaceContainer = Color(0xFF141414),
    surfaceContainerHigh = Color(0xFF1E1E1E),
    surfaceContainerHighest = Color(0xFF282828),
    surfaceVariant = Color(0xFF1E1E1E),
    outlineVariant = Color(0xFF222222),
)

/** Day: a light page, with a deeper amber that reads on it. */
val DayColors = lightColorScheme(
    primary = Color(0xFF8A5A12),
    onPrimary = Color.White,
    primaryContainer = Color(0xFFF6DDB4),
    onPrimaryContainer = Color(0xFF2B1D07),
    secondaryContainer = Color(0xFFEFE7DC),
    onSecondaryContainer = Color(0xFF8A5A12),
    background = Color(0xFFFBFAF8),
    onBackground = Color(0xFF1C1B1A),
    surface = Color(0xFFFBFAF8),
    onSurface = Color(0xFF1C1B1A),
    surfaceVariant = Color(0xFFEDEAE6),
    onSurfaceVariant = Color(0xFF5E5953),
    surfaceContainerLowest = Color.White,
    surfaceContainerLow = Color(0xFFF4F2EF),
    surfaceContainer = Color(0xFFEFECE8),
    surfaceContainerHigh = Color(0xFFE8E4DF),
    surfaceContainerHighest = Color(0xFFE1DCD6),
    outline = Color(0xFF8C867F),
    outlineVariant = Color(0xFFDDD8D2),
)

/** "#3e63dd" as a colour; nothing for anything but six hex digits (an empty accent is amber). */
private fun hex(text: String?): Color? = text?.trim()?.removePrefix("#")?.takeIf { Regex("[0-9a-fA-F]{6}").matches(it) }?.let {
    Color(("FF$it").toLong(16))
}

/** A desktop theme's colours on Android's colour roles. */
fun ColorScheme.with(theme: ThemeInfo): ColorScheme {
    val c = theme.colors
    var s = this
    hex(c["page"])?.let { s = s.copy(background = it, surface = it) }
    hex(c["sidebar"])?.let { s = s.copy(surfaceContainerLow = it, surfaceContainerLowest = it) }
    hex(c["card"])?.let { s = s.copy(surfaceContainer = it, surfaceVariant = it) }
    hex(c["card_hover"])?.let { s = s.copy(surfaceContainerHigh = it, surfaceContainerHighest = it) }
    hex(c["text"])?.let { s = s.copy(onSurface = it, onBackground = it) }
    hex(c["text_muted"])?.let { s = s.copy(onSurfaceVariant = it) }
    hex(c["accent"])?.let { s = s.copy(primary = it, onSecondaryContainer = it) }
    hex(c["accent_text"])?.let { s = s.copy(onPrimary = it) }
    hex(c["border"])?.let { s = s.copy(outline = it) }
    hex(c["border_soft"])?.let { s = s.copy(outlineVariant = it) }
    hex(c["danger"])?.let { s = s.copy(error = it) }
    return s
}

/** True while the full player is on screen, so the status bar icons are white on it. */
val playerShowing = androidx.compose.runtime.mutableStateOf(false)

/** The look chosen in Settings › Appearance. */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun NeedleTheme(content: @Composable () -> Unit) {
    val app by NeedleApp.instance.app.collectAsState()
    val themeVersion by NeedleApp.instance.themeVersion.collectAsState()
    val context = LocalContext.current
    val custom = remember(app.theme, themeVersion) {
        if (app.theme in listOf("night", "midnight", "day")) null
        else runCatching { NeedleApp.instance.core.themes() }.getOrNull()?.firstOrNull { it.id == app.theme }
    }
    val base = when (custom?.base ?: app.theme) {
        "day", "light" -> DayColors
        "midnight" -> MidnightColors
        else -> NeedleColors
    }
    val light = base == DayColors
    // From the wallpaper when asked (Android 12 and newer); the page stays the theme's own.
    // A chosen accent: Material's colour roles made from it, on the theme's own page.
    val accent = remember(app.accentColor, light, base) {
        hex(app.accentColor)?.let { seed ->
            val made = coverScheme(seed.toArgb(), dark = !light, black = base == MidnightColors)
            base.copy(
                primary = made.primary, onPrimary = made.onPrimary,
                primaryContainer = made.primaryContainer, onPrimaryContainer = made.onPrimaryContainer,
                secondaryContainer = made.secondaryContainer, onSecondaryContainer = made.primary,
                tertiary = made.tertiary, surfaceTint = made.surfaceTint,
            )
        }
    }
    val colors = when {
        custom != null -> base.with(custom)
        accent != null -> accent
        app.wallpaperColors && Build.VERSION.SDK_INT >= 31 -> {
            val dynamic = if (light) dynamicLightColorScheme(context) else dynamicDarkColorScheme(context)
            dynamic.copy(
                background = base.background,
                surface = base.surface,
                surfaceContainerLowest = base.surfaceContainerLowest,
            )
        }
        else -> base
    }
    val type = remember(app.titleFont, app.textScale) { needleType(app.titleFont, app.textScale) }
    // The clock and icons at the top follow Needle's page, not the phone's own light or dark mode.
    val view = androidx.compose.ui.platform.LocalView.current
    // The player is always dark, whatever the theme.
    val lightPage = colors.background.luminance() > 0.5f && !playerShowing.value
    if (!view.isInEditMode) androidx.compose.runtime.SideEffect {
        val window = (view.context as? android.app.Activity)?.window ?: return@SideEffect
        androidx.core.view.WindowCompat.getInsetsController(window, view).apply {
            isAppearanceLightStatusBars = lightPage
            isAppearanceLightNavigationBars = lightPage
        }
    }
    MaterialExpressiveTheme(
        colorScheme = colors,
        motionScheme = if (reduceMotion()) MotionScheme.standard() else MotionScheme.expressive(),
        typography = type,
    ) {
        androidx.compose.runtime.CompositionLocalProvider(
            LocalTitleFont provides app.titleFont,
            LocalRows provides Rows.of(app.density),
        ) {
            androidx.compose.foundation.layout.Box {
                content()
                if (app.grain > 0f) Grain(app.grain)
            }
        }
    }
}

/**
 * Film grain over the whole app, as on the desktop: a small tile of noise, repeated, faint.
 * It lets touches through.
 */
@Composable
private fun Grain(amount: Float) {
    val tile = remember {
        val size = 128
        val random = java.util.Random(7)
        val pixels = IntArray(size * size) {
            val v = random.nextInt(256)
            android.graphics.Color.argb(255, v, v, v)
        }
        android.graphics.Bitmap.createBitmap(pixels, size, size, android.graphics.Bitmap.Config.ARGB_8888).asImageBitmap()
    }
    val brush = remember(tile) {
        androidx.compose.ui.graphics.ShaderBrush(
            androidx.compose.ui.graphics.ImageShader(tile, androidx.compose.ui.graphics.TileMode.Repeated, androidx.compose.ui.graphics.TileMode.Repeated),
        )
    }
    androidx.compose.foundation.Canvas(androidx.compose.ui.Modifier.fillMaxSize()) {
        drawRect(brush, alpha = (amount * 0.12f).coerceIn(0f, 0.12f), blendMode = androidx.compose.ui.graphics.BlendMode.Overlay)
    }
}
