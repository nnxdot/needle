use super::{
    AppView, Event, pal,
    widgets::{faint, glyph, meta, small_button, strong},
};
use gpui::{prelude::*, *};
use gpui_component::{
    Disableable,
    button::ButtonVariants,
    slider::{Slider, SliderEvent, SliderState},
    switch::Switch,
};
use needle_core::{
    audio::Command,
    model::Track,
    stems::{self, STEMS},
};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

pub struct StemsState {
    pub sliders: Vec<Entity<SliderState>>,
    pub job: Option<(String, String, f32)>,
    pub cancel: Arc<AtomicBool>,
    _subscriptions: Vec<Subscription>,
}

impl StemsState {
    pub fn new(cx: &mut Context<AppView>) -> Self {
        let sliders: Vec<_> = STEMS
            .iter()
            .map(|_| {
                cx.new(|_| {
                    SliderState::new()
                        .min(0.)
                        .max(1.5)
                        .step(0.01)
                        .default_value(1.)
                })
            })
            .collect();
        let subscriptions = sliders
            .iter()
            .enumerate()
            .map(|(i, slider)| {
                cx.subscribe(
                    slider,
                    move |this: &mut AppView, _, event: &SliderEvent, _| {
                        let SliderEvent::Change(value) = event;
                        this.player.stem_mix().set(i, value.start());
                    },
                )
            })
            .collect();
        Self {
            sliders,
            job: None,
            cancel: Arc::new(AtomicBool::new(false)),
            _subscriptions: subscriptions,
        }
    }
}

/// Ready-made mixes: name, icon, and levels for drums, bass, other, vocals.
const PRESETS: [(&str, &str, [f32; 4]); 5] = [
    ("Full mix", "stems", [1., 1., 1., 1.]),
    ("Karaoke", "lyrics", [1., 1., 1., 0.]),
    ("Vocals only", "artists", [0., 0., 0., 1.]),
    ("No drums", "drum", [0., 1., 1., 1.]),
    ("Bass only", "wave", [0., 1., 0., 0.]),
];

/// Each stem's name, icon, and hue (so its card and meter are recognisable at a glance).
fn stem_style(name: &str) -> (&'static str, &'static str, f32) {
    match name {
        "drums" => ("Drums", "drum", 0.01),
        "bass" => ("Bass", "wave", 0.74),
        "vocals" => ("Vocals", "artists", 0.12),
        _ => ("Other", "smart", 0.47),
    }
}

impl AppView {
    fn set_stem_levels(&mut self, levels: [f32; 4], window: &mut Window, cx: &mut Context<Self>) {
        for (i, level) in levels.iter().enumerate() {
            self.player.stem_mix().set(i, *level);
            self.stems.sliders[i].update(cx, |s, cx| s.set_value(*level, window, cx));
        }
        cx.notify();
    }

    /// Download the model if needed, then split `track`. Runs in the background.
    pub(super) fn separate(&mut self, track: Track, cx: &mut Context<Self>) {
        if track.is_streamed() {
            return self
                .fail("Songs on a music server cannot be split. Save it to your music first.");
        }
        if self.stems.job.is_some() {
            return self.fail("Another song is being split. Wait for it, or stop it first.");
        }
        let cancel = Arc::new(AtomicBool::new(false));
        self.stems.cancel = cancel.clone();
        self.stems.job = Some((track.id.clone(), "Starting…".into(), 0.));
        let library = self.library.clone();
        let sender = self.sender.clone();
        std::thread::spawn(move || {
            let id = track.id.clone();
            let progress = |stage: String, fraction: f32| {
                let _ = sender.send(Event::StemsProgress(id.clone(), stage, fraction));
            };
            let result = (|| -> anyhow::Result<()> {
                if !stems::model_ready(&library) {
                    stems::download_model(&library, &cancel, |done, total| {
                        progress(
                            format!(
                                "Downloading the stem model · {} of {} MB",
                                done / 1_048_576,
                                total.max(1) / 1_048_576
                            ),
                            if total > 0 {
                                done as f32 / total as f32
                            } else {
                                0.
                            },
                        )
                    })?;
                }
                stems::separate(&library, &track, &cancel, |fraction| {
                    progress("Splitting into stems".into(), fraction)
                })?;
                Ok(())
            })();
            let _ = sender.send(Event::StemsDone(
                track.id.clone(),
                result.map_err(|e| format!("{e:#}")),
            ));
        });
        cx.notify();
    }

