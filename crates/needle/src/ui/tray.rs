//! Hide to tray: with it on, Needle has an icon in the notification area, and closing (or
//! "Hide to tray") hides the window while the music plays on. The icon's menu shows Needle
//! again, controls playback, or quits; clicking the icon shows Needle.
use super::{AppView, Event};
use gpui::*;

/// What the tray icon asks for.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TrayAction {
    Show,
    Toggle,
    Next,
    Previous,
    Quit,
}

/// The icon, while hide to tray is on.
pub struct Tray {
    #[cfg(windows)]
    _icon: tray_icon::TrayIcon,
}

impl Tray {
    #[cfg(windows)]
    fn new(sender: crossbeam_channel::Sender<Event>) -> anyhow::Result<Self> {
        use tray_icon::{
            Icon, MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent,
            menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
        };
        let menu = Menu::new();
        menu.append_items(&[
            &MenuItem::with_id("show", "Show Needle", true, None),
            &PredefinedMenuItem::separator(),
            &MenuItem::with_id("toggle", "Play / Pause", true, None),
            &MenuItem::with_id("next", "Next", true, None),
            &MenuItem::with_id("previous", "Previous", true, None),
            &PredefinedMenuItem::separator(),
            &MenuItem::with_id("quit", "Quit Needle", true, None),
        ])?;
        // The app icon Needle's build embeds as resource 1.
        let icon = Icon::from_resource(1, Some((32, 32)))?;
        let tray = TrayIconBuilder::new()
            .with_icon(icon)
            .with_tooltip("Needle")
            .with_menu(Box::new(menu))
            .with_menu_on_left_click(false)
            .build()?;
        let clicks = sender.clone();
        TrayIconEvent::set_event_handler(Some(move |event: TrayIconEvent| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let _ = clicks.send(Event::Tray(TrayAction::Show));
            }
        }));
        MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
            let action = match event.id.as_ref() {
                "show" => TrayAction::Show,
                "toggle" => TrayAction::Toggle,
                "next" => TrayAction::Next,
                "previous" => TrayAction::Previous,
                "quit" => TrayAction::Quit,
                _ => return,
            };
            let _ = sender.send(Event::Tray(action));
        }));
        Ok(Self { _icon: tray })
    }
    #[cfg(not(windows))]
    fn new(_sender: crossbeam_channel::Sender<Event>) -> anyhow::Result<Self> {
        anyhow::bail!("The tray is only on Windows")
    }
}

/// Show or hide the whole window (Windows only).
pub fn set_window_shown(window: &Window, shown: bool) {
    #[cfg(windows)]
    {
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
        use windows_sys::Win32::UI::WindowsAndMessaging::{SW_HIDE, SW_SHOW, ShowWindow};
        if let Ok(handle) = HasWindowHandle::window_handle(window)
            && let RawWindowHandle::Win32(win32) = handle.as_raw()
        {
            // SAFETY: the handle is this window's, and it is alive while `window` is.
            unsafe {
                ShowWindow(win32.hwnd.get() as _, if shown { SW_SHOW } else { SW_HIDE });
            }
        }
    }
    #[cfg(not(windows))]
    let _ = (window, shown);
}

impl AppView {
    /// Add or remove the tray icon to match the setting.
    pub(super) fn apply_tray(&mut self) {
        if !self.settings.tray {
            self.tray = None;
            return;
        }
        if self.tray.is_none() {
            match Tray::new(self.sender.clone()) {
                Ok(tray) => self.tray = Some(tray),
                Err(error) => {
                    needle_core::logfile::error(format!("Tray icon: {error:#}"));
                    // Saved off, so the next start does not fail the same way again.
                    self.settings.tray = false;
                    self.persist_settings();
                    self.fail(format!("Needle could not add its tray icon: {error:#}"));
                }
            }
        }
    }

    /// Hide the window; the music plays on and the tray icon brings it back.
    pub(super) fn hide_to_tray(&mut self, window: &mut Window) {
        if self.tray.is_none() {
            return;
        }
        self.hidden = true;
        set_window_shown(window, false);
    }

    pub(super) fn show_from_tray(&mut self, window: &mut Window) {
        self.hidden = false;
        set_window_shown(window, true);
        window.activate_window();
    }

    pub(super) fn tray_action(
        &mut self,
        action: TrayAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        use needle_core::audio::Command;
        match action {
            TrayAction::Show => self.show_from_tray(window),
            TrayAction::Toggle => self.player.send(Command::Toggle),
            TrayAction::Next => self.player.send(Command::Next),
            TrayAction::Previous => self.player.send(Command::Previous),
            TrayAction::Quit => {
                self.tray = None;
                cx.quit();
            }
        }
    }
}
