package fyi.nnx.needle.ui

import androidx.compose.material3.Typography
import androidx.compose.runtime.Composable
import androidx.compose.runtime.ProvidableCompositionLocal
import androidx.compose.runtime.compositionLocalOf
import androidx.compose.ui.text.ExperimentalTextApi
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.Font
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontVariation
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.TextUnit
import androidx.compose.ui.unit.sp
import fyi.nnx.needle.R

/*
 * Roboto Flex: one variable font (SIL Open Font License; its text ships in
 * assets/licenses/RobotoFlex-OFL.txt) that can be any weight and any width.
 * Text uses it at its usual width; big titles use it narrow and heavy, as posters do.
 * Titles can instead be Fraunces, the desktop's soft serif (assets/licenses/Fraunces-OFL.txt),
 * or Android's own font (Settings › Appearance).
 */

@OptIn(ExperimentalTextApi::class)
private fun flex(weight: Int, width: Float = 100f, grade: Int = 0) = Font(
    R.font.roboto_flex,
    weight = FontWeight(weight),
    variationSettings = FontVariation.Settings(
        FontVariation.weight(weight),
        FontVariation.width(width),
        FontVariation.grade(grade),
    ),
)

/** Text: Roboto Flex at its usual width. */
val Flex = FontFamily(
    flex(400), flex(500), flex(600), flex(700), flex(800), flex(900),
)

private val Fraunces = FontFamily(
    Font(R.font.fraunces_semibold, FontWeight.SemiBold),
    Font(R.font.fraunces_bold, FontWeight.Bold),
    Font(R.font.fraunces_bold, FontWeight.ExtraBold),
    Font(R.font.fraunces_bold, FontWeight.Black),
)

/** The title font chosen in Settings › Appearance. */
val LocalTitleFont: ProvidableCompositionLocal<String> = compositionLocalOf { "flex" }

private val flexTitles = HashMap<Int, FontFamily>()

/**
 * The family for titles in `font`. For Roboto Flex, `squeeze` goes from 0 (the usual width)
 * to 1 (the narrowest), so a title can grow narrower as its page scrolls; the others keep
 * their one width.
 */
fun titleFamilyFor(font: String, squeeze: Float = 0.4f): FontFamily = when (font) {
    "fraunces" -> Fraunces
    "system" -> FontFamily.Default
    else -> {
        // Six steps are enough to look smooth, and each is loaded once.
        val step = (squeeze.coerceIn(0f, 1f) * 5).toInt()
        flexTitles.getOrPut(step) {
            val width = 100f - step * 10f
            FontFamily(flex(800, width), flex(900, width), flex(1000, width))
        }
    }
}

@Composable
fun titleFamily(squeeze: Float = 0f): FontFamily = titleFamilyFor(LocalTitleFont.current, squeeze)

private val Base = Typography()

private fun TextUnit.times(k: Float): TextUnit = if (isSp) (value * k).sp else this

private fun TextStyle.scaled(k: Float) = copy(fontSize = fontSize.times(k), lineHeight = lineHeight.times(k))

private fun TextStyle.text(k: Float) = copy(fontFamily = Flex).scaled(k)

private fun TextStyle.title(font: String, weight: Int, tight: Double, k: Float) = copy(
    fontFamily = titleFamilyFor(font),
    // Fraunces is at its best a step lighter.
    fontWeight = FontWeight(if (font == "fraunces") minOf(weight, 700) else weight),
    // In sp, as Material's own styles are: a text field blends one style into another, and
    // it cannot blend em into sp.
    letterSpacing = (if (font == "flex") tight * fontSize.value else 0.0).sp,
).scaled(k)

/**
 * Needle's type: Material's sizes, in Roboto Flex; the large sizes in the chosen title font
 * (narrow and heavy by default); everything scaled by the chosen text size.
 */
fun needleType(font: String = "flex", scale: Float = 1f): Typography {
    val k = scale.coerceIn(0.8f, 1.4f)
    return Typography(
        displayLarge = Base.displayLarge.title(font, 900, -0.03, k),
        displayMedium = Base.displayMedium.title(font, 900, -0.03, k),
        displaySmall = Base.displaySmall.title(font, 900, -0.02, k),
        headlineLarge = Base.headlineLarge.title(font, 900, -0.02, k),
        headlineMedium = Base.headlineMedium.title(font, 800, -0.015, k),
        headlineSmall = Base.headlineSmall.title(font, 800, -0.01, k),
        titleLarge = Base.titleLarge.text(k).copy(fontWeight = FontWeight.Bold),
        titleMedium = Base.titleMedium.text(k).copy(fontWeight = FontWeight.SemiBold),
        titleSmall = Base.titleSmall.text(k).copy(fontWeight = FontWeight.SemiBold),
        bodyLarge = Base.bodyLarge.text(k),
        bodyMedium = Base.bodyMedium.text(k),
        bodySmall = Base.bodySmall.text(k),
        labelLarge = Base.labelLarge.text(k).copy(fontWeight = FontWeight.SemiBold),
        labelMedium = Base.labelMedium.text(k).copy(fontWeight = FontWeight.SemiBold),
        labelSmall = Base.labelSmall.text(k).copy(fontWeight = FontWeight.SemiBold),
    )
}

val NeedleType = needleType()
