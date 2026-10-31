//! Two of the player's tools, each a small pane over the player: the sleep timer (a moon,
//! lit while it runs, with the time left on hover) and speed and pitch. Both are the core's
//! (`audio::Command::Sleep`, `Command::Tempo`); this is only their look.
use super::{
    AppView, motion, pal,
    theme::MONO_FONT,
    widgets::{glyph, icon_button},
};
use gpui::{prelude::*, *};
use gpui_component::{ActiveTheme, Disableable, Sizable, button::ButtonVariants, switch::Switch};
use needle_core::{
    audio::{Command, SleepKind, SleepTimer},
    tempo::{MAX_SEMITONES, MAX_SPEED, MIN_SPEED, SPEED_PRESETS, Tempo},
};

/// Which tool's pane is open, where, and its number (for the opening motion).
#[derive(Clone, Copy, PartialEq)]
pub enum Tool {
    Sleep,
    Speed,
}

pub struct ToolPane {
    pub tool: Tool,
    pub position: Point<Pixels>,
    pub serial: usize,
}

/// The sleep timer's chips: their names and what they wait for.
const SLEEP_CHOICES: [(&str, SleepKind); 6] = [
    ("15 min", SleepKind::After(900.)),
    ("30 min", SleepKind::After(1800.)),
    ("45 min", SleepKind::After(2700.)),
    ("1 hour", SleepKind::After(3600.)),
    ("End of song", SleepKind::EndOfSong),
    ("End of album", SleepKind::EndOfAlbum),
];

/// "24:12", or "1:02:05".
pub(super) fn clock(seconds: f64) -> String {
    let s = seconds.max(0.).ceil() as u64;
    if s >= 3600 {
        format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
    } else {
        format!("{}:{:02}", s / 60, s % 60)
    }
}

/// "1.25×", "1×".
fn speed_name(speed: f32) -> String {
    let text = format!("{speed:.2}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    format!("{text}×")
}

/// "+2 semitones", "−1 semitone", "+0 semitones".
fn pitch_name(semitones: f32) -> String {
    let value = (semitones * 10.).round() / 10.;
    let sign = if value < 0. { "−" } else { "+" };
    let number = format!("{:.1}", value.abs());
    let number = number.trim_end_matches(".0");
    let unit = if value.abs() == 1. {
        "semitone"
    } else {
        "semitones"
    };
    format!("{sign}{number} {unit}")
}

/// The name of the system's own sleep, for "Then sleep the PC".
fn computer_name() -> &'static str {
    if cfg!(windows) { "PC" } else { "computer" }
}

/// Put the computer to sleep, as the Start menu's Sleep does.
pub fn sleep_computer() {
    #[cfg(windows)]
    unsafe {
        // Sleep, not hibernate; wake events stay allowed.
        windows_sys::Win32::System::Power::SetSuspendState(0, 0, 0);
    }
    #[cfg(target_os = "linux")]
    {
        let _ = std::process::Command::new("systemctl")
            .arg("suspend")
            .spawn();
    }
}

impl AppView {
    /// The moon: lit while the sleep timer runs, with the time left on hover.
    pub(super) fn sleep_button(&self, cx: &mut Context<Self>) -> AnyElement {
        let p = pal(cx);
        let sleep = self.playback.sleep;
        let tip = match sleep {
            Some(s) => format!("Sleep timer · {} left", clock(s.remaining)),
            None => "Sleep timer".into(),
        };
        div()
            .id("sleep-timer")
            .size(px(28.))
            .rounded(px(8.))
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .when(sleep.is_some(), |el| el.bg(p.accent.opacity(0.18)))
            .hover(|s| s.bg(p.raised_hover))
            .child(
                glyph(if sleep.is_some() { "moon-fill" } else { "moon" })
                    .size(px(16.))
                    .text_color(if sleep.is_some() { p.accent } else { p.ink_2 }),
            )
            .tooltip(move |window, cx| {
                gpui_component::tooltip::Tooltip::new(tip.clone()).build(window, cx)
            })
            .on_click(cx.listener(|this, _, window, cx| {
                let position = window.mouse_position() - point(px(0.), px(16.));
                this.open_tool(Tool::Sleep, position, cx);
            }))
            .into_any_element()
    }

