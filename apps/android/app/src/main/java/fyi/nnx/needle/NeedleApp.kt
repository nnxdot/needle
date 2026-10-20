package fyi.nnx.needle

import android.app.Application
import fyi.nnx.needle.core.AppSettings
import fyi.nnx.needle.core.Needle
import fyi.nnx.needle.core.Playback
import fyi.nnx.needle.core.ScanStatus
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import fyi.nnx.needle.ui.asPlayback
import java.io.File

/** Holds Needle's core for the whole app: the screens and the background player share it. */
class NeedleApp : Application() {
    lateinit var core: Needle
        private set

    /** Work that outlives any one screen. */
    val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)

    private val _playback = MutableStateFlow<Playback?>(null)
    /**
     * What plays, read from the core a few times a second; or what the computer plays, while
     * the phone shows that (see [remoteShown]).
     */
    val playback: StateFlow<Playback?> = _playback

    /**
     * The song playing, changing only when the song does: lists mark it without redrawing at
     * every tick of the playing position.
     */
    val playingId: StateFlow<String?> = _playback
        .map { it?.current?.id }
        .distinctUntilChanged()
        .stateIn(scope, SharingStarted.Eagerly, null)

    private val _remote = MutableStateFlow<fyi.nnx.needle.core.PcState?>(null)
    /** What Needle on the computer plays, while the phone is connected to it. */
    val remote: StateFlow<fyi.nnx.needle.core.PcState?> = _remote

    private val _remoteShown = MutableStateFlow(false)
    /**
     * The phone shows the computer's player: the phone is not playing, and the computer is
     * (or the phone has nothing of its own to show).
     */
    val remoteShown: StateFlow<Boolean> = _remoteShown

    private val _scan = MutableStateFlow(ScanStatus(false, 0u, 0u, "", null))
    val scan: StateFlow<ScanStatus> = _scan

    /** How many of Needle's screens are on view (0 when it is in the background). */
    @Volatile private var visible = 0

    /** Changes whenever the library may have changed (a scan finished), so lists load again. */
    val libraryVersion = MutableStateFlow(0)
    val themeVersion = MutableStateFlow(0)

    private val _app = MutableStateFlow(
        AppSettings(
            coverBackdrop = true, wallpaperColors = false, reduceMotion = false, liveLyrics = true,
            haptics = true, openPlayerOnPlay = false, keepScreenOnLyrics = false, theme = "night",
            coverColors = true, ambientColors = false, movingBackdrop = true, backdropBlur = 70f,
            seekStyle = "wave", miniPlayerColored = true, miniPlayerLyrics = false, coverShape = "square",
            titleFont = "flex", textScale = 1f, density = "comfortable", accentColor = "", grain = 0f,
            controlStyle = "expressive", sideButtons = "skip", volumeSlider = false, swipeCover = true, welcomed = false,
        ),
    )
    /** The app's own look and behaviour (Settings › Appearance). */
    val app: StateFlow<AppSettings> = _app

    /** Asks the screens to open the full player (when a song is chosen, if that is on). */
    val openPlayer = MutableSharedFlow<Unit>(extraBufferCapacity = 1)

    /**
     * Where settings are saved: one at a time, in the order they were made, so two quick
     * changes cannot be written the wrong way round (the older last).
     */
    @OptIn(kotlinx.coroutines.ExperimentalCoroutinesApi::class)
    val saves = kotlinx.coroutines.Dispatchers.IO.limitedParallelism(1)

    fun updateApp(change: (AppSettings) -> AppSettings) {
        val next = change(_app.value)
        _app.value = next
        scope.launch(saves) { runCatching { core.setAppSettings(next) } }
    }

    override fun onCreate() {
        super.onCreate()
        instance = this
        NativeBridge.initAndroid(this)
        core = Needle(File(filesDir, "needle").absolutePath)
        _app.value = core.appSettings()
        core.setDecoder(File(applicationInfo.nativeLibraryDir, "libneedle_ffmpeg.so").absolutePath)
        // The Discord status, when this build can show it and Discord is on the phone.
        Discord.load(this)
        if (Discord.available && Discord.installed(this)) core.startDiscord()
        // A reading notice left from a run that was closed mid-read.
        ScanNotice.hide(this)
        registerActivityLifecycleCallbacks(object : ActivityLifecycleCallbacks {
            override fun onActivityStarted(activity: android.app.Activity) { visible++ }
            override fun onActivityStopped(activity: android.app.Activity) { visible-- }
            override fun onActivityCreated(activity: android.app.Activity, state: android.os.Bundle?) {}
            override fun onActivityResumed(activity: android.app.Activity) {}
            override fun onActivityPaused(activity: android.app.Activity) {}
            override fun onActivitySaveInstanceState(activity: android.app.Activity, state: android.os.Bundle) {}
            override fun onActivityDestroyed(activity: android.app.Activity) {}
        })
        scope.launch {
            var wasScanning = false
            var shown: Pair<String?, Boolean>? = null
            while (isActive) {
                val local = core.playback()
                val pc = _remote.value
                // Once shown, the computer's player stays while nothing plays here, so pausing
                // the computer does not swap it for the phone's paused song.
                val showPc = pc?.current != null && !local.playing &&
                    (pc.playing || local.current == null || _remoteShown.value)
                _remoteShown.value = showPc
                val playback = if (showPc) fyi.nnx.needle.ui.Live.shape(pc!!).asPlayback() else local
                _playback.value = playback
                // The widgets follow the song and play or pause.
                val now = playback.current?.id to playback.playing
                if (now != shown) {
                    shown = now
                    launch { updateWidgets(this@NeedleApp) }
                }
                val scan = core.scanStatus()
                _scan.value = scan
                if (scan.running) ScanNotice.show(this@NeedleApp, scan)
                if (wasScanning && !scan.running) {
                    libraryVersion.value++
                    ScanNotice.hide(this@NeedleApp)
                    launch { updateWidgets(this@NeedleApp) }
                }
                wasScanning = scan.running
                // Nothing to follow closely while Needle is out of sight and nothing plays.
                val busy = visible > 0 || playback.playing || scan.running
                delay(if (busy) 200 else 2000)
            }
        }
        // Needle on the computer, once a second while connected to it.
        scope.launch(Dispatchers.IO) {
            while (isActive) {
                val heard = if (core.pcConnected()) runCatching { core.pcState() }.getOrNull() else null
                if (heard != null) fyi.nnx.needle.ui.Live.heardAt = android.os.SystemClock.uptimeMillis()
                _remote.value = heard
                delay(if (_remoteShown.value) 700 else 1500)
            }
        }
        // A newer Needle, asked about once a day when that is on (Settings › About).
        scope.launch(Dispatchers.IO) {
            val prefs = getSharedPreferences("needle", MODE_PRIVATE)
            val last = prefs.getLong("lastUpdateCheck", 0L)
            val now = System.currentTimeMillis()
            val on = runCatching { core.playbackSettings().checkUpdates }.getOrDefault(false)
            if (on && now - last > 24 * 3600 * 1000L) {
                delay(8000)
                runCatching { core.checkUpdate() }.onSuccess { found ->
                    prefs.edit().putLong("lastUpdateCheck", now).apply()
                    if (found != null) fyi.nnx.needle.ui.showMessage("Needle ${found.version} is out. Get it in Settings › About.")
                }
            }
        }
        // Crash reports from earlier runs, when that is on.
        scope.launch { runCatching { core.sendCrashReports() } }
        // Read the music folders again at start, for songs added since.
        if (core.folders().isNotEmpty()) core.rescan()
    }

    companion object {
        lateinit var instance: NeedleApp
            private set
    }
}
