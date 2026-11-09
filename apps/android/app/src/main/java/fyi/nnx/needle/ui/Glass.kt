package fyi.nnx.needle.ui

import android.os.Build
import androidx.annotation.RequiresApi
import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.BottomSheetDefaults
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.SheetState
import androidx.compose.material3.SheetValue
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.SideEffect
import androidx.compose.runtime.compositionLocalOf
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawWithCache
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.graphics.addOutline
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.clipPath
import androidx.compose.ui.graphics.luminance
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.DpOffset
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.DialogProperties
import androidx.compose.ui.window.DialogWindowProvider
import dev.chrisbanes.haze.HazeInput
import dev.chrisbanes.haze.HazeState
import dev.chrisbanes.haze.blur.HazeBlurStyle
import dev.chrisbanes.haze.blur.HazeColorEffect
import dev.chrisbanes.haze.blur.hazeBlur
import fyi.nnx.needle.NeedleApp

/*
 * Glass, the Needle look's one material for what floats (DESIGN.md › Glass): the dock, a tablet's
 * player, side pane, and side bar, the sheets, the menus, and the dialogs. All of it is drawn here,
 * so every part matches:
 * - a fill of the page's raised colour, see-through where what is under it is blurred;
 * - for what plays, the cover's colour coming in from the left;
 * - the edge light: a brighter line along the top that fades down the sides, and a faint outline;
 * - a soft shadow.
 * Android 12 and newer blur what is under it (Haze over the pages; the system's blur behind a
 * sheet's or a dialog's own window). Before Android 12, on a phone with little memory, and for the
 * big panes on a TV, the fill is solid.
 */

/**
 * What the floating glass blurs: the pages under it (`hazeSource` in Root.kt). Null where
 * nothing is captured; the glass is then a solid fill.
 */
val LocalHaze = compositionLocalOf<HazeState?> { null }

/** Real blur behind glass needs Android 12 (RenderEffect); before it, a solid fill. */
internal val canBlur = Build.VERSION.SDK_INT >= Build.VERSION_CODES.S

/**
 * Whether glass here may blur: Android 12 or newer, and not a phone with little memory. A big
 * pane (`large`: the side pane, the page's top edge, a sheet, a dialog) does not blur on a TV,
 * whose chips are often slow; the small floating player still does.
 */
@Composable
fun blurAllowed(large: Boolean = false): Boolean {
    val context = LocalContext.current
    val tv = isTv()
    val lowRam = remember {
        context.getSystemService(android.app.ActivityManager::class.java)?.isLowRamDevice == true
    }
    return canBlur && !lowRam && !(large && tv)
}

/** The page is dark (Night, Midnight, a dark cover's scheme). */
@Composable
internal fun darkPage(): Boolean = MaterialTheme.colorScheme.background.luminance() < 0.5f

/**
 * Glass's edge, as on the computer: a 1 dp line, brighter along the top and fading down the
 * sides, and a faint outline the rest of the way round. Drawn inside `shape`, over the content.
 * `dark`: the glass is dark (a dark page, or the side pane's dark glass in any theme).
 */
fun Modifier.edgeLight(shape: Shape, dark: Boolean): Modifier = drawWithCache {
    val outline = shape.createOutline(size, layoutDirection, this)
    val path = Path().apply { addOutline(outline) }
    val top = if (dark) Color.White.copy(alpha = 0.24f) else Color.White.copy(alpha = 0.9f)
    val faint = if (dark) Color.White.copy(alpha = 0.07f) else Color.Black.copy(alpha = 0.08f)
    // The light runs down about 28 dp, so short bars and tall panes look alike.
    val reach = if (size.height > 0f) (28.dp.toPx() / size.height).coerceIn(0.05f, 1f) else 1f
    val brush = Brush.verticalGradient(0f to top, reach to faint, 1f to faint)
    // Twice as wide, half of it clipped away: the line sits just inside the edge.
    val stroke = Stroke(2.dp.toPx())
    onDrawWithContent {
        drawContent()
        clipPath(path) { drawPath(path, brush, style = stroke) }
    }
}

/**
 * The edge light's top alone (see [edgeLight]), for a part that sits at the top of a taller
 * glass shape: a sheet's drag handle row, whose sheet runs on below it. `shape` is the sheet's.
 */
