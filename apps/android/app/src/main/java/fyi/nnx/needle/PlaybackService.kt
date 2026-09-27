package fyi.nnx.needle

import android.net.Uri
import android.os.Handler
import android.os.Looper
import androidx.media3.common.MediaItem
import androidx.media3.common.MediaMetadata
import androidx.media3.common.Player
import androidx.media3.common.SimpleBasePlayer
import androidx.media3.common.util.UnstableApi
import androidx.media3.session.MediaSession
import androidx.media3.session.MediaSessionService
import com.google.common.util.concurrent.Futures
import com.google.common.util.concurrent.ListenableFuture
import fyi.nnx.needle.core.Needle
import fyi.nnx.needle.core.Song
import java.io.File

/**
 * Keeps Needle playing in the background, with the player in the notification, on the lock
 * screen, and for Bluetooth buttons. Needle's own core plays the sound; this only tells Android
 * what plays and passes its buttons on.
 */
class PlaybackService : MediaSessionService() {
    private var session: MediaSession? = null

    @UnstableApi
    override fun onCreate() {
        super.onCreate()
        val player = NeedlePlayer(Looper.getMainLooper(), NeedleApp.instance.core)
        session = MediaSession.Builder(this, player).build()
    }

    override fun onGetSession(controllerInfo: MediaSession.ControllerInfo) = session

    override fun onDestroy() {
        session?.run {
            player.release()
            release()
        }
        session = null
        super.onDestroy()
    }
}

/** Android's view of Needle's player: the song playing and the songs up next. */
@UnstableApi
private class NeedlePlayer(looper: Looper, private val core: Needle) : SimpleBasePlayer(looper) {
    private val handler = Handler(looper)
    private val tick = object : Runnable {
        override fun run() {
            invalidateState()
            handler.postDelayed(this, 500)
        }
    }

    init {
        handler.post(tick)
    }

    override fun getState(): State {
        val playback = core.playback()
        val commands = Player.Commands.Builder().addAll(
            COMMAND_PLAY_PAUSE,
            COMMAND_SEEK_TO_NEXT,
            COMMAND_SEEK_TO_NEXT_MEDIA_ITEM,
            COMMAND_SEEK_TO_PREVIOUS,
            COMMAND_SEEK_TO_PREVIOUS_MEDIA_ITEM,
            COMMAND_SEEK_IN_CURRENT_MEDIA_ITEM,
            COMMAND_GET_CURRENT_MEDIA_ITEM,
            COMMAND_GET_TIMELINE,
            COMMAND_GET_METADATA,
        ).build()
        val state = State.Builder()
            .setAvailableCommands(commands)
            .setPlayWhenReady(playback.playing, PLAY_WHEN_READY_CHANGE_REASON_USER_REQUEST)
        val current = playback.current ?: return state.setPlaybackState(STATE_IDLE).build()
        // The song playing, then a few up next, so Android offers Next.
        val items = listOf(current) + core.upNext().take(10)
        return state
            .setPlaylist(items.mapIndexed { i, song -> item(song, i) })
            .setCurrentMediaItemIndex(0)
            .setContentPositionMs((playback.position * 1000).toLong())
            .setPlaybackState(if (playback.loading) STATE_BUFFERING else STATE_READY)
            .build()
    }

    private fun item(song: Song, index: Int): MediaItemData {
        val metadata = MediaMetadata.Builder()
            .setTitle(song.title)
            .setArtist(song.artist)
            .setAlbumTitle(song.album)
            .setArtworkUri(song.artwork?.let { Uri.fromFile(File(it)) })
            .build()
        val mediaItem = MediaItem.Builder()
            .setMediaId("$index:${song.id}")
            .setMediaMetadata(metadata)
            .build()
        return MediaItemData.Builder("$index:${song.id}")
            .setMediaItem(mediaItem)
            .setDurationUs((song.duration * 1_000_000).toLong())
            .build()
    }

    override fun handleSetPlayWhenReady(playWhenReady: Boolean): ListenableFuture<*> {
        if (playWhenReady != core.playback().playing) core.toggle()
        return Futures.immediateVoidFuture()
    }

    override fun handleSeek(
        mediaItemIndex: Int,
        positionMs: Long,
        seekCommand: Int,
    ): ListenableFuture<*> {
        when (seekCommand) {
            COMMAND_SEEK_TO_NEXT, COMMAND_SEEK_TO_NEXT_MEDIA_ITEM -> core.next()
            COMMAND_SEEK_TO_PREVIOUS, COMMAND_SEEK_TO_PREVIOUS_MEDIA_ITEM -> core.previous()
            else -> core.seek(positionMs / 1000.0)
        }
        return Futures.immediateVoidFuture()
    }

    override fun handleRelease(): ListenableFuture<*> {
        handler.removeCallbacks(tick)
        return Futures.immediateVoidFuture()
    }
}
