package fyi.nnx.needle

import android.app.Application
import fyi.nnx.needle.core.Needle
import fyi.nnx.needle.core.Playback
import fyi.nnx.needle.core.ScanStatus
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.delay
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

    override fun onCreate() {
        super.onCreate()
        instance = this
        NativeBridge.initAndroid(this)
        core = Needle(File(filesDir, "needle").absolutePath)
        scope.launch {
            var wasScanning = false
            while (isActive) {
                val playback = core.playback()
                if (playback.error != _playback.value?.error || playback.current?.id != _playback.value?.current?.id) {
                    android.util.Log.i("Needle", "now: ${playback.current?.title} playing=${playback.playing} error=${playback.error}")
                }
                _playback.value = playback
                val scan = core.scanStatus()
                _scan.value = scan
                if (wasScanning && !scan.running) libraryVersion.value++
                wasScanning = scan.running
                delay(if (scan.running) 300 else 250)
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
