//! Opening folders and showing files in the system's file manager: File Explorer on Windows;
//! on Linux the desktop's file manager (Nautilus, Dolphin, Nemo…), through D-Bus or xdg-open.
use std::path::Path;

/// The menu item that shows a file in its folder.
pub const SHOW_IN_FOLDER: &str = if cfg!(windows) {
    "Show in File Explorer"
} else {
    "Show in folder"
};

/// Open `folder` in the file manager.
pub fn open_folder(folder: &Path) {
    #[cfg(windows)]
    let _ = std::process::Command::new("explorer").arg(folder).spawn();
    #[cfg(not(windows))]
    let _ = std::process::Command::new("xdg-open").arg(folder).spawn();
}

/// Open the folder that holds `file`, with the file selected where the file manager can.
pub fn show_file(file: &Path) {
    #[cfg(windows)]
    {
        let _ = std::process::Command::new("explorer")
            .arg("/select,")
            .arg(file)
            .spawn();
    }
    #[cfg(not(windows))]
    {
        // The freedesktop FileManager1 interface selects the file; without it, open the folder.
        std::thread::spawn({
            let file = file.to_path_buf();
            move || {
                if !select_in_file_manager(&file)
                    && let Some(folder) = file.parent()
                {
                    open_folder(folder);
                }
            }
        });
    }
}

#[cfg(not(windows))]
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
#[cfg(not(windows))]
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
