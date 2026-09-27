package fyi.nnx.needle.ui

import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.MaterialExpressiveTheme
import androidx.compose.material3.MotionScheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color

/** Needle's Night palette (warm charcoal) with its amber accent, as on desktop. */
val NeedleColors = darkColorScheme(
    primary = Color(0xFFE2B46C),
    onPrimary = Color(0xFF231A0C),
    primaryContainer = Color(0xFF5B4526),
    onPrimaryContainer = Color(0xFFF6DDB4),
    secondary = Color(0xFFD4BFA0),
    onSecondary = Color(0xFF2A2116),
    secondaryContainer = Color(0xFF41372A),
    onSecondaryContainer = Color(0xFFF1DFC6),
    tertiary = Color(0xFFB9C7FF),
    background = Color(0xFF201E1D),
    onBackground = Color(0xFFEFEDEB),
    surface = Color(0xFF201E1D),
    onSurface = Color(0xFFEFEDEB),
    surfaceVariant = Color(0xFF353230),
    onSurfaceVariant = Color(0xFFBDB8B3),
    surfaceContainerLowest = Color(0xFF191716),
    surfaceContainerLow = Color(0xFF242221),
    surfaceContainer = Color(0xFF2B2927),
    surfaceContainerHigh = Color(0xFF353230),
    surfaceContainerHighest = Color(0xFF3F3B38),
    outline = Color(0xFF6B6560),
    outlineVariant = Color(0xFF3D3936),
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
