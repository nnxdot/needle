package fyi.nnx.needle

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.media.AudioAttributes
import android.media.AudioFocusRequest
import android.media.AudioManager
import android.os.Handler
import android.os.Looper
import androidx.core.content.ContextCompat
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
 * Needle's sound and the phone's other sounds, as Android's own players share them: Needle asks
 * for the audio while it plays on this phone, pauses when another app takes it (and plays again
 * after a call or a short sound), and pauses when headphones come out rather than go on through
 * the speaker. Android lowers Needle by itself for a moment's sound, such as a map's voice.
 * The core plays the sound itself, so it takes none of this on its own.
 */
internal class AudioFocus(private val context: Context) {
    private val audio = context.getSystemService(AudioManager::class.java)
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main)
    private var held = false

    /** Paused for a call or another app's short sound, to play again when it gives the audio back. */
    private var pausedForOther = false

    private val request = AudioFocusRequest.Builder(AudioManager.AUDIOFOCUS_GAIN)
        .setAudioAttributes(
            AudioAttributes.Builder()
                .setUsage(AudioAttributes.USAGE_MEDIA)
                .setContentType(AudioAttributes.CONTENT_TYPE_MUSIC)
                .build(),
        )
        .setOnAudioFocusChangeListener(::changed, Handler(Looper.getMainLooper()))
        .build()

    private val unplugged = object : BroadcastReceiver() {
        override fun onReceive(context: Context, intent: Intent) {
            if (intent.action == AudioManager.ACTION_AUDIO_BECOMING_NOISY && playingHere()) core().toggle()
        }
    }

    private fun core() = NeedleApp.instance.core

    /** Needle's own player plays (not the computer's, shown on the phone). */
    private fun playingHere() = !NeedleApp.instance.remoteShown.value && core().playback().playing

    fun start() {
        ContextCompat.registerReceiver(
            context,
            unplugged,
            IntentFilter(AudioManager.ACTION_AUDIO_BECOMING_NOISY),
            ContextCompat.RECEIVER_NOT_EXPORTED,
        )
        scope.launch {
            combine(NeedleApp.instance.playback, NeedleApp.instance.remoteShown) { playback, pc -> playback?.playing == true && !pc }
                .distinctUntilChanged()
                .collectLatest { playing ->
                    if (playing) {
                        pausedForOther = false
                        // Android may say no (a call is on, or Needle is not yet in the
                        // foreground): asked again each second while it plays.
                        while (!held) {
                            held = audio.requestAudioFocus(request) == AudioManager.AUDIOFOCUS_REQUEST_GRANTED
                            if (!held) delay(1000)
                        }
                    } else if (held && !pausedForOther) {
                        // Paused by the person: the audio goes back to the others.
                        audio.abandonAudioFocusRequest(request)
                        held = false
                    }
                }
        }
    }

    private fun changed(change: Int) {
        when (change) {
            AudioManager.AUDIOFOCUS_LOSS -> {
                held = false
                pausedForOther = false
                if (playingHere()) core().toggle()
            }
            AudioManager.AUDIOFOCUS_LOSS_TRANSIENT -> if (playingHere()) {
                pausedForOther = true
                core().toggle()
            }
            AudioManager.AUDIOFOCUS_GAIN -> {
                if (pausedForOther && !core().playback().playing) core().toggle()
                pausedForOther = false
            }
        }
    }

    fun stop() {
        scope.cancel()
        runCatching { context.unregisterReceiver(unplugged) }
        if (held) audio.abandonAudioFocusRequest(request)
        held = false
    }
}
