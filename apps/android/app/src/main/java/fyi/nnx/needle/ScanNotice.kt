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

    fun show(context: Context, scan: ScanStatus) {
        val open = PendingIntent.getActivity(
            context, 0, Intent(context, MainActivity::class.java), PendingIntent.FLAG_IMMUTABLE,
        )
        val builder = Notification.Builder(context, CHANNEL)
            .setSmallIcon(android.R.drawable.stat_notify_sync)
            .setContentTitle("Reading your music")
            .setContentText("${scan.scanned} files · ${scan.imported} new")
            .setOnlyAlertOnce(true)
            .setOngoing(true)
            .setContentIntent(open)
            .setProgress(0, 0, true)
        if (Build.VERSION.SDK_INT >= 36) {
            builder.setShortCriticalText("${scan.scanned}")
            builder.setFlag(Notification.FLAG_PROMOTED_ONGOING, true)
        }
        runCatching { manager(context).notify(ID, builder.build()) }
    }

    fun hide(context: Context) {
        runCatching { manager(context).cancel(ID) }
    }
}
