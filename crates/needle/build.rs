//! Embeds the Windows app icon (resource 1, which GPUI uses for the window and taskbar).
fn main() {
    println!("cargo:rerun-if-changed=needle.rc");
    println!("cargo:rerun-if-changed=assets/needle.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        embed_resource::compile("needle.rc", embed_resource::NONE)
            .manifest_optional()
            .expect("could not embed the app icon");
    }
}
