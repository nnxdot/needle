package fyi.nnx.needle.ui

import android.content.Intent
import android.net.Uri
import android.os.Build
import android.os.Environment
import android.provider.DocumentsContract
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.animateContentSize
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.detectVerticalDragGestures
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.KeyboardArrowRight
import androidx.compose.material.icons.rounded.Delete
import androidx.compose.material.icons.rounded.Equalizer
import androidx.compose.material.icons.rounded.Folder
import androidx.compose.material.icons.rounded.GraphicEq
import androidx.compose.material.icons.rounded.Info
import androidx.compose.material.icons.rounded.Extension
import androidx.compose.material.icons.rounded.Computer
import androidx.compose.material.icons.rounded.Sync
import androidx.compose.material.icons.rounded.NewReleases
import androidx.compose.material.icons.rounded.LibraryMusic
import androidx.compose.material.icons.rounded.Palette
import androidx.compose.material.icons.rounded.PlayCircle
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.FilterChip
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LinearWavyProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.SegmentedButton
import androidx.compose.material3.SegmentedButtonDefaults
import androidx.compose.material3.SingleChoiceSegmentedButtonRow
import androidx.compose.material3.Slider
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.unit.dp
import fyi.nnx.needle.NeedleApp
import fyi.nnx.needle.core.Accounts
import fyi.nnx.needle.core.PlaybackSettings
import fyi.nnx.needle.core.SoundSettings
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.io.File

enum class SettingsSection(val title: String, val summary: String, val icon: ImageVector, val tint: Color) {
    Library("Library", "Music folders and online lookups", Icons.Rounded.LibraryMusic, Color(0xFFE2B46C)),
    Playback("Playback", "Crossfade and even loudness", Icons.Rounded.PlayCircle, Color(0xFF7FB8E8)),
    Sound("Sound", "Equalizer, balance, and headphones", Icons.Rounded.Equalizer, Color(0xFFB39DDB)),
    Appearance("Appearance", "Colours, motion, and lyrics", Icons.Rounded.Palette, Color(0xFFE8A07F)),
    Scrobbling("Scrobbling", "Last.fm and ListenBrainz", Icons.Rounded.GraphicEq, Color(0xFFD1706B)),
    Plugins("Plugins", "Music servers, NetEase lyrics, effects", Icons.Rounded.Extension, Color(0xFF7FC8B8)),
    Computer("Your computer", "Control Needle there, play its music here", Icons.Rounded.Computer, Color(0xFF9FB4E8)),
    Sync("Sync", "Move history, ratings, and playlists", Icons.Rounded.Sync, Color(0xFFE8C77F)),
    WhatsNew("What's new", "In this version of Needle", Icons.Rounded.NewReleases, Color(0xFFE89FC4)),
    About("About", "Version, updates, and privacy", Icons.Rounded.Info, Color(0xFF9CC99A)),
}

/** Sections that are pages of their own elsewhere in the app. */
private fun SettingsSection.route(): Route = when (this) {
    SettingsSection.Plugins -> Route.Plugins
    SettingsSection.Computer -> Route.Connect
    SettingsSection.Sync -> Route.Sync
    SettingsSection.WhatsNew -> Route.WhatsNew
    else -> Route.SettingsPage(this)
}

@Composable
fun SettingsScreen(open: (Route) -> Unit) {
    LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(bottom = 24.dp)) {
        item { LargeTitle("Settings") }
        item {
            Group {
                SettingsSection.entries.forEachIndexed { i, section ->
                    Row(
                        Modifier
                            .fillMaxWidth()
                            .clickable { open(section.route()) }
                            .padding(horizontal = 16.dp, vertical = 14.dp),
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(16.dp),
                    ) {
                        Box(
                            Modifier.size(40.dp).clip(CircleShape).background(section.tint.copy(alpha = 0.18f)),
                            contentAlignment = Alignment.Center,
                        ) { Icon(section.icon, contentDescription = null, tint = section.tint) }
                        Column(Modifier.weight(1f)) {
                            Text(section.title, style = MaterialTheme.typography.titleMedium)
                            Text(section.summary, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                        }
                        Icon(Icons.AutoMirrored.Rounded.KeyboardArrowRight, contentDescription = null, tint = MaterialTheme.colorScheme.onSurfaceVariant)
                    }
                    if (i < SettingsSection.entries.size - 1) Divider()
                }
            }
        }
    }
}

