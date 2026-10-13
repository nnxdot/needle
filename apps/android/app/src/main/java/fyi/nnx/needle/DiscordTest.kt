package fyi.nnx.needle

import android.app.Activity
import android.os.Handler
import android.os.HandlerThread
import android.util.Log

/**
 * A test only: shows a fixed "Listening to…" on Discord through Discord's Social SDK, to see
 * whether Discord's phone app shows it, and keeps it while Needle is in the background. It
 * sends it again every minute, so the log tells whether changes still arrive from the
 * background. Does nothing when the build has no SDK (see scripts/android-discord.sh).
 * Watch with: adb logcat -s NeedleDiscord
 */
object DiscordTest {
    private const val TAG = "NeedleDiscord"
    private const val APP_ID = 1552097597454295110L
    private var started = false

    fun start(activity: Activity) {
        if (started) return
        try {
            System.loadLibrary("discord_partner_sdk")
            System.loadLibrary("needle_discord")
        } catch (e: UnsatisfiedLinkError) {
            Log.i(TAG, "no Discord SDK in this build")
            return
        }
        started = true
        // Called by name, so the app still builds without the SDK.
        Class.forName("com.discord.socialsdk.DiscordSocialSdkInit")
            .getMethod("setEngineActivity", Activity::class.java)
            .invoke(null, activity)
        val handler = Handler(HandlerThread("discord").apply { start() }.looper)
        val since = System.currentTimeMillis() / 1000
        handler.post { nativeStart(APP_ID) }
        val show = object : Runnable {
            override fun run() {
                Log.i(TAG, "sending")
                nativeShow("Discord test", "Needle on Android", "Social SDK test", since)
                handler.postDelayed(this, 60_000)
            }
        }
        handler.postDelayed(show, 2_000)
        val pump = object : Runnable {
            override fun run() {
                nativeRunCallbacks()
                handler.postDelayed(this, 100)
            }
        }
        handler.post(pump)
    }

    private external fun nativeStart(appId: Long)
    private external fun nativeShow(title: String, artist: String, album: String, started: Long)
    private external fun nativeRunCallbacks()
}
