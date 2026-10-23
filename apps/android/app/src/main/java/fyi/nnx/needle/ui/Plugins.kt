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
import androidx.lifecycle.repeatOnLifecycle
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

/** Reads the plugins again every second while Needle is on screen, as they load and sync. */
@Composable
private fun rememberPlugins(): List<PluginItem>? {
    var list by remember { mutableStateOf<List<PluginItem>?>(null) }
    val lifecycle = androidx.lifecycle.compose.LocalLifecycleOwner.current.lifecycle
    LaunchedEffect(lifecycle) {
        lifecycle.repeatOnLifecycle(androidx.lifecycle.Lifecycle.State.STARTED) {
            while (true) {
                list = withContext(Dispatchers.IO) { runCatching { core.plugins() }.getOrNull() }
                delay(1000)
            }
        }
    }
    return list
}

@Composable
fun PluginsScreen(open: (Route) -> Unit) {
    // What is built into Needle (its music servers) is not a plugin to the user: it has its own place.
    val plugins = rememberPlugins()?.filter { !it.builtIn }
    LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(bottom = 32.dp)) {
        item { LargeTitle("Plugins") }
        item {
            Text(
                "Plugins add lyrics from NetEase, sound effects, and commands. They run inside Needle, with only the permissions they list. Music servers (Navidrome, Subsonic) are in Settings › Music servers.",
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
                    SoonSwitch(plugin.enabled, plugin.name) { core.setPluginEnabled(plugin.id, it) }
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
                    SoonSwitch(plugin.enabled, plugin.name) { core.setPluginEnabled(plugin.id, it) }
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
        if (plugin.source != null) item { ServerSection(plugin) }
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

/**
 * A music server's sign-in, or once signed in, its songs and its switches: on the Music servers
 * page for the servers built into Needle, and on a plugin's page for a plugin's own.
 */
@OptIn(androidx.compose.material3.ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun ServerSection(plugin: PluginItem) {
    val source = plugin.source ?: return
    val fields = remember(plugin.id) { mutableStateMapOf<String, String>() }
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
                        modifier = Modifier.remoteEscape().fillMaxWidth(),
                    )
                }
                // Once, for a few seconds: a server can take that long to answer.
                var signingIn by remember { mutableStateOf(false) }
                LaunchedEffect(signingIn) {
                    if (signingIn) {
                        delay(5000)
                        signingIn = false
                    }
                }
                Button(enabled = !signingIn, onClick = {
                    signingIn = true
                    val sent = fields.toMap()
                    NeedleApp.instance.scope.launch(Dispatchers.IO) { runCatching { core.sourceSignIn(plugin.id, sent) } }
                }) { Text(if (signingIn) "Signing in…" else "Sign in") }
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
                SoonSwitch(switch.on, switch.label) { core.sourceSwitch(plugin.id, switch.id, it) }
            }
        }
    }
}

/**
 * Settings › Music servers: Navidrome and Subsonic, built into Needle. Signed in, the server's
 * songs are in the library beside the phone's own.
 */
@Composable
fun MusicServers() {
    val servers = rememberPlugins()?.filter { it.builtIn && it.source != null } ?: return
    Text(
        "Play the music on your own Navidrome or Subsonic server. Needle brings in the server's list of songs, and plays them from the server, or from this phone once you keep them here.",
        style = MaterialTheme.typography.bodyMedium,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        modifier = Modifier.padding(horizontal = Edge, vertical = 4.dp),
    )
    servers.forEach { server ->
        ServerSection(server)
        if (server.source?.signedIn == true) PhoneStorage(server)
    }
}

/** What one server's songs take on this phone: its count of songs and their size, in words. */
private fun usage(context: android.content.Context, use: List<ULong>?): String {
    val songs = use?.getOrNull(0)?.toInt() ?: 0
    val bytes = use?.getOrNull(1)?.toLong() ?: 0L
    return if (songs == 0) "None" else "${count(songs, "song")} · ${android.text.format.Formatter.formatShortFileSize(context, bytes)}"
}

/**
 * What a server's songs take on this phone: the songs kept on purpose, and the songs played
 * lately (which play again without the network), each with a way to give the space back,
 * after a question. As on the iPhone and the Mac.
 */