    /// Speed and pitch: lit when a song plays faster, slower, higher, or lower.
    pub(super) fn speed_button(&self, cx: &mut Context<Self>) -> AnyElement {
        let p = pal(cx);
        let tempo = self.playback.tempo;
        let changed = !tempo.is_neutral() && self.playback.tempo_available;
        icon_button(
            "speed-pitch",
            "speed",
            if changed {
                format!("Speed and pitch · {}", speed_name(tempo.speed))
            } else {
                "Speed and pitch".into()
            },
        )
        .with_size(px(super::chrome::BAR_ICON))
        .when(changed, |b| b.text_color(p.accent))
        .on_click(cx.listener(|this, _, window, cx| {
            let position = window.mouse_position() - point(px(0.), px(16.));
            this.open_tool(Tool::Speed, position, cx);
        }))
        .into_any_element()
    }

    pub(super) fn open_tool(
        &mut self,
        tool: Tool,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        if self.tool_pane.as_ref().is_some_and(|t| t.tool == tool) {
            self.tool_pane = None;
        } else {
            self.menu_serial += 1;
            self.tool_pane = Some(ToolPane {
                tool,
                position,
                serial: self.menu_serial,
            });
        }
        cx.notify();
    }

    /// The sleep timer ended: put the computer to sleep, when asked to.
    pub(super) fn watch_sleep_timer(&mut self) {
        if self.playback.sleep_ended > self.sleep_ended_seen {
            self.sleep_ended_seen = self.playback.sleep_ended;
            if self.settings.sleep_computer {
                sleep_computer();
            }
        }
    }

    fn tool_chip(
        &self,
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        on: bool,
        cx: &App,
    ) -> Stateful<Div> {
        let p = pal(cx);
        div()
            .id(id)
            .px(px(12.))
            .py(px(5.))
            .rounded_full()
            .text_size(px(13.))
            .cursor_pointer()
            .map(|el| {
                if on {
                    el.bg(p.accent).text_color(p.accent_ink)
                } else {
                    el.bg(p.raised_hover)
                        .text_color(p.ink)
                        .hover(|s| s.bg(p.line))
                }
            })
            .child(label.into())
    }

