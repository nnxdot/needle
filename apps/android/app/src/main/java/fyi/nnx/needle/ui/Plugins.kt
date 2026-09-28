package fyi.nnx.needle.ui

import androidx.compose.animation.animateContentSize
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.KeyboardArrowRight
import androidx.compose.material.icons.rounded.Verified
import androidx.compose.material3.Button
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Slider
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.unit.dp
import fyi.nnx.needle.NeedleApp
import fyi.nnx.needle.core.Effect
import fyi.nnx.needle.core.PluginItem
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.text.DateFormat
import java.util.Date

/** Reads the plugins again every second while `watch` holds, as they load and sync. */
@Composable
private fun rememberPlugins(): List<PluginItem>? {
    var list by remember { mutableStateOf<List<PluginItem>?>(null) }
    LaunchedEffect(Unit) {
        while (true) {
            list = withContext(Dispatchers.IO) { runCatching { core.plugins() }.getOrNull() }
            delay(1000)
        }
    }
    return list
}

@Composable
fun PluginsScreen(open: (Route) -> Unit) {
    val plugins = rememberPlugins()
    LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(bottom = 32.dp)) {
        item { LargeTitle("Plugins") }
        item {
            Text(
                "Plugins add music servers (Navidrome, Subsonic), lyrics from NetEase, sound effects, and commands. They run inside Needle, with only the permissions they list.",
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(horizontal = Edge, vertical = 4.dp),
            )
        }
        item {
            Row(Modifier.padding(horizontal = Edge, vertical = 12.dp), horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                FilledTonalButton(onClick = {
                    NeedleApp.instance.scope.launch {
                        val added = runCatching { core.installBundledPlugins() }.getOrNull()
                        showMessage(if ((added ?: 0u) > 0u) "Added ${count(added!!.toInt(), "plugin")}" else "The plugins that come with Needle are all here")
                    }
                }) { Text("Add Needle's plugins") }
            }
        }
        if (plugins != null && plugins.isEmpty()) {
            item { Text("No plugins yet.", color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.padding(horizontal = Edge)) }
        }
        items(plugins.orEmpty(), key = { it.id }) { plugin ->
            Column(Modifier.fillMaxWidth().clickable { open(Route.Plugin(plugin.id)) }) {
                Row(
                    Modifier.fillMaxWidth().heightIn(min = LocalRows.current.height + 4.dp).padding(start = Edge, end = 12.dp, top = LocalRows.current.pad / 1.5f, bottom = LocalRows.current.pad / 1.5f),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(12.dp),
                ) {
                    Column(Modifier.weight(1f)) {
                        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                            Text(plugin.name, style = MaterialTheme.typography.bodyLarge)
                            if (plugin.verified) Icon(Icons.Rounded.Verified, contentDescription = "Comes with Needle", tint = MaterialTheme.colorScheme.primary, modifier = Modifier.padding(0.dp))
                        }
                        Text(
                            plugin.error ?: plugin.description,
                            style = MaterialTheme.typography.bodyMedium,
                            color = if (plugin.error != null) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onSurfaceVariant,
                            maxLines = 2,
                        )
                    }
                    Switch(checked = plugin.enabled, onCheckedChange = { core.setPluginEnabled(plugin.id, it) })
                    Icon(Icons.AutoMirrored.Rounded.KeyboardArrowRight, contentDescription = null, tint = MaterialTheme.colorScheme.onSurfaceVariant)
                }
                HorizontalDivider(color = MaterialTheme.colorScheme.outlineVariant, modifier = Modifier.padding(start = Edge))
            }
        }
    }
}

