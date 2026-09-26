//! System media controls, through `souvlaki`: on Windows, media keys, the lock screen, and the
//! volume flyout (System Media Transport Controls); on Linux, MPRIS, which desktops, media
//! keys, and tools like playerctl use; on macOS, media keys and Now Playing in Control Center.
use super::{AppView, Event};
use gpui::Window;
use needle_core::audio::Command;
use std::time::{Duration, Instant};

/// A button pressed in Windows' media controls.
#[derive(Clone, Copy, Debug)]
pub enum Key {
    Play,
    Pause,
    Toggle,
    Next,
    Previous,
    Stop,
    Seek(f64),
    SeekTo(f64),
}

pub struct MediaKeys {
    #[cfg(any(windows, target_os = "linux", target_os = "macos"))]
    controls: souvlaki::MediaControls,
    /// Song id, playing, and when the position was last told.
    shown: Option<(String, bool, Instant)>,
    /// The Needle logo, shown for songs without a cover.
    logo: Option<String>,
}

impl MediaKeys {
    /// Connect the main window to the system's media controls.
    #[cfg(any(windows, target_os = "linux", target_os = "macos"))]
    pub fn new(
        window: &Window,
        sender: crossbeam_channel::Sender<Event>,
        data: &std::path::Path,
    ) -> Option<Self> {
        use souvlaki::{MediaControlEvent as E, SeekDirection as D};
        #[cfg(windows)]
        let hwnd = {
            use raw_window_handle::{HasWindowHandle, RawWindowHandle};
            let handle = HasWindowHandle::window_handle(window).ok()?;
            let RawWindowHandle::Win32(win32) = handle.as_raw() else {
                return None;
            };
            Some(win32.hwnd.get() as *mut std::ffi::c_void)
        };
        #[cfg(not(windows))]
        let hwnd = {
            let _ = window;
            None
        };
        let mut controls = souvlaki::MediaControls::new(souvlaki::PlatformConfig {
            display_name: "Needle",
            dbus_name: "needle",
            hwnd,
        })
        .ok()?;
        controls
            .attach(move |event| {
                let key = match event {
                    E::Play => Key::Play,
                    E::Pause => Key::Pause,
                    E::Toggle => Key::Toggle,
                    E::Next => Key::Next,
                    E::Previous => Key::Previous,
                    E::Stop => Key::Stop,
                    E::Seek(D::Forward) => Key::Seek(10.),
                    E::Seek(D::Backward) => Key::Seek(-10.),
                    E::SeekBy(D::Forward, by) => Key::Seek(by.as_secs_f64()),
                    E::SeekBy(D::Backward, by) => Key::Seek(-by.as_secs_f64()),
                    E::SetPosition(p) => Key::SeekTo(p.0.as_secs_f64()),
                    _ => return,
                };
                let _ = sender.send(Event::MediaKey(key));
            })
            .ok()?;
        // Keep a copy of the logo as a PNG file for songs without a cover.
        let logo = data.join("artwork").join("needle-logo.png");
        if !logo.exists() {
            let _ = std::fs::write(&logo, include_bytes!("../../assets/needle-1024.png"));
        }
        Some(Self {
            controls,
            shown: None,
            logo: logo.to_str().map(str::to_string),
        })
    }

    #[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
    pub fn new(
        _: &Window,
        _: crossbeam_channel::Sender<Event>,
        _: &std::path::Path,
    ) -> Option<Self> {
        None
    }
}

impl AppView {
    /// Tell Windows what is playing: on a new song, on play/pause, and every few seconds so
    /// the position stays right.
    pub(super) fn update_media_keys(&mut self) {
        #[cfg(any(windows, target_os = "linux", target_os = "macos"))]
        {
            use souvlaki::{MediaMetadata, MediaPlayback, MediaPosition};
            let Some(keys) = self.media_keys.as_mut() else {
                return;
            };
            let playing = self.playback.playing;
            let position = MediaPosition(Duration::from_secs_f64(self.playback.position.max(0.)));
            let Some(item) = self.playback.current.as_ref() else {
                if keys.shown.take().is_some() {
                    let _ = keys.controls.set_playback(MediaPlayback::Stopped);
                }
                return;
            };
            let track = &item.track;
            let new_song = keys.shown.as_ref().is_none_or(|(id, _, _)| *id != track.id);
            if new_song {
                let artist = track.display_artist().to_string();
                let base = MediaMetadata {
                    title: Some(&track.title),
                    artist: Some(&artist),
                    album: Some(&track.album),
                    duration: (track.duration > 0.)
                        .then(|| Duration::from_secs_f64(track.duration)),
                    cover_url: None,
                };
                // Text first, so it shows even if the picture cannot be read.
                let _ = keys.controls.set_metadata(base.clone());
                let cover = track
                    .artwork
                    .as_deref()
                    .map(str::to_string)
                    .or_else(|| keys.logo.clone())
                    // Windows needs a full path without the long-path prefix.
                    .and_then(|p| std::path::absolute(p).ok())
                    .map(|p| {
                        p.to_string_lossy()
                            .trim_start_matches("\\\\?\\")
                            .to_string()
                    });
                if let Some(cover) = cover {
                    let url = format!("file://{cover}");
                    let _ = keys.controls.set_metadata(MediaMetadata {
                        cover_url: Some(&url),
                        ..base
                    });
                }
            }
            let changed = keys.shown.as_ref().is_none_or(|(_, was, at)| {
                *was != playing || at.elapsed() > Duration::from_secs(5)
            });
            if new_song || changed {
                let _ = keys.controls.set_playback(if playing {
                    MediaPlayback::Playing {
                        progress: Some(position),
                    }
                } else {
                    MediaPlayback::Paused {
                        progress: Some(position),
                    }
                });
                keys.shown = Some((track.id.clone(), playing, Instant::now()));
            }
        }
    }

    /// Act on a button pressed in Windows' media controls.
    pub(super) fn media_key(
        &mut self,
        key: Key,
        window: &mut Window,
        cx: &mut gpui::Context<Self>,
    ) {
        let _ = window;
        let playing = self.playback.playing;
        match key {
            Key::Toggle => self.toggle_playback(cx),
            Key::Play if !playing => self.toggle_playback(cx),
            Key::Pause | Key::Stop if playing => self.toggle_playback(cx),
            Key::Next => self.player.send(Command::Next),
            Key::Previous => self.player.send(Command::Previous),
            Key::Seek(by) => self
                .player
                .send(Command::Seek((self.playback.position + by).max(0.))),
            Key::SeekTo(at) => self.player.send(Command::Seek(at.max(0.))),
            _ => {}
        }
        // Tell Windows the new state on the next poll, without reloading the song's details.
        if let Some((_, _, at)) = self.media_keys.as_mut().and_then(|k| k.shown.as_mut()) {
            *at = Instant::now()
                .checked_sub(Duration::from_secs(10))
                .unwrap_or(*at);
        }
    }
}
