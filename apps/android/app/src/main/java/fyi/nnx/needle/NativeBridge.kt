package fyi.nnx.needle

import android.content.Context

/** The one call into Needle's Rust library that is not through UniFFI: see android.rs. */
object NativeBridge {
    init {
        System.loadLibrary("needle_mobile")
    }

    /** Gives Rust the app's context, which the audio output needs. Call once, at start. */
    external fun initAndroid(context: Context)
}
