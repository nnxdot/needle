//! Whether Needle can open a file picker. On Linux the picker comes from the desktop portal
//! (or zenity); a system with neither would otherwise open nothing and say nothing.
use super::AppView;
use gpui::{Context, Window};

/// Shown when no picker can open.
pub const MISSING: &str = "Needle could not open a file picker. Install xdg-desktop-portal with the portal for your desktop (xdg-desktop-portal-gnome, -kde, or -gtk), or zenity.";

/// Whether a file picker can open. Checked once, then remembered.
pub fn available() -> bool {
    #[cfg(target_os = "linux")]
    {
        static AVAILABLE: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        *AVAILABLE.get_or_init(|| portal_present() || on_path("zenity"))
    }
    #[cfg(not(target_os = "linux"))]
    true
}

/// Whether the desktop portal is running, or D-Bus can start it.
#[cfg(target_os = "linux")]
fn portal_present() -> bool {
    const PORTAL: &str = "org.freedesktop.portal.Desktop";
    let Ok(bus) = zbus::blocking::Connection::session() else {
        return false;
    };
    ["ListNames", "ListActivatableNames"]
        .into_iter()
        .any(|method| {
            bus.call_method(
                Some("org.freedesktop.DBus"),
                "/org/freedesktop/DBus",
                Some("org.freedesktop.DBus"),
                method,
                &(),
            )
            .ok()
            .and_then(|reply| reply.body().deserialize::<Vec<String>>().ok())
            .is_some_and(|names| names.iter().any(|n| n == PORTAL))
        })
}

#[cfg(target_os = "linux")]
fn on_path(program: &str) -> bool {
    std::env::var_os("PATH")
        .is_some_and(|paths| std::env::split_paths(&paths).any(|dir| dir.join(program).is_file()))
}

impl AppView {
    /// Open a file dialog (`dialog`, with rfd) off the window's thread, then act on what was
    /// chosen. A dialog on the window's own thread lets Windows call back into the window
    /// while it is busy, which crashes.
    pub(super) fn pick_then<T: Send + 'static>(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        dialog: impl FnOnce() -> Option<T> + Send + 'static,
        then: impl FnOnce(&mut Self, T, &mut Window, &mut Context<Self>) + 'static,
    ) {
        if !self.can_pick(cx) {
            return;
        }
        // A thread of its own: Windows' file dialogs need one (on GPUI's background pool the
        // dialog does not open).
        let (tx, rx) = crossbeam_channel::bounded(1);
        std::thread::spawn(move || {
            let _ = tx.send(dialog());
        });
        cx.spawn_in(window, async move |this, cx| {
            let chosen = loop {
                match rx.try_recv() {
                    Ok(chosen) => break chosen,
                    Err(crossbeam_channel::TryRecvError::Empty) => {
                        cx.background_executor()
                            .timer(std::time::Duration::from_millis(50))
                            .await
                    }
                    Err(crossbeam_channel::TryRecvError::Disconnected) => break None,
                }
            };
            if let Some(chosen) = chosen {
                let _ = this.update_in(cx, |this, window, cx| {
                    then(this, chosen, window, cx);
                    cx.notify();
                });
            }
        })
        .detach();
    }

    /// Whether a file picker can open; if not, say so and why.
    pub(super) fn can_pick(&mut self, cx: &mut Context<Self>) -> bool {
        if available() {
            return true;
        }
        self.fail(MISSING);
        cx.notify();
        false
    }
}
