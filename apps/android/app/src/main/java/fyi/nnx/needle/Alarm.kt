package fyi.nnx.needle

import android.app.AlarmManager
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.media.AudioManager
import androidx.core.app.NotificationCompat
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import java.util.Calendar

/**
 * Wake up to music (Android only): at the time chosen, on the days chosen, Needle plays a
 * playlist, a smart mix, or everything shuffled, fading in from quiet over two minutes, with a
 * notification to Snooze (10 minutes) or Stop. Kept on the phone; set with Android's alarm
 * clock, so it rings on time, shows in the status bar, and is set again after a restart.
 */
object WakeAlarm {
    private const val Prefs = "needle-alarm"
    private const val Channel = "alarm"
    const val Fire = "fyi.nnx.needle.ALARM"
    const val Snooze = "fyi.nnx.needle.ALARM_SNOOZE"
    const val Stop = "fyi.nnx.needle.ALARM_STOP"
    private const val Notice = 4401

    /** What wakes you: "playlist" (with its id and name), "mix", or "shuffle". */
    data class Setting(
        val on: Boolean = false,
        val hour: Int = 7,
        val minute: Int = 30,
        /** Monday first; none chosen: once, at the next such time. */
        val days: Set<Int> = setOf(0, 1, 2, 3, 4),
        val what: String = "mix",
        val playlistId: String = "",
        val playlistName: String = "",
        val fadeIn: Boolean = true,
        val snooze: Boolean = true,
    )

    fun read(context: Context): Setting {
        val p = context.getSharedPreferences(Prefs, Context.MODE_PRIVATE)
        return Setting(
            on = p.getBoolean("on", false),
            hour = p.getInt("hour", 7),
            minute = p.getInt("minute", 30),
            days = p.getString("days", "0,1,2,3,4").orEmpty().split(',').mapNotNull { it.toIntOrNull() }.toSet(),
            what = p.getString("what", "mix") ?: "mix",
            playlistId = p.getString("playlistId", "").orEmpty(),
            playlistName = p.getString("playlistName", "").orEmpty(),
            fadeIn = p.getBoolean("fadeIn", true),
            snooze = p.getBoolean("snooze", true),
        )
    }

    /** Keeps the setting and sets (or takes away) Android's alarm for it. */
    fun save(context: Context, s: Setting) {
        context.getSharedPreferences(Prefs, Context.MODE_PRIVATE).edit()
            .putBoolean("on", s.on).putInt("hour", s.hour).putInt("minute", s.minute)
            .putString("days", s.days.sorted().joinToString(","))
            .putString("what", s.what).putString("playlistId", s.playlistId).putString("playlistName", s.playlistName)
            .putBoolean("fadeIn", s.fadeIn).putBoolean("snooze", s.snooze)
            .apply()
        schedule(context)
    }

    /** May Needle set exact alarms? Android 12 and newer ask the person (Settings › Alarms & reminders). */
    fun allowed(context: Context): Boolean =
        android.os.Build.VERSION.SDK_INT < 31 || context.getSystemService(AlarmManager::class.java).canScheduleExactAlarms()

    /** When it rings next (milliseconds), or null while it is off. */
    fun next(s: Setting, from: Long = System.currentTimeMillis()): Long? {
        if (!s.on) return null
        val c = Calendar.getInstance().apply {
            timeInMillis = from
            set(Calendar.HOUR_OF_DAY, s.hour)
            set(Calendar.MINUTE, s.minute)
            set(Calendar.SECOND, 0)
            set(Calendar.MILLISECOND, 0)
        }
        repeat(8) {
            // Calendar: Sunday 1 … Saturday 7; here Monday 0 … Sunday 6.
            val day = (c.get(Calendar.DAY_OF_WEEK) + 5) % 7
            if (c.timeInMillis > from && (s.days.isEmpty() || day in s.days)) return c.timeInMillis
            c.add(Calendar.DAY_OF_YEAR, 1)
        }
        return null
    }

    private fun ring(context: Context): PendingIntent = PendingIntent.getBroadcast(
        context, 0, Intent(context, AlarmReceiver::class.java).setAction(Fire), PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
    )

    fun schedule(context: Context, at: Long? = next(read(context))) {
        val alarms = context.getSystemService(AlarmManager::class.java)
        alarms.cancel(ring(context))
        if (at == null || !allowed(context)) return
        val show = PendingIntent.getActivity(context, 1, Intent(context, MainActivity::class.java), PendingIntent.FLAG_IMMUTABLE)
        alarms.setAlarmClock(AlarmManager.AlarmClockInfo(at, show), ring(context))
    }

