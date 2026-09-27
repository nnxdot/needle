package fyi.nnx.needle.ui

import androidx.compose.material3.Typography
import androidx.compose.ui.text.ExperimentalTextApi
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.Font
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontVariation
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.em
import androidx.compose.ui.unit.sp
import fyi.nnx.needle.R

/*
 * Roboto Flex: one variable font (SIL Open Font License; its text ships in
 * assets/licenses/RobotoFlex-OFL.txt) that can be any weight and any width.
 * Text uses it at its usual width; big titles use it narrow and heavy, as posters do.
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

/**
 * Titles: narrow and heavy. `squeeze` goes from 0 (the usual width) to 1 (the narrowest), so a
 * title can grow narrower as the page scrolls.
 */
private val titleFamilies = HashMap<Int, FontFamily>()
fun titleFamily(squeeze: Float = 0f): FontFamily {
    // Six steps are enough to look smooth, and each is loaded once.
    val step = (squeeze.coerceIn(0f, 1f) * 5).toInt()
    return titleFamilies.getOrPut(step) {
        val width = 100f - step * 10f
        FontFamily(flex(800, width), flex(900, width), flex(1000, width))
    }
}

private val Base = Typography()

private fun TextStyle.text() = copy(fontFamily = Flex)
private fun TextStyle.title(weight: Int, tight: Double) = copy(
    fontFamily = titleFamily(0.4f),
    fontWeight = FontWeight(weight),
    letterSpacing = tight.em,
)

/** Needle's type: Material's sizes, in Roboto Flex; the large sizes narrow and heavy. */
val NeedleType = Typography(
    displayLarge = Base.displayLarge.title(900, -0.03),
    displayMedium = Base.displayMedium.title(900, -0.03),
    displaySmall = Base.displaySmall.title(900, -0.02),
    headlineLarge = Base.headlineLarge.title(900, -0.02),
    headlineMedium = Base.headlineMedium.title(800, -0.015),
    headlineSmall = Base.headlineSmall.title(800, -0.01),
    titleLarge = Base.titleLarge.text().copy(fontWeight = FontWeight.Bold, letterSpacing = (-0.005).em),
    titleMedium = Base.titleMedium.text().copy(fontWeight = FontWeight.SemiBold),
    titleSmall = Base.titleSmall.text().copy(fontWeight = FontWeight.SemiBold),
    bodyLarge = Base.bodyLarge.text().copy(fontSize = 16.sp, letterSpacing = 0.01.em),
    bodyMedium = Base.bodyMedium.text(),
    bodySmall = Base.bodySmall.text(),
    labelLarge = Base.labelLarge.text().copy(fontWeight = FontWeight.SemiBold),
    labelMedium = Base.labelMedium.text().copy(fontWeight = FontWeight.SemiBold),
    labelSmall = Base.labelSmall.text().copy(fontWeight = FontWeight.SemiBold),
)
