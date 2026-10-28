//! Settings › Keyboard › Shortcuts everywhere: a row for each action with its keys (or "Not
//! set"); click the keys and press new ones ("Press keys…"; Esc stops, Backspace clears). A
//! row whose keys another app holds says so.
use super::{
    AppView,
    hotkeys::{ACTIONS, Chord},
    pal,
    theme::MONO_FONT,
    widgets::meta,
};
use gpui::{prelude::*, *};
use needle_core::audio::Command;

impl AppView {
    /// What a key everywhere does.
    pub(super) fn run_hotkey(&mut self, action: &str, window: &mut Window, cx: &mut Context<Self>) {
        match action {
            "play" => self.toggle_playback(cx),
            "next" => self.player.send(Command::Next),
            "previous" => self.player.send(Command::Previous),
            "favorite" => self.toggle_favorite_playing(),
            "mini" => self.toggle_float(window, cx),
            "volume_up" | "volume_down" => {
                let step = if action == "volume_up" { 0.05 } else { -0.05 };
                let volume = (self.playback.volume + step).clamp(0., 1.);
                self.playback.volume = volume;
                self.player.send(Command::Volume(volume));
            }
            _ => {}
        }
        cx.notify();
    }

    /// The keys pressed while a row waits for them.
    pub(super) fn record_hotkey(&mut self, keystroke: &Keystroke, cx: &mut Context<Self>) {
        let Some(action) = self.recording_keys.clone() else {
            return;
        };
        let m = &keystroke.modifiers;
        let key = keystroke.key.as_str();
        if ["control", "alt", "shift", "platform", "function"].contains(&key) {
            return;
        }
        let plain = !(m.control || m.alt || m.shift || m.platform);
        match key {
            "escape" if plain => self.recording_keys = None,
            "backspace" | "delete" if plain => {
                self.recording_keys = None;
                self.settings.global_shortcuts.retain(|(a, _)| *a != action);
                self.keys_changed();
            }
            _ => {
                let chord = Chord {
                    ctrl: m.control,
                    alt: m.alt,
                    shift: m.shift,
                    win: m.platform,
                    key: key.to_lowercase(),
                };
                match Chord::parse(&chord.text()) {
                    Some(chord) => {
                        let text = chord.text();
                        // One action per keys: the keys leave any other action.
                        self.settings
                            .global_shortcuts
                            .retain(|(a, k)| *a != action && *k != text);
                        self.settings.global_shortcuts.push((action, text));
                        self.recording_keys = None;
                        self.keys_changed();
                    }
                    None => self.notify("Use Ctrl, Alt, or the Windows key with it, or an F key."),
                }
            }
        }
        cx.notify();
    }

    /// Hold the keys now chosen, and keep them.
    pub(super) fn keys_changed(&mut self) {
        if let Some(keys) = &self.hotkeys {
            keys.set(&self.settings.global_shortcuts);
        }
        self.persist_settings();
    }

    /// The group, for the Keyboard settings page.
    pub(super) fn global_shortcut_rows(&self, cx: &mut Context<Self>) -> Div {
        let p = pal(cx);
        let status = self
            .hotkeys
            .as_ref()
            .map(|k| k.status())
            .unwrap_or_default();
        if !status.supported {
            return div().pt_2().child(meta(
                if cfg!(target_os = "linux") {
                    "On Wayland, choose keys in your desktop's keyboard settings: Needle answers the media keys and playerctl."
                } else {
                    "This system lets no app take keys for itself."
                },
                cx,
            ));
        }
        let cap = |text: String, accent: bool| {
            div()
                .px(px(8.))
                .py(px(2.))
                .rounded(px(5.))
                .border_1()
                .border_color(if accent { p.accent } else { p.line })
                .text_color(if accent { p.accent } else { p.ink })
                .font_family(MONO_FONT)
                .text_size(px(11.5))
                .child(text)
        };
        let rows = ACTIONS.iter().enumerate().map(|(i, (action, name))| {
            let action = action.to_string();
            let recording = self.recording_keys.as_deref() == Some(action.as_str());
            let keys = self
                .settings
                .global_shortcuts
                .iter()
                .find(|(a, _)| *a == action)
                .and_then(|(_, k)| Chord::parse(k));
            let taken = keys.is_some() && status.taken.contains(&action);
            let shown: AnyElement = if recording {
                cap("Press keys…".into(), true).into_any_element()
            } else if let Some(chord) = &keys {
                div()
                    .flex()
                    .gap(px(6.))
                    .children(chord.caps().into_iter().map(|c| cap(c, false)))
                    .into_any_element()
            } else {
                div()
                    .text_size(px(12.5))
                    .text_color(p.ink_3)
                    .child("Not set")
                    .into_any_element()
            };
            div()
                .id(("global-key", i))
                .py(px(10.))
                .when(i > 0, |el| el.border_t_1().border_color(p.line_soft))
                .flex()
                .items_center()
                .gap_3()
                .cursor_pointer()
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .child(div().text_size(px(13.)).child(name.to_string()))
                        .when(taken, |el| {
                            el.child(
                                div()
                                    .text_size(px(11.5))
                                    .text_color(p.danger)
                                    .child("Another app uses these keys. Press other keys."),
                            )
                        }),
                )
                .child(shown)
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.recording_keys = if this.recording_keys.as_deref() == Some(action.as_str())
                    {
                        None
                    } else {
                        Some(action.clone())
                    };
                    cx.notify();
                }))
        });
        div().pt_1().flex().flex_col().children(rows)
    }
}

impl AppView {
    /// The taskbar's buttons follow Play/Pause and Favorite.
    #[cfg(windows)]
    pub(super) fn update_taskbar(&mut self) {
        let favorite = self.playback.current.as_ref().is_some_and(|c| {
            self.tracks
                .iter()
                .find(|t| t.id == c.track.id)
                .map_or(c.track.rating, |t| t.rating)
                >= 4
        });
        let playing = self.playback.playing;
        if let Some(taskbar) = &mut self.taskbar {
            taskbar.update(playing, favorite);
        }
    }
}