    pub(super) fn stems_view(
        &self,
        track: Option<Track>,
        big: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let p = pal(cx);
        let Some(track) = track else {
            return div()
                .child(meta("Play or select a song to split it into stems.", cx))
                .into_any_element();
        };
        let ready = stems::stems_for(&self.library, &track.id).is_some();
        let active = self.playback.stems.as_deref() == Some(track.id.as_str());
        let playing_this = self
            .playback
            .current
            .as_ref()
            .is_some_and(|c| c.track.id == track.id);
        let job = self.stems.job.clone().filter(|(id, _, _)| *id == track.id);
        let busy_elsewhere = self.stems.job.is_some() && job.is_none();
        let exclusive = self.settings.exclusive;
        let channels: Vec<AnyElement> = if ready {
            STEMS
                .iter()
                .enumerate()
                .map(|(i, name)| {
                    self.stem_channel(i, name, active, big, cx)
                        .into_any_element()
                })
                .collect()
        } else {
            vec![]
        };
        let presets = ready.then(|| self.stem_presets(cx).into_any_element());
        div()
            .flex()
            .flex_col()
            .gap_3()
            .when(big, |el| el.child(strong(format!("Stems · {}", track.title))))
            .when(!ready && job.is_none(), |el| {
                el.child(meta(
                    if stems::model_ready(&self.library) {
                        "Split this song into drums, bass, vocals, and everything else, right on this computer. It takes about half the song's length."
                    } else {
                        "Split this song into drums, bass, vocals, and everything else, right on this computer. The first time, Needle downloads the 166 MB separation model."
                    },
                    cx,
                ).w_full())
                .child(div().child(
                    small_button("stems-split", "Split into stems").icon(super::widgets::icon("stems")).primary().disabled(busy_elsewhere).on_click({
                        let track = track.clone();
                        cx.listener(move |this, _, _, cx| this.separate(track.clone(), cx))
                    }),
                ))
            })
            .when_some(job, |el, (_, stage, fraction)| {
                let cancel = self.stems.cancel.clone();
                el.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(meta(stage, cx))
                        .child(div().h(px(4.)).rounded_full().bg(p.line).child(div().h_full().rounded_full().bg(p.accent).w(relative(fraction.clamp(0., 1.)))))
                        .child(div().child(small_button("stems-stop", "Stop").ghost().on_click(move |_, _, _| cancel.store(true, Ordering::Relaxed)))),
                )
            })
            .when(ready, |el| {
                let id = track.id.clone();
                el.child(
                    div()
                        .flex()
                        .items_center()
                        .gap_3()
                        .child(div().flex_1().child(strong("Play from stems")).child(faint(if exclusive { "Unavailable with exclusive output" } else if playing_this { "Changes apply as you listen" } else { "Starts this song from its stems" }, cx)))
                        .child(Switch::new("stems-on").checked(active).disabled(exclusive).on_click({
                            let track = track.clone();
                            cx.listener(move |this, on: &bool, _, cx| {
                                let dir = stems::stems_for(&this.library, &track.id);
                                if *on && !this.playback.current.as_ref().is_some_and(|c| c.track.id == track.id) {
                                    this.play_tracks(vec![track.clone()], "Played from its stems");
                                }
                                this.player.send(Command::Stems(if *on { dir.map(|d| (track.id.clone(), d)) } else { None }));
                                cx.notify();
                            })
                        })),
                )
                .children(channels)
                .children(presets)
                .child(div().child(small_button("stems-delete", "Delete these stems").ghost().on_click(cx.listener(move |this, _, _, cx| {
                    if this.playback.stems.as_deref() == Some(id.as_str()) {
                        this.player.send(Command::Stems(None));
                    }
                    match stems::delete_stems(&this.library, &id) {
                        Ok(()) => this.notify("Stems deleted. The song itself is untouched."),
                        Err(e) => this.fail(format!("{e:#}")),
                    }
                    cx.notify();
                }))))
            })
            .when(big && !ready, |el| el.child(div().flex().items_center().gap_2().child(glyph("info").size(px(14.)).text_color(p.ink_3)).child(faint("Stems use HT-Demucs, running locally. Nothing is uploaded.", cx))))
            .into_any_element()
    }

    pub(super) fn stems_settings(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let model = stems::model_ready(&self.library);
        let used = stems::cache_size(&self.library) as f64 / 1_048_576.;
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(meta(
                format!(
                    "Model: {} · Split songs use {:.0} MB",
                    if model { "downloaded (166 MB)" } else { "not downloaded" },
                    used
                ),
                cx,
            ))
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(small_button("stems-clear", "Delete all split songs").ghost().disabled(used == 0.).on_click(cx.listener(|this, _, _, cx| {
                        this.player.send(Command::Stems(None));
                        let _ = std::fs::remove_dir_all(this.library.directory.join("stems"));
                        this.notify("All stems deleted.");
                        cx.notify();
                    })))
                    .child(small_button("stems-model-delete", "Delete the model").ghost().disabled(!model).on_click(cx.listener(|this, _, _, cx| {
                        let _ = std::fs::remove_file(stems::model_path(&this.library));
                        this.notify("Stem model deleted. It downloads again the next time you split a song.");
                        cx.notify();
                    }))),
            )
    }
}