@OptIn(androidx.compose.material3.ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun PluginScreen(id: String) {
    val plugins = rememberPlugins()
    val plugin = plugins?.firstOrNull { it.id == id } ?: return
    val fields = remember(id) { mutableStateMapOf<String, String>() }
    LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(bottom = 32.dp)) {
        item { LargeTitle(plugin.name) }
        item {
            Column(Modifier.padding(horizontal = Edge), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                Text(plugin.description, style = MaterialTheme.typography.bodyLarge)
                Text(
                    listOfNotNull(plugin.version.ifBlank { null }?.let { "Version $it" }, plugin.author.ifBlank { null }?.let { "by $it" }).joinToString(" · "),
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                plugin.error?.let { Text(it, color = MaterialTheme.colorScheme.error) }
            }
        }
        item {
            Section(title = "Turned on") {
                Row(Modifier.fillMaxWidth().padding(16.dp), verticalAlignment = Alignment.CenterVertically) {
                    Text(if (plugin.enabled) "On" else "Off", style = MaterialTheme.typography.bodyLarge, modifier = Modifier.weight(1f))
                    Switch(checked = plugin.enabled, onCheckedChange = { core.setPluginEnabled(plugin.id, it) })
                }
            }
        }
        if (plugin.permissions.isNotEmpty()) {
            item {
                Section(title = "It may") {
                    Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                        plugin.permissions.forEach { Text("• $it", style = MaterialTheme.typography.bodyMedium) }
                    }
                }
            }
        }
        plugin.source?.let { source ->
            item {
                Section(title = source.name) {
                    Column(Modifier.padding(16.dp).animateContentSize(), verticalArrangement = Arrangement.spacedBy(10.dp)) {
                        if (source.signedIn) {
                            Text("Signed in · ${count(source.songs.toInt(), "song")}", style = MaterialTheme.typography.bodyLarge)
                            source.syncedAt?.let {
                                Text(
                                    "Songs brought in ${DateFormat.getDateTimeInstance(DateFormat.MEDIUM, DateFormat.SHORT).format(Date(it * 1000))}",
                                    style = MaterialTheme.typography.bodyMedium,
                                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                                )
                            }
                            if (source.syncing) {
                                // The server sends its list in pages; the count grows as they come.
                                Text(
                                    if (source.received > 0u) "Bringing in the song list… ${count(source.received.toInt(), "song")} so far" else "Asking the server for its songs…",
                                    color = MaterialTheme.colorScheme.primary,
                                )
                                androidx.compose.material3.LinearWavyProgressIndicator(modifier = Modifier.fillMaxWidth())
                            } else if (source.coversLeft > 0u) {
                                Text("Bringing in covers… ${count(source.coversLeft.toInt(), "cover")} to go", color = MaterialTheme.colorScheme.primary)
                                androidx.compose.material3.LinearWavyProgressIndicator(modifier = Modifier.fillMaxWidth())
                            }
                            Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                                FilledTonalButton(enabled = !source.syncing, onClick = { core.sourceSync(plugin.id) }) { Text("Bring in songs again") }
                                TextButton(onClick = { core.sourceSignOut(plugin.id) }) { Text("Sign out") }
                            }
                        } else {
                            source.fields.forEach { field ->
                                OutlinedTextField(
                                    value = fields[field.id] ?: "",
                                    onValueChange = { fields[field.id] = it },
                                    label = { Text(field.label) },
                                    placeholder = { Text(field.placeholder) },
                                    singleLine = true,
                                    visualTransformation = if (field.secret) PasswordVisualTransformation() else androidx.compose.ui.text.input.VisualTransformation.None,
                                    keyboardOptions = KeyboardOptions(keyboardType = if (field.secret) KeyboardType.Password else KeyboardType.Uri),
                                    modifier = Modifier.fillMaxWidth(),
                                )
                            }
                            Button(onClick = { core.sourceSignIn(plugin.id, fields.toMap()) }) { Text("Sign in") }
                        }
                        source.error?.let { Text(it, color = MaterialTheme.colorScheme.error) }
                    }
                    source.switches.forEach { switch ->
                        HorizontalDivider(color = MaterialTheme.colorScheme.outlineVariant, modifier = Modifier.padding(horizontal = 16.dp))
                        Row(Modifier.fillMaxWidth().padding(16.dp), verticalAlignment = Alignment.CenterVertically) {
                            Column(Modifier.weight(1f)) {
                                Text(switch.label, style = MaterialTheme.typography.bodyLarge)
                                if (switch.detail.isNotBlank()) Text(switch.detail, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                            }
                            Switch(checked = switch.on, onCheckedChange = { core.sourceSwitch(plugin.id, switch.id, it) })
                        }
                    }
                }
            }
        }
        val standalone = plugin.commands.filter { !it.forSongs }
        if (standalone.isNotEmpty() && plugin.enabled) {
            item {
                Section(title = "Commands") {
                    standalone.forEach { command ->
                        Row(
                            Modifier.fillMaxWidth().clickable { core.runPluginCommand(plugin.id, command.id, emptyList()) }.padding(16.dp),
                            verticalAlignment = Alignment.CenterVertically,
                        ) {
                            Text(command.title, style = MaterialTheme.typography.bodyLarge, modifier = Modifier.weight(1f))
                            Icon(Icons.AutoMirrored.Rounded.KeyboardArrowRight, contentDescription = null)
                        }
                    }
                }
            }
        }
        if (plugin.findsLyrics) {
            item {
                Text(
                    "Finds lyrics: when a song has none of its own, Needle asks this plugin.",
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.padding(horizontal = Edge, vertical = 8.dp),
                )
            }
        }
        if (plugin.effects.isNotEmpty()) {
            item {
                Text(
                    "Sound effects: ${plugin.effects.joinToString(", ")}. Turn them on in Settings › Sound.",
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.padding(horizontal = Edge, vertical = 8.dp),
                )
            }
        }
    }
}

