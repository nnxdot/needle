package fyi.nnx.needle.ui

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.ArrowBack
import androidx.compose.material.icons.automirrored.rounded.QueueMusic
import androidx.compose.material.icons.rounded.AutoAwesome
import androidx.compose.material.icons.rounded.Build
import androidx.compose.material.icons.rounded.Extension
import androidx.compose.material.icons.rounded.ExpandMore
import androidx.compose.material.icons.rounded.Fullscreen
import androidx.compose.material.icons.rounded.Language
import androidx.compose.material.icons.rounded.Lyrics
import androidx.compose.material.icons.rounded.Menu
import androidx.compose.material.icons.rounded.PictureInPicture
import androidx.compose.material.icons.rounded.ViewAgenda
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.rotate
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.LinkAnnotation
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.TextLinkStyles
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.fromHtml
import androidx.compose.ui.unit.dp

// ---------- What's new

/** One line of the release notes: the lead story, a feature with its icon, or a fix. */
private sealed interface NoteLine {
    data class Lead(val title: String, val text: String) : NoteLine
    data class Feature(val icon: ImageVector, val title: String, val text: String) : NoteLine
    data class Fix(val text: String) : NoteLine
}

private data class Release(val version: String, val tagline: String?, val lines: List<NoteLine>)

/** The desktop's icon names, as the nearest Material icons. */
private fun noteIcon(name: String): ImageVector = when (name) {
    "globe" -> Icons.Rounded.Language
    "panel" -> Icons.Rounded.ViewAgenda
    "menu" -> Icons.Rounded.Menu
    "chevron-left" -> Icons.AutoMirrored.Rounded.ArrowBack
    "mini" -> Icons.Rounded.PictureInPicture
    "fullscreen" -> Icons.Rounded.Fullscreen
    "lyrics" -> Icons.Rounded.Lyrics
    "playlist" -> Icons.AutoMirrored.Rounded.QueueMusic
    "plugin" -> Icons.Rounded.Extension
    else -> Icons.Rounded.AutoAwesome
}

/** "**Title.** words" → the title and the words. */
private fun split(text: String): Pair<String, String> {
    val m = Regex("^\\*\\*(.+?)\\*\\*\\s*(.*)$").find(text) ?: return "" to text
    return m.groupValues[1].trimEnd('.') to m.groupValues[2]
}

private fun parse(version: String, text: String): Release {
    var tagline: String? = null
    val lines = mutableListOf<NoteLine>()
    text.lines().map { it.trim() }.filter { it.isNotEmpty() }.forEach { line ->
        when {
            line.startsWith("! ") -> split(line.removePrefix("! ")).let { (t, w) -> lines += NoteLine.Lead(t, w) }
            line.startsWith("- [") -> {
                val icon = line.substringAfter("[").substringBefore("]")
                val (t, w) = split(line.substringAfter("]").trim())
                lines += if (t.isEmpty()) NoteLine.Fix(w) else NoteLine.Feature(noteIcon(icon), t, w)
            }
            line.startsWith("- ") -> split(line.removePrefix("- ")).let { (t, w) ->
                lines += if (t.isEmpty()) NoteLine.Fix(w) else NoteLine.Feature(Icons.Rounded.AutoAwesome, t, w)
            }
            tagline == null && lines.isEmpty() -> tagline = line
            else -> lines += NoteLine.Fix(line)
        }
    }
    return Release(version, tagline, lines)
}

/**
 * What's new, told like a magazine: each version's name large, its lead story on a coloured
 * card, its features each with an icon in the Needle shape, and its smaller fixes folded away.
 * The newest version is open; older ones open with a tap.
 */
@Composable
fun WhatsNewPage() {
    val releases = remember { core.whatsNew().take(8).map { parse(it.version, it.text) } }
    var open by rememberSaveable { mutableStateOf(releases.firstOrNull()?.version) }
    LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(bottom = 32.dp + LocalBottomInset.current)) {
        item { LargeTitle("What's new") }
        releases.forEachIndexed { index, release ->
            item(key = release.version) {
                ReleaseCard(release, open == release.version, index == 0) {
                    open = if (open == release.version) null else release.version
                }
            }
        }
    }
}

