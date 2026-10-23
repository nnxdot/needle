package fyi.nnx.needle.ui

import androidx.compose.material.icons.rounded.Dns
import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.togetherWith
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.saveable.rememberSaveable
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
import androidx.compose.foundation.border
import androidx.compose.material.icons.rounded.Check
import androidx.compose.material.icons.rounded.Forward10
import androidx.compose.material.icons.rounded.PlayArrow
import androidx.compose.material.icons.rounded.SkipNext
import androidx.compose.material.icons.rounded.SkipPrevious
import androidx.compose.material3.toShape
import androidx.compose.runtime.mutableFloatStateOf
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
import androidx.compose.material.icons.automirrored.rounded.HelpOutline
import androidx.compose.material.icons.rounded.WavingHand
import androidx.compose.material.icons.rounded.Wallpaper
import androidx.compose.material.icons.rounded.VolumeUp
import androidx.compose.material.icons.rounded.VolumeDown
import androidx.compose.material.icons.rounded.ViewHeadline
import androidx.compose.material.icons.rounded.Vibration
import androidx.compose.material.icons.rounded.Verified
import androidx.compose.material.icons.rounded.Update
import androidx.compose.material.icons.rounded.Tune
import androidx.compose.material.icons.rounded.Title
import androidx.compose.material.icons.rounded.Timer
import androidx.compose.material.icons.rounded.Swipe
import androidx.compose.material.icons.rounded.SwapHoriz
import androidx.compose.material.icons.rounded.Subtitles
import androidx.compose.material.icons.rounded.SmartDisplay
import androidx.compose.material.icons.rounded.Shield
import androidx.compose.material.icons.rounded.Replay10
import androidx.compose.material.icons.rounded.Radio
import androidx.compose.material.icons.rounded.Pause
import androidx.compose.material.icons.rounded.OpenInFull
import androidx.compose.material.icons.rounded.MusicNote
import androidx.compose.material.icons.rounded.Movie
import androidx.compose.material.icons.rounded.MotionPhotosOff
import androidx.compose.material.icons.rounded.Lyrics
import androidx.compose.material.icons.rounded.LooksTwo
import androidx.compose.material.icons.rounded.LooksOne
import androidx.compose.material.icons.rounded.Looks3
import androidx.compose.material.icons.rounded.LinearScale
import androidx.compose.material.icons.rounded.LightMode
import androidx.compose.material.icons.rounded.Language
import androidx.compose.material.icons.rounded.Key
import androidx.compose.material.icons.rounded.Image
import androidx.compose.material.icons.rounded.Hearing
import androidx.compose.material.icons.rounded.Headphones
import androidx.compose.material.icons.rounded.Grain
import androidx.compose.material.icons.rounded.Fullscreen
import androidx.compose.material.icons.rounded.FormatSize
import androidx.compose.material.icons.rounded.FileOpen
import androidx.compose.material.icons.rounded.Edit
import androidx.compose.material.icons.rounded.Download
import androidx.compose.material.icons.rounded.CreateNewFolder
import androidx.compose.material.icons.rounded.ColorLens
import androidx.compose.material.icons.rounded.Build
import androidx.compose.material.icons.rounded.BugReport
import androidx.compose.material.icons.rounded.BlurOn
import androidx.compose.material.icons.rounded.BlurLinear
import androidx.compose.material.icons.rounded.Backup
import androidx.compose.material.icons.rounded.AutoAwesome
import androidx.compose.material.icons.rounded.Animation
import androidx.compose.material.icons.rounded.AllInclusive
import androidx.compose.material.icons.rounded.Album
import androidx.compose.material.icons.rounded.Add
import androidx.compose.material.icons.automirrored.rounded.KeyboardArrowRight
import androidx.compose.material.icons.rounded.Delete
import androidx.compose.material.icons.rounded.Equalizer
import androidx.compose.material.icons.rounded.Folder
import androidx.compose.material.icons.rounded.GraphicEq
import androidx.compose.material.icons.rounded.Info
import androidx.compose.material.icons.rounded.Extension
import androidx.compose.material.icons.rounded.Forum
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
import androidx.compose.material.icons.rounded.Star
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.DropdownMenu
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
import androidx.compose.foundation.focusable
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.progressBarRangeInfo
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.setProgress
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.selection.toggleable
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.graphics.luminance
import androidx.compose.ui.unit.sp
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
import fyi.nnx.needle.Discord
import fyi.nnx.needle.core.Accounts
import fyi.nnx.needle.core.DiscordSettings
import fyi.nnx.needle.core.PlaybackSettings
import fyi.nnx.needle.core.SoundSettings
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.io.File

enum class SettingsSection(val title: String, val summary: String, val icon: ImageVector, val tint: Color) {
    Library("Library", "Music folders and online lookups", Icons.Rounded.LibraryMusic, Color(0xFFE2B46C)),
    Servers("Music servers", "Navidrome and Subsonic", Icons.Rounded.Dns, Color(0xFF8FC6E8)),
    Playback("Playback", "Crossfade and even loudness", Icons.Rounded.PlayCircle, Color(0xFF7FB8E8)),
    Sound("Sound", "Equalizer, balance, and headphones", Icons.Rounded.Equalizer, Color(0xFFB39DDB)),
    Appearance("Appearance", "Colours, motion, and lyrics", Icons.Rounded.Palette, Color(0xFFE8A07F)),
    Scrobbling("Scrobbling", "Last.fm and ListenBrainz", Icons.Rounded.GraphicEq, Color(0xFFD1706B)),
    Discord("Discord", "Show what you play on your profile", Icons.Rounded.Forum, Color(0xFF8C9EFF)),
    Plugins("Plugins", "NetEase lyrics, effects, and commands", Icons.Rounded.Extension, Color(0xFF7FC8B8)),
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
    if (isTwoPane()) {
        TwoPaneSettings(open)
        return
    }
    LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(bottom = 24.dp)) {
        item { LargeTitle("Settings") }
        item { SectionList(null) { open(it.route()) } }
    }
}

/**
 * Settings on a tablet, as Android's own Settings there: the sections in a list at the left, and
 * the one chosen open beside it.
 */
@Composable
private fun TwoPaneSettings(open: (Route) -> Unit) {
    var chosen by rememberSaveable { mutableStateOf(SettingsSection.Library) }
    Row(Modifier.fillMaxSize()) {
        LazyColumn(Modifier.width(380.dp).fillMaxHeight(), contentPadding = PaddingValues(bottom = 24.dp)) {
            item { LargeTitle("Settings") }
            item { SectionList(chosen) { chosen = it } }
        }
        // The section is not a page of its own here, so it has no way back of its own.
        Box(Modifier.weight(1f).fillMaxHeight()) {
            CompositionLocalProvider(LocalCanGoBack provides false) {
                AnimatedContent(chosen, transitionSpec = { fadeIn(tween(200)) togetherWith fadeOut(tween(120)) }, label = "section") { section ->
                    when (section) {
                        SettingsSection.Plugins -> PluginsScreen(open)
                        SettingsSection.Computer -> ConnectScreen()
                        SettingsSection.Sync -> SyncScreen()
                        SettingsSection.WhatsNew -> WhatsNewScreen()
                        else -> SettingsSectionScreen(section, open)
                    }
                }
            }
        }
    }
}

