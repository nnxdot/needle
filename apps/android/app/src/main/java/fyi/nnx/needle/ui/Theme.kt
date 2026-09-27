package fyi.nnx.needle.ui

import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.MaterialExpressiveTheme
import androidx.compose.material3.MotionScheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color

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

@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun NeedleTheme(content: @Composable () -> Unit) {
    MaterialExpressiveTheme(
        colorScheme = NeedleColors,
        motionScheme = MotionScheme.expressive(),
        content = content,
    )
}
