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
@androidx.annotation.OptIn(markerClass = [UnstableApi::class])
class PlaybackService : MediaLibraryService() {
    private var session: MediaLibrarySession? = null
    private var library: Library? = null

    @UnstableApi
    override fun onCreate() {
        super.onCreate()
        val core = NeedleApp.instance.core
        val player = NeedlePlayer(Looper.getMainLooper(), core)
        val callback = Library(core)
        library = callback
        session = MediaLibrarySession.Builder(this, player, callback).build()
    }

    override fun onGetSession(controllerInfo: MediaSession.ControllerInfo) = session

    override fun onDestroy() {
        session?.run {
            player.release()
            release()
        }
        session = null
        library?.close()
        library = null
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
    private val executor = com.google.common.util.concurrent.MoreExecutors.listeningDecorator(java.util.concurrent.Executors.newSingleThreadExecutor())
    fun close() { executor.shutdownNow() }
    private fun folder(id: String, title: String, subtitle: String? = null, art: String? = null): MediaItem =
        MediaItem.Builder()
            .setMediaId(id)
            .setMediaMetadata(
                MediaMetadata.Builder()
                    .setTitle(title)
                    .setSubtitle(subtitle)
                    .setArtworkUri(ArtworkProvider.uri(art))
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
        if (page < 0 || pageSize !in 1..1000 || page.toLong() * pageSize > UInt.MAX_VALUE.toLong()) {
            return Futures.immediateFailedFuture(IllegalArgumentException("Invalid library page"))
        }
        val offset = (page.toLong() * pageSize).toUInt()
        val size = pageSize.toUInt()
        return executor.submit<LibraryResult<ImmutableList<MediaItem>>> {
            val items: List<MediaItem> =
                when {
                    parentId == ROOT -> listOf(
                        folder(RECENT, "Recently added"),
                        folder(ALBUMS, "Albums"),
                        folder(PLAYLISTS, "Playlists"),
                        folder(FAVORITES, "Favorites"),
                    ).drop(offset.toLong().coerceAtMost(4).toInt()).take(pageSize)
                    parentId == RECENT -> core.home().added.drop(offset.toLong().coerceAtMost(20).toInt()).take(pageSize).map(::album)
                    parentId == ALBUMS -> core.albumsPage(offset, size).map(::album)
                    parentId == PLAYLISTS -> core.playlistsPage(offset, size).map { folder("playlist:${it.id}", it.name, null, it.artwork) }
                    parentId == FAVORITES -> songs(core.searchSongsPage("rating >= 4 order by title", offset, size))
                    parentId.startsWith("album:") -> songs(core.albumSongsPage(parentId.removePrefix("album:"), offset, size))
                    parentId.startsWith("playlist:") -> songs(core.playlistSongsPage(parentId.removePrefix("playlist:"), offset, size))
                    else -> emptyList()
                }
            LibraryResult.ofItemList(ImmutableList.copyOf(items), params)
        }
    }

    /** The car picked something: the songs it named are played by Needle. */
    override fun onAddMediaItems(
        mediaSession: MediaSession,
        controller: MediaSession.ControllerInfo,
        mediaItems: MutableList<MediaItem>,
    ): ListenableFuture<MutableList<MediaItem>> = executor.submit<MutableList<MediaItem>> {
        mediaItems.flatMap { item ->
            val query = item.requestMetadata.searchQuery
            if (query == null) listOf(item) else songs(core.searchSongsPage(query, 0u, 300u))
        }.toMutableList()
    }

    override fun onSearch(
        session: MediaLibraryService.MediaLibrarySession,
        browser: MediaSession.ControllerInfo,
        query: String,
        params: MediaLibraryService.LibraryParams?,
    ): ListenableFuture<LibraryResult<Void>> = executor.submit<LibraryResult<Void>> {
        val count = core.searchSongsCount(query).toInt()
        Handler(session.player.applicationLooper).post {
            session.notifySearchResultChanged(browser, query, count, params)
        }
        LibraryResult.ofVoid(params)
    }

    override fun onGetSearchResult(
        session: MediaLibraryService.MediaLibrarySession,
        browser: MediaSession.ControllerInfo,
        query: String,
        page: Int,
        pageSize: Int,
        params: MediaLibraryService.LibraryParams?,
    ): ListenableFuture<LibraryResult<ImmutableList<MediaItem>>> {
        if (page < 0 || pageSize !in 1..1000 || page.toLong() * pageSize > UInt.MAX_VALUE.toLong()) {
            return Futures.immediateFailedFuture(IllegalArgumentException("Invalid search page"))
        }
        return executor.submit<LibraryResult<ImmutableList<MediaItem>>> {
            val found = core.searchSongsPage(query, (page.toLong() * pageSize).toUInt(), pageSize.toUInt())
            LibraryResult.ofItemList(ImmutableList.copyOf(songs(found)), params)
        }
    }
}

/** The playing cover as a small picture, made once per cover. */
private var coverBytes: Pair<String, ByteArray?>? = null

private fun coverData(path: String?): ByteArray? {
    path ?: return null
    coverBytes?.let { (p, bytes) -> if (p == path) return bytes }
    val bytes = runCatching {
        val bounds = android.graphics.BitmapFactory.Options().apply { inJustDecodeBounds = true }
        android.graphics.BitmapFactory.decodeFile(path, bounds)
        val step = maxOf(1, minOf(bounds.outWidth, bounds.outHeight) / 512)
        val bitmap = android.graphics.BitmapFactory.decodeFile(path, android.graphics.BitmapFactory.Options().apply { inSampleSize = step })
            ?.let { fyi.nnx.needle.ui.trimBars(it) } ?: return@runCatching null
        java.io.ByteArrayOutputStream().also { bitmap.compress(android.graphics.Bitmap.CompressFormat.JPEG, 88, it) }.toByteArray()
    }.getOrNull()
    coverBytes = path to bytes
    return bytes
}

/**
 * A song for Android. The one playing carries its cover as a picture, so the lock screen and
 * the notification never have to open a file; songs up next carry none; songs in Android
 * Auto's lists name theirs, which it opens in the background.
 */
fun songItem(song: Song, id: String, queued: Boolean = false, playing: Boolean = false): MediaItem {
    val metadata = MediaMetadata.Builder()
        .setTitle(song.title)
        .setArtist(song.artist)
        .setAlbumTitle(song.album)
        .apply {
            val data = if (playing) coverData(song.artwork) else null
            if (data != null) setArtworkData(data, MediaMetadata.PICTURE_TYPE_FRONT_COVER)
            else if (!queued) setArtworkUri(ArtworkProvider.uri(song.artwork))
        }
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

    private var playlist: List<MediaItemData> = emptyList()
    private var playlistKey: Pair<String, ULong>? = null

    /** The last state with a song, and when it was made. */
    private var lastSong: State? = null
    private var lastSongAt = 0L

    override fun getState(): State {
        val playback = core.playback()
        // A change of sound settings closes and reopens the sound for a moment, with no song
        // in between. Android would take the notification away in that moment and might then
        // stop Needle in the background, so the song is kept on show through short gaps.
        if (playback.current == null) {
            val kept = lastSong
            if (kept != null && android.os.SystemClock.uptimeMillis() - lastSongAt < 3000) return kept
        }
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
        // The song playing, then a few up next, so Android offers Next. Made again only when
        // the song or the queue changes: the same list each tick tells Android nothing new,
        // where a fresh one would make it redraw the notification and its cover every time.
        val key = current.id to playback.queueVersion
        val items = if (key == playlistKey) playlist else {
            (listOf(current) + core.upNext().take(10)).mapIndexed { i, song ->
                MediaItemData.Builder("$i:${song.id}")
                    .setMediaItem(songItem(song, "$i:${song.id}", queued = true, playing = i == 0))
                    .setDurationUs((song.duration * 1_000_000).toLong())
                    .build()
            }.also { playlist = it; playlistKey = key }
        }
        return state
            .setPlaylist(items)
            .setCurrentMediaItemIndex(0)
            .setContentPositionMs((playback.position * 1000).toLong())
            .setPlaybackState(if (playback.loading) STATE_BUFFERING else STATE_READY)
            .build()
            .also { lastSong = it; lastSongAt = android.os.SystemClock.uptimeMillis() }
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