fun Modifier.topLight(shape: Shape, dark: Boolean): Modifier = drawWithCache {
    val tall = androidx.compose.ui.geometry.Size(size.width, size.height + 400.dp.toPx())
    val path = Path().apply { addOutline(shape.createOutline(tall, layoutDirection, this@drawWithCache)) }
    val top = if (dark) Color.White.copy(alpha = 0.24f) else Color.White.copy(alpha = 0.9f)
    val faint = if (dark) Color.White.copy(alpha = 0.07f) else Color.Black.copy(alpha = 0.08f)
    val brush = Brush.verticalGradient(0f to top, (28.dp.toPx() / tall.height) to faint, 1f to faint, endY = tall.height)
    val stroke = Stroke(2.dp.toPx())
    onDrawWithContent {
        drawContent()
        clipPath(path) { drawPath(path, brush, style = stroke) }
    }
}

/** The cover's colour for glass, from the left: stronger on a dark page, where it shows less. */
private fun coverBrush(cover: Color, dark: Boolean): Brush =
    Brush.horizontalGradient(0f to cover.copy(alpha = cover.alpha * if (dark) 0.42f else 0.3f), 0.8f to cover.copy(alpha = 0f))

/**
 * Frosted glass (the dock, a tablet's player and side bar): the page behind it blurred and tinted
 * with the page's raised colour, with `cover`'s colour from the left when given, the edge light,
 * and a soft shadow. Where it cannot blur (see [blurAllowed]), the raised colour (or `solid`),
 * solid.
 */
@Composable
fun Modifier.glass(shape: Shape, cover: Color? = null, large: Boolean = false, shadow: Dp = 18.dp, solid: Color? = null): Modifier {
    val scheme = MaterialTheme.colorScheme
    val dark = darkPage()
    val haze = LocalHaze.current
    val blur = haze != null && blurAllowed(large)
    val fill = scheme.surfaceContainer.copy(alpha = if (dark) 0.62f else 0.72f)
    val back = scheme.background
    val style = remember(fill, back, cover, dark) {
        HazeBlurStyle {
            blurRadius(28.dp)
            noiseFactor(0.06f)
            backgroundColor(back)
            colorEffects(listOfNotNull(HazeColorEffect.tint(fill), cover?.let { HazeColorEffect.tint(coverBrush(it, dark)) }))
        }
    }
    return this
        .then(if (shadow > 0.dp) Modifier.shadow(shadow, shape, ambientColor = Color.Black.copy(alpha = 0.35f), spotColor = Color.Black.copy(alpha = 0.35f)) else Modifier)
        .clip(shape)
        .then(
            if (blur) {
                Modifier.hazeBlur(HazeInput.Sources(haze), style)
            } else {
                Modifier
                    .background(solid ?: scheme.surfaceContainerHigh)
                    .then(if (cover != null) Modifier.background(coverBrush(cover, dark)) else Modifier)
            },
        )
        .edgeLight(shape, dark)
}

/**
 * Dark glass in a cover's colour, for a tablet's side pane (lyrics, Up next), whose white words
 * read on it in any theme: the page behind blurred, the cover's deep colour over it, darker
 * toward the foot. Only where it can blur ([blurAllowed] for a big pane, and a captured page);
 * elsewhere the pane keeps the player's solid backdrop.
 */
@Composable
fun Modifier.coverGlass(shape: Shape, cover: Color): Modifier {
    val haze = LocalHaze.current ?: return this
    val night = Color(0xFF0F0E0D)
    val style = remember(cover) {
        HazeBlurStyle {
            blurRadius(36.dp)
            noiseFactor(0.05f)
            backgroundColor(night)
            colorEffects(listOf(HazeColorEffect.tint(Brush.verticalGradient(listOf(cover.copy(alpha = 0.8f), night.copy(alpha = 0.88f))))))
        }
    }
    return this
        .shadow(18.dp, shape, ambientColor = Color.Black.copy(alpha = 0.4f), spotColor = Color.Black.copy(alpha = 0.4f))
        .clip(shape)
        .hazeBlur(HazeInput.Sources(haze), style)
        .edgeLight(shape, dark = true)
}

/**
 * The playing cover's colour for the glass under the dock and a tablet's player, or null: only
 * with Settings › Appearance › "Mini player in the song's colour", as the player on it takes it.
 * A middle tone, so it shows on Night and on Day alike; a change of song blends over.
 */
@Composable
fun playingGlassCover(): Color? {
    val app by NeedleApp.instance.app.collectAsState()
    val playback = NeedleApp.instance.playback.collectAsState()
    val art by remember { derivedStateOf { playback.value?.current?.artwork } }
    if (!app.miniPlayerColored) return null
    return glassCover(art)
}

