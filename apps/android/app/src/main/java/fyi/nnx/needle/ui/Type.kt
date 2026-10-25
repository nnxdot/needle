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
 * The fonts Needle carries, the same on every system (SIL Open Font License; each one's text
 * ships in assets/licenses). Titles: Poster (Roboto Flex, narrow and heavy, as posters are),
 * Fraunces, Instrument Serif, DM Serif Display, Space Grotesk, Bricolage Grotesque, and Outfit.
 * Text: Inter, Atkinson Hyperlegible, Lexend, and IBM Plex Mono. "system" is Android's own font
 * (Plain), and "own:<path>" a .ttf or .otf file chosen on the phone (Settings › Appearance).
 * The files come from assets/fonts, as every Needle's (copied in by Gradle, see copyFonts);
 * Poster on Android is Roboto Flex, narrow and heavy (res/font).
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
 * A family from Needle's static fonts (assets/fonts): each weight from 400 to 1000 takes the
 * nearest file at or below it (else the lightest), declared at that weight so nothing is
 * drawn bolder than the file is. A family with one weight is never made bold.
 */
private fun family(vararg files: Pair<Int, Int>): FontFamily {
    val sorted = files.sortedBy { it.first }
    return FontFamily((400..1000 step 100).map { w ->
        val file = sorted.lastOrNull { it.first <= w } ?: sorted.first()
        Font(file.second, FontWeight(w))
    })
}

private val Fraunces = family(600 to R.font.fraunces72ptsoft_semibold, 700 to R.font.fraunces72ptsoft_bold)
private val InstrumentSerif = family(400 to R.font.instrumentserif_400)
private val DmSerif = family(400 to R.font.dmserifdisplay_400)
private val SpaceGrotesk = family(600 to R.font.spacegrotesk_600, 700 to R.font.spacegrotesk_700)
private val Bricolage = family(600 to R.font.bricolagegrotesque_600, 800 to R.font.bricolagegrotesque_800)
private val Outfit = family(600 to R.font.outfit_600, 700 to R.font.outfit_700)
private val Inter = family(400 to R.font.inter_400, 500 to R.font.inter_500, 600 to R.font.inter_600, 700 to R.font.inter_700)
private val Atkinson = family(400 to R.font.atkinsonhyperlegible_400, 700 to R.font.atkinsonhyperlegible_700)
private val Lexend = family(400 to R.font.lexend_400, 500 to R.font.lexend_500, 600 to R.font.lexend_600, 700 to R.font.lexend_700)
private val PlexMono = family(400 to R.font.ibmplexmono_400, 700 to R.font.ibmplexmono_700)

private val ownFonts = HashMap<String, FontFamily>()

/** A font file chosen on the phone, read once; Android's own font if it cannot be read. */
private fun own(path: String): FontFamily = ownFonts.getOrPut(path) {
    runCatching {
        val file = java.io.File(path)
        if (!file.isFile) error("gone")
        FontFamily(androidx.compose.ui.text.font.Font(file))
    }.getOrDefault(FontFamily.Default)
}

/** The fonts for titles, as Settings › Appearance › Titles names them, Needle's order. */
val TitleFonts = listOf(
    "system" to "Plain", "poster" to "Poster", "fraunces" to "Fraunces", "instrument_serif" to "Instrument Serif",
    "dm_serif" to "DM Serif Display", "space_grotesk" to "Space Grotesk", "bricolage" to "Bricolage Grotesque", "outfit" to "Outfit",
)

/** The fonts for text, as Settings › Appearance › Text names them. */
val TextFonts = listOf(
    "system" to "Plain", "inter" to "Inter", "atkinson" to "Atkinson Hyperlegible", "lexend" to "Lexend", "plex_mono" to "IBM Plex Mono",
)

/** The title font's name for what is stored ("flex" from before 2.0 is Poster). */
fun titleFontId(stored: String): String = if (stored == "flex" || stored == "bahnschrift") "poster" else stored

/** The family for text in `font`. */
fun textFamilyFor(font: String): FontFamily = when {
    font == "inter" -> Inter
    font == "atkinson" -> Atkinson
    font == "lexend" -> Lexend
    font == "plex_mono" -> PlexMono
    font.startsWith("own:") -> own(font.removePrefix("own:"))
    else -> FontFamily.Default
}

/** The title font chosen in Settings › Appearance. */
val LocalTitleFont: ProvidableCompositionLocal<String> = compositionLocalOf { "flex" }

private val flexTitles = HashMap<Int, FontFamily>()

/**
 * The family for titles in `font`. For Roboto Flex, `squeeze` goes from 0 (the usual width)
 * to 1 (the narrowest), so a title can grow narrower as its page scrolls; the others keep
 * their one width.
 */
