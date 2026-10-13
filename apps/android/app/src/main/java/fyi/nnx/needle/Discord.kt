package fyi.nnx.needle

import android.app.Activity
import android.app.Application
import android.content.Context

/**
 * Discord's library (its Social SDK), when this build has it: it hands Needle's "Listening to"
 * status to the Discord app on the phone. Needle's core does the rest (discord.rs).
 */
object Discord {
    /** This build has Discord's library, and it is ready. */
    var available = false
        private set

    /** Loads Discord's library. Call once, at start, on the main thread. */
    fun load(app: Application) {
        available = runCatching {
            // Called by name, so the app also builds without the library.
            Class.forName("com.discord.socialsdk.DiscordSocialSdkInit")
                .getMethod("setEngineActivity", Activity::class.java)
                .invoke(null, AppActivity(app))
        }.isSuccess
    }

    /** The Discord app is on this phone. */
    fun installed(context: Context): Boolean =
        runCatching { context.packageManager.getPackageInfo("com.discord", 0) }.isSuccess

    /**
     * Discord's library connects to the Discord app through the activity it is given. This
     * stand-in passes everything to the app's context, so the connection lasts as long as the
     * app, not only while a screen is open.
     */
    private class AppActivity(app: Application) : Activity() {
        init {
            attachBaseContext(app)
        }
    }
}
