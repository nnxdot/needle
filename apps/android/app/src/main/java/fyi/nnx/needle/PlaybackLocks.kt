package fyi.nnx.needle

import android.content.Context
import android.net.wifi.WifiManager
import android.os.PowerManager
import fyi.nnx.needle.ui.fromServer
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.collectLatest
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.launch

/**
 * Keeps the phone awake for the music while it plays, as Android's own players do: with the
 * screen off and locked, a phone or tablet otherwise lets its processor sleep between sounds
 * and puts its Wi-Fi to sleep. A song streamed from a music server then stops coming, and
 * after the core's waits (about a minute and a half), it stops playing.
 *
 * While music plays: the processor stays awake. While a server's song plays, or the phone
 * shows the computer's player: the Wi-Fi stays awake too. Both go when it pauses.
 */
internal class PlaybackLocks(context: Context) {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main)
    private val wake = context.getSystemService(PowerManager::class.java)
        ?.newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, "needle:playback")
        ?.apply { setReferenceCounted(false) }

    @Suppress("DEPRECATION")
    private val wifi = (context.applicationContext.getSystemService(Context.WIFI_SERVICE) as? WifiManager)
        // High performance: Wi-Fi without its power saving, which can hold back a stream for
        // seconds at a time with the screen off.
        ?.createWifiLock(WifiManager.WIFI_MODE_FULL_HIGH_PERF, "needle:stream")
        ?.apply { setReferenceCounted(false) }

    fun start() {
        val app = NeedleApp.instance
        scope.launch {
            combine(app.playback, app.remoteShown) { playback, pc ->
                val playing = playback?.playing == true
                playing to (playing && (pc || playback?.current?.fromServer() == true))
            }
                .distinctUntilChanged()
                .collectLatest { (playing, network) ->
                    runCatching { if (network) wifi?.acquire() else if (wifi?.isHeld == true) wifi.release() }
                    if (!playing) {
                        runCatching { if (wake?.isHeld == true) wake.release() }
                        return@collectLatest
                    }
                    // Taken for a while at a time and taken again, so a lock is never left
                    // held for good, whatever happens to Needle.
                    while (true) {
                        runCatching { wake?.acquire(10 * 60 * 1000L) }
                        delay(5 * 60 * 1000L)
                    }
                }
        }
    }

    fun stop() {
        scope.cancel()
        runCatching { if (wake?.isHeld == true) wake.release() }
        runCatching { if (wifi?.isHeld == true) wifi.release() }
    }
}
