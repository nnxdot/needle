//! Windows: Previous, Play/Pause, Next, and Favorite under Needle's preview on the taskbar
//! (the thumbnail toolbar, `ITaskbarList3`). A click comes back as the window's `WM_COMMAND`,
//! caught by a subclass of the window. Windows only; Linux taskbars have no such buttons.
pub use imp::Taskbar;

/// The buttons, by their number: what each does.
pub const ACTIONS: [&str; 4] = ["previous", "play", "next", "favorite"];

mod imp {
    use super::ACTIONS;
    use crate::ui::Event;
    use std::sync::OnceLock;
    use windows::{
        Win32::{
            Foundation::{HWND, LPARAM, LRESULT, WPARAM},
            System::Com::{
                CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
            },
            UI::{
                Shell::{
                    DefSubclassProc, ITaskbarList3, SetWindowSubclass, THB_FLAGS, THB_ICON,
                    THB_TOOLTIP, THBF_ENABLED, THBN_CLICKED, THUMBBUTTON, TaskbarList,
                },
                WindowsAndMessaging::{
                    CreateIcon, DestroyIcon, HICON, RegisterWindowMessageW, WM_COMMAND,
                },
            },
        },
        core::w,
    };

    /// Where the window's clicks go (the subclass procedure has no other way to the app).
    static EVENTS: OnceLock<crossbeam_channel::Sender<Event>> = OnceLock::new();
    /// "TaskbarButtonCreated": Explorer (re)made Needle's taskbar button, so the buttons must
    /// be added again.
    static CREATED: OnceLock<u32> = OnceLock::new();

    /// The look the buttons were last given, to change them only when it changes.
    #[derive(Clone, Copy, PartialEq)]
    struct Shown {
        playing: bool,
        favorite: bool,
        light: bool,
    }

    pub struct Taskbar {
        hwnd: HWND,
        list: Option<ITaskbarList3>,
        added: bool,
        shown: Option<Shown>,
        icons: Vec<HICON>,
        /// The taskbar's light or dark, looked up every few seconds.
        light: (bool, std::time::Instant),
    }

    unsafe extern "system" fn subclass(
        hwnd: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
        _id: usize,
        _data: usize,
    ) -> LRESULT {
        if message == WM_COMMAND && (wparam.0 >> 16) as u32 & 0xffff == THBN_CLICKED {
            let button = wparam.0 & 0xffff;
            if let (Some(events), Some(action)) = (EVENTS.get(), ACTIONS.get(button)) {
                let _ = events.send(Event::Hotkey(action.to_string()));
            }
            return LRESULT(0);
        }
        if CREATED.get() == Some(&message)
            && let Some(events) = EVENTS.get()
        {
            let _ = events.send(Event::TaskbarCreated);
        }
        unsafe { DefSubclassProc(hwnd, message, wparam, lparam) }
    }