    /** It rings: the music starts, fading in, with the notification; the next one is set. */
    fun fire(context: Context) {
        val app = context.applicationContext
        val s = read(app)
        // A one-time alarm (no days) is spent once it rings.
        if (s.days.isEmpty()) save(app, s.copy(on = false)) else schedule(app)
        NeedleApp.instance.scope.launch(Dispatchers.IO) {
            val core = NeedleApp.instance.core
            val audio = app.getSystemService(AudioManager::class.java)
            val stream = AudioManager.STREAM_MUSIC
            val target = audio.getStreamVolume(stream).coerceAtLeast(audio.getStreamMaxVolume(stream) / 3)
            if (s.fadeIn) audio.setStreamVolume(stream, 1, 0)
            runCatching {
                when (s.what) {
                    "playlist" -> core.play(core.playlistSongs(s.playlistId).map { it.id }.shuffled(), 0u)
                    "shuffle" -> core.play(core.searchSongsPage("shuffle limit 200", 0u, 200u).map { it.id }, 0u)
                    else -> {
                        // A smart mix from the song played last, or everything shuffled.
                        val last = runCatching { core.history(0u, 1u).firstOrNull()?.song?.id }.getOrNull()
                        if (last != null) core.startRadio(last) else core.play(core.searchSongsPage("shuffle limit 200", 0u, 200u).map { it.id }, 0u)
                    }
                }
            }.onFailure { NeedleLog.info("The alarm could not play: ${it.message}") }
            startPlaybackService(app)
            notify(app, s)
            if (s.fadeIn) {
                // From quiet to the volume it had, over two minutes.
                val steps = (target - 1).coerceAtLeast(1)
                for (i in 1..steps) {
                    delay(120_000L / steps)
                    audio.setStreamVolume(stream, 1 + i, 0)
                }
            }
        }
    }

    fun snooze(context: Context) {
        pause()
        context.getSystemService(NotificationManager::class.java).cancel(Notice)
        schedule(context, System.currentTimeMillis() + 10 * 60_000L)
        fyi.nnx.needle.ui.showMessage("Snoozing for 10 minutes")
    }

    fun stop(context: Context) {
        pause()
        context.getSystemService(NotificationManager::class.java).cancel(Notice)
        schedule(context)
    }

    /** Pauses the music, if it plays (the core has a toggle). */
    private fun pause() {
        runCatching { NeedleApp.instance.core.let { if (it.playback().playing) it.toggle() } }
    }

    private fun notify(context: Context, s: Setting) {
        val manager = context.getSystemService(NotificationManager::class.java)
        manager.createNotificationChannel(NotificationChannel(Channel, "Alarm", NotificationManager.IMPORTANCE_HIGH))
        fun action(what: String) = PendingIntent.getBroadcast(
            context, what.hashCode(), Intent(context, AlarmReceiver::class.java).setAction(what), PendingIntent.FLAG_IMMUTABLE,
        )
        val open = PendingIntent.getActivity(context, 2, Intent(context, MainActivity::class.java), PendingIntent.FLAG_IMMUTABLE)
        val builder = NotificationCompat.Builder(context, Channel)
            .setSmallIcon(R.drawable.ic_tile)
            .setContentTitle("Good morning")
            .setContentText(
                when (s.what) {
                    "playlist" -> s.playlistName.ifBlank { "Your playlist" }
                    "shuffle" -> "Everything, shuffled"
                    else -> "A smart mix"
                },
            )
            .setCategory(NotificationCompat.CATEGORY_ALARM)
            .setPriority(NotificationCompat.PRIORITY_HIGH)
            .setContentIntent(open)
            .setAutoCancel(true)
            .addAction(0, "Stop", action(Stop))
        if (s.snooze) builder.addAction(0, "Snooze 10 min", action(Snooze))
        runCatching { manager.notify(Notice, builder.build()) }
    }
}

/** Android's alarm, its notification's buttons, and the phone starting (to set it again). */
class AlarmReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        when (intent.action) {
            WakeAlarm.Fire -> WakeAlarm.fire(context)
            WakeAlarm.Snooze -> WakeAlarm.snooze(context)
            WakeAlarm.Stop -> WakeAlarm.stop(context)
            Intent.ACTION_BOOT_COMPLETED, AlarmManager.ACTION_SCHEDULE_EXACT_ALARM_PERMISSION_STATE_CHANGED -> WakeAlarm.schedule(context)
        }
    }
}
