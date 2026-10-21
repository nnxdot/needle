package fyi.nnx.needle

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.os.Build
import fyi.nnx.needle.core.ScanStatus

/**
 * While Needle reads the music folders, a notice shows how far it is; on Android 16 and newer
 * it is a live update, in the status bar. It goes when the reading ends.
 */
object ScanNotice {
    private const val CHANNEL = "library"
    private const val ID = 7

    private fun manager(context: Context): NotificationManager {
        val manager = context.getSystemService(NotificationManager::class.java)
        if (manager.getNotificationChannel(CHANNEL) == null) {
            manager.createNotificationChannel(
                NotificationChannel(CHANNEL, "Reading your music", NotificationManager.IMPORTANCE_LOW).apply {
                    description = "Shows while Needle reads your music folders."
                },
            )
        }
        return manager
    }

    private var lastText: String? = null
    private var lastAt = 0L

    fun show(context: Context, scan: ScanStatus) {
        val text = if (scan.scanned == 0u && scan.current.isNotEmpty()) {
            "Looking in ${scan.current.trimEnd('/').substringAfterLast('/')}"
        } else {
            "${scan.scanned} files · ${scan.imported} new"
        }
        // Posting a notice costs the phone some work: about once a second at most, and when
        // nothing changed only often enough to keep it from timing out.
        val now = android.os.SystemClock.uptimeMillis()
        if (now - lastAt < 1000 || (text == lastText && now - lastAt < 20_000)) return
        lastText = text
        lastAt = now
        val open = PendingIntent.getActivity(
            context, 0, Intent(context, MainActivity::class.java), PendingIntent.FLAG_IMMUTABLE,
        )
        val builder = Notification.Builder(context, CHANNEL)
            .setSmallIcon(android.R.drawable.stat_notify_sync)
            .setContentTitle("Reading your music")
            .setContentText(text)
            .setOnlyAlertOnce(true)
            .setOngoing(true)
            .setContentIntent(open)
            .setProgress(0, 0, true)
            // If Needle is closed mid-read, the notice goes by itself instead of staying.
            .setTimeoutAfter(60_000)
        if (Build.VERSION.SDK_INT >= 36) {
            builder.setShortCriticalText("${scan.scanned}")
            // Asks Android to show it as a live update: Android 17 has a call for it, and sets the
            // flag itself if it does; Android 16 has only the flag, which the app sets.
            if (Build.VERSION.SDK_INT >= 37) builder.setRequestPromotedOngoing(true)
            else builder.setFlag(Notification.FLAG_PROMOTED_ONGOING, true)
        }
        runCatching { manager(context).notify(ID, builder.build()) }
    }

    fun hide(context: Context) {
        lastText = null
        lastAt = 0L
        runCatching { manager(context).cancel(ID) }
    }
}
