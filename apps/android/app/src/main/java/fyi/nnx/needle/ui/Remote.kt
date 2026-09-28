package fyi.nnx.needle.ui

import fyi.nnx.needle.NeedleApp
import fyi.nnx.needle.core.PcSong
import fyi.nnx.needle.core.PcState
import fyi.nnx.needle.core.Playback
import fyi.nnx.needle.core.RepeatMode
import fyi.nnx.needle.core.Song
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch

/*
 * Needle on a computer, played from the phone as if the phone played it: while the phone is
 * not playing and the computer is, the mini player, the player, and their buttons show and
 * work the computer's song. Playing something on the phone takes over again.
 */

/** Songs from the computer have ids starting with this, so screens can tell them apart. */
const val PcPrefix = "pc:"

fun Song.onComputer() = id.startsWith(PcPrefix)

fun PcSong.asSong() = Song(
    id = PcPrefix + id,
    title = title,
    artist = artist,
    album = album,
    duration = duration,
    trackNumber = 0,
    format = "",
    artwork = cover,
    rating = 0,
)

/** The computer's state, told the way the phone tells its own. */
fun PcState.asPlayback() = Playback(
    current = current?.asSong(),
    playing = playing,
    position = position,
    volume = volume,
    repeat = when (repeat) {
        "all" -> RepeatMode.ALL
        "one" -> RepeatMode.ONE
        else -> RepeatMode.OFF
    },
    upNext = upNext.size.toUInt(),
    // Changes when the song or the list up next changes, as the phone's own does.
    queueVersion = ((current?.id ?: "") + upNext.joinToString { it.id }).hashCode().toULong(),
    loading = false,
    error = null,
)

/** Whether the phone is showing (and working) the computer's player just now. */
fun onComputer(): Boolean = NeedleApp.instance.remoteShown.value

/**
 * The player's buttons: they work the phone's own player, or the computer's when the phone is
 * showing that. A computer that does not answer says so.
 */
object Controls {
    private fun pc(work: () -> Unit) {
        NeedleApp.instance.scope.launch(Dispatchers.IO) {
            runCatching(work).onFailure { showMessage(it.message ?: "Your computer did not answer") }
        }
    }

    fun toggle() = if (onComputer()) {
        Live.toggled(NeedleApp.instance.playback.value?.playing != true)
        pc { core.pcDo("toggle") }
    } else core.toggle()
    fun next() = if (onComputer()) pc { core.pcDo("next") } else core.next()
    fun previous() = if (onComputer()) pc { core.pcDo("previous") } else core.previous()
    fun seek(seconds: Double) = if (onComputer()) {
        Live.seeked(seconds)
        pc { core.pcSeek(seconds) }
    } else core.seek(seconds)

    /** The computer's volume: shown at once, sent at most a few times a second while dragged. */
    private var lastVolumeSent = 0L
    fun volume(value: Float, final: Boolean) {
        Live.volumed(value)
        val t = android.os.SystemClock.uptimeMillis()
        if (final || t - lastVolumeSent > 120) {
            lastVolumeSent = t
            pc { core.pcVolume(value) }
        }
    }
    fun shuffle() = if (onComputer()) pc { core.pcDo("shuffle") } else core.shuffle()

    fun setRepeat(mode: RepeatMode) = if (onComputer()) {
        pc {
            core.pcRepeat(
                when (mode) {
                    RepeatMode.ALL -> "all"
                    RepeatMode.ONE -> "one"
                    RepeatMode.OFF -> "off"
                },
            )
        }
    } else core.setRepeat(mode)

    /** Plays the song `index` places into what is up next. */
    fun jump(index: Int, song: Song) = if (song.onComputer()) {
        pc { core.pcJump(index.toUInt(), song.id.removePrefix(PcPrefix)) }
    } else core.jump(index.toUInt())

    /** What is up next, on the phone or the computer. */
    fun upNext(): List<Song> =
        if (onComputer()) NeedleApp.instance.remote.value?.upNext?.map { it.asSong() }.orEmpty()
        else runCatching { core.upNext() }.getOrDefault(emptyList())
}

/**
 * The computer is heard from about once a second. So the player never jumps, the phone fills
 * in between: the song's place moves on by itself while it plays, and what the listener just
 * did (play or pause, a new place, a new volume) shows at once and is kept until the computer
 * says the same, or for a few seconds if it never does.
 */
object Live {
    /** When the last state came from the computer (uptime, ms). */
    @Volatile var heardAt = 0L

    private class Wish<T>(val value: T, val at: Long)

    @Volatile private var seek: Wish<Double>? = null
    @Volatile private var playing: Wish<Boolean>? = null
    @Volatile private var volume: Wish<Float>? = null

    private const val KEEP = 3000L
    private fun now() = android.os.SystemClock.uptimeMillis()

    fun seeked(seconds: Double) { seek = Wish(seconds, now()) }
    fun toggled(to: Boolean) { playing = Wish(to, now()) }
    fun volumed(value: Float) { volume = Wish(value, now()) }

    /** The computer's state, with the listener's wishes over it and the place moved on. */
    fun shape(pc: PcState): PcState {
        val t = now()
        val wantPlay = playing?.takeIf { t - it.at < KEEP && it.value != pc.playing }
        if (wantPlay == null) playing = null
        val isPlaying = wantPlay?.value ?: pc.playing

        // Where the song is: from the wished place (until the computer is near it), or from
        // the last one heard, moved on by the time since while it plays.
        val wantSeek = seek?.let { w ->
            val expected = w.value + if (isPlaying) (t - w.at) / 1000.0 else 0.0
            w.takeIf { t - w.at < KEEP && kotlin.math.abs(pc.position - expected) > 1.5 }
        }
        if (wantSeek == null) seek = null
        val (base, since) = if (wantSeek != null) wantSeek.value to wantSeek.at else pc.position to heardAt
        val length = pc.current?.duration ?: Double.MAX_VALUE
        val position = (base + if (isPlaying) (t - since).coerceAtLeast(0) / 1000.0 else 0.0).coerceIn(0.0, length)

        val wantVolume = volume?.takeIf { t - it.at < KEEP && kotlin.math.abs(it.value - pc.volume) > 0.02f }
        if (wantVolume == null) volume = null

        return pc.copy(playing = isPlaying, position = position, volume = wantVolume?.value ?: pc.volume)
    }
}