    fn pane_row(
        &self,
        title: &str,
        line: Option<&str>,
        control: impl IntoElement,
        cx: &App,
    ) -> Div {
        let p = pal(cx);
        div()
            .flex()
            .items_center()
            .gap_3()
            .py(px(9.))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .child(
                        div()
                            .text_size(px(13.))
                            .text_color(p.ink)
                            .child(title.to_string()),
                    )
                    .children(line.map(|l| {
                        div()
                            .text_size(px(11.5))
                            .text_color(p.ink_3)
                            .child(l.to_string())
                    })),
            )
            .child(control)
    }

    fn sleep_pane(&self, cx: &mut Context<Self>) -> Div {
        let p = pal(cx);
        let sleep = self.playback.sleep;
        let fade = sleep.map_or(self.settings.sleep_fade, |s| s.fade);
        let chips = SLEEP_CHOICES
            .iter()
            .enumerate()
            .map(|(i, (label, kind))| {
                let kind = *kind;
                let on = sleep.is_some_and(|s| match (s.kind, kind) {
                    (SleepKind::After(a), SleepKind::After(b)) => (a - b).abs() < 1.,
                    (a, b) => a == b,
                });
                self.tool_chip(("sleep-choice", i), *label, on, cx)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        // The chip that runs turns the timer off.
                        if on {
                            this.player.send(Command::Sleep(None));
                        } else {
                            let fade = this
                                .playback
                                .sleep
                                .map_or(this.settings.sleep_fade, |s| s.fade);
                            this.player
                                .send(Command::Sleep(Some(SleepTimer { kind, fade })));
                        }
                        cx.notify();
                    }))
            })
            .collect::<Vec<_>>();
        div()
            .flex()
            .flex_col()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .mb(px(10.))
                    .child(glyph("moon-fill").size(px(18.)).text_color(p.accent))
                    .child(
                        div()
                            .text_size(px(15.))
                            .font_weight(FontWeight::BOLD)
                            .text_color(p.ink)
                            .child("Sleep timer"),
                    )
                    .child(div().flex_1())
                    .children(sleep.map(|s| {
                        div()
                            .font_family(MONO_FONT)
                            .text_size(px(13.))
                            .text_color(p.accent)
                            .child(clock(s.remaining))
                    })),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(px(6.))
                    .pb(px(10.))
                    .children(chips),
            )
            .child(div().h(px(1.)).bg(p.line_soft))
            .child(
                self.pane_row(
                    "Fade out",
                    Some("Over the last minute"),
                    Switch::new("sleep-fade")
                        .checked(fade)
                        .on_click(cx.listener(|this, checked: &bool, _, cx| {
                            this.settings.sleep_fade = *checked;
                            this.player.send(Command::SleepFade(*checked));
                            this.persist_settings();
                            cx.notify();
                        })),
                    cx,
                ),
            )
            .child(div().h(px(1.)).bg(p.line_soft))
            .child(
                self.pane_row(
                    &format!("Then sleep the {}", computer_name()),
                    None,
                    Switch::new("sleep-computer")
                        .checked(self.settings.sleep_computer)
                        .on_click(cx.listener(|this, checked: &bool, _, cx| {
                            this.settings.sleep_computer = *checked;
                            this.persist_settings();
                            cx.notify();
                        })),
                    cx,
                ),
            )
    }

    /// Send a new speed and pitch, and keep it in this window's settings too (they are saved
    /// whole), unless it is for this song only.
    fn set_tempo(&mut self, tempo: Tempo, song_only: bool, cx: &mut Context<Self>) {
        let tempo = tempo.clamped();
        self.player.send(Command::Tempo(tempo, song_only));
        // Shown at once, before the player says so.
        self.playback.tempo = tempo;
        self.playback.tempo_song_only = song_only;
        if !song_only {
            self.settings.speed = tempo.speed;
            self.settings.pitch = tempo.semitones;
            self.settings.keep_pitch = tempo.keep_pitch;
        }
        cx.notify();
    }

    fn stepper(
        &self,
        id: &'static str,
        value: String,
        down: impl Fn(&mut Self, &mut Context<Self>) + 'static,
        up: impl Fn(&mut Self, &mut Context<Self>) + 'static,
        enabled: bool,
        cx: &mut Context<Self>,
    ) -> Div {
        let p = pal(cx);
        div()
            .flex()
            .items_center()
            .gap_1()
            .child(
                icon_button((id, 0usize), "minus", "Less")
                    .xsmall()
                    .disabled(!enabled)
                    .on_click(cx.listener(move |this, _, _, cx| down(this, cx))),
            )
            .child(
                div()
                    .min_w(px(96.))
                    .text_center()
                    .text_size(px(13.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(if enabled { p.accent } else { p.ink_3 })
                    .child(value),
            )
            .child(
                icon_button((id, 1usize), "plus", "More")
                    .xsmall()
                    .disabled(!enabled)
                    .on_click(cx.listener(move |this, _, _, cx| up(this, cx))),
            )
    }

    fn speed_pane(&self, cx: &mut Context<Self>) -> Div {
        let p = pal(cx);
        let tempo = self.playback.tempo;
        let song_only = self.playback.tempo_song_only;
        let available = self.playback.tempo_available;
        let chips = SPEED_PRESETS
            .iter()
            .enumerate()
            .map(|(i, speed)| {
                let speed = *speed;
                self.tool_chip(
                    ("speed-choice", i),
                    speed_name(speed),
                    available && (tempo.speed - speed).abs() < 0.005,
                    cx,
                )
                .when(available, |el| {
                    el.on_click(cx.listener(move |this, _, _, cx| {
                        let t = this.playback.tempo;
                        let only = this.playback.tempo_song_only;
                        this.set_tempo(Tempo { speed, ..t }, only, cx);
                    }))
                })
            })
            .collect::<Vec<_>>();
        let step = |delta: f32| {
            move |this: &mut Self, cx: &mut Context<Self>| {
                let t = this.playback.tempo;
                let speed = ((t.speed + delta) * 20.).round() / 20.;
                let only = this.playback.tempo_song_only;
                this.set_tempo(
                    Tempo {
                        speed: speed.clamp(MIN_SPEED, MAX_SPEED),
                        ..t
                    },
                    only,
                    cx,
                );
            }
        };
        let shift = |delta: f32| {
            move |this: &mut Self, cx: &mut Context<Self>| {
                let t = this.playback.tempo;
                let only = this.playback.tempo_song_only;
                this.set_tempo(
                    Tempo {
                        semitones: (t.semitones + delta).clamp(-MAX_SEMITONES, MAX_SEMITONES),
                        ..t
                    },
                    only,
                    cx,
                );
            }
        };
        div()
            .flex()
            .flex_col()
            .child(
                div()
                    .text_size(px(15.))
                    .font_weight(FontWeight::BOLD)
                    .text_color(p.ink)
                    .mb(px(8.))
                    .child("Speed and pitch"),
            )
            .when(!available, |el| {
                el.child(
                    div()
                        .text_size(px(12.))
                        .text_color(p.ink_3)
                        .mb(px(6.))
                        .child("Off while exclusive output is on, so the sound stays untouched."),
                )
            })
            .child(self.pane_row(
                "Speed",
                None,
                self.stepper(
                    "speed-step",
                    speed_name(tempo.speed),
                    step(-0.05),
                    step(0.05),
                    available,
                    cx,
                ),
                cx,
            ))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(px(6.))
                    .pb(px(8.))
                    .children(chips),
            )
            .child(self.pane_row(
                "Pitch",
                None,
                self.stepper(
                    "pitch-step",
                    pitch_name(tempo.semitones),
                    shift(-1.),
                    shift(1.),
                    available,
                    cx,
                ),
                cx,
            ))
            .child(div().h(px(1.)).bg(p.line_soft))
            .child(
                self.pane_row(
                    "Keep the pitch",
                    Some("Faster without the chipmunk voice"),
                    Switch::new("keep-pitch")
                        .checked(tempo.keep_pitch)
                        .disabled(!available)
                        .on_click(cx.listener(|this, checked: &bool, _, cx| {
                            let t = this.playback.tempo;
                            let only = this.playback.tempo_song_only;
                            this.set_tempo(
                                Tempo {
                                    keep_pitch: *checked,
                                    ..t
                                },
                                only,
                                cx,
                            );
                        })),
                    cx,
                ),
            )
            .child(
                self.pane_row(
                    "Only for this song",
                    None,
                    Switch::new("tempo-song-only")
                        .checked(song_only)
                        .disabled(!available)
                        .on_click(cx.listener(|this, checked: &bool, _, cx| {
                            let t = this.playback.tempo;
                            this.set_tempo(t, *checked, cx);
                        })),
                    cx,
                ),
            )
            .when(available && !tempo.is_neutral(), |el| {
                el.child(
                    div().pt(px(4.)).child(
                        super::widgets::small_button("tempo-reset", "As recorded")
                            .ghost()
                            .on_click(cx.listener(|this, _, _, cx| {
                                let only = this.playback.tempo_song_only;
                                this.set_tempo(Tempo::default(), only, cx);
                            })),
                    ),
                )
            })
    }

    /// The open tool's pane, over the player.
    pub(super) fn tool_pane_view(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        let pane = self.tool_pane.as_ref()?;
        let content = match pane.tool {
            Tool::Sleep => self.sleep_pane(cx),
            Tool::Speed => self.speed_pane(cx),
        };
        let body = div()
            .id("tool-pane")
            .occlude()
            .w(px(290.))
            .p(px(16.))
            .map(|el| super::glass::pane(el, 16., 22., cx.theme().popover, cx))
            .border_1()
            .border_color(pal(cx).line)
            .shadow_lg()
            .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                this.tool_pane = None;
                cx.notify();
            }))
            .child(content);
        let body = motion::animate(body, ("tool-pane-in", pane.serial), 140, cx, |el, t| {
            el.opacity(t).mt(px(-6. * (1. - t)))
        });
        Some(
            deferred(
                anchored()
                    .position(pane.position)
                    .anchor(Corner::BottomRight)
                    .snap_to_window_with_margin(px(8.))
                    .child(body),
            )
            .with_priority(2),
        )
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn names_read_plainly() {
        assert_eq!(super::clock(1452.), "24:12");
        assert_eq!(super::clock(3725.), "1:02:05");
        assert_eq!(super::speed_name(1.25), "1.25×");
        assert_eq!(super::speed_name(1.), "1×");
        assert_eq!(super::speed_name(0.5), "0.5×");
        assert_eq!(super::pitch_name(0.), "+0 semitones");
        assert_eq!(super::pitch_name(-1.), "−1 semitone");
        assert_eq!(super::pitch_name(2.5), "+2.5 semitones");
    }
}