@Composable
private fun ReleaseCard(release: Release, expanded: Boolean, newest: Boolean, onToggle: () -> Unit) {
    val turn by animateFloatAsState(if (expanded) 180f else 0f, label = "chevron")
    Column(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 6.dp)) {
        Row(
            Modifier.fillMaxWidth().clip(RoundedCornerShape(20.dp)).clickable(onClick = onToggle).padding(horizontal = 8.dp, vertical = 10.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Column(Modifier.weight(1f)) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Text(release.version, style = MaterialTheme.typography.headlineMedium)
                    if (newest) {
                        Text(
                            "Latest",
                            style = MaterialTheme.typography.labelMedium,
                            color = MaterialTheme.colorScheme.onPrimary,
                            modifier = Modifier.padding(start = 10.dp).clip(CircleShape).background(MaterialTheme.colorScheme.primary).padding(horizontal = 8.dp, vertical = 2.dp),
                        )
                    }
                }
                release.tagline?.let { Text(it, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant) }
            }
            Icon(Icons.Rounded.ExpandMore, contentDescription = if (expanded) "Fold" else "Open", modifier = Modifier.rotate(turn))
        }
        AnimatedVisibility(expanded) {
            Column(verticalArrangement = Arrangement.spacedBy(10.dp), modifier = Modifier.padding(top = 6.dp)) {
                release.lines.filterIsInstance<NoteLine.Lead>().forEach { lead ->
                    Column(
                        Modifier.fillMaxWidth().clip(RoundedCornerShape(28.dp)).background(MaterialTheme.colorScheme.primaryContainer).padding(20.dp),
                        verticalArrangement = Arrangement.spacedBy(8.dp),
                    ) {
                        ShapedIcon(Icons.Rounded.AutoAwesome, 44, MaterialTheme.colorScheme.primary, MaterialTheme.colorScheme.onPrimary)
                        Text(lead.title, style = MaterialTheme.typography.headlineSmall, color = MaterialTheme.colorScheme.onPrimaryContainer)
                        Text(lead.text, style = MaterialTheme.typography.bodyLarge, color = MaterialTheme.colorScheme.onPrimaryContainer)
                    }
                }
                release.lines.filterIsInstance<NoteLine.Feature>().forEach { feature ->
                    Row(
                        Modifier.fillMaxWidth().clip(RoundedCornerShape(20.dp)).background(MaterialTheme.colorScheme.surfaceContainer).padding(16.dp),
                        horizontalArrangement = Arrangement.spacedBy(14.dp),
                    ) {
                        ShapedIcon(feature.icon, 40)
                        Column(Modifier.weight(1f)) {
                            Text(feature.title, style = MaterialTheme.typography.titleMedium)
                            Text(feature.text, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                        }
                    }
                }
                val fixes = release.lines.filterIsInstance<NoteLine.Fix>()
                if (fixes.isNotEmpty()) {
                    Column(
                        Modifier.fillMaxWidth().clip(RoundedCornerShape(20.dp)).background(MaterialTheme.colorScheme.surfaceContainerLow).padding(16.dp),
                        verticalArrangement = Arrangement.spacedBy(8.dp),
                    ) {
                        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                            Icon(Icons.Rounded.Build, contentDescription = null, tint = MaterialTheme.colorScheme.primary, modifier = Modifier.size(18.dp))
                            Text("Fixes and smaller changes", style = MaterialTheme.typography.titleSmall)
                        }
                        fixes.forEach { fix ->
                            Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                                Box(Modifier.padding(top = 8.dp).size(5.dp).clip(CircleShape).background(MaterialTheme.colorScheme.onSurfaceVariant))
                                Text(fix.text, style = MaterialTheme.typography.bodyMedium)
                            }
                        }
                    }
                }
            }
        }
    }
}

// ---------- Privacy and Help, from the website's own pages

/** A piece of a page: a heading, a question and its answer, or a paragraph. */
private sealed interface DocBlock {
    data class Heading(val text: String) : DocBlock
    data class Sub(val text: String) : DocBlock
    data class Question(val question: String, val answer: String) : DocBlock
    data class Para(val html: String) : DocBlock
    data class Item(val html: String) : DocBlock
}

private fun strip(html: String) = html.replace(Regex("<[^>]+>"), "").replace("&amp;", "&").replace("&nbsp;", " ").trim()

/**
 * Reads the page's article into blocks. The pages are Needle's own, written by hand, so a
 * simple reading of their few tags is enough.
 */
