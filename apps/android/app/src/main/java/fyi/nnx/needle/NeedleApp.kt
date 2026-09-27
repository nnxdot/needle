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
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import java.io.File

/** Holds Needle's core for the whole app: the screens and the background player share it. */
class NeedleApp : Application() {
    lateinit var core: Needle
        private set

    /** Work that outlives any one screen. */
    val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)

    private val _playback = MutableStateFlow<Playback?>(null)
    /** What plays, read from the core a few times a second. */
    val playback: StateFlow<Playback?> = _playback

    private val _scan = MutableStateFlow(ScanStatus(false, 0u, 0u, "", null))
    val scan: StateFlow<ScanStatus> = _scan

    /** Changes whenever the library may have changed (a scan finished), so lists load again. */
    val libraryVersion = MutableStateFlow(0)

    private val _app = MutableStateFlow(
        AppSettings(
            coverBackdrop = true, wallpaperColors = false, reduceMotion = false, liveLyrics = true,
            haptics = true, openPlayerOnPlay = false, keepScreenOnLyrics = false,
        ),
    )
    /** The app's own look and behaviour (Settings › Appearance). */
    val app: StateFlow<AppSettings> = _app

    /** Asks the screens to open the full player (when a song is chosen, if that is on). */
    val openPlayer = MutableSharedFlow<Unit>(extraBufferCapacity = 1)

    fun updateApp(change: (AppSettings) -> AppSettings) {
        val next = change(_app.value)
        _app.value = next
        scope.launch { runCatching { core.setAppSettings(next) } }
    }

    override fun onCreate() {
        super.onCreate()
        instance = this
        NativeBridge.initAndroid(this)
        core = Needle(File(filesDir, "needle").absolutePath)
        _app.value = core.appSettings()
        scope.launch {
            var wasScanning = false
            while (isActive) {
                _playback.value = core.playback()
                val scan = core.scanStatus()
                _scan.value = scan
                if (wasScanning && !scan.running) libraryVersion.value++
                wasScanning = scan.running
                delay(if (scan.running) 300 else 200)
            }
        }
        // Read the music folders again at start, for songs added since.
        if (core.folders().isNotEmpty()) core.rescan()
    }

    companion object {
        lateinit var instance: NeedleApp
            private set
    }
}