/** A cover's colour for glass (see [playingGlassCover]); clear while there is none. */
@Composable
fun glassCover(path: String?): Color {
    val context = LocalContext.current
    val dark = darkPage()
    var seed by remember { mutableStateOf<Int?>(null) }
    LaunchedEffect(path) { seed = path?.let { coverSeed(context, it) } }
    val target = seed?.let {
        val hct = com.materialkolor.hct.Hct.fromInt(it)
        Color(com.materialkolor.hct.Hct.from(hct.hue, hct.chroma, if (dark) 45.0 else 70.0).toInt())
    } ?: Color.Transparent
    return animateColorAsState(target, tween(400), label = "glass cover").value
}

/**
 * The soft edge at the top of a page: what scrolls up blurs more and more and fades into the
 * page's colour before it goes under the status bar (or the top of a tablet's sheet), rather than
 * stopping at a hard line. `solid` is how far down the page's colour is solid (where the pages
 * stop: the status bar), `fade` how far below that it reaches. On a TV, or before Android 12, the
 * fade alone.
 */
@Composable
fun TopEdge(solid: Dp, fade: Dp, modifier: Modifier = Modifier) {
    val page = MaterialTheme.colorScheme.background
    val haze = LocalHaze.current
    val blur = haze != null && blurAllowed(large = true)
    val height = solid + fade
    val from = if (height > 0.dp) solid / height else 0f
    val brush = Brush.verticalGradient(
        0f to page,
        from to page,
        (from + (1f - from) * 0.45f) to page.copy(alpha = 0.6f),
        1f to page.copy(alpha = 0f),
    )
    val style = remember(page, from) {
        HazeBlurStyle {
            blurRadius(16.dp)
            noiseFactor(0f)
            backgroundColor(page)
            // The blur itself fades out toward the foot, so it has no edge of its own.
            mask(Brush.verticalGradient(0f to Color.Black, from to Color.Black, (from + (1f - from) * 0.55f) to Color.Transparent))
            colorEffects(emptyList())
        }
    }
    Box(
        modifier
            .fillMaxWidth()
            .height(height)
            .then(if (blur) Modifier.hazeBlur(HazeInput.Sources(haze), style) else Modifier)
            .background(brush),
    )
}

/**
 * The system's blur of what is behind this window (a sheet's or a dialog's), for glass in it:
 * Android 12 and newer, where the system has it turned on (not in battery saver, not on a TV or a
 * phone with little memory). It comes in as the window opens and goes as `shown` turns false.
 * Returns whether it is on, so the glass can be clearer.
 */
@Composable
fun windowBlur(shown: Boolean = true): Boolean {
    val allowed = blurAllowed(large = true)
    val view = LocalView.current
    val window = remember(view) {
        generateSequence(view.parent) { it.parent }.filterIsInstance<DialogWindowProvider>().firstOrNull()?.window
    }
    if (!allowed || window == null || Build.VERSION.SDK_INT < Build.VERSION_CODES.S) return false
    return windowBlur31(window, shown)
}

@RequiresApi(Build.VERSION_CODES.S)
@Composable
private fun windowBlur31(window: android.view.Window, shown: Boolean): Boolean {
    val manager = remember(window) { window.windowManager }
    var enabled by remember { mutableStateOf(manager.isCrossWindowBlurEnabled) }
    DisposableEffect(manager) {
        val listener = java.util.function.Consumer<Boolean> { enabled = it }
        manager.addCrossWindowBlurEnabledListener(listener)
        onDispose { manager.removeCrossWindowBlurEnabledListener(listener) }
    }
    val radius = with(LocalDensity.current) { 22.dp.toPx() }
    // Read only by the window below, so its frames do not compose the sheet or dialog again.
    val amount = animateFloatAsState(if (enabled && shown) 1f else 0f, tween(if (shown) 260 else 180), label = "window blur")
    val flags = android.view.WindowManager.LayoutParams.FLAG_BLUR_BEHIND or android.view.WindowManager.LayoutParams.FLAG_DIM_BEHIND
    // How the window dimmed the page before (a dialog does; a sheet draws its own shade).
    val plain = remember(window) { window.attributes.let { (it.flags and android.view.WindowManager.LayoutParams.FLAG_DIM_BEHIND != 0) to it.dimAmount } }
    LaunchedEffect(window, enabled) {
        val (dimmed, dim) = plain
        window.attributes = window.attributes.apply {
            if (enabled) {
                // The system draws the blur with the window's dim, and not at all with no dim: a
                // faint one at least. The page shows through the blur: a lighter shade than a
                // dialog's plain dim.
                this.flags = this.flags or flags
                dimAmount = if (dimmed) minOf(dim, 0.3f) else 0.04f
            } else {
                this.flags = if (dimmed) this.flags and android.view.WindowManager.LayoutParams.FLAG_BLUR_BEHIND.inv() else this.flags and flags.inv()
                dimAmount = dim
            }
        }
        snapshotFlow { amount.value }.collect { a ->
            window.attributes = window.attributes.apply { blurBehindRadius = (a * radius).toInt() }
        }
    }
    return enabled
}