@Composable
fun SettingsSectionScreen(section: SettingsSection, open: (Route) -> Unit) {
    LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(bottom = 32.dp)) {
        item { LargeTitle(section.title) }
        item {
            when (section) {
                SettingsSection.Library -> LibrarySettings()
                SettingsSection.Playback -> PlaybackSettingsPage()
                SettingsSection.Sound -> SoundSettingsPage()
                SettingsSection.Appearance -> AppearanceSettings()
                SettingsSection.Scrobbling -> ScrobblingSettings()
                SettingsSection.About -> AboutPage()
                else -> {}
            }
        }
    }
}

// ---------- Building blocks

/** Rows grouped on one rounded surface, as in Android's own Settings. */
@Composable
private fun Group(title: String? = null, footer: String? = null, content: @Composable () -> Unit) {
    Column(Modifier.padding(horizontal = 16.dp, vertical = 8.dp)) {
        if (title != null) {
            Text(
                title,
                style = MaterialTheme.typography.titleSmall,
                color = MaterialTheme.colorScheme.primary,
                modifier = Modifier.padding(start = 8.dp, bottom = 8.dp, top = 8.dp),
            )
        }
        Column(
            Modifier
                .fillMaxWidth()
                .clip(RoundedCornerShape(24.dp))
                .background(MaterialTheme.colorScheme.surfaceContainer)
                .animateContentSize(),
        ) { content() }
        if (footer != null) {
            Text(
                footer,
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(start = 8.dp, end = 8.dp, top = 8.dp),
            )
        }
    }
}

@Composable
private fun Divider() = HorizontalDivider(color = MaterialTheme.colorScheme.outlineVariant, modifier = Modifier.padding(horizontal = 16.dp))