fun titleFamilyFor(font: String, squeeze: Float = 0.4f): FontFamily = when (titleFontId(font)) {
    "fraunces" -> Fraunces
    "system" -> FontFamily.Default
    "instrument_serif" -> InstrumentSerif
    "dm_serif" -> DmSerif
    "space_grotesk" -> SpaceGrotesk
    "bricolage" -> Bricolage
    "outfit" -> Outfit
    else -> if (font.startsWith("own:")) own(font.removePrefix("own:")) else {
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

private fun TextStyle.scaled(k: Float) = copy(fontSize = fontSize.times(k), lineHeight = lineHeight.times(k))

private fun TextStyle.text(k: Float, family: FontFamily) = copy(fontFamily = family).scaled(k)

private fun TextStyle.title(font: String, weight: Int, tight: Double, k: Float) = copy(
    fontFamily = titleFamilyFor(font),
    // Fraunces is at its best a step lighter.
    fontWeight = FontWeight(if (font == "fraunces") minOf(weight, 700) else weight),
    // In sp, as Material's own styles are: a text field blends one style into another, and
    // it cannot blend em into sp.
    letterSpacing = (if (titleFontId(font) == "poster") tight * fontSize.value else 0.0).sp,
).scaled(k)

/**
 * Needle's type: Material's sizes, in Roboto Flex; the large sizes in the chosen title font
 * (narrow and heavy by default); everything scaled by the chosen text size.
 */
@OptIn(androidx.compose.material3.ExperimentalMaterial3ExpressiveApi::class)
fun needleType(font: String = "poster", scale: Float = 1f, textFont: String = "system"): Typography {
    val k = scale.coerceIn(0.8f, 1.4f)
    val t = textFamilyFor(textFont)
    return Typography(
        displayLarge = Base.displayLarge.title(font, 900, -0.03, k),
        displayMedium = Base.displayMedium.title(font, 900, -0.03, k),
        displaySmall = Base.displaySmall.title(font, 900, -0.02, k),
        headlineLarge = Base.headlineLarge.title(font, 900, -0.02, k),
        headlineMedium = Base.headlineMedium.title(font, 800, -0.015, k),
        headlineSmall = Base.headlineSmall.title(font, 800, -0.01, k),
        titleLarge = Base.titleLarge.text(k, t).copy(fontWeight = FontWeight.Bold),
        titleMedium = Base.titleMedium.text(k, t).copy(fontWeight = FontWeight.SemiBold),
        titleSmall = Base.titleSmall.text(k, t).copy(fontWeight = FontWeight.SemiBold),
        bodyLarge = Base.bodyLarge.text(k, t),
        bodyMedium = Base.bodyMedium.text(k, t),
        bodySmall = Base.bodySmall.text(k, t),
        labelLarge = Base.labelLarge.text(k, t).copy(fontWeight = FontWeight.SemiBold),
        labelMedium = Base.labelMedium.text(k, t).copy(fontWeight = FontWeight.SemiBold),
        labelSmall = Base.labelSmall.text(k, t).copy(fontWeight = FontWeight.SemiBold),
        // Material 3 Expressive's emphasized styles: a step heavier, for what a page leads with
        // (section titles, the main button). Large ones in the title font, as the others.
        displayLargeEmphasized = Base.displayLarge.title(font, 1000, -0.03, k),
        displayMediumEmphasized = Base.displayMedium.title(font, 1000, -0.03, k),
        displaySmallEmphasized = Base.displaySmall.title(font, 1000, -0.02, k),
        headlineLargeEmphasized = Base.headlineLarge.title(font, 1000, -0.02, k),
        headlineMediumEmphasized = Base.headlineMedium.title(font, 900, -0.015, k),
        headlineSmallEmphasized = Base.headlineSmall.title(font, 900, -0.01, k),
        titleLargeEmphasized = Base.titleLarge.title(font, 800, -0.01, k),
        titleMediumEmphasized = Base.titleMedium.text(k, t).copy(fontWeight = FontWeight.Bold),
        titleSmallEmphasized = Base.titleSmall.text(k, t).copy(fontWeight = FontWeight.Bold),
        bodyLargeEmphasized = Base.bodyLarge.text(k, t).copy(fontWeight = FontWeight.Medium),
        bodyMediumEmphasized = Base.bodyMedium.text(k, t).copy(fontWeight = FontWeight.Medium),
        bodySmallEmphasized = Base.bodySmall.text(k, t).copy(fontWeight = FontWeight.Medium),
        labelLargeEmphasized = Base.labelLarge.text(k, t).copy(fontWeight = FontWeight.Bold),
        labelMediumEmphasized = Base.labelMedium.text(k, t).copy(fontWeight = FontWeight.Bold),
        labelSmallEmphasized = Base.labelSmall.text(k, t).copy(fontWeight = FontWeight.Bold),
    )
}

val NeedleType = needleType()