/** A titled group of rows on one rounded surface. */
@Composable
private fun Section(title: String, content: @Composable () -> Unit) {
    Column(Modifier.padding(horizontal = 16.dp, vertical = 8.dp)) {
        Text(title, style = MaterialTheme.typography.titleSmall, color = MaterialTheme.colorScheme.primary, modifier = Modifier.padding(start = 8.dp, bottom = 8.dp, top = 8.dp))
        Column(Modifier.fillMaxWidth().clip(RoundedCornerShape(24.dp)).background(MaterialTheme.colorScheme.surfaceContainer)) { content() }
    }
}

/** The plugins' sound effects, for the Sound page: each on or off, with its sliders. */
@Composable
fun PluginEffects() {
    var effects by remember { mutableStateOf<List<Effect>?>(null) }
    var refresh by remember { mutableIntStateOf(0) }
    LaunchedEffect(refresh) { effects = withContext(Dispatchers.IO) { runCatching { core.effects() }.getOrNull() } }
    val list = effects ?: return
    Column(Modifier.padding(horizontal = 16.dp, vertical = 8.dp)) {
        Text("Effects from plugins", style = MaterialTheme.typography.titleSmall, color = MaterialTheme.colorScheme.primary, modifier = Modifier.padding(start = 8.dp, bottom = 8.dp, top = 8.dp))
        Column(Modifier.fillMaxWidth().clip(RoundedCornerShape(24.dp)).background(MaterialTheme.colorScheme.surfaceContainer).animateContentSize()) {
            if (list.isEmpty()) {
                Text(
                    "Plugins can add effects, such as a compressor or reverb. None of your plugins do yet.",
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.padding(16.dp),
                )
            }
            list.forEachIndexed { i, effect ->
                if (i > 0) HorizontalDivider(color = MaterialTheme.colorScheme.outlineVariant, modifier = Modifier.padding(horizontal = 16.dp))
                Row(Modifier.fillMaxWidth().padding(16.dp), verticalAlignment = Alignment.CenterVertically) {
                    Column(Modifier.weight(1f)) {
                        Text(effect.name, style = MaterialTheme.typography.bodyLarge)
                        Text(effect.failure ?: "${effect.pluginName} · ${effect.description}", style = MaterialTheme.typography.bodyMedium, color = if (effect.failure != null) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onSurfaceVariant)
                    }
                    Switch(checked = effect.on, onCheckedChange = { on ->
                        NeedleApp.instance.scope.launch { runCatching { core.setEffect(effect.plugin, effect.id, on) }; delay(300); refresh++ }
                    })
                }
                if (effect.on) {
                    effect.params.forEach { param ->
                        var value by remember(effect.id, param.id, param.value) { mutableStateOf(param.value) }
                        Column(Modifier.padding(horizontal = 16.dp)) {
                            Row {
                                Text(param.name, style = MaterialTheme.typography.bodyMedium, modifier = Modifier.weight(1f))
                                Text(if (value == param.value) param.shown else "%.2f".format(value), style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.primary)
                            }
                            Slider(
                                value = value,
                                onValueChange = { value = it },
                                onValueChangeFinished = {
                                    NeedleApp.instance.scope.launch { runCatching { core.setEffectParam(effect.plugin, effect.id, param.id, value) }; delay(300); refresh++ }
                                },
                                valueRange = param.min..param.max,
                            )
                        }
                    }
                }
            }
        }
    }
}