@Composable
private fun SwitchRow(title: String, summary: String?, checked: Boolean, enabled: Boolean = true, onChange: (Boolean) -> Unit) {
    val haptics = rememberHaptics()
    Row(
        Modifier
            .fillMaxWidth()
            .heightIn(min = 64.dp)
            .clickable(enabled = enabled) { haptics(false); onChange(!checked) }
            .padding(horizontal = 16.dp, vertical = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        Column(Modifier.weight(1f)) {
            Text(title, style = MaterialTheme.typography.bodyLarge, color = if (enabled) MaterialTheme.colorScheme.onSurface else MaterialTheme.colorScheme.onSurfaceVariant)
            if (summary != null) Text(summary, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
        Switch(checked = checked, enabled = enabled, onCheckedChange = { haptics(false); onChange(it) })
    }
}

@Composable
private fun SliderRow(
    title: String,
    shown: String,
    value: Float,
    range: ClosedFloatingPointRange<Float>,
    steps: Int = 0,
    summary: String? = null,
    enabled: Boolean = true,
    onChange: (Float) -> Unit,
    onDone: () -> Unit,
) {
    Column(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 12.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Text(title, style = MaterialTheme.typography.bodyLarge, modifier = Modifier.weight(1f))
            Text(shown, style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.primary)
        }
        if (summary != null) Text(summary, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
        Slider(value = value, onValueChange = onChange, onValueChangeFinished = onDone, valueRange = range, steps = steps, enabled = enabled)
    }
}

/** Runs a call to the core off the main thread. */
private fun background(work: () -> Unit) {
    NeedleApp.instance.scope.launch { runCatching { work() } }
}

// ---------- Library

/**
 * A folder chosen in Android's picker, as a path Needle can read ("primary:Music" is the
 * phone's own Music folder; other volumes, like an SD card, have their own id).
 */
private fun treePath(uri: Uri): String? {
    val id = runCatching { DocumentsContract.getTreeDocumentId(uri) }.getOrNull() ?: return null
    val volume = id.substringBefore(':')
    val rest = id.substringAfter(':', "")
    val root = if (volume == "primary") Environment.getExternalStorageDirectory().path else "/storage/$volume"
    return if (rest.isEmpty()) root else "$root/$rest"
}

@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun LibrarySettings() {
    val folders by rememberLoaded { folders() }
    val scan by NeedleApp.instance.scan.collectAsState()
    val count by rememberLoaded(scan.running) { songCount() }
    var playback by remember { mutableStateOf<PlaybackSettings?>(null) }
    var removing by remember { mutableStateOf<String?>(null) }
    LaunchedEffect(Unit) { playback = withContext(Dispatchers.IO) { runCatching { core.playbackSettings() }.getOrNull() } }
    val pick = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocumentTree()) { uri ->
        val path = uri?.let(::treePath) ?: return@rememberLauncherForActivityResult
        background { core.addFolder(path) }
        NeedleApp.instance.libraryVersion.value++
    }
    val music = File(Environment.getExternalStorageDirectory(), Environment.DIRECTORY_MUSIC).path

    Group("Music folders", footer = "Needle reads these folders, and reads them again each time it starts.") {
        folders.orEmpty().forEachIndexed { i, folder ->
            Row(
                Modifier.fillMaxWidth().padding(start = 16.dp, end = 4.dp, top = 6.dp, bottom = 6.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(16.dp),
            ) {
                Icon(Icons.Rounded.Folder, contentDescription = null, tint = MaterialTheme.colorScheme.primary)
                Column(Modifier.weight(1f)) {
                    Text(folder.substringAfterLast('/').ifBlank { folder }, style = MaterialTheme.typography.bodyLarge)
                    Text(folder, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                }
                IconButton(onClick = { removing = folder }) {
                    Icon(Icons.Rounded.Delete, contentDescription = "Stop reading this folder", tint = MaterialTheme.colorScheme.onSurfaceVariant)
                }
            }
            Divider()
        }
        Row(Modifier.fillMaxWidth().padding(16.dp), horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            Button(onClick = { pick.launch(null) }) { Text("Add a folder") }
            if (folders?.contains(music) == false) {
                FilledTonalButton(onClick = { background { core.addFolder(music) }; NeedleApp.instance.libraryVersion.value++ }) {
                    Text("Use Music")
                }
            }
        }
        val context = LocalContext.current
        TextButton(onClick = { background { core.addDemoLibrary(File(context.filesDir, "demo").absolutePath) } }, modifier = Modifier.padding(start = 8.dp, bottom = 8.dp)) {
            Text("Add a few demo songs")
        }
    }
    Group("Reading") {
        Column(Modifier.fillMaxWidth().padding(16.dp).animateContentSize(), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            if (scan.running) {
                LinearWavyProgressIndicator(modifier = Modifier.fillMaxWidth())
                Text("Reading ${scan.scanned} files, ${scan.imported} new", color = MaterialTheme.colorScheme.onSurfaceVariant)
            } else {
                Text("${count(count?.toInt() ?: 0, "song")} in your library", style = MaterialTheme.typography.bodyLarge)
                scan.error?.let { Text(it, color = MaterialTheme.colorScheme.error) }
                if (folders?.isNotEmpty() == true) {
                    FilledTonalButton(onClick = { core.rescan() }) { Text("Read the folders again") }
                }
            }
        }
    }
    playback?.let { p ->
        Group("Online", footer = "When a song has no lyrics of its own, Needle asks LRCLIB for them with the song's title, artist, album, and length. Nothing else is sent.") {
            SwitchRow("Look up lyrics online", "From LRCLIB, for songs without their own", p.onlineMedia) {
                val next = p.copy(onlineMedia = it)
                playback = next
                background { core.setPlaybackSettings(next) }
            }
        }
    }
    removing?.let { folder ->
        AlertDialog(
            onDismissRequest = { removing = null },
            title = { Text("Stop reading this folder?") },
            text = { Text("Its songs leave your library. The files stay on your phone.") },
            confirmButton = {
                TextButton(onClick = {
                    background { core.removeFolder(folder); NeedleApp.instance.libraryVersion.value++ }
                    removing = null
                }) { Text("Stop reading") }
            },
            dismissButton = { TextButton(onClick = { removing = null }) { Text("Cancel") } },
        )
    }
}

// ---------- Playback

@Composable
private fun PlaybackSettingsPage() {
    var settings by remember { mutableStateOf<PlaybackSettings?>(null) }
    LaunchedEffect(Unit) { settings = withContext(Dispatchers.IO) { runCatching { core.playbackSettings() }.getOrNull() } }
    val app by NeedleApp.instance.app.collectAsState()
    val s = settings ?: return
    fun save(next: PlaybackSettings) {
        settings = next
        background { core.setPlaybackSettings(next) }
    }
    Group("Between songs", footer = "Gapless albums always play without a gap, crossfade or not.") {
        var fade by remember { mutableStateOf(s.crossfade) }
        SliderRow(
            "Crossfade",
            if (fade < 0.5f) "Off" else "${fade.toInt()} s",
            fade,
            0f..12f,
            steps = 11,
            summary = "Songs overlap as one ends and the next begins",
            onChange = { fade = it },
            onDone = { save(s.copy(crossfade = fade)) },
        )
    }
    Group("Loudness", footer = "Uses the ReplayGain tags in your songs. Album keeps an album's quiet and loud songs as the artist made them.") {
        Column(Modifier.padding(16.dp)) {
            Text("Even out loudness", style = MaterialTheme.typography.bodyLarge, modifier = Modifier.padding(bottom = 12.dp))
            val choice = when {
                !s.replayGain -> 0
                s.albumGain -> 2
                else -> 1
            }
            SingleChoiceSegmentedButtonRow(Modifier.fillMaxWidth()) {
                listOf("Off", "Song", "Album").forEachIndexed { i, label ->
                    SegmentedButton(
                        selected = choice == i,
                        onClick = { save(s.copy(replayGain = i > 0, albumGain = i == 2)) },
                        shape = SegmentedButtonDefaults.itemShape(i, 3),
                    ) { Text(label) }
                }
            }
        }
    }
    Group("When you choose a song") {
        SwitchRow("Open the player", "Show the full player as soon as a song starts", app.openPlayerOnPlay) { on ->
            NeedleApp.instance.updateApp { it.copy(openPlayerOnPlay = on) }
        }
    }
}

// ---------- Sound

@Composable
private fun SoundSettingsPage() {
    var sound by remember { mutableStateOf<SoundSettings?>(null) }
    val presets = remember { core.eqPresets() }
    val bands = remember { core.eqBands() }
    LaunchedEffect(Unit) { sound = withContext(Dispatchers.IO) { runCatching { core.soundSettings() }.getOrNull() } }
    val s = sound ?: return
    fun save(next: SoundSettings) {
        sound = next
        background { core.setSoundSettings(next) }
    }
    Group("Equalizer") {
        SwitchRow("Equalizer", "Shape the sound with ten bands", s.eq) { save(s.copy(eq = it)) }
        AnimatedVisibility(s.eq) {
            Column {
                LazyRow(
                    contentPadding = PaddingValues(horizontal = 16.dp),
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    items(presets) { preset ->
                        FilterChip(
                            selected = s.preset == preset.name,
                            onClick = { save(s.copy(preset = preset.name, preamp = preset.preamp, bands = preset.bands)) },
                            label = { Text(preset.name) },
                        )
                    }
                }
                EqualizerBands(s.bands, bands) { changed -> save(s.copy(bands = changed, preset = "Custom")) }
                var preamp by remember(s.preamp) { mutableStateOf(s.preamp) }
                SliderRow(
                    "Preamp",
                    "%+.1f dB".format(preamp),
                    preamp,
                    -12f..6f,
                    summary = "Lower it when boosted bands make songs clip",
                    onChange = { preamp = it },
                    onDone = { save(s.copy(preamp = preamp)) },
                )
            }
        }
    }
    Group("Headphones and speakers") {
        SwitchRow("Crossfeed", "Blends a little of each side into the other, as speakers do, for easier listening on headphones", s.crossfeed) {
            save(s.copy(crossfeed = it))
        }
        Divider()
        SwitchRow("Mono", "Both sides play the same, for one earbud", s.mono) { save(s.copy(mono = it)) }
        Divider()
        UsbBitPerfect()
        var balance by remember(s.balance) { mutableStateOf(s.balance) }
        SliderRow(
            "Balance",
            when {
                balance < -0.02f -> "Left ${(-balance * 100).toInt()}%"
                balance > 0.02f -> "Right ${(balance * 100).toInt()}%"
                else -> "Centre"
            },
            balance,
            -1f..1f,
            onChange = { balance = if (kotlin.math.abs(it) < 0.04f) 0f else it },
            onDone = { save(s.copy(balance = balance)) },
        )
    }
    PluginEffects()
}

/** Ten upright bars, dragged up and down, with a line through their tops. */
@Composable
private fun EqualizerBands(gains: List<Float>, frequencies: List<Float>, onChange: (List<Float>) -> Unit) {
    var live by remember(gains) { mutableStateOf(gains) }
    val haptics = rememberHaptics()
    Row(
        Modifier.fillMaxWidth().height(220.dp).padding(horizontal = 12.dp, vertical = 16.dp),
        horizontalArrangement = Arrangement.SpaceBetween,
    ) {
        live.forEachIndexed { i, gain ->
            Column(Modifier.weight(1f), horizontalAlignment = Alignment.CenterHorizontally) {
                Text("%+.0f".format(gain), style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                BoxWithConstraints(
                    Modifier
                        .weight(1f)
                        .width(28.dp)
                        .padding(vertical = 6.dp)
                        .pointerInput(i) {
                            detectVerticalDragGestures(
                                onDragEnd = { haptics(false); onChange(live) },
                            ) { change, _ ->
                                val y = change.position.y.coerceIn(0f, size.height.toFloat())
                                val value = 12f - 24f * y / size.height
                                val snapped = (Math.round(value * 2) / 2f).coerceIn(-12f, 12f)
                                live = live.toMutableList().also { it[i] = snapped }
                            }
                        },
                    contentAlignment = Alignment.Center,
                ) {
                    val fraction by animateFloatAsState((gain + 12f) / 24f, label = "band")
                    // The track, the level from the middle, and the knob.
                    Box(Modifier.width(6.dp).fillMaxHeight().clip(CircleShape).background(MaterialTheme.colorScheme.surfaceContainerHighest))
                    val knobY = maxHeight * (1f - fraction) - maxHeight / 2
                    Box(
                        Modifier
                            .align(Alignment.Center)
                            .offset(y = knobY)
                            .size(20.dp)
                            .clip(CircleShape)
                            .background(MaterialTheme.colorScheme.primary),
                    )
                }
                Text(frequencyLabel(frequencies.getOrElse(i) { 0f }), style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
        }
    }
}

private fun frequencyLabel(hz: Float) = if (hz >= 1000) "${(hz / 1000).toInt()}k" else hz.toInt().toString()

// ---------- Appearance

@Composable
private fun AppearanceSettings() {
    val app by NeedleApp.instance.app.collectAsState()
    val update = NeedleApp.instance::updateApp
    val context = LocalContext.current
    var themes by remember { mutableStateOf(core.themes()) }
    val importTheme = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument()) { uri ->
        val path = uri?.let { copyToCache(context, it) } ?: return@rememberLauncherForActivityResult
        runCatching { core.importTheme(path) }
            .onSuccess { name -> themes = core.themes(); showMessage("Added $name") }
            .onFailure { showMessage(it.message ?: "Could not add that theme") }
    }
    Group("Look", footer = "Themes made on desktop, or from needle.nnx.fyi/themes, work here too.") {
        Row(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 12.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            listOf("night" to "Night", "midnight" to "Midnight", "day" to "Day").forEach { (id, label) ->
                FilterChip(selected = app.theme == id, onClick = { update { it.copy(theme = id) } }, label = { Text(label) })
            }
        }
        themes.forEach { theme ->
            Divider()
            Row(
                Modifier.fillMaxWidth().clickable { update { it.copy(theme = theme.id) } }.padding(horizontal = 16.dp, vertical = 14.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Text(theme.name, style = MaterialTheme.typography.bodyLarge, modifier = Modifier.weight(1f))
                if (app.theme == theme.id) Text("On", color = MaterialTheme.colorScheme.primary)
            }
        }
        Divider()
        TextButton(onClick = { importTheme.launch(arrayOf("*/*")) }, modifier = Modifier.padding(8.dp)) { Text("Add a theme file") }
    }
    Group("Colour") {
        SwitchRow("Cover behind the player", "The playing cover, blurred, fills the player", app.coverBackdrop) { on -> update { it.copy(coverBackdrop = on) } }
        if (Build.VERSION.SDK_INT >= 31) {
            Divider()
            SwitchRow("Colours from your wallpaper", "Instead of Needle's amber", app.wallpaperColors) { on -> update { it.copy(wallpaperColors = on) } }
        }
    }
    Group("Motion", footer = "Android's own \"Remove animations\" turns motion off too.") {
        SwitchRow("Reduce motion", "Cross-fades instead of sliding and moving covers", app.reduceMotion) { on -> update { it.copy(reduceMotion = on) } }
        Divider()
        SwitchRow("Vibration", "A light tap on play, pause, and long presses", app.haptics) { on -> update { it.copy(haptics = on) } }
    }
    Group("Lyrics") {
        SwitchRow("Follow the song", "The line playing is lit and stays in view", app.liveLyrics) { on -> update { it.copy(liveLyrics = on) } }
        Divider()
        SwitchRow("Keep the screen on", "While the lyrics show", app.keepScreenOnLyrics) { on -> update { it.copy(keepScreenOnLyrics = on) } }
    }
}

// ---------- Scrobbling

@Composable
private fun ScrobblingSettings() {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    var accounts by remember { mutableStateOf<Accounts?>(null) }
    var message by remember { mutableStateOf<String?>(null) }
    var busy by remember { mutableStateOf(false) }
    var waitingForBrowser by remember { mutableStateOf(false) }
    var refresh by remember { mutableIntStateOf(0) }
    LaunchedEffect(refresh) { accounts = withContext(Dispatchers.IO) { core.accounts() } }
    fun run(work: () -> String?) {
        busy = true
        scope.launch {
            message = withContext(Dispatchers.IO) { runCatching { work() }.fold({ it }, { it.message }) }
            busy = false
            refresh++
        }
    }
    val a = accounts ?: return

    Group("Last.fm", footer = "Songs you play past half their length (or four minutes) are sent, with the time you played them.") {
        if (a.lastfmUser != null) {
            SwitchRow("Send to Last.fm", if (a.lastfmUser!!.isBlank()) "Signed in" else "Signed in as ${a.lastfmUser}", a.lastfmOn) { on ->
                run { core.setScrobbling(on, null); null }
            }
            Divider()
            TextButton(onClick = { run { core.signOut("lastfm"); "Signed out of Last.fm." } }, modifier = Modifier.padding(8.dp)) { Text("Sign out") }
        } else {
            var key by remember { mutableStateOf("") }
            var secret by remember { mutableStateOf("") }
            Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                if (!a.lastfmKeyBuiltIn) {
                    Text("This build needs your own Last.fm API account: create one at last.fm/api, then paste its key and secret.", color = MaterialTheme.colorScheme.onSurfaceVariant)
                    OutlinedTextField(key, { key = it }, label = { Text("API key") }, singleLine = true, modifier = Modifier.fillMaxWidth())
                    OutlinedTextField(secret, { secret = it }, label = { Text("Shared secret") }, singleLine = true, visualTransformation = PasswordVisualTransformation(), modifier = Modifier.fillMaxWidth())
                }
                if (!waitingForBrowser) {
                    Button(enabled = !busy, onClick = {
                        run {
                            val url = core.lastfmBegin(key, secret)
                            context.startActivity(Intent(Intent.ACTION_VIEW, Uri.parse(url)).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
                            waitingForBrowser = true
                            null
                        }
                    }) { Text("Sign in with Last.fm") }
                } else {
                    Text("Allow Needle on the Last.fm page, then come back here.", color = MaterialTheme.colorScheme.onSurfaceVariant)
                    Button(enabled = !busy, onClick = {
                        run {
                            val user = core.lastfmComplete()
                            waitingForBrowser = false
                            if (user.isBlank()) "Signed in to Last.fm." else "Signed in as $user."
                        }
                    }) { Text("I allowed it") }
                }
            }
        }
    }
    Group("ListenBrainz", footer = "Your user token is on listenbrainz.org, under Settings.") {
        if (a.listenbrainzUser != null) {
            SwitchRow("Send to ListenBrainz", if (a.listenbrainzUser!!.isBlank()) "Signed in" else "Signed in as ${a.listenbrainzUser}", a.listenbrainzOn) { on ->
                run { core.setScrobbling(null, on); null }
            }
            Divider()
            TextButton(onClick = { run { core.signOut("listenbrainz"); "Signed out of ListenBrainz." } }, modifier = Modifier.padding(8.dp)) { Text("Sign out") }
        } else {
            var token by remember { mutableStateOf("") }
            Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                OutlinedTextField(
                    token, { token = it }, label = { Text("User token") }, singleLine = true,
                    visualTransformation = PasswordVisualTransformation(),
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Password),
                    modifier = Modifier.fillMaxWidth(),
                )
                Button(enabled = !busy && token.isNotBlank(), onClick = {
                    run { val user = core.listenbrainzSignIn(token.trim()); if (user.isBlank()) "Signed in to ListenBrainz." else "Signed in as $user." }
                }) { Text("Sign in") }
            }
        }
    }
    if (a.waiting > 0u) {
        Text(
            "${count(a.waiting.toInt(), "listen")} waiting to be sent.",
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.padding(horizontal = 24.dp, vertical = 4.dp),
        )
    }
    (message ?: a.problem)?.let {
        Text(it, style = MaterialTheme.typography.bodyMedium, modifier = Modifier.padding(horizontal = 24.dp, vertical = 8.dp))
    }
}

// ---------- About

@Composable
private fun AboutPage() {
    val context = LocalContext.current
    fun visit(url: String) = context.startActivity(Intent(Intent.ACTION_VIEW, Uri.parse(url)).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
    val version = remember { runCatching { context.packageManager.getPackageInfo(context.packageName, 0).versionName }.getOrNull() ?: "" }
    Group {
        Column(Modifier.padding(16.dp)) {
            Text("Needle for Android", style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.Bold)
            Text("Version $version · preview", color = MaterialTheme.colorScheme.onSurfaceVariant)
            Spacer(Modifier.height(8.dp))
            Text("Your music, on your phone. No account, no ads, no tracking.", style = MaterialTheme.typography.bodyMedium)
        }
    }
    Group("Updates") { UpdatesRow() }
    Group("Privacy", footer = "A crash report holds what went wrong, with file paths and names taken out. Nothing is sent otherwise.") {
        var crashes by remember { mutableStateOf(core.crashReports()) }
        SwitchRow("Send crash reports", "To needle.nnx.fyi, when Needle stops by mistake", crashes) { on ->
            crashes = on
            background { core.setCrashReports(on) }
        }
    }
    Group {
        LinkRow("Website", "needle.nnx.fyi") { visit("https://needle.nnx.fyi") }
        Divider()
        LinkRow("Privacy", "What Needle sends, and when") { visit("https://needle.nnx.fyi/privacy") }
        Divider()
        LinkRow("Help", "Questions and answers") { visit("https://needle.nnx.fyi/help") }
    }
}

@Composable
private fun LinkRow(title: String, summary: String, onClick: () -> Unit) {
    Row(
        Modifier.fillMaxWidth().clickable(onClick = onClick).padding(horizontal = 16.dp, vertical = 14.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(Modifier.weight(1f)) {
            Text(title, style = MaterialTheme.typography.bodyLarge)
            Text(summary, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
        Icon(Icons.AutoMirrored.Rounded.KeyboardArrowRight, contentDescription = null, tint = MaterialTheme.colorScheme.onSurfaceVariant)
    }
}
