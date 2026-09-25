//! Opening folders and showing files in the system's file manager: File Explorer on Windows,
//! Finder on macOS, and on Linux the desktop's file manager (Nautilus, Dolphin, Nemo…),
//! through D-Bus or xdg-open.
use std::path::Path;

/// The menu item that shows a file in its folder.
pub const SHOW_IN_FOLDER: &str = if cfg!(windows) {
    "Show in File Explorer"
} else if cfg!(target_os = "macos") {
    "Show in Finder"
} else {
    "Show in folder"
};

/// The file manager's program, by its full path, so a program of the same name elsewhere on
/// `PATH` is never run instead: Windows' own `explorer.exe`, macOS's `open`, or the system's
/// `xdg-open`.
fn file_manager() -> Option<std::path::PathBuf> {
    #[cfg(windows)]
    {
        let windows = std::env::var_os("SystemRoot").unwrap_or_else(|| r"C:\Windows".into());
        Some(Path::new(&windows).join("explorer.exe")).filter(|p| p.is_file())
    }
    #[cfg(target_os = "macos")]
    {
        Some(std::path::PathBuf::from("/usr/bin/open")).filter(|p| p.is_file())
    }
    #[cfg(target_os = "linux")]
    {
        ["/usr/bin/xdg-open", "/bin/xdg-open"]
            .iter()
            .map(std::path::PathBuf::from)
            .find(|p| p.is_file())
    }
}

/// Open `folder` in the file manager. False when none could be started.
pub fn open_folder(folder: &Path) -> bool {
    file_manager().is_some_and(|program| {
        std::process::Command::new(program)
            .arg(folder)
            .spawn()
            .is_ok()
    })
}

impl super::AppView {
    /// Open `folder`, or say where it is when no file manager can.
    pub(super) fn open_folder(&mut self, folder: &Path, cx: &mut gpui::Context<Self>) {
        let _ = std::fs::create_dir_all(folder);
        if !open_folder(folder) {
            self.fail(format!(
                "No file manager could open the folder. It is at {}",
                folder.display()
            ));
            cx.notify();
        }
    }
}

/// Open the folder that holds `file`, with the file selected where the file manager can.
pub fn show_file(file: &Path) {
    #[cfg(windows)]
    {
        if let Some(explorer) = file_manager() {
            let _ = std::process::Command::new(explorer)
                .arg("/select,")
                .arg(file)
                .spawn();
        }
    }
    #[cfg(target_os = "macos")]
    {
        // `open -R` shows the file selected in Finder.
        if let Some(open) = file_manager() {
            let _ = std::process::Command::new(open).arg("-R").arg(file).spawn();
        }
    }
    #[cfg(target_os = "linux")]
    {
        // The freedesktop FileManager1 interface selects the file; without it, open the folder.
        std::thread::spawn({
            let file = file.to_path_buf();
            move || {
                if !select_in_file_manager(&file)
                    && let Some(folder) = file.parent()
                {
                    let _ = open_folder(folder);
                }
            }
        });
    }
}

#[cfg(target_os = "linux")]
fn select_in_file_manager(file: &Path) -> bool {
    let Ok(file) = std::path::absolute(file) else {
        return false;
    };
    let uri = format!("file://{}", encode_path(&file.to_string_lossy()));
    let Ok(bus) = zbus::blocking::Connection::session() else {
        return false;
    };
    bus.call_method(
        Some("org.freedesktop.FileManager1"),
        "/org/freedesktop/FileManager1",
        Some("org.freedesktop.FileManager1"),
        "ShowItems",
        &(vec![uri], ""),
    )
    .is_ok()
}

/// Percent-encode a path for a file:// URI, keeping the slashes.
#[cfg(target_os = "linux")]
fn encode_path(path: &str) -> String {
    path.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'/' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}
