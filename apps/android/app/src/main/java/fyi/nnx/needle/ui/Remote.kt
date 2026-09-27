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

    fun toggle() = if (onComputer()) pc { core.pcDo("toggle") } else core.toggle()
    fun next() = if (onComputer()) pc { core.pcDo("next") } else core.next()
    fun previous() = if (onComputer()) pc { core.pcDo("previous") } else core.previous()
    fun seek(seconds: Double) = if (onComputer()) pc { core.pcSeek(seconds) } else core.seek(seconds)
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