impl AppView {
    /// One stem as a small channel card: coloured icon, name and level, a live meter while it
    /// plays, the level slider, and Solo / Mute.
    fn stem_channel(
        &self,
        i: usize,
        name: &str,
        active: bool,
        big: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let p = pal(cx);
        let (label, icon, hue) = stem_style(name);
        let color = hsla(hue, 0.72, if p.dark { 0.66 } else { 0.44 }, 1.);
        let levels: Vec<f32> = (0..STEMS.len())
            .map(|k| self.player.stem_mix().get(k))
            .collect();
        let level = levels[i];
        let muted = level <= 0.001;
        let soloed = level > 0.001
            && levels
                .iter()
                .enumerate()
                .all(|(k, l)| k == i || *l <= 0.001);
        let live = active && self.playback.playing && !muted;
        let pill = |id: &'static str, text: &'static str, tip: &'static str, on: bool| {
            div()
                .id((id, i))
                .size(px(22.))
                .flex_shrink_0()
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(6.))
                .text_size(px(11.))
                .font_weight(FontWeight::SEMIBOLD)
                .cursor_pointer()
                .when(on, |el| {
                    el.bg(color)
                        .text_color(if p.dark { gpui::black() } else { gpui::white() })
                })
                .when(!on, |el| {
                    el.text_color(p.ink_3)
                        .hover(|s| s.bg(p.ink.opacity(0.08)).text_color(p.ink))
                })
                .child(text)
                .tooltip(move |window, cx| {
                    gpui_component::tooltip::Tooltip::new(tip).build(window, cx)
                })
        };
        // One compact row per stem: coloured icon, name and level, slider, live meter, S and M.
        div()
            .h(px(if big { 46. } else { 42. }))
            .px_2()
            .rounded(px(10.))
            .bg(color.opacity(if live { 0.1 } else { 0.05 }))
            .border_1()
            .border_color(color.opacity(if live { 0.35 } else { 0.12 }))
            .flex()
            .items_center()
            .gap_2()
            .when(!active, |el| el.opacity(0.55))
            .child(
                div()
                    .size(px(24.))
                    .flex_shrink_0()
                    .rounded(px(7.))
                    .bg(color.opacity(0.18))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(glyph(icon).size(px(14.)).text_color(color)),
            )
            .child(
                div()
                    .w(px(52.))
                    .flex_shrink_0()
                    .child(
                        div()
                            .text_size(px(12.5))
                            .font_weight(FontWeight::MEDIUM)
                            .child(label),
                    )
                    .child(
                        div()
                            .text_size(px(10.5))
                            .text_color(p.ink_3)
                            .child(if muted {
                                "Muted".to_string()
                            } else {
                                format!("{:.0}%", level * 100.)
                            }),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .child(Slider::new(&self.stems.sliders[i])),
            )
            .child(
                div()
                    .opacity(if muted { 0.3 } else { 0.4 + 0.6 * level })
                    .child(super::motion::equalizer(
                        format!("stem-meter-{i}"),
                        color,
                        live,
                        cx,
                    )),
            )
            .child(pill("stem-solo", "S", "Solo", soloed).on_click(cx.listener(
                move |this, _, window, cx| {
                    // Solo again to bring everything back.
                    let mut levels = [0.; 4];
                    if soloed {
                        levels = [1.; 4];
                    } else {
                        levels[i] = 1.;
                    }
                    this.set_stem_levels(levels, window, cx);
                },
            )))
            .child(pill("stem-mute", "M", "Mute", muted).on_click(cx.listener(
                move |this, _, window, cx| {
                    let mut levels: [f32; 4] =
                        std::array::from_fn(|k| this.player.stem_mix().get(k));
                    levels[i] = if levels[i] <= 0.001 { 1. } else { 0. };
                    this.set_stem_levels(levels, window, cx);
                },
            )))
    }

    /// The ready-made mixes as chips; the one matching the current levels is lit.
    fn stem_presets(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = pal(cx);
        let current: Vec<f32> = (0..STEMS.len())
            .map(|k| self.player.stem_mix().get(k))
            .collect();
        let per_row = 3;
        let rows: Vec<AnyElement> = PRESETS
            .chunks(per_row)
            .enumerate()
            .map(|(r, row)| {
                div()
                    .flex()
                    .gap_1()
                    .children(row.iter().enumerate().map(|(j, (name, icon, levels))| {
                        let levels = *levels;
                        let on = current
                            .iter()
                            .zip(levels)
                            .all(|(a, b)| (a - b).abs() < 0.01);
                        div()
                            .id(("stem-preset", r * per_row + j))
                            .flex_1()
                            .h(px(28.))
                            .px_2()
                            .rounded(px(8.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .gap(px(6.))
                            .cursor_pointer()
                            .text_size(px(11.5))
                            .font_weight(FontWeight::MEDIUM)
                            .when(on, |el| el.bg(p.accent).text_color(p.accent_ink))
                            .when(!on, |el| {
                                el.bg(p.raised)
                                    .text_color(p.ink_2)
                                    .hover(|s| s.bg(p.raised_hover).text_color(p.ink))
                            })
                            .child(glyph(icon).size(px(14.)).text_color(if on {
                                p.accent_ink
                            } else {
                                p.ink_3
                            }))
                            .child(div().truncate().child(*name))
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.set_stem_levels(levels, window, cx)
                            }))
                    }))
                    .into_any_element()
            })
            .collect();
        div()
            .flex()
            .flex_col()
            .gap_1()
            .child(faint("Mixes", cx))
            .children(rows)
    }
}