    /// The taskbar is light (Windows' "Choose your default Windows mode").
    fn light_taskbar() -> bool {
        use windows_sys::Win32::System::Registry::{
            HKEY_CURRENT_USER, RRF_RT_REG_DWORD, RegGetValueW,
        };
        let key: Vec<u16> = "Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize\0"
            .encode_utf16()
            .collect();
        let name: Vec<u16> = "SystemUsesLightTheme\0".encode_utf16().collect();
        let mut value = 0u32;
        let mut size = 4u32;
        let found = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                key.as_ptr(),
                name.as_ptr(),
                RRF_RT_REG_DWORD,
                std::ptr::null_mut(),
                &mut value as *mut u32 as *mut _,
                &mut size,
            )
        };
        found == 0 && value == 1
    }

    /// One of Needle's glyphs as a 32-pixel icon in one colour.
    fn icon(name: &str, color: &str) -> Option<HICON> {
        use gpui::AssetSource;
        let svg = crate::ui::assets::Assets
            .load(&format!("needle/{name}.svg"))
            .ok()??;
        let svg = String::from_utf8_lossy(&svg).replace("currentColor", color);
        let tree = usvg::Tree::from_str(&svg, &usvg::Options::default()).ok()?;
        const SIZE: u32 = 32;
        let mut pixmap = resvg::tiny_skia::Pixmap::new(SIZE, SIZE)?;
        let scale = SIZE as f32 / 24.;
        resvg::render(
            &tree,
            resvg::tiny_skia::Transform::from_scale(scale, scale),
            &mut pixmap.as_mut(),
        );
        // Colour as BGRA with its alpha; the mask is all clear (the alpha decides).
        let mut color_bits = Vec::with_capacity((SIZE * SIZE * 4) as usize);
        for p in pixmap.pixels() {
            let c = p.demultiply();
            color_bits.extend([c.blue(), c.green(), c.red(), c.alpha()]);
        }
        let mask = vec![0u8; (SIZE * SIZE / 8) as usize];
        unsafe {
            CreateIcon(
                None,
                SIZE as i32,
                SIZE as i32,
                1,
                32,
                mask.as_ptr(),
                color_bits.as_ptr(),
            )
            .ok()
        }
    }

    fn tip(text: &str) -> [u16; 260] {
        let mut tip = [0u16; 260];
        for (slot, c) in tip.iter_mut().zip(text.encode_utf16().take(259)) {
            *slot = c;
        }
        tip
    }

    impl Taskbar {
        /// For the window `hwnd`, sending clicks as `Event::Hotkey` (the same actions as keys).
        pub fn new(hwnd: isize, events: crossbeam_channel::Sender<Event>) -> Self {
            let hwnd = HWND(hwnd as *mut _);
            let _ = EVENTS.set(events);
            let _ = CREATED
                .get_or_init(|| unsafe { RegisterWindowMessageW(w!("TaskbarButtonCreated")) });
            let list = unsafe {
                let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
                let list: Option<ITaskbarList3> =
                    CoCreateInstance(&TaskbarList, None, CLSCTX_INPROC_SERVER).ok();
                list.filter(|l| l.HrInit().is_ok())
            };
            unsafe {
                let _ = SetWindowSubclass(hwnd, Some(subclass), 0x6e65, 0);
            }
            Self {
                hwnd,
                list,
                added: false,
                shown: None,
                icons: vec![],
                light: (light_taskbar(), std::time::Instant::now()),
            }
        }

        /// Explorer made the taskbar button again: add the buttons again.
        pub fn recreated(&mut self) {
            self.added = false;
            self.shown = None;
        }

        /// Show the buttons for what plays now (cheap when nothing changed).
        pub fn update(&mut self, playing: bool, favorite: bool) {
            let Some(list) = &self.list else { return };
            if self.light.1.elapsed() > std::time::Duration::from_secs(3) {
                self.light = (light_taskbar(), std::time::Instant::now());
            }
            let shown = Shown {
                playing,
                favorite,
                light: self.light.0,
            };
            if self.added && self.shown == Some(shown) {
                return;
            }
            let color = if shown.light { "#1f1f1f" } else { "#ffffff" };
            let accent = if shown.light { "#8c5a12" } else { "#e2b46c" };
            let wanted = [
                ("previous", color, "Previous"),
                (
                    if playing { "pause" } else { "play" },
                    color,
                    if playing { "Pause" } else { "Play" },
                ),
                ("next", color, "Next"),
                (
                    if favorite { "heart-fill" } else { "heart" },
                    if favorite { accent } else { color },
                    if favorite {
                        "Remove from favorites"
                    } else {
                        "Add to favorites"
                    },
                ),
            ];
            let icons: Vec<HICON> = wanted
                .iter()
                .filter_map(|(name, color, _)| icon(name, color))
                .collect();
            if icons.len() != wanted.len() {
                return;
            }
            let buttons: Vec<THUMBBUTTON> = wanted
                .iter()
                .zip(&icons)
                .enumerate()
                .map(|(i, ((_, _, label), icon))| THUMBBUTTON {
                    dwMask: THB_ICON | THB_TOOLTIP | THB_FLAGS,
                    iId: i as u32,
                    hIcon: *icon,
                    szTip: tip(label),
                    dwFlags: THBF_ENABLED,
                    ..Default::default()
                })
                .collect();
            let done = unsafe {
                if self.added {
                    list.ThumbBarUpdateButtons(self.hwnd, &buttons)
                } else {
                    list.ThumbBarAddButtons(self.hwnd, &buttons)
                }
            };
            // The taskbar keeps its own copies of the icons.
            for old in std::mem::replace(&mut self.icons, icons) {
                unsafe {
                    let _ = DestroyIcon(old);
                }
            }
            if done.is_ok() {
                self.added = true;
                self.shown = Some(shown);
            }
        }
    }

    impl Drop for Taskbar {
        fn drop(&mut self) {
            for icon in self.icons.drain(..) {
                unsafe {
                    let _ = DestroyIcon(icon);
                }
            }
        }
    }

    #[cfg(test)]
    mod tests {
        #[test]
        fn the_buttons_icons_draw() {
            for name in ["previous", "play", "pause", "next", "heart", "heart-fill"] {
                let icon = super::icon(name, "#ffffff").expect(name);
                unsafe {
                    let _ = windows::Win32::UI::WindowsAndMessaging::DestroyIcon(icon);
                }
            }
            assert_eq!(super::tip("Play")[..5], [80, 108, 97, 121, 0]);
        }
    }
}
