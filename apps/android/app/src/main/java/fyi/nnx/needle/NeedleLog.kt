package fyi.nnx.needle

/**
 * A line from the app itself in Needle's log (and logcat), beside the core's own: what only
 * Android knows, such as a USB DAC's formats, can then be read in a report.
 */
internal object NeedleLog {
    fun info(message: String) {
        runCatching { NeedleApp.instance.core.logInfo(message) }
            .onFailure { android.util.Log.i("Needle", message) }
    }
}
