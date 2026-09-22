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

const PRESETS: [(&str, [f32; 4]); 5] = [
    ("Full mix", [1., 1., 1., 1.]),
    ("Karaoke", [1., 1., 1., 0.]),
    ("Vocals only", [0., 0., 0., 1.]),
    ("No drums", [0., 1., 1., 1.]),
    ("Bass only", [0., 1., 0., 0.]),
];

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
                .children(STEMS.iter().enumerate().map(|(i, name)| {
                    let level = self.player.stem_mix().get(i);
                    div()
                        .flex()
                        .items_center()
                        .gap_3()
                        .when(!active, |el| el.opacity(0.5))
                        .child(div().w(px(56.)).text_size(px(13.)).child(match *name {
                            "drums" => "Drums",
                            "bass" => "Bass",
                            "vocals" => "Vocals",
                            _ => "Other",
                        }))
                        .child(Slider::new(&self.stems.sliders[i]).flex_1())
                        .child(faint(format!("{:.0}%", level * 100.), cx).w(px(40.)).text_right())
                        .child(
                            div()
                                .id(("stem-solo", i))
                                .px_2()
                                .rounded(px(4.))
                                .text_size(px(11.))
                                .cursor_pointer()
                                .border_1()
                                .border_color(p.line)
                                .text_color(p.ink_2)
                                .hover(|s| s.text_color(p.ink))
                                .child("Solo")
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    let mut levels = [0.; 4];
                                    levels[i] = 1.;
                                    this.set_stem_levels(levels, window, cx);
                                })),
                        )
                }))
                .child(div().flex().gap_2().children(PRESETS.iter().enumerate().map(|(i, (name, levels))| {
                    let levels = *levels;
                    small_button(("stem-preset", i), *name).ghost().on_click(cx.listener(move |this, _, window, cx| this.set_stem_levels(levels, window, cx)))
                })))
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
