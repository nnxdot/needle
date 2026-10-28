package fyi.nnx.needle

import android.content.Context
import android.graphics.drawable.Icon
import android.service.quicksettings.Tile
import android.service.quicksettings.TileService
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch

/**
 * Needle's Quick Settings tile (Android only): Play and Pause from the pull-down shade, even with
 * Needle closed. With nothing to go on with, it plays what was chosen in Settings › Playback ›
 * Quick Settings tile: a smart mix, everything shuffled, or the music with a 30-minute sleep
 * timer. A long press opens that setting.
 */
class NeedleTile : TileService() {
    override fun onStartListening() {
        super.onStartListening()
        show()
    }

    override fun onClick() {
        super.onClick()
        val app = applicationContext
        NeedleApp.instance.scope.launch(Dispatchers.IO) {
            val core = NeedleApp.instance.core
            runCatching {
                val state = core.playback()
                when {
                    state.playing -> core.toggle()
                    state.current != null && TileAction.read(app) != "sleep" -> core.toggle()
                    else -> {
                        when (TileAction.read(app)) {
                            "shuffle" -> core.play(core.searchSongsPage("shuffle limit 200", 0u, 200u).map { it.id }, 0u)
                            "sleep" -> {
                                if (state.current == null) core.play(core.searchSongsPage("shuffle limit 200", 0u, 200u).map { it.id }, 0u)
                                else if (!state.playing) core.toggle()
                                core.startSleepTimer("minutes", 30u, core.sleepFade())
                                fyi.nnx.needle.ui.showMessage("Sleep timer: 30 minutes")
                            }
                            else -> {
                                val last = state.current?.id ?: core.history(0u, 1u).firstOrNull()?.song?.id
                                if (last != null) core.startRadio(last)
                                else core.play(core.searchSongsPage("shuffle limit 200", 0u, 200u).map { it.id }, 0u)
                            }
                        }
                    }
                }
            }.onFailure { NeedleLog.info("The tile could not play: ${it.message}") }
            startPlaybackService(app)
            NeedleApp.instance.wake()
            kotlinx.coroutines.delay(400)
            launch(Dispatchers.Main) { show() }
        }
    }

    /** On while music plays; the song under the name. */
    private fun show() {
        val tile = qsTile ?: return
        val state = runCatching { NeedleApp.instance.core.playback() }.getOrNull()
        tile.label = "Needle"
        tile.icon = Icon.createWithResource(this, R.drawable.ic_tile)
        tile.state = if (state?.playing == true) Tile.STATE_ACTIVE else Tile.STATE_INACTIVE
        if (android.os.Build.VERSION.SDK_INT >= 29) {
            tile.subtitle = state?.current?.title ?: when (TileAction.read(this)) {
                "shuffle" -> "Shuffle everything"
                "sleep" -> "Sleep timer · 30 min"
                else -> "Smart mix"
            }
        }
        tile.updateTile()
    }
}

/** What the tile plays when nothing is there to go on with: "mix", "shuffle", or "sleep". */
object TileAction {
    fun read(context: Context): String =
        context.getSharedPreferences("needle", Context.MODE_PRIVATE).getString("tile_action", "mix") ?: "mix"

    fun set(context: Context, value: String) {
        context.getSharedPreferences("needle", Context.MODE_PRIVATE).edit().putString("tile_action", value).apply()
    }
}
