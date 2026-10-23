package fyi.nnx.needle.ui

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.animateContentSize
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Check
import androidx.compose.material.icons.rounded.ExpandMore
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.rotate
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp

/*
 * The parts every Settings page is made of, as Android 16's own Settings: rows in rounded groups,
 * each with a small icon, a short name, one short line, and its control at the right. Colour is
 * kept for what is chosen: a thin accent line and a check.
 */

/** A row's small icon, on a quiet rounded square. */
@Composable
internal fun SettingIcon(icon: ImageVector?, tint: Color = MaterialTheme.colorScheme.onSurfaceVariant) {
    if (icon == null) return
    Box(
        Modifier.size(36.dp).clip(RoundedCornerShape(11.dp)).background(MaterialTheme.colorScheme.surfaceContainerHighest),
        contentAlignment = Alignment.Center,
    ) { Icon(icon, contentDescription = null, tint = tint, modifier = Modifier.size(20.dp)) }
}

/**
 * One setting: its icon, its name, one short line, and its control at the right. Large text
 * may wrap the line rather than cut it.
 */
@Composable
internal fun SettingRow(
    icon: ImageVector?,
    title: String,
    summary: String?,
    modifier: Modifier = Modifier,
    enabled: Boolean = true,
    titleColor: Color = Color.Unspecified,
    trailing: (@Composable () -> Unit)? = null,
) {
    Row(
        modifier
            .fillMaxWidth()
            .heightIn(min = LocalRows.current.height + 4.dp)
            .padding(horizontal = 16.dp, vertical = LocalRows.current.pad),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        SettingIcon(icon)
        Column(Modifier.weight(1f)) {
            Text(
                title,
                style = MaterialTheme.typography.bodyLarge,
                color = when {
                    titleColor != Color.Unspecified -> titleColor
                    enabled -> MaterialTheme.colorScheme.onSurface
                    else -> MaterialTheme.colorScheme.onSurfaceVariant
                },
            )
            if (!summary.isNullOrBlank()) {
                Text(summary, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant, maxLines = 2, overflow = TextOverflow.Ellipsis)
            }
        }
        trailing?.invoke()
    }
}

/**
 * A row that opens, with an arrow: closed, it shows its current choice at the right ([value]);
 * open, what is under it ([content]), such as a form or a longer explanation.
 */
@Composable
internal fun ExpandRow(
    icon: ImageVector?,
    title: String,
    summary: String?,
    value: String? = null,
    key: String = title,
    content: @Composable ColumnScope.(close: () -> Unit) -> Unit,
) {
    var open by rememberSaveable(key) { mutableStateOf(false) }
    val turn by animateFloatAsState(if (open) 180f else 0f, label = "open")
    Column(Modifier.animateContentSize()) {
        SettingRow(
            icon, title, summary,
            Modifier
                .clickable(role = Role.Button, onClickLabel = if (open) "Close" else "Open") { open = !open }
                .semantics { stateDescription = if (open) "Open" else "Closed" },
        ) {
            if (!open && value != null) {
                Text(
                    value,
                    style = MaterialTheme.typography.labelLarge,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                    modifier = Modifier.widthIn(max = 140.dp),
                )
            }
            Icon(Icons.Rounded.ExpandMore, contentDescription = null, tint = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.rotate(turn))
        }
        AnimatedVisibility(open) {
            // Under the row's words, clear of its icon.
            Column(Modifier.fillMaxWidth().padding(start = if (icon != null) 68.dp else 16.dp, end = 16.dp, bottom = 16.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
                content { open = false }
            }
        }
    }
}

/** One choice of a [SketchRow]: its name, and a small grey sketch with the changing part in [accent]. */
internal class Sketch(val id: String, val label: String, val draw: @Composable (accent: Color, grey: Color) -> Unit)

/**
 * A choice of layout (the cover's shape, the buttons, the rows): its name and line, and under
 * them each option as a small grey sketch where only the part that changes is in the accent.
 * The chosen one has a thin accent line and a check.
 */
@Composable
internal fun SketchRow(icon: ImageVector?, title: String, summary: String?, options: List<Sketch>, current: String, onChange: (String) -> Unit) {
    Column {
        SettingRow(icon, title, summary)
        FlowRow(
            Modifier.fillMaxWidth().padding(start = if (icon != null) 68.dp else 16.dp, end = 16.dp, bottom = 14.dp),
            horizontalArrangement = Arrangement.spacedBy(10.dp),
            verticalArrangement = Arrangement.spacedBy(10.dp),
        ) {
            options.forEach { option ->
                val chosen = option.id == current
                val accent = MaterialTheme.colorScheme.primary
                val grey = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.35f)
                Column(
                    Modifier
                        .clip(RoundedCornerShape(12.dp))
                        .selectable(chosen, role = Role.RadioButton) { onChange(option.id) }
                        .padding(2.dp),
                    horizontalAlignment = Alignment.CenterHorizontally,
                    verticalArrangement = Arrangement.spacedBy(6.dp),
                ) {
                    Box(
                        Modifier
                            .size(width = 76.dp, height = 52.dp)
                            .clip(RoundedCornerShape(10.dp))
                            .background(MaterialTheme.colorScheme.surfaceContainerHighest)
                            .border(if (chosen) 2.dp else 1.dp, if (chosen) accent else MaterialTheme.colorScheme.outlineVariant, RoundedCornerShape(10.dp)),
                        contentAlignment = Alignment.Center,
                    ) {
                        option.draw(accent, grey)
                        if (chosen) ChosenMark(Modifier.align(Alignment.TopEnd).padding(4.dp))
                    }
                    Text(
                        option.label,
                        style = MaterialTheme.typography.labelMedium,
                        color = if (chosen) MaterialTheme.colorScheme.onSurface else MaterialTheme.colorScheme.onSurfaceVariant,
                        fontWeight = if (chosen) FontWeight.SemiBold else FontWeight.Normal,
                        maxLines = 1,
                    )
                }
            }
        }
    }
}

