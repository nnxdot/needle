//! Window materials: Mica, Acrylic, or clear glass behind Needle's back layer on Windows;
//! Liquid Glass on macOS.
//!
//! GPUI draws with premultiplied alpha into a DirectComposition surface, so any pixel Needle
//! leaves see-through shows whatever Windows draws behind the window: the Mica or Acrylic
//! system backdrop, or the desktop itself.
//!
//! On macOS the window's back is a translucent system backdrop, and the sidebar and the
//! player float on the system's glass (`probe`): Liquid Glass on macOS 26 and newer, the
//! sidebar's vibrancy before. AppKit draws these under GPUI's layer, so Needle leaves them
//! see-through.
use gpui::{IntoElement, Styled, Window, canvas, point, px};

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Material {
    Solid,
    Mica,
    Acrylic,
    Clear,
    /// macOS: the system backdrop, with the sidebar and the player on glass.
    Liquid,
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
        // macOS has one kind of glass: on unless turned off.
        if cfg!(target_os = "macos") {
            return if name == "solid" {
                Self::Solid
            } else {
                Self::Liquid
            };
        }
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
    // macOS makes its glass opaque by itself under Reduce transparency.
    #[cfg(not(windows))]
    cfg!(target_os = "macos")
}

/// Whether this Mac has Liquid Glass (macOS 26 and newer); older Macs get the sidebar's
/// frosted vibrancy instead.
pub fn liquid_glass() -> bool {
    #[cfg(target_os = "macos")]
    {
        static FOUND: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        *FOUND.get_or_init(|| {
            std::process::Command::new("sw_vers")
                .arg("-productVersion")
                .output()
                .ok()
                .and_then(|o| {
                    String::from_utf8_lossy(&o.stdout)
                        .trim()
                        .split('.')
                        .next()?
                        .parse::<u32>()
                        .ok()
                })
                .is_some_and(|major| major >= 26)
        })
    }
    #[cfg(not(target_os = "macos"))]
    false
}

/// An invisible element that puts the system's glass under its parent (macOS), inset from its
/// edges by `inset` (left, top, right, bottom), with round corners. With `to_top`, the glass
/// reaches up to the window's top, past the title bar, as macOS's own sidebars do.
pub fn probe(
    name: &'static str,
    inset: (f32, f32, f32, f32),
    to_top: bool,
    radius: f32,
) -> impl IntoElement {
    canvas(
        move |bounds, window, _| {
            let top = if to_top {
                px(inset.1)
            } else {
                bounds.top() + px(inset.1)
            };
            let corners = gpui::Bounds::from_corners(
                point(bounds.left() + px(inset.0), top),
                point(bounds.right() - px(inset.2), bounds.bottom() - px(inset.3)),
            );
            window.set_glass_region(
                name,
                Some(gpui::GlassRegion {
                    bounds: corners,
                    corner_radius: px(radius),
                }),
            );
        },
        |_, _, _, _| {},
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
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
pub fn apply(
    window: &mut Window,
    material: Material,
    #[cfg_attr(not(windows), allow(unused_variables))] dark: bool,
) {
    use gpui::WindowBackgroundAppearance as A;
    // Acrylic and clear glass go through GPUI; Mica (and Acrylic on Windows 11) through DWM.
    let win11 = windows_11();
    // macOS: the system's own controls and glass follow Needle's dark or light theme.
    #[cfg(target_os = "macos")]
    window.set_dark_appearance(Some(dark));
    window.set_background_appearance(match material {
        Material::Clear | Material::Liquid => A::Transparent,
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
