package fyi.nnx.needle

import android.net.Uri
import android.os.Handler
import android.os.Looper
import androidx.media3.common.MediaItem
import androidx.media3.common.MediaMetadata
import androidx.media3.common.Player
import androidx.media3.common.SimpleBasePlayer
import androidx.media3.common.util.UnstableApi
import androidx.media3.session.LibraryResult
import androidx.media3.session.MediaLibraryService
import androidx.media3.session.MediaSession
import com.google.common.collect.ImmutableList
import com.google.common.util.concurrent.Futures
import com.google.common.util.concurrent.ListenableFuture
import fyi.nnx.needle.core.Album
import fyi.nnx.needle.core.Needle
import fyi.nnx.needle.core.Song
import java.io.File

/**
 * Keeps Needle playing in the background, with the player in the notification, on the lock
 * screen, and for Bluetooth buttons; and offers the library to Android Auto and other
 * browsers. Needle's own core plays the sound; this tells Android what plays and passes its
 * buttons on.
 */
class PlaybackService : MediaLibraryService() {
    private var session: MediaLibrarySession? = null

    @UnstableApi
    override fun onCreate() {
        super.onCreate()
        val core = NeedleApp.instance.core
        val player = NeedlePlayer(Looper.getMainLooper(), core)
        session = MediaLibrarySession.Builder(this, player, Library(core)).build()
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

private const val ROOT = "root"
private const val ALBUMS = "albums"
private const val RECENT = "recent"
private const val PLAYLISTS = "playlists"
private const val FAVORITES = "favorites"

/** The library as a car shows it: shelves, then albums and playlists, then songs. */
@UnstableApi
private class Library(private val core: Needle) : MediaLibraryService.MediaLibrarySession.Callback {
    private fun folder(id: String, title: String, subtitle: String? = null, art: String? = null): MediaItem =
        MediaItem.Builder()
            .setMediaId(id)
            .setMediaMetadata(
                MediaMetadata.Builder()
                    .setTitle(title)
                    .setSubtitle(subtitle)
                    .setArtworkUri(art?.let { Uri.fromFile(File(it)) })
                    .setIsBrowsable(true)
                    .setIsPlayable(false)
                    .build(),
            )
            .build()

    private fun album(a: Album) = folder("album:${a.key}", a.title.ifBlank { "Unknown album" }, a.artist, a.artwork)

    private fun songs(list: List<Song>): List<MediaItem> = list.map { songItem(it, "song:${it.id}") }

    override fun onGetLibraryRoot(
        session: MediaLibraryService.MediaLibrarySession,
        browser: MediaSession.ControllerInfo,
        params: MediaLibraryService.LibraryParams?,
    ): ListenableFuture<LibraryResult<MediaItem>> =
        Futures.immediateFuture(LibraryResult.ofItem(folder(ROOT, "Needle"), params))

    override fun onGetChildren(
        session: MediaLibraryService.MediaLibrarySession,
        browser: MediaSession.ControllerInfo,
        parentId: String,
        page: Int,
        pageSize: Int,
        params: MediaLibraryService.LibraryParams?,
    ): ListenableFuture<LibraryResult<ImmutableList<MediaItem>>> {
        val items: List<MediaItem> = runCatching {
            when {
                parentId == ROOT -> listOf(
                    folder(RECENT, "Recently added"),
                    folder(ALBUMS, "Albums"),
                    folder(PLAYLISTS, "Playlists"),
                    folder(FAVORITES, "Favorites"),
                )
                parentId == RECENT -> core.home().added.map(::album)
                parentId == ALBUMS -> core.albums().map(::album)
                parentId == PLAYLISTS -> core.playlists().map { folder("playlist:${it.id}", it.name, null, it.artwork) }
                parentId == FAVORITES -> songs(core.favorites())
                parentId.startsWith("album:") -> songs(core.albumSongs(parentId.removePrefix("album:")))
                parentId.startsWith("playlist:") -> songs(core.playlistSongs(parentId.removePrefix("playlist:")))
                else -> emptyList()
            }
        }.getOrDefault(emptyList())
        val from = (page * pageSize).coerceAtMost(items.size)
        val to = (from + pageSize).coerceAtMost(items.size)
        return Futures.immediateFuture(LibraryResult.ofItemList(ImmutableList.copyOf(items.subList(from, to)), params))
    }

    /** The car picked something: the songs it named are played by Needle. */
    override fun onAddMediaItems(
        mediaSession: MediaSession,
        controller: MediaSession.ControllerInfo,
        mediaItems: MutableList<MediaItem>,
    ): ListenableFuture<MutableList<MediaItem>> = Futures.immediateFuture(mediaItems)
}

fun songItem(song: Song, id: String): MediaItem {
    val metadata = MediaMetadata.Builder()
        .setTitle(song.title)
        .setArtist(song.artist)
        .setAlbumTitle(song.album)
        .setArtworkUri(song.artwork?.let { Uri.fromFile(File(it)) })
        .setIsBrowsable(false)
        .setIsPlayable(true)
        .build()
    return MediaItem.Builder().setMediaId(id).setMediaMetadata(metadata).build()
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
            COMMAND_SET_MEDIA_ITEM,
            COMMAND_CHANGE_MEDIA_ITEMS,
            COMMAND_PREPARE,
        ).build()
        val state = State.Builder()
            .setAvailableCommands(commands)
            .setPlayWhenReady(playback.playing, PLAY_WHEN_READY_CHANGE_REASON_USER_REQUEST)
        val current = playback.current ?: return state.setPlaybackState(STATE_IDLE).build()
        // The song playing, then a few up next, so Android offers Next.
        val items = listOf(current) + core.upNext().take(10)
        return state
            .setPlaylist(
                items.mapIndexed { i, song ->
                    MediaItemData.Builder("$i:${song.id}")
                        .setMediaItem(songItem(song, "$i:${song.id}"))
                        .setDurationUs((song.duration * 1_000_000).toLong())
                        .build()
                },
            )
            .setCurrentMediaItemIndex(0)
            .setContentPositionMs((playback.position * 1000).toLong())
            .setPlaybackState(if (playback.loading) STATE_BUFFERING else STATE_READY)
            .build()
    }

    override fun handleSetPlayWhenReady(playWhenReady: Boolean): ListenableFuture<*> {
        if (playWhenReady != core.playback().playing) core.toggle()
        return Futures.immediateVoidFuture()
    }

    /** Songs chosen in Android Auto (or another browser): Needle plays them. */
    override fun handleSetMediaItems(mediaItems: MutableList<MediaItem>, startIndex: Int, startPositionMs: Long): ListenableFuture<*> {
        val ids = mediaItems.mapNotNull { it.mediaId.substringAfter("song:", "").ifBlank { null } }
        if (ids.isNotEmpty()) {
            core.play(ids, startIndex.coerceAtLeast(0).toUInt())
            if (startPositionMs > 0) core.seek(startPositionMs / 1000.0)
        }
        return Futures.immediateVoidFuture()
    }

    override fun handlePrepare(): ListenableFuture<*> = Futures.immediateVoidFuture()

    override fun handleSeek(mediaItemIndex: Int, positionMs: Long, seekCommand: Int): ListenableFuture<*> {
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