@Composable
private fun PhoneStorage(server: PluginItem) {
    val context = androidx.compose.ui.platform.LocalContext.current
    var refresh by remember { mutableIntStateOf(0) }
    var kept by remember { mutableStateOf<List<ULong>?>(null) }
    var cached by remember { mutableStateOf<List<ULong>?>(null) }
    // Read again now and then: songs are kept and played while the page is open.
    LaunchedEffect(server.id, refresh) {
        while (true) {
            withContext(Dispatchers.IO) {
                kept = runCatching { core.keptUsage(server.id) }.getOrNull()
                cached = runCatching { core.streamCacheUsage(server.id) }.getOrNull()
            }
            delay(5000)
        }
    }
    var asking by remember { mutableStateOf<String?>(null) }
    val keptSongs = kept?.getOrNull(0)?.toInt() ?: 0
    val cachedSongs = cached?.getOrNull(0)?.toInt() ?: 0
    val cachedSize = android.text.format.Formatter.formatShortFileSize(context, cached?.getOrNull(1)?.toLong() ?: 0L)
    Section(title = "On this phone") {
        StorageRow("Kept offline", "Songs you chose to keep.", usage(context, kept), if (keptSongs > 0) "Stop keeping all" else null) { asking = "kept" }
        HorizontalDivider(color = MaterialTheme.colorScheme.outlineVariant, modifier = Modifier.padding(horizontal = 16.dp))
        StorageRow("Played lately", "They play again without the network.", usage(context, cached), if (cachedSongs > 0) "Free space" else null) { asking = "cache" }
    }
    when (asking) {
        "kept" -> androidx.compose.material3.AlertDialog(
            onDismissRequest = { asking = null },
            title = { Text("Stop keeping ${count(keptSongs, "song")}?") },
            text = { Text("They stay on this phone until the space is needed, then they stream again.") },
            confirmButton = {
                TextButton(onClick = {
                    asking = null
                    NeedleApp.instance.scope.launch(Dispatchers.IO) {
                        runCatching { core.forgetKept(server.id) }
                            .onSuccess { showMessage("Stopped keeping ${count(it.toInt(), "song")}") }
                            .onFailure { showMessage(it.message ?: "Could not stop keeping the songs") }
                        refresh++
                    }
                }) { Text("Stop keeping") }
            },
            dismissButton = { TextButton(onClick = { asking = null }) { Text("Cancel") } },
        )
        "cache" -> androidx.compose.material3.AlertDialog(
            onDismissRequest = { asking = null },
            title = { Text("Free $cachedSize?") },
            text = { Text("These songs stream again the next time you play them. Songs you kept stay.") },
            confirmButton = {
                TextButton(onClick = {
                    asking = null
                    NeedleApp.instance.scope.launch(Dispatchers.IO) {
                        runCatching { core.clearStreamCache(server.id) }
                            .onSuccess { showMessage("Freed the space of ${count(it.toInt(), "song")}") }
                            .onFailure { showMessage(it.message ?: "Could not free the space") }
                        refresh++
                    }
                }) { Text("Free space") }
            },
            dismissButton = { TextButton(onClick = { asking = null }) { Text("Cancel") } },
        )
    }
}

/** One kind of song on the phone: what it is, how much, and its button (none when empty). */
@Composable
private fun StorageRow(title: String, detail: String, amount: String, action: String?, onAction: () -> Unit) {
    Row(Modifier.fillMaxWidth().padding(16.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
        Column(Modifier.weight(1f)) {
            Text(title, style = MaterialTheme.typography.bodyLarge)
            Text(detail, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
            Text(amount, style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.primary, modifier = Modifier.padding(top = 4.dp))
        }
        if (action != null) FilledTonalButton(onClick = onAction) { Text(action) }
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
                var shownOn by remember(effect.id, effect.on) { mutableStateOf(effect.on) }
                Row(Modifier.fillMaxWidth().padding(16.dp), verticalAlignment = Alignment.CenterVertically) {
                    Column(Modifier.weight(1f)) {
                        Text(effect.name, style = MaterialTheme.typography.bodyLarge)
                        Text(effect.failure ?: "${effect.pluginName} · ${effect.description}", style = MaterialTheme.typography.bodyMedium, color = if (effect.failure != null) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onSurfaceVariant)
                    }
                    // Moves at once, rather than when the effect is read again a moment later.
                    Switch(checked = shownOn, modifier = Modifier.named(effect.name), onCheckedChange = { on ->
                        shownOn = on
                        // In order with the effect's other changes (the saves line), so none undoes another.
                        NeedleApp.instance.scope.launch { withContext(NeedleApp.instance.saves) { runCatching { core.setEffect(effect.plugin, effect.id, on) } }; delay(300); refresh++ }
                    })
                }
                if (shownOn) {
                    effect.params.forEach { param ->
                        var value by remember(effect.id, param.id, param.value) { mutableStateOf(param.value) }
                        Column(Modifier.padding(horizontal = 16.dp)) {
                            Row {
                                Text(param.name, style = MaterialTheme.typography.bodyMedium, modifier = Modifier.weight(1f))
                                Text(if (value == param.value) param.shown else "%.2f".format(value), style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.primary)
                            }
                            // A plugin's range the wrong way round (or empty) gets no slider, only its value.
                            if (param.max > param.min) Slider(
                                modifier = Modifier.named(param.name, if (value == param.value) param.shown else "%.2f".format(value)),
                                value = value,
                                onValueChange = { value = it },
                                onValueChangeFinished = {
                                    NeedleApp.instance.scope.launch { withContext(NeedleApp.instance.saves) { runCatching { core.setEffectParam(effect.plugin, effect.id, param.id, value) } }; delay(300); refresh++ }
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

/**
 * A switch for what a plugin turns on a moment later (the list is read once a second): it moves
 * at once, then follows the plugin's answer, or goes back if none comes.
 */
@Composable
private fun SoonSwitch(on: Boolean, name: String, change: (Boolean) -> Unit) {
    var asked by remember(on) { mutableStateOf<Boolean?>(null) }
    LaunchedEffect(asked) {
        if (asked == null) return@LaunchedEffect
        kotlinx.coroutines.delay(3000)
        asked = null
    }
    Switch(checked = asked ?: on, onCheckedChange = { asked = it; change(it) }, modifier = Modifier.named(name))
}
