package fyi.nnx.needle

import android.os.Build
import java.io.File
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

/**
 * Crashes in the app's own code (Kotlin), which the core's crash hook never sees: each one is
 * written as a crash report beside the core's (`<data>/crashes/crash-*.txt`), so the next start
 * sends it the same way, when crash reports are on, with file paths and names taken out
 * (see logfile.rs in needle-core). Then Android's own handler ends the app as before.
 */
internal object CrashCatcher {
    fun install(app: android.app.Application, data: File) {
        // Read now: while crashing, as little as possible should run.
        val version = runCatching { app.packageManager.getPackageInfo(app.packageName, 0).versionName }.getOrNull().orEmpty()
        val tv = app.packageManager.hasSystemFeature(android.content.pm.PackageManager.FEATURE_LEANBACK)
        val previous = Thread.getDefaultUncaughtExceptionHandler()
        Thread.setDefaultUncaughtExceptionHandler { thread, error ->
            runCatching { write(File(data, "crashes"), version, tv, thread, error) }
            if (previous != null) previous.uncaughtException(thread, error)
            else {
                android.os.Process.killProcess(android.os.Process.myPid())
                kotlin.system.exitProcess(10)
            }
        }
    }

    private fun write(folder: File, version: String, tv: Boolean, thread: Thread, error: Throwable) {
        folder.mkdirs()
        val report = buildString {
            append("Needle $version on Android ${Build.VERSION.RELEASE} ")
            append("(API ${Build.VERSION.SDK_INT}, ${Build.SUPPORTED_ABIS.firstOrNull().orEmpty()}")
            if (tv) append(", TV")
            append(")\n")
            append("Thread: ${thread.name}\n")
            append("Crash: ${error.javaClass.name}: ${error.message.orEmpty()}\n\n")
            append(error.stackTraceToString())
        }
        val stamp = SimpleDateFormat("yyyyMMdd-HHmmss-SSS", Locale.ROOT).format(Date())
        for (n in 0 until 8) {
            val file = File(folder, "crash-$stamp-kotlin-$n.txt")
            if (file.createNewFile()) {
                file.writeText(report)
                return
            }
        }
    }
}
