package fyi.nnx.needle

import android.content.Context
import java.io.File
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

/**
 * A line from the app itself in Needle's log (files/needle/logs/needle.log), beside the core's
 * own, in the same form: what only Android knows, such as a USB DAC's formats, can then be read
 * in a report. The core keeps the file to its size; a line added here only adds to it.
 */
internal object NeedleLog {
    private val time = SimpleDateFormat("yyyy-MM-dd HH:mm:ss.SSS", Locale.ROOT)

    fun info(context: Context, message: String) {
        android.util.Log.i("Needle", message)
        runCatching {
            val file = File(context.filesDir, "needle/logs/needle.log")
            val line = "${synchronized(time) { time.format(Date()) }} INFO  ${message.replace("\n", "\n    ")}\n"
            // Appended in one write, so it never lands inside one of the core's lines.
            java.io.FileOutputStream(file, true).use { it.write(line.toByteArray()) }
        }
    }
}
