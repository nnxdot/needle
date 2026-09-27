//! Hands the app's Java context to Rust at start, for the audio output: cpal asks Android for
//! its devices through it. The app calls `NativeBridge.initAndroid(context)` once, after
//! `System.loadLibrary("needle_mobile")`.
use jni::{JNIEnv, objects::JObject, sys::jobject};

#[unsafe(no_mangle)]
pub extern "system" fn Java_fyi_nnx_needle_NativeBridge_initAndroid(
    mut env: JNIEnv,
    _this: JObject,
    context: JObject,
) {
    let Ok(vm) = env.get_java_vm() else {
        return;
    };
    // The application context lives as long as the app, so its global reference is kept.
    let context = env
        .call_method(
            &context,
            "getApplicationContext",
            "()Landroid/content/Context;",
            &[],
        )
        .and_then(|value| value.l())
        .unwrap_or(context);
    let Ok(global) = env.new_global_ref(context) else {
        return;
    };
    let raw: jobject = global.as_obj().as_raw();
    std::mem::forget(global);
    // SAFETY: a live JavaVM and a global reference that is never released.
    unsafe { ndk_context::initialize_android_context(vm.get_java_vm_pointer().cast(), raw.cast()) };
}