/** The sections, each with its icon and what is in it; `chosen` is lit in two panes. */
@Composable
private fun SectionList(chosen: SettingsSection?, onChoose: (SettingsSection) -> Unit) {
    // Discord only in builds that have Discord's library.
    val sections = SettingsSection.entries.filter { it != SettingsSection.Discord || Discord.available }
    // The tints are made for the dark pages; on a light page they are deepened to read.
    val light = MaterialTheme.colorScheme.background.luminance() > 0.5f
    Group {
        sections.forEachIndexed { i, section ->
            val tint = if (light) androidx.compose.ui.graphics.lerp(section.tint, Color.Black, 0.45f) else section.tint
            Row(
                Modifier
                    .fillMaxWidth()
                    .background(if (section == chosen) MaterialTheme.colorScheme.secondaryContainer else Color.Transparent)
                    // Beside its page on a tablet, TalkBack says which section is open.
                    .then(
                        if (chosen != null) Modifier.selectable(section == chosen, role = Role.Tab) { onChoose(section) }
                        else Modifier.clickable { onChoose(section) },
                    )
                    .padding(horizontal = 16.dp, vertical = LocalRows.current.pad + 2.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(16.dp),
            ) {
                Box(
                    Modifier.size(42.dp).clip(iconShape()).background(tint.copy(alpha = 0.18f)),
                    contentAlignment = Alignment.Center,
                ) { Icon(section.icon, contentDescription = null, tint = tint) }
                Column(Modifier.weight(1f)) {
                    Text(section.title, style = MaterialTheme.typography.titleMedium)
                    Text(section.summary, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                }
                if (chosen == null) Icon(Icons.AutoMirrored.Rounded.KeyboardArrowRight, contentDescription = null, tint = MaterialTheme.colorScheme.onSurfaceVariant)
            }
            if (i < sections.size - 1) Divider()
        }
    }
}

@Composable
fun SettingsSectionScreen(section: SettingsSection, open: (Route) -> Unit) {
    LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(bottom = 32.dp)) {
        item { LargeTitle(section.title) }
        item {
            when (section) {
                SettingsSection.Library -> LibrarySettings(open)
                SettingsSection.Servers -> MusicServers()
                SettingsSection.Playback -> PlaybackSettingsPage()
                SettingsSection.Sound -> SoundSettingsPage()
                SettingsSection.Appearance -> AppearanceSettings(open)
                SettingsSection.Scrobbling -> ScrobblingSettings()
                SettingsSection.Discord -> DiscordSettingsPage()
                SettingsSection.About -> AboutPage(open)
                else -> {}
            }
        }
    }
}

// ---------- Building blocks

/**
 * Rows grouped on one rounded surface, as in Android 16's own Settings: the group's ends are
 * rounded, and its rows stand a hair apart (each [Divider] is a gap the page shows through).
 * Its heading is plain: colour is kept for what is chosen.
 */
@Composable
internal fun Group(title: String? = null, footer: String? = null, content: @Composable () -> Unit) {
    Column(Modifier.padding(horizontal = 16.dp, vertical = 8.dp)) {
        if (title != null) {
            Text(
                title,
                style = MaterialTheme.typography.titleSmall,
                fontWeight = FontWeight.SemiBold,
                color = MaterialTheme.colorScheme.onSurface,
                modifier = Modifier.padding(start = 8.dp, bottom = 8.dp, top = 10.dp).semantics { heading() },
            )
        }
        Column(
            Modifier
                .fillMaxWidth()
                .clip(RoundedCornerShape(24.dp))
                .background(groupColor())
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

/** Between two rows of a [Group]: a gap of the page, rather than a line. */
@Composable
internal fun Divider() = Box(Modifier.fillMaxWidth().height(2.dp).background(MaterialTheme.colorScheme.background))

/** A setting that is on or off: the whole row is the one switch, one stop for a remote, and TalkBack says on or off. */
@Composable
internal fun SwitchRow(title: String, summary: String?, checked: Boolean, enabled: Boolean = true, icon: ImageVector? = null, onChange: (Boolean) -> Unit) {
    val haptics = rememberHaptics()
    SettingRow(
        icon, title, summary,
        Modifier.toggleable(checked, enabled = enabled, role = Role.Switch) { haptics(false); onChange(it) },
        enabled = enabled,
    ) { Switch(checked = checked, enabled = enabled, onCheckedChange = null) }
}

/** A setting on a scale: its row (with the value at the right), and the slider under it. */
@Composable
internal fun SliderRow(
    title: String,
    shown: String,
    value: Float,
    range: ClosedFloatingPointRange<Float>,
    steps: Int = 0,
    summary: String? = null,
    enabled: Boolean = true,
    icon: ImageVector? = null,
    onChange: (Float) -> Unit,
    onDone: () -> Unit,
) {
    Column {
        SettingRow(icon, title, summary, Modifier, enabled) {
            Text(shown, style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
        Slider(
            value = value, onValueChange = onChange, onValueChangeFinished = onDone, valueRange = range, steps = steps, enabled = enabled,
            modifier = Modifier.padding(start = if (icon != null) 68.dp else 16.dp, end = 16.dp, bottom = 8.dp).named(title, shown),
        )
    }
}

/** A few values side by side, one chosen: inside a row that opens. */
@Composable
internal fun <T> Segments(options: List<Pair<T, String>>, current: T, onChange: (T) -> Unit) {
    SingleChoiceSegmentedButtonRow(Modifier.fillMaxWidth()) {
        options.forEachIndexed { i, (value, label) ->
            SegmentedButton(
                selected = value == current,
                onClick = { onChange(value) },
                shape = SegmentedButtonDefaults.itemShape(i, options.size),
                icon = {},
            ) {
                // Smaller rather than cut, where large text leaves a segment too narrow.
                Text(
                    label,
                    maxLines = 1,
                    softWrap = false,
                    autoSize = androidx.compose.foundation.text.TextAutoSize.StepBased(minFontSize = 9.sp, maxFontSize = MaterialTheme.typography.labelLarge.fontSize),
                )
            }
        }
    }
}

/** A row that goes somewhere: its icon, name and line, what it is set to, and an arrow. */
@Composable
internal fun LinkRow(title: String, summary: String?, icon: ImageVector? = null, value: String? = null, onClick: () -> Unit) {
    SettingRow(icon, title, summary, Modifier.clickable(role = Role.Button, onClick = onClick)) {
        if (value != null) Text(value, style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.onSurfaceVariant, maxLines = 1)
        Icon(Icons.AutoMirrored.Rounded.KeyboardArrowRight, contentDescription = null, tint = MaterialTheme.colorScheme.onSurfaceVariant)
    }
}

/** A longer explanation, inside a row that opens. */
@Composable
internal fun MoreText(text: String) {
    Text(text, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurface)
}

/**
 * Runs a call to the core off the main thread, one at a time in order (settings saved the wrong
 * way round would undo the newer), then `then` on the main thread; a failure says so.
 */
private fun background(then: () -> Unit = {}, work: () -> Unit) {
    NeedleApp.instance.scope.launch(NeedleApp.instance.saves) {
        runCatching { work() }
            .onSuccess { withContext(Dispatchers.Main) { then() } }
            .onFailure { showMessage(it.message ?: "That did not work") }
    }
}

// ---------- Library

/**
 * A folder chosen in Android's picker, as a path Needle can read ("primary:Music" is the
 * phone's own Music folder; other volumes, like an SD card, have their own id).
 */
internal fun treePath(uri: Uri): String? {
    val id = runCatching { DocumentsContract.getTreeDocumentId(uri) }.getOrNull() ?: return null
    val volume = id.substringBefore(':')
    val rest = id.substringAfter(':', "")
    val root = if (volume == "primary") Environment.getExternalStorageDirectory().path else "/storage/$volume"
    val path = if (rest.isEmpty()) root else "$root/$rest"
    // Other places in the picker (Downloads, cloud drives) are not folders Needle can read.
    if (!File(path).isDirectory) {
        showMessage("Needle can read folders on the phone or an SD card. Choose one there.")
        return null
    }
    return path
}

@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun LibrarySettings(open: (Route) -> Unit) {
    val foldersLoad = rememberLoaded { folders() }
    val folders by foldersLoad
    val scan by NeedleApp.instance.scan.collectAsState()
    val countLoad = rememberLoaded(scan.running) { songCount() }
    val count by countLoad
    var playback by remember { mutableStateOf<PlaybackSettings?>(null) }
    var removing by remember { mutableStateOf<String?>(null) }
    LaunchedEffect(Unit) { playback = withContext(Dispatchers.IO) { runCatching { core.playbackSettings() }.getOrNull() } }
    val pick = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocumentTree()) { uri ->
        val path = uri?.let(::treePath) ?: return@rememberLauncherForActivityResult
        // The lists are read again once the folder is in (a scan may hold it back a moment).
        background(then = { NeedleApp.instance.libraryChanged() }) { core.addFolder(path) }
    }
    val music = File(Environment.getExternalStorageDirectory(), Environment.DIRECTORY_MUSIC).path

    MusicAccessNotice(Modifier.padding(horizontal = 16.dp, vertical = 8.dp))
    FolderCoversAccess(Modifier.padding(horizontal = 16.dp, vertical = 8.dp))
    Group("Music folders", footer = "Needle reads them again each time it starts.") {
        folders.orEmpty().forEach { folder ->
            val name = folder.substringAfterLast('/').ifBlank { folder }
            SettingRow(Icons.Rounded.Folder, name, folder) {
                IconButton(onClick = { removing = folder }) {
                    Icon(Icons.Rounded.Delete, contentDescription = "Stop reading $name", tint = MaterialTheme.colorScheme.onSurfaceVariant)
                }
            }
            Divider()
        }
        LinkRow("Add a folder", "On the phone or an SD card", Icons.Rounded.CreateNewFolder) { tryOpen("file picker") { pick.launch(null) } }
        if (folders?.contains(music) == false) {
            Divider()
            LinkRow("Use the Music folder", "The usual place for music files", Icons.Rounded.LibraryMusic) {
                background(then = { NeedleApp.instance.libraryChanged() }) { core.addFolder(music) }
            }
        }
    }
    Group("Reading") {
        Column(Modifier.animateContentSize()) {
            SettingRow(
                Icons.Rounded.Sync,
                if (scan.running) "Reading your music" else "${count(count?.toInt() ?: 0, "song")} in your library",
                if (scan.running) "${scan.scanned} files, ${scan.imported} new" else scan.error,
            ) {
                if (!scan.running && folders?.isNotEmpty() == true) TextButton(onClick = { core.rescan() }) { Text("Read again") }
            }
            if (scan.running) LinearWavyProgressIndicator(modifier = Modifier.fillMaxWidth().padding(start = 68.dp, end = 16.dp, bottom = 14.dp))
        }
    }
    val context = LocalContext.current
    val backup = rememberLauncherForActivityResult(ActivityResultContracts.CreateDocument("application/octet-stream")) { uri ->
        uri ?: return@rememberLauncherForActivityResult
        NeedleApp.instance.scope.launch(Dispatchers.IO) {
            runCatching {
                val file = File(context.cacheDir, "needle-backup.db")
                core.backupLibrary(file.absolutePath)
                context.contentResolver.openOutputStream(uri)!!.use { out -> file.inputStream().use { it.copyTo(out) } }
                file.delete()
            }.onSuccess { showMessage("Saved a backup of your library") }.onFailure { showMessage(it.message ?: "Could not save the backup") }
        }
    }
    Group("Tools") {
        LinkRow("Fix my library", "Duplicates, covers, tags, tidy files", Icons.Rounded.Build) { open(Route.FixLibrary) }
        Divider()
        LinkRow("Import", "Plays, stars, and playlists from elsewhere", Icons.Rounded.Download) { open(Route.Import) }
        Divider()
        LinkRow("Back up the library", "Plays, stars, playlists, settings, in one file", Icons.Rounded.Backup) {
            tryOpen("file picker") { backup.launch("needle-library-${java.time.LocalDate.now()}.db") }
        }
    }
    playback?.let { p ->
        Group("Online") {
            SwitchRow("Look up lyrics and artists", "LRCLIB for lyrics, fanart.tv for artists", p.onlineMedia, icon = Icons.Rounded.Language) {
                val next = p.copy(onlineMedia = it)
                playback = next
                background { core.setPlaybackSettings(next) }
            }
            Divider()
            ExpandRow(Icons.Rounded.Info, "What is sent", "Only what a lookup needs") {
                MoreText("When a song has no lyrics of its own, Needle asks LRCLIB for them with the song's title, artist, album, and length. For an artist's page, Needle asks MusicBrainz for the artist's ID by name, and sends fanart.tv only that ID. Nothing else is sent.")
            }
            if (p.onlineMedia) {
                Divider()
                FanartKeyRow()
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
                    background { core.removeFolder(folder); NeedleApp.instance.libraryChanged() }
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
    Group("Between songs") {
        var fade by remember { mutableStateOf(s.crossfade) }
        SliderRow(
            "Crossfade",
            if (fade < 0.5f) "Off" else "${fade.toInt()} s",
            fade,
            0f..12f,
            steps = 11,
            summary = "Songs overlap; albums stay gapless",
            icon = Icons.Rounded.SwapHoriz,
            onChange = { fade = it },
            onDone = { save(s.copy(crossfade = fade)) },
        )
        Divider()
        val loudness = when {
            !s.replayGain -> 0
            s.albumGain -> 2
            else -> 1
        }
        ExpandRow(Icons.Rounded.GraphicEq, "Even out loudness", "From the songs' ReplayGain tags", listOf("Off", "Song", "Album")[loudness]) {
            Segments(listOf(0 to "Off", 1 to "Song", 2 to "Album"), loudness) { i -> save(s.copy(replayGain = i > 0, albumGain = i == 2)) }
            MoreText("Album keeps an album's quiet and loud songs as the artist made them.")
        }
    }
    Group("When you choose a song") {
        SwitchRow("Open the player", "Shown as soon as a song starts", app.openPlayerOnPlay, icon = Icons.Rounded.OpenInFull) { on ->
            NeedleApp.instance.updateApp { it.copy(openPlayerOnPlay = on) }
        }
    }
    Group("When Up next ends") {
        var query by remember(s.autoplayQuery) { mutableStateOf(s.autoplayQuery) }
        // Checked off the main thread as it is typed.
        val problem by androidx.compose.runtime.produceState<String?>(null, query) {
            value = if (query.isBlank()) null else withContext(Dispatchers.IO) {
                runCatching { if (core.isRule(query)) core.checkRule(query) else null }.getOrNull()
            }
        }
        val keep = { if (query != s.autoplayQuery && problem == null) { save(s.copy(autoplayQuery = query.trim())); showMessage("Saved") } }
        ExpandRow(Icons.Rounded.AllInclusive, "Keep playing", "Songs that match a search", s.autoplayQuery.ifBlank { "Off" }) {
            OutlinedTextField(
                query, { query = it },
                label = { Text("Search") },
                placeholder = { Text("rating >= 4") },
                singleLine = true,
                isError = problem != null,
                supportingText = { Text(problem ?: "Leave it empty to stop when Up next ends.") },
                trailingIcon = {
                    if (query != s.autoplayQuery && problem == null) TextButton(onClick = keep) { Text("Save") }
                },
                // The keyboard's Done saves too: on a TV the Save beside the words is out of the remote's reach.
                keyboardOptions = KeyboardOptions(imeAction = androidx.compose.ui.text.input.ImeAction.Done),
                keyboardActions = androidx.compose.foundation.text.KeyboardActions(onDone = { keep() }),
                modifier = Modifier.remoteEscape().fillMaxWidth(),
            )
        }
    }
    Group("Radio") {
        val measuredLoad = rememberLoaded(s.soundAnalysis) { measured() }
        val measured by measuredLoad
        SwitchRow(
            "Measure songs for radio",
            measured?.let { m -> if (m.size == 2 && m[1] > 0u) "${m[0]} of ${m[1]} measured" else null } ?: "How each song sounds, read on this phone",
            s.soundAnalysis,
            icon = Icons.Rounded.Radio,
        ) { save(s.copy(soundAnalysis = it)) }
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
    val context = LocalContext.current
    var mine by remember { mutableStateOf(core.myPresets()) }
    var naming by remember { mutableStateOf(false) }
    fun reload() {
        NeedleApp.instance.scope.launch {
            sound = withContext(Dispatchers.IO) { runCatching { core.soundSettings() }.getOrNull() }
            mine = withContext(Dispatchers.IO) { core.myPresets() }
        }
    }
    val correction = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument()) { uri ->
        pickedFile(context, uri, { core.loadEqFile(it) }) {
            showMessage("Loaded a correction with ${count(it.toInt(), "band")}")
            reload()
        }
    }
    Group("Equalizer", footer = if (s.eq && s.mode == "parametric") "Loads AutoEq (ParametricEQ.txt) or Equalizer APO files." else null) {
        SwitchRow("Equalizer", "Shape the sound", s.eq, icon = Icons.Rounded.Equalizer) { save(s.copy(eq = it)) }
        AnimatedVisibility(s.eq) {
            Column {
                SingleChoiceSegmentedButtonRow(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 8.dp)) {
                    listOf("graphic" to "Ten bands", "parametric" to "Parametric").forEachIndexed { i, (mode, label) ->
                        SegmentedButton(
                            selected = s.mode == mode,
                            onClick = { save(s.copy(mode = mode)) },
                            shape = SegmentedButtonDefaults.itemShape(i, 2),
                        ) { Text(label) }
                    }
                }
                LazyRow(
                    contentPadding = PaddingValues(horizontal = 16.dp),
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    items(mine) { name ->
                        FilterChip(
                            selected = s.preset == name,
                            onClick = { background(then = { reload() }) { core.usePreset(name) } },
                            label = { Text(name) },
                            leadingIcon = { Icon(Icons.Rounded.Star, contentDescription = "Yours", modifier = Modifier.size(16.dp)) },
                        )
                    }
                    if (s.mode == "graphic") items(presets) { preset ->
                        FilterChip(
                            selected = s.preset == preset.name,
                            onClick = { save(s.copy(preset = preset.name, preamp = preset.preamp, bands = preset.bands)) },
                            label = { Text(preset.name) },
                        )
                    }
                }
                if (s.mode == "graphic") {
                    EqualizerBands(s.bands, bands) { changed -> save(s.copy(bands = changed, preset = "Custom")) }
                } else {
                    ParametricBands(s.parametric) { changed -> save(s.copy(parametric = changed, preset = "Custom")) }
                }
                var preamp by remember(s.preamp) { mutableStateOf(s.preamp) }
                SliderRow(
                    "Preamp",
                    "%+.1f dB".format(preamp),
                    preamp,
                    -12f..6f,
                    summary = "Lower it if boosts make songs clip",
                    icon = Icons.Rounded.VolumeDown,
                    onChange = { preamp = it },
                    onDone = { save(s.copy(preamp = preamp)) },
                )
                Row(Modifier.padding(horizontal = 8.dp, vertical = 4.dp)) {
                    TextButton(onClick = { naming = true }) { Text("Save as a preset") }
                    if (s.preset in mine) TextButton(onClick = { background { core.deletePreset(s.preset) }; mine = mine - s.preset }) { Text("Delete ${s.preset}") }
                }
                TextButton(onClick = { tryOpen("file picker") { correction.launch(arrayOf("text/plain", "*/*")) } }, modifier = Modifier.padding(start = 8.dp, bottom = 8.dp)) {
                    Text("Load a headphone correction…")
                }
            }
        }
    }
    if (naming) {
        var name by remember { mutableStateOf("") }
        AlertDialog(
            onDismissRequest = { naming = false },
            title = { Text("Save as a preset") },
            text = { OutlinedTextField(name, { name = it }, label = { Text("Name") }, singleLine = true, modifier = Modifier.remoteEscape()) },
            confirmButton = {
                TextButton(enabled = name.isNotBlank(), onClick = {
                    naming = false
                    NeedleApp.instance.scope.launch {
                        withContext(Dispatchers.IO) { runCatching { core.savePreset(name) } }
                            .onSuccess { showMessage("Saved $name") }
                            .onFailure { showMessage(it.message ?: "Could not save it") }
                        reload()
                    }
                }) { Text("Save") }
            },
            dismissButton = { TextButton(onClick = { naming = false }) { Text("Cancel") } },
        )
    }
    Group("Headphones and speakers") {
        SwitchRow("Crossfeed", "Sides blend a little, as with speakers", s.crossfeed, icon = Icons.Rounded.Headphones) {
            save(s.copy(crossfeed = it))
        }
        Divider()
        SwitchRow("Mono", "Both sides the same, for one earbud", s.mono, icon = Icons.Rounded.Hearing) { save(s.copy(mono = it)) }
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
            icon = Icons.Rounded.Tune,
            onChange = { balance = if (kotlin.math.abs(it) < 0.04f) 0f else it },
            onDone = { save(s.copy(balance = balance)) },
        )
    }
    PluginEffects()
}

/**
 * Ten upright bars, dragged up and down, with a line through their tops. With a remote or keys,
 * each bar takes the focus; OK takes hold of it, up and down then move it half a decibel, and
 * OK or Back lets go. Not held, up and down move on through the page as anywhere else.
 */
@Composable
private fun EqualizerBands(gains: List<Float>, frequencies: List<Float>, onChange: (List<Float>) -> Unit) {
    var live by remember(gains) { mutableStateOf(gains) }
    val haptics = rememberHaptics()
    // A drag outlives the composition it began in: it saves through the latest settings.
    val save by androidx.compose.runtime.rememberUpdatedState(onChange)
    Row(
        Modifier.fillMaxWidth().height(220.dp).padding(horizontal = 12.dp, vertical = 16.dp),
        horizontalArrangement = Arrangement.SpaceBetween,
    ) {
        live.forEachIndexed { i, gain ->
            Column(Modifier.weight(1f), horizontalAlignment = Alignment.CenterHorizontally) {
                // Half steps show as such (+1.5); whole ones without a point (+2).
                var held by remember { mutableStateOf(false) }
                // Back lets go of a held band, before it would leave the page.
                androidx.activity.compose.BackHandler(enabled = held) { held = false }
                Text(
                    if (gain % 1f == 0f) "%+.0f".format(gain) else "%+.1f".format(gain),
                    style = MaterialTheme.typography.labelSmall,
                    color = if (held) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurfaceVariant,
                    maxLines = 1,
                    softWrap = false,
                )
                val focus = remember { androidx.compose.foundation.interaction.MutableInteractionSource() }
                val nudge: (Float) -> Unit = { by ->
                    live = live.toMutableList().also { it[i] = (it[i] + by).coerceIn(-12f, 12f) }
                    save(live)
                }
                BoxWithConstraints(
                    Modifier
                        .weight(1f)
                        // The band's whole share of the row to take hold of, not a thin strip.
                        .fillMaxWidth()
                        .padding(vertical = 6.dp)
                        .focusRing(focus, 14.dp)
                        .onFocusChanged { if (!it.isFocused) held = false }
                        .onKeyEvent {
                            val down = it.type == androidx.compose.ui.input.key.KeyEventType.KeyDown
                            when (it.key) {
                                androidx.compose.ui.input.key.Key.DirectionCenter, androidx.compose.ui.input.key.Key.Enter, androidx.compose.ui.input.key.Key.NumPadEnter -> {
                                    if (down) held = !held
                                    true
                                }
                                androidx.compose.ui.input.key.Key.Back, androidx.compose.ui.input.key.Key.Escape -> {
                                    if (!held) return@onKeyEvent false
                                    if (down) held = false
                                    true
                                }
                                androidx.compose.ui.input.key.Key.DirectionUp, androidx.compose.ui.input.key.Key.DirectionDown -> {
                                    if (!held) return@onKeyEvent false
                                    if (down) nudge(if (it.key == androidx.compose.ui.input.key.Key.DirectionUp) 0.5f else -0.5f)
                                    true
                                }
                                else -> false
                            }
                        }
                        .semantics {
                            contentDescription = frequencies.getOrNull(i)?.let { f -> if (f >= 1000f) "${(f / 1000f).let { k -> if (k % 1f == 0f) k.toInt().toString() else k.toString() }} kHz" else "${f.toInt()} Hz" } ?: "Band ${i + 1}"
                            progressBarRangeInfo = androidx.compose.ui.semantics.ProgressBarRangeInfo(live[i], -12f..12f, 48)
                            setProgress { v -> nudge(((Math.round(v * 2) / 2f) - live[i])); true }
                        }
                        .focusable(interactionSource = focus)
                        .pointerInput(i, gains) {
                            detectVerticalDragGestures(
                                onDragEnd = { haptics(false); save(live) },
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

/**
 * Appearance, calm: pictures only where the choice is how Needle looks (the themes), small grey
 * sketches for the choices of layout (only the part that changes in the accent), and plain rows
 * with a switch for the rest.
 */
@OptIn(androidx.compose.material3.ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun AppearanceSettings(open: (Route) -> Unit) {
    val app by NeedleApp.instance.app.collectAsState()
    val update = NeedleApp.instance::updateApp
    val context = LocalContext.current
    var themes by remember { mutableStateOf(core.themes()) }
    val importTheme = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument()) { uri ->
        pickedFile(context, uri, { core.importTheme(it) }) { name ->
            themes = core.themes()
            NeedleApp.instance.themeVersion.value++
            showMessage("Added $name")
        }
    }
    Group("Look") {
        LookCards(BuiltInLooks + themes.map(::lookCard), app.theme) { id -> update { it.copy(theme = id) } }
        Divider()
        LinkRow("New theme", "Your own colours", Icons.Rounded.Add) { open(Route.ThemeEditor(null)) }
        Divider()
        LinkRow("Add a theme file", "From the desktop or needle.nnx.fyi/themes", Icons.Rounded.FileOpen) { tryOpen("file picker") { importTheme.launch(arrayOf("*/*")) } }
        if (themes.any { it.id == app.theme }) {
            Divider()
            LinkRow("Edit this theme", themes.firstOrNull { it.id == app.theme }?.name, Icons.Rounded.Edit) { open(Route.ThemeEditor(app.theme)) }
        }
    }
    // A theme of your own sets its accent itself, so these would do nothing while it is on.
    val ownTheme = app.theme !in listOf("night", "midnight", "day", "light")
    Group("Accent") {
        if (ownTheme) {
            SettingRow(Icons.Rounded.Palette, "Set by your theme", "Choose Night, Midnight, or Day to pick one")
        } else {
            SettingRow(Icons.Rounded.Palette, "Accent", "Where no cover gives the colour")
            val swatches = listOf(
                "" to Color(0xFFE2B46C), "#E5484D" to Color(0xFFE5484D), "#E93D82" to Color(0xFFE93D82),
                "#8E4EC6" to Color(0xFF8E4EC6), "#3E63DD" to Color(0xFF3E63DD), "#0090FF" to Color(0xFF0090FF),
                "#12A594" to Color(0xFF12A594), "#46A758" to Color(0xFF46A758), "#F76B15" to Color(0xFFF76B15),
            )
            // What TalkBack calls each one.
            val names = listOf("Amber", "Red", "Pink", "Purple", "Blue", "Sky blue", "Teal", "Green", "Orange")
            // Each a full finger's width to tap, onto a second line where the screen is narrow.
            androidx.compose.foundation.layout.FlowRow(
                Modifier.fillMaxWidth().padding(start = 60.dp, end = 12.dp, bottom = 10.dp),
                horizontalArrangement = Arrangement.spacedBy(2.dp),
            ) {
                swatches.forEachIndexed { n, (id, color) ->
                    val chosen = app.accentColor.equals(id, ignoreCase = true)
                    Box(
                        Modifier
                            .size(46.dp)
                            .clip(CircleShape)
                            .selectable(chosen, role = Role.RadioButton) { update { it.copy(accentColor = id) } }
                            .semantics { contentDescription = names[n] },
                        contentAlignment = Alignment.Center,
                    ) {
                        Box(
                            Modifier.size(30.dp).clip(CircleShape).background(color)
                                .border(if (chosen) 2.dp else 0.dp, MaterialTheme.colorScheme.onSurface, CircleShape),
                            contentAlignment = Alignment.Center,
                        ) {
                            if (chosen) Icon(Icons.Rounded.Check, contentDescription = null, tint = Color.White, modifier = Modifier.size(16.dp))
                        }
                    }
                }
            }
        }
    }
    Group("Colour") {
        SwitchRow("Pages in their cover's colours", "Albums, playlists, and artists", app.coverColors, icon = Icons.Rounded.ColorLens) { on -> update { it.copy(coverColors = on) } }
        Divider()
        SwitchRow("Colours from what's playing", "The whole app takes the song's colours", app.ambientColors, icon = Icons.Rounded.AutoAwesome) { on -> update { it.copy(ambientColors = on) } }
        Divider()
        SwitchRow("Mini player in the song's colour", "Otherwise it matches the page", app.miniPlayerColored, icon = Icons.Rounded.SmartDisplay) { on -> update { it.copy(miniPlayerColored = on) } }
        if (Build.VERSION.SDK_INT >= 31) {
            Divider()
            SwitchRow("Colours from your wallpaper", "In place of Needle's amber", app.wallpaperColors, icon = Icons.Rounded.Wallpaper) { on -> update { it.copy(wallpaperColors = on) } }
        }
    }
    Group("Player") {
        SketchRow(Icons.Rounded.Album, "Cover", "Its shape in the player", listOf(
            Sketch("square", "Square") { accent, grey -> PlayerSketch(grey) { Box(Modifier.size(22.dp).clip(RoundedCornerShape(4.dp)).background(accent)) } },
            Sketch("round", "Record") { accent, grey -> PlayerSketch(grey) { Box(Modifier.size(22.dp).clip(CircleShape).background(accent)) } },
            Sketch("shape", "Shape") { accent, grey -> PlayerSketch(grey) { Box(Modifier.size(22.dp).clip(androidx.compose.material3.MaterialShapes.Cookie12Sided.toShape()).background(accent)) } },
        ), app.coverShape) { id -> update { it.copy(coverShape = id) } }
        Divider()
        SketchRow(Icons.Rounded.PlayCircle, "Buttons", "Play, pause, and skip", listOf(
            Sketch("expressive", "Expressive") { accent, grey ->
                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(3.dp)) {
                    SketchBar(12, 10, grey, 5); SketchBar(20, 16, accent, 5); SketchBar(12, 10, grey, 5)
                }
            },
            Sketch("round", "Round") { accent, grey ->
                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(3.dp)) {
                    SketchBar(11, 11, grey, 6); SketchBar(18, 18, accent, 9); SketchBar(11, 11, grey, 6)
                }
            },
            Sketch("minimal", "Minimal") { accent, grey ->
                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(0.dp)) {
                    Icon(Icons.Rounded.SkipPrevious, null, Modifier.size(14.dp), tint = grey)
                    Icon(Icons.Rounded.PlayArrow, null, Modifier.size(22.dp), tint = accent)
                    Icon(Icons.Rounded.SkipNext, null, Modifier.size(14.dp), tint = grey)
                }
            },
        ), app.controlStyle) { id -> update { it.copy(controlStyle = id) } }
        Divider()
        SketchRow(Icons.Rounded.SkipNext, "Beside play", "Skip a song, or jump 10 seconds", listOf(
            Sketch("skip", "Skip songs") { accent, grey ->
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Icon(Icons.Rounded.SkipPrevious, null, Modifier.size(16.dp), tint = accent)
                    Icon(Icons.Rounded.PlayArrow, null, Modifier.size(20.dp), tint = grey)
                    Icon(Icons.Rounded.SkipNext, null, Modifier.size(16.dp), tint = accent)
                }
            },
            Sketch("jump", "10 seconds") { accent, grey ->
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Icon(Icons.Rounded.Replay10, null, Modifier.size(16.dp), tint = accent)
                    Icon(Icons.Rounded.PlayArrow, null, Modifier.size(20.dp), tint = grey)
                    Icon(Icons.Rounded.Forward10, null, Modifier.size(16.dp), tint = accent)
                }
            },
        ), app.sideButtons) { id -> update { it.copy(sideButtons = id) } }
        Divider()
        SketchRow(Icons.Rounded.LinearScale, "Seek bar", "The line of the song's time", SeekStyle.entries.map { style ->
            Sketch(style.id, style.label) { accent, grey ->
                Box(Modifier.width(56.dp)) { ProgressLine(0.6f, true, accent, grey, style = style) }
            }
        }, app.seekStyle) { id -> update { it.copy(seekStyle = id) } }
        Divider()
        SwitchRow("Immersive cover", "Fills the top of the player, edge to edge", app.immersiveCover, icon = Icons.Rounded.Fullscreen) { on -> update { it.copy(immersiveCover = on) } }
        Divider()
        SwitchRow("Moving covers", "cover.mp4, .webp, or .gif beside the songs", app.animatedCovers, icon = Icons.Rounded.Movie) { on -> update { it.copy(animatedCovers = on) } }
        Divider()
        SwitchRow("Volume in the player", "A slider under the buttons", app.volumeSlider, icon = Icons.Rounded.VolumeUp) { on ->
            update { it.copy(volumeSlider = on) }
        }
        Divider()
        SwitchRow("Swipe the cover to skip", "Left for next, right for back", app.swipeCover, icon = Icons.Rounded.Swipe) { on ->
            update { it.copy(swipeCover = on) }
        }
    }
    Group("Backdrop") {
        SwitchRow(
            "Cover behind the player",
            if (Build.VERSION.SDK_INT < 31) "Its colour (Android 12 blurs it)" else if (app.immersiveCover) "Blurred, behind lyrics and the queue" else "Blurred, behind the player",
            app.coverBackdrop,
            icon = Icons.Rounded.BlurOn,
        ) { on -> update { it.copy(coverBackdrop = on) } }
        Divider()
        SwitchRow("Moving backdrop", "The blurred cover turns slowly", app.movingBackdrop, icon = Icons.Rounded.Animation) { on -> update { it.copy(movingBackdrop = on) } }
        Divider()
        var blur by remember(app.backdropBlur) { mutableFloatStateOf(app.backdropBlur) }
        SliderRow("Softness", "${blur.toInt()}", blur, 20f..120f, summary = "How blurred the cover is", icon = Icons.Rounded.BlurLinear, onChange = { blur = it }, onDone = { update { it.copy(backdropBlur = blur) } })
        Divider()
        var grain by remember(app.grain) { mutableFloatStateOf(app.grain) }
        SliderRow(
            "Grain",
            if (grain < 0.05f) "None" else "${(grain * 100).toInt()}%",
            grain, 0f..1f,
            summary = "A fine film grain over the app",
            icon = Icons.Rounded.Grain,
            onChange = { grain = it },
            onDone = { update { it.copy(grain = if (grain < 0.05f) 0f else grain) } },
        )
    }
    Group("Song lists") {
        SketchRow(Icons.Rounded.ViewHeadline, "Rows", "How close songs sit", listOf("compact" to "Compact", "comfortable" to "Comfortable", "spacious" to "Spacious").map { (id, label) ->
            Sketch(id, label) { accent, grey ->
                val gap = when (id) { "compact" -> 2.dp; "spacious" -> 7.dp; else -> 4.dp }
                Column(verticalArrangement = Arrangement.spacedBy(gap)) {
                    repeat(3) {
                        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                            SketchBar(7, 7, accent); SketchBar(36, 4, grey)
                        }
                    }
                }
            }
        }, app.density) { id -> update { it.copy(density = id) } }
        Divider()
        SwitchRow("Mark server songs", "A small server icon beside the artist", app.serverMark, icon = Icons.Rounded.Dns) { on -> update { it.copy(serverMark = on) } }
    }
    Group("Text and motion") {
        SketchRow(Icons.Rounded.Title, "Titles", "The font of page titles", listOf("flex" to "Poster", "fraunces" to "Serif", "system" to "Plain").map { (id, label) ->
            Sketch(id, label) { accent, _ ->
                Text("Aa", color = accent, style = MaterialTheme.typography.titleLarge.copy(fontFamily = titleFamilyFor(id), fontWeight = FontWeight.Black))
            }
        }, app.titleFont) { id -> update { it.copy(titleFont = id) } }
        Divider()
        var scale by remember(app.textScale) { mutableFloatStateOf(app.textScale) }
        SliderRow(
            "Text size",
            when {
                scale < 0.95f -> "Smaller"
                scale > 1.15f -> "Largest"
                scale > 1.05f -> "Larger"
                else -> "Usual"
            },
            scale, 0.85f..1.3f, steps = 8,
            summary = "Beside Android's own size",
            icon = Icons.Rounded.FormatSize,
            onChange = { scale = it },
            onDone = { update { it.copy(textScale = scale) } },
        )
        Divider()
        SwitchRow("Reduce motion", "Fades in place of slides and moving covers", app.reduceMotion, icon = Icons.Rounded.MotionPhotosOff) { on -> update { it.copy(reduceMotion = on) } }
        Divider()
        SwitchRow("Vibration", "A light tap on play, pause, and long presses", app.haptics, icon = Icons.Rounded.Vibration) { on -> update { it.copy(haptics = on) } }
    }
    Group("Lyrics") {
        SwitchRow("Follow the song", "The line playing is lit and in view", app.liveLyrics, icon = Icons.Rounded.Lyrics) { on -> update { it.copy(liveLyrics = on) } }
        Divider()
        SwitchRow("Keep the screen on", "While the lyrics show", app.keepScreenOnLyrics, icon = Icons.Rounded.LightMode) { on -> update { it.copy(keepScreenOnLyrics = on) } }
        Divider()
        SwitchRow("Lyrics in the mini player", "Mid-bar on a tablet, under it on a phone", app.miniPlayerLyrics, icon = Icons.Rounded.Subtitles) { on -> update { it.copy(miniPlayerLyrics = on) } }
    }
}

/** A tiny player for a sketch: grey lines of words, and [cover] (the part that changes). */
@Composable
private fun PlayerSketch(grey: Color, cover: @Composable () -> Unit) {
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
        cover()
        Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
            SketchBar(26, 4, grey)
            SketchBar(18, 4, grey)
            SketchBar(30, 3, grey)
        }
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
    var connectingLastfm by remember { mutableStateOf(false) }
    var connectingLb by remember { mutableStateOf(false) }
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
    var playback by remember { mutableStateOf<PlaybackSettings?>(null) }
    LaunchedEffect(Unit) { playback = withContext(Dispatchers.IO) { runCatching { core.playbackSettings() }.getOrNull() } }
    var key by remember { mutableStateOf("") }
    var secret by remember { mutableStateOf("") }
    // Last.fm's page, where Needle is allowed: opened at once with Needle's own key, else after
    // the key and secret of the person's own API account are in.
    fun openLastfm() {
        run {
            val url = core.lastfmBegin(key, secret)
            // Without a browser there is no page to allow Needle on; say so plainly.
            if (tryOpen("web browser to sign in with") { context.startActivity(Intent(Intent.ACTION_VIEW, Uri.parse(url)).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)) }) {
                waitingForBrowser = true
            }
            null
        }
    }

    Group("Services", footer = "Songs count once played past half, or four minutes.") {
        if (a.lastfmUser != null) {
            SwitchRow("Last.fm", if (a.lastfmUser!!.isBlank()) "Signed in" else "Signed in as ${a.lastfmUser}", a.lastfmOn, icon = Icons.Rounded.MusicNote) { on ->
                run { core.setScrobbling(on, null); null }
            }
            TextButton(onClick = { run { core.signOut("lastfm"); "Signed out of Last.fm." } }, modifier = Modifier.padding(start = 56.dp, bottom = 4.dp)) { Text("Sign out of Last.fm") }
        } else {
            SettingRow(Icons.Rounded.MusicNote, "Last.fm", if (waitingForBrowser) "Waiting for Last.fm" else "Not connected") {
                if (connectingLastfm || waitingForBrowser) {
                    TextButton(onClick = { connectingLastfm = false; waitingForBrowser = false }) { Text("Cancel") }
                } else {
                    FilledTonalButton(enabled = !busy, onClick = {
                        // With Needle's own key there is nothing to fill in: straight to Last.fm.
                        if (a.lastfmKeyBuiltIn) openLastfm() else connectingLastfm = true
                    }) { Text("Connect") }
                }
            }
            AnimatedVisibility(waitingForBrowser || connectingLastfm) {
                Column(Modifier.fillMaxWidth().padding(start = 68.dp, end = 16.dp, bottom = 16.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
                    if (waitingForBrowser) {
                        Text("Allow Needle on the Last.fm page, then come back here.", style = MaterialTheme.typography.bodyMedium)
                        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                            Button(enabled = !busy, onClick = {
                                run {
                                    val user = core.lastfmComplete()
                                    waitingForBrowser = false
                                    connectingLastfm = false
                                    if (user.isBlank()) "Signed in to Last.fm." else "Signed in as $user."
                                }
                            }) { Text("I allowed it") }
                            // The page was closed, or the key was wrong: ask for a new page.
                            TextButton(enabled = !busy, onClick = { waitingForBrowser = false; if (a.lastfmKeyBuiltIn) openLastfm() }) { Text("Open the page again") }
                        }
                    } else {
                        Text("This build needs your own Last.fm API account, from last.fm/api.", style = MaterialTheme.typography.bodyMedium)
                        OutlinedTextField(key, { key = it }, label = { Text("API key") }, singleLine = true, modifier = Modifier.remoteEscape().fillMaxWidth())
                        OutlinedTextField(secret, { secret = it }, label = { Text("Shared secret") }, singleLine = true, visualTransformation = PasswordVisualTransformation(), modifier = Modifier.remoteEscape().fillMaxWidth())
                        Button(enabled = !busy && key.isNotBlank() && secret.isNotBlank(), onClick = { openLastfm() }) { Text("Sign in with Last.fm") }
                    }
                }
            }
        }
        Divider()
        if (a.listenbrainzUser != null) {
            SwitchRow("ListenBrainz", if (a.listenbrainzUser!!.isBlank()) "Signed in" else "Signed in as ${a.listenbrainzUser}", a.listenbrainzOn, icon = Icons.Rounded.Headphones) { on ->
                run { core.setScrobbling(null, on); null }
            }
            TextButton(onClick = { run { core.signOut("listenbrainz"); "Signed out of ListenBrainz." } }, modifier = Modifier.padding(start = 56.dp, bottom = 4.dp)) { Text("Sign out of ListenBrainz") }
        } else {
            SettingRow(Icons.Rounded.Headphones, "ListenBrainz", "Not connected") {
                if (connectingLb) TextButton(onClick = { connectingLb = false }) { Text("Cancel") }
                else FilledTonalButton(enabled = !busy, onClick = { connectingLb = true }) { Text("Connect") }
            }
            AnimatedVisibility(connectingLb) {
                var token by remember { mutableStateOf("") }
                Column(Modifier.fillMaxWidth().padding(start = 68.dp, end = 16.dp, bottom = 16.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
                    OutlinedTextField(
                        token, { token = it }, label = { Text("User token") }, singleLine = true,
                        visualTransformation = PasswordVisualTransformation(),
                        keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Password),
                        supportingText = { Text("On listenbrainz.org, under Settings.") },
                        modifier = Modifier.remoteEscape().fillMaxWidth(),
                    )
                    Button(enabled = !busy && token.isNotBlank(), onClick = {
                        run {
                            val user = core.listenbrainzSignIn(token.trim())
                            connectingLb = false
                            if (user.isBlank()) "Signed in to ListenBrainz." else "Signed in as $user."
                        }
                    }) { Text("Sign in") }
                }
            }
        }
    }
    playback?.let { p ->
        Group("Names") {
            SwitchRow("Fix artist names", "Apple Music's spelling, such as KiiiKiii", p.scrobbleCorrections, icon = Icons.Rounded.Edit) { on ->
                val next = p.copy(scrobbleCorrections = on)
                playback = next
                background { core.setPlaybackSettings(next) }
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

// ---------- Discord

@Composable
private fun DiscordSettingsPage() {
    val context = LocalContext.current
    var settings by remember { mutableStateOf<DiscordSettings?>(null) }
    LaunchedEffect(Unit) { settings = withContext(Dispatchers.IO) { runCatching { core.discordSettings() }.getOrNull() } }
    val s = settings ?: return
    val installed = remember { Discord.installed(context) }
    fun save(next: DiscordSettings) {
        settings = next
        background {
            core.setDiscordSettings(next)
            // Discord may have come onto the phone since Needle started.
            if (next.on && Discord.installed(context)) core.startDiscord()
        }
    }
    Group {
        SwitchRow(
            "Show on Discord",
            if (installed) "What you play, on your profile" else "Discord is not on this phone",
            s.on,
            icon = Icons.Rounded.Forum,
        ) { save(s.copy(on = it)) }
        Divider()
        ExpandRow(Icons.Rounded.Info, "How it works", "Nothing to sign in to") {
            MoreText("Your profile shows what plays and a time bar, through the Discord app on this phone. Needle talks only to the Discord app.")
            MoreText("Discord can only show pictures from the web, so Needle finds the cover in Apple's iTunes catalog by artist and song name. Your files never leave this phone. Without a picture, Discord hides the third line.")
        }
    }
    if (!s.on) return
    fun <T> label(options: List<Pair<T, String>>, current: T) = options.firstOrNull { it.first == current }?.second
    Group("The card") {
        val titles = listOf("song" to "Song", "artist" to "Artist", "needle" to "Needle")
        ExpandRow(Icons.Rounded.Title, "Listening to …", "The card's heading", label(titles, s.title)) { Segments(titles, s.title) { save(s.copy(title = it)) } }
        Divider()
        val tops = listOf("artist" to "Artist", "song" to "Song", "album" to "Album")
        ExpandRow(Icons.Rounded.LooksOne, "First line", null, label(tops, s.top)) { Segments(tops, s.top) { save(s.copy(top = it)) } }
        Divider()
        val middles = listOf("song" to "Song", "artist" to "Artist", "album" to "Album", "none" to "None")
        ExpandRow(Icons.Rounded.LooksTwo, "Second line", null, label(middles, s.middle)) { Segments(middles, s.middle) { save(s.copy(middle = it)) } }
        Divider()
        val bottoms = listOf("album" to "Album", "song" to "Song", "artist" to "Artist", "none" to "None")
        ExpandRow(Icons.Rounded.Looks3, "Third line", null, label(bottoms, s.bottom)) { Segments(bottoms, s.bottom) { save(s.copy(bottom = it)) } }
    }
    Group("Picture") {
        SwitchRow("Look up covers", "From Apple's iTunes catalog, by name", s.covers, icon = Icons.Rounded.Image) { save(s.copy(covers = it)) }
        Divider()
        SwitchRow("Show the Needle logo", "A badge on the cover", s.logo, icon = Icons.Rounded.Verified) { save(s.copy(logo = it)) }
    }
    Group("When nothing plays") {
        SwitchRow("Show while paused", "The song stays, marked Paused", s.paused, icon = Icons.Rounded.Pause) { save(s.copy(paused = it)) }
        Divider()
        val idle = listOf(5u to "5 min", 10u to "10 min", 30u to "30 min", 60u to "1 h", 0u to "Never")
        ExpandRow(Icons.Rounded.Timer, "Clear after", "Then the status goes, until you play", label(idle, s.idleMinutes)) {
            Segments(idle, s.idleMinutes) { save(s.copy(idleMinutes = it)) }
        }
    }
}

// ---------- About

@Composable
private fun AboutPage(open: (Route) -> Unit) {
    val context = LocalContext.current
    fun visit(url: String) = tryOpen("web browser") { context.startActivity(Intent(Intent.ACTION_VIEW, Uri.parse(url)).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)) }
    val version = remember { runCatching { context.packageManager.getPackageInfo(context.packageName, 0).versionName }.getOrNull() ?: "" }
    Group {
        SettingRow(Icons.Rounded.Info, "Needle for Android", "Version $version · no account, no ads, no tracking")
    }
    var playback by remember { mutableStateOf<PlaybackSettings?>(null) }
    LaunchedEffect(Unit) { playback = withContext(Dispatchers.IO) { runCatching { core.playbackSettings() }.getOrNull() } }
    Group("Updates") {
        UpdatesRow()
        playback?.let { p ->
            Divider()
            SwitchRow("Check once a day", "Asks needle.nnx.fyi for a newer version", p.checkUpdates, icon = Icons.Rounded.Update) { on ->
                val next = p.copy(checkUpdates = on)
                playback = next
                background { core.setPlaybackSettings(next) }
            }
        }
    }
    Group("Privacy") {
        var crashes by remember { mutableStateOf(core.crashReports()) }
        SwitchRow("Send crash reports", "Without file paths or names", crashes, icon = Icons.Rounded.BugReport) { on ->
            crashes = on
            background { core.setCrashReports(on) }
        }
    }
    Group {
        LinkRow("Website", "needle.nnx.fyi", Icons.Rounded.Language) { visit("https://needle.nnx.fyi") }
        Divider()
        LinkRow("Privacy", "What Needle sends, and when", Icons.Rounded.Shield) { open(Route.Document(Doc.Privacy)) }
        Divider()
        LinkRow("Help", "Questions and answers", Icons.AutoMirrored.Rounded.HelpOutline) { open(Route.Document(Doc.Help)) }
        Divider()
        LinkRow("Welcome guide", "See it again", Icons.Rounded.WavingHand) { Ui.welcome.value = true }
    }
}

private val BandKinds = listOf("peak" to "Peak", "lowshelf" to "Low shelf", "highshelf" to "High shelf", "lowpass" to "Low pass", "highpass" to "High pass", "notch" to "Notch")

/** The parametric equalizer: bands of any kind, each with its frequency, gain, and width. */
@Composable
private fun ParametricBands(bands: List<fyi.nnx.needle.core.EqBand>, onChange: (List<fyi.nnx.needle.core.EqBand>) -> Unit) {
    var live by remember(bands) { mutableStateOf(bands) }
    fun set(i: Int, band: fyi.nnx.needle.core.EqBand, done: Boolean) {
        live = live.toMutableList().also { it[i] = band }
        if (done) onChange(live)
    }
    Column(Modifier.padding(horizontal = 16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        live.forEachIndexed { i, band ->
            Column(
                Modifier.fillMaxWidth().clip(RoundedCornerShape(16.dp)).background(MaterialTheme.colorScheme.surfaceContainerHigh).padding(12.dp),
            ) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    var open by remember { mutableStateOf(false) }
                    Box {
                        TextButton(onClick = { open = true }) { Text(BandKinds.firstOrNull { it.first == band.kind }?.second ?: band.kind) }
                        DropdownMenu(open, { open = false }) {
                            BandKinds.forEach { (kind, label) ->
                                DropdownMenuItem(text = { Text(label) }, onClick = { open = false; set(i, band.copy(kind = kind), true) })
                            }
                        }
                    }
                    Text(
                        "${if (band.frequency >= 1000) "%.1f kHz".format(band.frequency / 1000) else "${band.frequency.toInt()} Hz"} · %+.1f dB · Q %.2f".format(band.gain, band.q),
                        style = MaterialTheme.typography.labelLarge,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.weight(1f),
                    )
                    Switch(band.on, { set(i, band.copy(on = it), true) }, Modifier.named("Band ${i + 1}"))
                    IconButton(onClick = { onChange(live.filterIndexed { j, _ -> j != i }) }) {
                        Icon(Icons.Rounded.Delete, contentDescription = "Remove band ${i + 1}", tint = MaterialTheme.colorScheme.onSurfaceVariant)
                    }
                }
                // Frequency on a log scale, as ears hear it.
                Text("Frequency", style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                Slider(
                    kotlin.math.log10(band.frequency.coerceIn(20f, 20000f)),
                    { set(i, band.copy(frequency = Math.pow(10.0, it.toDouble()).toFloat()), false) },
                    valueRange = kotlin.math.log10(20f)..kotlin.math.log10(20000f),
                    onValueChangeFinished = { onChange(live) },
                    modifier = Modifier.named("Band ${i + 1} frequency", "${band.frequency.toInt()} Hz"),
                )
                if (band.kind in listOf("peak", "lowshelf", "highshelf")) {
                    Text("Gain", style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                    Slider(band.gain, { set(i, band.copy(gain = it), false) }, valueRange = -12f..12f, onValueChangeFinished = { onChange(live) }, modifier = Modifier.named("Band ${i + 1} gain", "%+.1f dB".format(band.gain)))
                }
                Text("Width (Q)", style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                Slider(band.q, { set(i, band.copy(q = it), false) }, valueRange = 0.2f..6f, onValueChangeFinished = { onChange(live) }, modifier = Modifier.named("Band ${i + 1} width", "%.1f".format(band.q)))
            }
        }
        if (live.size < 20) TextButton(onClick = {
            onChange(live + fyi.nnx.needle.core.EqBand(kind = "peak", frequency = 1000f, gain = 0f, q = 1f, on = true))
        }) { Text("Add a band") }
    }
}

/**
 * fanart.tv's key, for artists' logos and wide pictures: Needle has one of its own, and a key of
 * the person's own (saved in Needle's private storage) is used instead when set. A row that
 * opens, showing whose key is used.
 */
@Composable
private fun FanartKeyRow() {
    var own by remember { mutableStateOf<Boolean?>(null) }
    LaunchedEffect(Unit) { own = withContext(Dispatchers.IO) { runCatching { core.hasOwnFanartKey() }.getOrDefault(false) } }
    var key by remember { mutableStateOf("") }
    ExpandRow(Icons.Rounded.Key, "Your own fanart.tv key", "Not needed: Needle has its own", if (own == true) "Yours" else "Needle's") {
        if (own == true) {
            MoreText("Saved. Needle uses it for artists' pictures and logos.")
            TextButton(onClick = {
                own = false
                background { core.setFanartKey("") }
            }) { Text("Remove your key") }
        } else if (own == false) {
            MoreText("A key of yours comes from fanart.tv, under your account.")
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                OutlinedTextField(
                    key, { key = it },
                    label = { Text("fanart.tv key") },
                    singleLine = true,
                    visualTransformation = PasswordVisualTransformation(),
                    modifier = Modifier.remoteEscape().weight(1f),
                )
                TextButton(enabled = key.isNotBlank(), onClick = {
                    val saved = key.trim()
                    key = ""
                    own = true
                    background { core.setFanartKey(saved) }
                }) { Text("Save") }
            }
        }
    }
}
