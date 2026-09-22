//! Window materials: Mica, Acrylic, or clear glass behind Needle's back layer.
//!
//! GPUI draws with premultiplied alpha into a DirectComposition surface, so any pixel Needle
//! leaves see-through shows whatever Windows draws behind the window: the Mica or Acrylic
//! system backdrop, or the desktop itself.
use gpui::Window;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Material {
    Solid,
    Mica,
    Acrylic,
    Clear,
}

impl Material {
    /// Settings value, the name people see, and what it does.
    pub const ALL: [(&'static str, &'static str, &'static str); 4] = [
        (
            "mica",
            "Mica",
            "Soft color from your wallpaper. Calm and smooth.",
        ),
        (
            "acrylic",
            "Acrylic",
            "Frosted glass: a blur of what is behind the window.",
        ),
        ("clear", "Clear", "See straight through, with no blur."),
        ("solid", "Solid", "No glass."),
    ];
    pub fn from_name(name: &str) -> Self {
        match name {
            "solid" => Self::Solid,
            "acrylic" => Self::Acrylic,
            "clear" => Self::Clear,
            _ => Self::Mica,
        }
    }
}

/// Windows Settings › Personalization › Colors › Transparency effects.
pub fn system_allows_transparency() -> bool {
    #[cfg(windows)]
    {
        read_dword(
            windows_sys::Win32::System::Registry::HKEY_CURRENT_USER,
            "Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize",
            "EnableTransparency",
        )
        .is_none_or(|v| v != 0)
    }
    #[cfg(not(windows))]
    false
}

/// Windows 11 is build 22000 and later.
pub fn windows_11() -> bool {
    #[cfg(windows)]
    {
        read_string(
            windows_sys::Win32::System::Registry::HKEY_LOCAL_MACHINE,
            "SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion",
            "CurrentBuildNumber",
        )
        .and_then(|b| b.trim().parse::<u32>().ok())
        .is_some_and(|b| b >= 22000)
    }
    #[cfg(not(windows))]
    false
}

#[cfg(windows)]
fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain([0]).collect()
}

#[cfg(windows)]
fn read_dword(
    root: windows_sys::Win32::System::Registry::HKEY,
    key: &str,
    value: &str,
) -> Option<u32> {
    use windows_sys::Win32::System::Registry::{RRF_RT_REG_DWORD, RegGetValueW};
    let (key, value) = (wide(key), wide(value));
    let mut data = 0u32;
    let mut size = 4u32;
    let status = unsafe {
        RegGetValueW(
            root,
            key.as_ptr(),
            value.as_ptr(),
            RRF_RT_REG_DWORD,
            std::ptr::null_mut(),
            &mut data as *mut u32 as *mut _,
            &mut size,
        )
    };
    (status == 0).then_some(data)
}

#[cfg(windows)]
fn read_string(
    root: windows_sys::Win32::System::Registry::HKEY,
    key: &str,
    value: &str,
) -> Option<String> {
    use windows_sys::Win32::System::Registry::{RRF_RT_REG_SZ, RegGetValueW};
    let (key, value) = (wide(key), wide(value));
    let mut data = [0u16; 64];
    let mut size = (data.len() * 2) as u32;
    let status = unsafe {
        RegGetValueW(
            root,
            key.as_ptr(),
            value.as_ptr(),
            RRF_RT_REG_SZ,
            std::ptr::null_mut(),
            data.as_mut_ptr() as *mut _,
            &mut size,
        )
    };
    if status != 0 {
        return None;
    }
    let len = data.iter().position(|c| *c == 0).unwrap_or(data.len());
    Some(String::from_utf16_lossy(&data[..len]))
}

/// Ask Windows to draw `material` behind the window, in dark or light.
pub fn apply(window: &mut Window, material: Material, dark: bool) {
    use gpui::WindowBackgroundAppearance as A;
    // Acrylic and clear glass go through GPUI; Mica (and Acrylic on Windows 11) through DWM.
    let win11 = windows_11();
    window.set_background_appearance(match material {
        Material::Clear => A::Transparent,
        Material::Acrylic if !win11 => A::Blurred,
        _ => A::Opaque,
    });
    #[cfg(windows)]
    {
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
        use windows_sys::Win32::Graphics::Dwm::{
            DwmExtendFrameIntoClientArea, DwmSetWindowAttribute,
        };
        use windows_sys::Win32::UI::Controls::MARGINS;
        let Ok(handle) = HasWindowHandle::window_handle(window) else {
            return;
        };
        let RawWindowHandle::Win32(win32) = handle.as_raw() else {
            return;
        };
        let hwnd = win32.hwnd.get() as _;
        // DWMWA_USE_IMMERSIVE_DARK_MODE = 20, DWMWA_SYSTEMBACKDROP_TYPE = 38.
        // Backdrop types: 1 none, 2 Mica, 3 Acrylic.
        let dark_mode: i32 = dark as i32;
        let backdrop: i32 = match material {
            Material::Mica => 2,
            Material::Acrylic if win11 => 3,
            _ => 1,
        };
        let edge = 0;
        let margins = MARGINS {
            cxLeftWidth: edge,
            cxRightWidth: edge,
            cyTopHeight: edge,
            cyBottomHeight: edge,
        };
        unsafe {
            DwmSetWindowAttribute(hwnd, 20, &dark_mode as *const i32 as *const _, 4);
            if win11 {
                DwmSetWindowAttribute(hwnd, 38, &backdrop as *const i32 as *const _, 4);
            }
            DwmExtendFrameIntoClientArea(hwnd, &margins);
        }
    }
}