private fun readDoc(html: String, skip: Set<String>): Pair<String, List<DocBlock>> {
    val body = html.substringAfter("<article", html).substringAfter(">").substringBefore("</article>")
    val title = strip(Regex("<h1[^>]*>(.*?)</h1>", RegexOption.DOT_MATCHES_ALL).find(body)?.groupValues?.get(1) ?: "")
    val blocks = mutableListOf<DocBlock>()
    var skipping = false
    val tag = Regex("<(h2|h3|dt|dd|p|li)[^>]*>(.*?)</\\1>", RegexOption.DOT_MATCHES_ALL)
    var question: String? = null
    tag.findAll(body.substringAfter("</h1>")).forEach { m ->
        val (name, inner) = m.destructured
        when (name) {
            "h2" -> {
                skipping = strip(inner) in skip
                if (!skipping) blocks += DocBlock.Heading(strip(inner))
            }
            "h3" -> if (!skipping) blocks += DocBlock.Sub(strip(inner))
            "dt" -> question = strip(inner)
            "dd" -> if (!skipping) blocks += DocBlock.Question(question ?: "", inner.trim()).also { question = null }
            "p" -> if (!skipping) blocks += DocBlock.Para(inner.trim())
            "li" -> if (!skipping) blocks += DocBlock.Item(inner.trim())
        }
    }
    return title to blocks
}

/** The website's words, with its links made whole. */
@Composable
private fun html(text: String): AnnotatedString {
    val link = TextLinkStyles(SpanStyle(color = MaterialTheme.colorScheme.primary, fontWeight = FontWeight.SemiBold))
    val whole = text
        .replace("href=\"/", "href=\"https://needle.nnx.fyi/")
        .replace("<code>", "<tt>").replace("</code>", "</tt>")
    return AnnotatedString.fromHtml(whole, linkStyles = link)
}

enum class Doc(val file: String, val title: String, val skip: Set<String>) {
    Privacy("privacy.html", "Privacy", emptySet()),
    // Installing is about computers; the phone app installs from its own page.
    Help("help.html", "Help", setOf("Installing")),
}

/** Privacy or Help, read from the website's own page that ships inside the app. */
@Composable
fun DocScreen(doc: Doc) {
    val context = LocalContext.current
    val read = remember(doc) {
        runCatching { context.assets.open("site/${doc.file}").bufferedReader().readText() }.getOrNull()?.let { readDoc(it, doc.skip) }
    }
    val (title, blocks) = read ?: ("" to emptyList())
    LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(bottom = 32.dp + LocalBottomInset.current)) {
        item { LargeTitle(doc.title) }
        if (title.isNotBlank()) {
            item { Text(title, style = MaterialTheme.typography.displaySmall, modifier = Modifier.padding(horizontal = Edge, vertical = 4.dp)) }
        }
        itemsIndexed(blocks) { _, block ->
            when (block) {
                is DocBlock.Heading -> Text(block.text, style = MaterialTheme.typography.headlineSmall, modifier = Modifier.padding(start = Edge, end = Edge, top = 28.dp, bottom = 8.dp))
                is DocBlock.Sub -> Text(block.text, style = MaterialTheme.typography.titleMedium, color = MaterialTheme.colorScheme.primary, modifier = Modifier.padding(start = Edge, end = Edge, top = 14.dp, bottom = 2.dp))
                is DocBlock.Para -> Text(html(block.html), style = MaterialTheme.typography.bodyLarge, modifier = Modifier.padding(horizontal = Edge, vertical = 6.dp))
                is DocBlock.Item -> Row(Modifier.padding(horizontal = Edge, vertical = 4.dp), horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                    Box(Modifier.padding(top = 9.dp).size(5.dp).clip(CircleShape).background(MaterialTheme.colorScheme.primary))
                    Text(html(block.html), style = MaterialTheme.typography.bodyLarge)
                }
                is DocBlock.Question -> Question(block.question, html(block.answer))
            }
        }
    }
}

/** A question that opens to its answer. */
@Composable
private fun Question(question: String, answer: AnnotatedString) {
    var open by rememberSaveable(question) { mutableStateOf(false) }
    val turn by animateFloatAsState(if (open) 180f else 0f, label = "chevron")
    Column(
        Modifier
            .fillMaxWidth()
            .padding(horizontal = 16.dp, vertical = 4.dp)
            .clip(RoundedCornerShape(20.dp))
            .background(MaterialTheme.colorScheme.surfaceContainer)
            .clickable { open = !open }
            .padding(16.dp),
    ) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Text(question, style = MaterialTheme.typography.titleMedium, modifier = Modifier.weight(1f))
            Icon(Icons.Rounded.ExpandMore, contentDescription = null, modifier = Modifier.rotate(turn))
        }
        AnimatedVisibility(open) {
            Text(answer, style = MaterialTheme.typography.bodyLarge, color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.padding(top = 10.dp))
        }
    }
}