/** How see-through a sheet's or a dialog's glass is, over the blurred page; solid without it. */
@Composable
private fun windowFill(base: Color, blurred: Boolean): Color {
    val alpha by animateFloatAsState(if (blurred) (if (darkPage()) 0.84f else 0.88f) else 1f, tween(200), label = "glass fill")
    return base.copy(alpha = alpha)
}

/**
 * A sheet from the bottom on glass: the page behind it blurred where the system allows (see
 * [windowBlur]), the sheet's fill clear enough to show it, and the edge light along its top.
 * Elsewhere, the sheet's usual solid fill with the edge light.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun GlassSheet(
    onDismissRequest: () -> Unit,
    sheetState: SheetState = rememberModalBottomSheetState(),
    content: @Composable ColumnScope.() -> Unit,
) {
    var blurred by remember { mutableStateOf(false) }
    val shape = BottomSheetDefaults.ExpandedShape
    ModalBottomSheet(
        onDismissRequest = onDismissRequest,
        sheetState = sheetState,
        shape = shape,
        containerColor = windowFill(MaterialTheme.colorScheme.surfaceContainerLow, blurred),
        // The edge light along the sheet's top, drawn with the handle: the sheet's own outer box
        // is not where the sheet shows as it slides.
        dragHandle = {
            Box(Modifier.fillMaxWidth().topLight(shape, darkPage()), contentAlignment = androidx.compose.ui.Alignment.Center) {
                BottomSheetDefaults.DragHandle()
            }
        },
    ) {
        val on = windowBlur(sheetState.targetValue != SheetValue.Hidden)
        SideEffect { blurred = on }
        content()
    }
}

/**
 * Material's dialog on glass (see [GlassSheet]): the page behind it blurred where the system
 * allows, its fill clear enough to show it, and the edge light.
 */
@Composable
fun GlassAlertDialog(
    onDismissRequest: () -> Unit,
    confirmButton: @Composable () -> Unit,
    modifier: Modifier = Modifier,
    dismissButton: (@Composable () -> Unit)? = null,
    icon: (@Composable () -> Unit)? = null,
    title: (@Composable () -> Unit)? = null,
    text: (@Composable () -> Unit)? = null,
    properties: DialogProperties = DialogProperties(),
) {
    var blurred by remember { mutableStateOf(false) }
    val shape = RoundedCornerShape(28.dp)
    AlertDialog(
        onDismissRequest = onDismissRequest,
        // The blur is set from inside the dialog's own window: the button row is always there.
        confirmButton = {
            val on = windowBlur()
            SideEffect { blurred = on }
            confirmButton()
        },
        modifier = modifier.edgeLight(shape, darkPage()),
        dismissButton = dismissButton,
        icon = icon,
        title = title,
        text = text,
        shape = shape,
        containerColor = windowFill(MaterialTheme.colorScheme.surfaceContainerHigh, blurred),
        properties = properties,
    )
}

/** A menu's corners (sort, view, and the like). */
val MenuShape = RoundedCornerShape(18.dp)

/**
 * A small dropdown menu with the glass's edge light. A menu is its own small window that the
 * page's blur does not reach, and the system's blur behind it would blur the whole screen for a
 * few rows, so its fill stays solid.
 */
@Composable
fun GlassMenu(
    expanded: Boolean,
    onDismissRequest: () -> Unit,
    modifier: Modifier = Modifier,
    offset: DpOffset = DpOffset(0.dp, 0.dp),
    content: @Composable ColumnScope.() -> Unit,
) {
    DropdownMenu(
        expanded = expanded,
        onDismissRequest = onDismissRequest,
        modifier = modifier.edgeLight(MenuShape, darkPage()),
        offset = offset,
        shape = MenuShape,
        containerColor = MaterialTheme.colorScheme.surfaceContainerHigh,
        content = content,
    )
}