/** The small check on what is chosen. */
@Composable
internal fun ChosenMark(modifier: Modifier = Modifier) {
    Box(modifier.size(16.dp).clip(CircleShape).background(MaterialTheme.colorScheme.primary), contentAlignment = Alignment.Center) {
        Icon(Icons.Rounded.Check, contentDescription = null, tint = MaterialTheme.colorScheme.onPrimary, modifier = Modifier.size(12.dp))
    }
}

/** A grey (or accent) bar of a sketch. */
@Composable
internal fun SketchBar(width: Int, height: Int, color: Color, round: Int = 2) {
    Box(Modifier.size(width = width.dp, height = height.dp).clip(RoundedCornerShape(round.dp)).background(color))
}

/** A theme as a small picture of a page: its page colour, a bar, three tiles, and its accent. */
internal class LookCard(val id: String, val name: String, val detail: String, val page: Color, val panel: Color, val ink: Color, val accent: Color)

/** The theme cards of Settings › Appearance › Look and the welcome guide: two to a row on a phone. */
@Composable
internal fun LookCards(cards: List<LookCard>, current: String, onChoose: (String) -> Unit) {
    // As many columns as fit (two at least, four at most), each card the same width.
    androidx.compose.foundation.layout.BoxWithConstraints(Modifier.fillMaxWidth().padding(16.dp)) {
    val columns = ((maxWidth + 12.dp) / 132.dp).toInt().coerceIn(2, 4)
    val cardWidth = (maxWidth - 12.dp * (columns - 1)) / columns
    FlowRow(
        Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.spacedBy(12.dp),
        verticalArrangement = Arrangement.spacedBy(14.dp),
    ) {
        cards.forEach { card ->
            val chosen = card.id == current
            Column(
                Modifier
                    .width(cardWidth)
                    .clip(RoundedCornerShape(16.dp))
                    .selectable(chosen, role = Role.RadioButton) { onChoose(card.id) },
                verticalArrangement = Arrangement.spacedBy(6.dp),
            ) {
                Box(
                    Modifier
                        .fillMaxWidth()
                        .height(76.dp)
                        .clip(RoundedCornerShape(14.dp))
                        .background(card.page)
                        .border(if (chosen) 2.dp else 1.dp, if (chosen) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.outlineVariant, RoundedCornerShape(14.dp))
                        .padding(10.dp),
                ) {
                    Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
                        SketchBar(34, 4, card.ink.copy(alpha = 0.8f))
                        Row(horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                            repeat(3) { Box(Modifier.size(width = 24.dp, height = 18.dp).clip(RoundedCornerShape(4.dp)).background(card.panel)) }
                        }
                        SketchBar(22, 6, card.accent, round = 3)
                    }
                    if (chosen) ChosenMark(Modifier.align(Alignment.TopEnd))
                }
                Column(Modifier.padding(horizontal = 4.dp)) {
                    Text(card.name, style = MaterialTheme.typography.labelLarge, fontWeight = if (chosen) FontWeight.SemiBold else FontWeight.Normal, maxLines = 1, overflow = TextOverflow.Ellipsis)
                    Text(card.detail, style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.onSurfaceVariant, maxLines = 1, overflow = TextOverflow.Ellipsis)
                }
            }
        }
    }
    }
}

/** Needle's three looks, as cards. */
internal val BuiltInLooks = listOf(
    LookCard("night", "Night", "Charcoal grey", Color(0xFF141312), Color(0xFF2B2927), Color(0xFFF1EFED), Color(0xFFE2B46C)),
    LookCard("midnight", "Midnight", "True black", Color.Black, Color(0xFF1E1E1E), Color(0xFFF1EFED), Color(0xFFE2B46C)),
    LookCard("day", "Day", "Light and airy", Color(0xFFFBFAF8), Color(0xFFE8E4DF), Color(0xFF1C1B1A), Color(0xFF8A5A12)),
)

/** A theme of your own as a card, from the colours it sets, else from the look it is made on. */
internal fun lookCard(theme: fyi.nnx.needle.core.ThemeInfo): LookCard {
    val base = BuiltInLooks.firstOrNull { it.id == theme.base || (theme.base == "dark" && it.id == "night") || (theme.base == "light" && it.id == "day") } ?: BuiltInLooks[0]
    fun color(slot: String, fallback: Color) = theme.colors[slot]?.let { hex ->
        runCatching { Color(android.graphics.Color.parseColor(hex)) }.getOrNull()
    } ?: fallback
    return LookCard(
        theme.id, theme.name, if (theme.readOnly) "From a plugin" else "Yours",
        color("page", base.page), color("panel", base.panel), color("text", base.ink), color("accent", base.accent),
    )
}
