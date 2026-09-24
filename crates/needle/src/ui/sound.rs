use super::{
    AppView, pal,
    widgets::{faint, heading, meta, page_title, setting_row, small_button, strong},
};
use gpui::{prelude::*, *};
use gpui_component::{
    Sizable,
    button::ButtonVariants,
    input::{Input, InputState},
    slider::{Slider, SliderEvent, SliderState},
    switch::Switch,
};
use needle_core::{
    audio::Command,
    dsp::{
        BANDS, Dsp, MAX_GAIN_DB, MAX_PARAMETRIC, PARAMETRIC_KINDS, PRESETS, ParamBand, UserPreset,
    },
    effects::EffectSlot,
};
use std::collections::HashMap;

pub struct SoundControls {
    preamp: Entity<SliderState>,
    balance: Entity<SliderState>,
    bands: Vec<Entity<SliderState>>,
    /// Effect sliders by (slot uid, parameter id), made when an effect first shows.
    effect_sliders: HashMap<(String, String), (Entity<SliderState>, Subscription)>,
    /// Parametric band sliders by (band uid, "frequency" | "gain" | "q").
    band_sliders: HashMap<(String, &'static str), (Entity<SliderState>, Subscription)>,
    preset_name: Entity<InputState>,
    _subscriptions: Vec<Subscription>,
}

/// Parametric sliders run from 0 to 1000 on a log scale: 10 Hz to 24 kHz (all a band can
/// hold), and Q 0.1 to 20.
fn to_frequency(v: f32) -> f32 {
    10. * 2400f32.powf(v / 1000.)
}
fn from_frequency(f: f32) -> f32 {
    (1000. * (f.max(10.) / 10.).ln() / 2400f32.ln()).clamp(0., 1000.)
}
fn to_q(v: f32) -> f32 {
    0.1 * 200f32.powf(v / 1000.)
}
fn from_q(q: f32) -> f32 {
    (1000. * (q.max(0.1) / 0.1).ln() / 200f32.ln()).clamp(0., 1000.)
}
fn hertz(f: f32) -> String {
    if f >= 1000. {
        format!("{:.1} kHz", f / 1000.)
    } else {
        format!("{f:.0} Hz")
    }
}

fn band_label(frequency: f64) -> String {
    if frequency >= 1000. {
        format!("{}k", frequency / 1000.)
    } else {
        format!("{frequency}")
    }
}

impl SoundControls {
    pub fn new(dsp: &Dsp, window: &mut Window, cx: &mut Context<AppView>) -> Self {
        let slider = |min: f32, max: f32, step: f32, value: f32, cx: &mut Context<AppView>| {
            cx.new(|_| {
                SliderState::new()
                    .min(min)
                    .max(max)
                    .step(step)
                    .default_value(value)
            })
        };
        let preamp = slider(-24., 12., 0.5, dsp.preamp_db, cx);
        let balance = slider(-1., 1., 0.05, dsp.balance, cx);
        let bands: Vec<_> = dsp
            .bands
            .iter()
            .map(|g| slider(-MAX_GAIN_DB, MAX_GAIN_DB, 0.5, *g, cx))
            .collect();
        let mut subscriptions = vec![
            cx.subscribe(&preamp, |this, _, event, _| {
                let SliderEvent::Change(value) = event;
                this.edit_dsp(|d| d.preamp_db = value.start());
            }),
            cx.subscribe(&balance, |this, _, event, _| {
                let SliderEvent::Change(value) = event;
                // Snap to centre so "almost balanced" does not linger.
                let v = value.start();
                this.edit_dsp(|d| d.balance = if v.abs() < 0.04 { 0. } else { v });
            }),
        ];
        for (index, band) in bands.iter().enumerate() {
            subscriptions.push(cx.subscribe(band, move |this, _, event, _| {
                let SliderEvent::Change(value) = event;
                this.edit_dsp(|d| {
                    d.bands[index] = value.start();
                    d.preset = "Custom".into();
                });
            }));
        }
        let preset_name =
            cx.new(|cx| InputState::new(window, cx).placeholder("Name for this sound"));
        Self {
            preamp,
            balance,
            bands,
            effect_sliders: HashMap::new(),
            band_sliders: HashMap::new(),
            preset_name,
            _subscriptions: subscriptions,
        }
    }
    pub fn forget_slider(&mut self, uid: &str, param: &str) {
        self.effect_sliders
            .remove(&(uid.to_string(), param.to_string()));
    }
}

impl AppView {
    /// Make sliders for parametric bands that do not have them yet, and drop old ones.
    fn sync_band_sliders(&mut self, cx: &mut Context<Self>) {
        let bands = self.settings.dsp.parametric.clone();
        self.sound
            .band_sliders
            .retain(|(uid, _), _| bands.iter().any(|b| b.uid == *uid));
        for band in bands {
            for (which, value) in [
                ("frequency", from_frequency(band.frequency)),
                ("gain", band.gain),
                ("q", from_q(band.q)),
            ] {
                let key = (band.uid.clone(), which);
                if self.sound.band_sliders.contains_key(&key) {
                    continue;
                }
                let slider = cx.new(|_| {
                    let state = SliderState::new();
                    if which == "gain" {
                        state.min(-24.).max(24.).step(0.5).default_value(value)
                    } else {
                        state.min(0.).max(1000.).step(1.).default_value(value)
                    }
                });
                let uid = band.uid.clone();
                let subscription = cx.subscribe(&slider, move |this, _, event, cx| {
                    let SliderEvent::Change(value) = event;
                    let value = value.start();
                    this.edit_dsp(|d| {
                        if let Some(band) = d.parametric.iter_mut().find(|b| b.uid == uid) {
                            match which {
                                "frequency" => band.frequency = to_frequency(value),
                                "gain" => band.gain = value,
                                _ => band.q = to_q(value),
                            }
                            d.preset = "Custom".into();
                        }
                    });
                    cx.notify();
                });
                self.sound.band_sliders.insert(key, (slider, subscription));
            }
        }
    }

    /// Make sliders for effects that do not have them yet, and drop those of removed effects.
    pub(super) fn sync_effect_sliders(&mut self, cx: &mut Context<Self>) {
        self.sync_band_sliders(cx);
        let registry = self.player.effects().clone();
        let slots = self.settings.dsp.effects.clone();
        self.sound
            .effect_sliders
            .retain(|(uid, _), _| slots.iter().any(|s| s.uid == *uid));
        for slot in slots {
            let Some(def) = registry.find(&slot.plugin, &slot.effect) else {
                continue;
            };
            for (param, value) in def.params.iter().zip(def.values(&slot)) {
                let key = (slot.uid.clone(), param.id.clone());
                if self.sound.effect_sliders.contains_key(&key) {
                    continue;
                }
                let slider = cx.new(|_| {
                    SliderState::new()
                        .min(param.min)
                        .max(param.max)
                        .step(param.step())
                        .default_value(value)
                });
                let (uid, id) = key.clone();
                let subscription = cx.subscribe(&slider, move |this, _, event, cx| {
                    let SliderEvent::Change(value) = event;
                    let value = value.start();
                    this.edit_dsp(|d| {
                        if let Some(slot) = d.effects.iter_mut().find(|s| s.uid == uid) {
                            slot.params.insert(id.clone(), value);
                        }
                    });
                    cx.notify();
                });
                self.sound
                    .effect_sliders
                    .insert(key, (slider, subscription));
            }
        }
    }

    fn edit_dsp(&mut self, change: impl FnOnce(&mut Dsp)) {
        change(&mut self.settings.dsp);
        self.player.send(Command::Dsp(self.settings.dsp.clone()));
    }

    /// Move every slider to match `dsp` (after choosing a preset or resetting).
    fn apply_dsp(&mut self, dsp: Dsp, window: &mut Window, cx: &mut Context<Self>) {
        // Parametric sliders are made again from the new bands when Sound is drawn next.
        self.sound.band_sliders.clear();
        self.sound
            .preamp
            .update(cx, |s, cx| s.set_value(dsp.preamp_db, window, cx));
        self.sound
            .balance
            .update(cx, |s, cx| s.set_value(dsp.balance, window, cx));
        for (slider, gain) in self.sound.bands.iter().zip(dsp.bands) {
            slider.update(cx, |s, cx| s.set_value(gain, window, cx));
        }
        self.settings.dsp = dsp;
        self.player.send(Command::Dsp(self.settings.dsp.clone()));
        cx.notify();
    }

    pub(super) fn sound_view(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("sound-scroll")
            .size_full()
            .overflow_y_scroll()
            .child(
                div()
                    .max_w(px(820.))
                    .px_8()
                    .pt_6()
                    .pb_16()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(page_title("Sound"))
                    .child(self.sound_body(cx)),
            )
    }

    /// The equalizer, effects, and listening tools, for the Sound page and Settings › Sound.
    pub(super) fn sound_body(&self, cx: &mut Context<Self>) -> Div {
        let p = pal(cx);
        let dsp = self.settings.dsp.clone();
        let exclusive = self.settings.exclusive;
        let clip_risk = dsp.eq && dsp.preamp_db > dsp.suggested_preamp() + 0.01;
        let mut preset_rows: Vec<Div> = vec![];
        for (i, (name, preamp, bands)) in PRESETS.iter().enumerate() {
            if i % 5 == 0 {
                preset_rows.push(div().w_full().flex().gap(px(6.)));
            }
            let active = dsp.preset == *name;
            let (preamp, bands) = (*preamp, *bands);
            let chip = div()
                .id(("preset", i))
                .px(px(10.))
                .py(px(5.))
                .rounded_full()
                .border_1()
                .text_size(px(12.5))
                .cursor_pointer()
                .when(active, |el| {
                    el.bg(p.accent_soft)
                        .border_color(p.accent.opacity(0.5))
                        .text_color(p.accent)
                })
                .when(!active, |el| {
                    el.border_color(p.line)
                        .text_color(p.ink_2)
                        .hover(|s| s.text_color(p.ink).border_color(p.ink_3))
                })
                .child(*name)
                .on_click(cx.listener(move |this, _, window, cx| {
                    let dsp = Dsp {
                        eq: true,
                        mode: "graphic".into(),
                        preamp_db: preamp,
                        bands,
                        preset: name.to_string(),
                        ..this.settings.dsp.clone()
                    };
                    this.apply_dsp(dsp, window, cx);
                }));
            let row = preset_rows.pop().unwrap_or_else(div);
            preset_rows.push(row.child(chip));
        }
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(meta(
                        if exclusive {
                            "Exclusive output is on, so these tools are bypassed and each file plays bit-for-bit. Turn it off in Settings to use them."
                        } else {
                            "Shape the sound on shared output. Changes apply while you listen."
                        },
                        cx,
                    ).text_color(if exclusive { p.accent } else { p.ink_2 }))
                    // Equalizer
                    .child(
                        div()
                            .mt_6()
                            .p_5()
                            .rounded(px(10.))
                            .bg(p.chrome)
                            .border_1()
                            .border_color(p.line_soft)
                            .flex()
                            .flex_col()
                            .gap_4()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_4()
                                    .child(div().flex_1().flex().flex_col().gap_1().child(heading("Equalizer")).child(meta(format!("Preset: {}", dsp.preset), cx)))
                                    .child(small_button("eq-reset", "Reset").ghost().on_click(cx.listener(|this, _, window, cx| {
                                        let dsp = Dsp { preamp_db: 0., bands: [0.; 10], parametric: vec![], preset: "Flat".into(), ..this.settings.dsp.clone() };
                                        this.apply_dsp(dsp, window, cx);
                                    })))
                                    .child(Switch::new("eq-on").checked(dsp.eq).on_click(cx.listener(|this, checked: &bool, _, cx| {
                                        let on = *checked;
                                        this.edit_dsp(|d| d.eq = on);
                                        cx.notify();
                                    }))),
                            )
                            .child(self.eq_mode_row(&dsp, cx))
                            // Rows of five: GPUI's flex_wrap inside a column reports the height of one chip per line.
                            .children(preset_rows)
                            .children(self.user_preset_rows(&dsp, cx))
                            .when(!dsp.parametric_mode(), |el| {
                                el.child(
                                    div()
                                        .flex()
                                        .gap_2()
                                        .when(!dsp.eq, |el| el.opacity(0.45))
                                        .child(self.band_column("Preamp", &self.sound.preamp, dsp.preamp_db, true, cx))
                                        .child(div().w(px(1.)).h(px(210.)).bg(p.line).mx_2())
                                        .children(BANDS.iter().enumerate().map(|(i, f)| self.band_column(&band_label(*f), &self.sound.bands[i], dsp.bands[i], false, cx))),
                                )
                            })
                            .when(dsp.parametric_mode(), |el| el.child(self.parametric_view(&dsp, cx)))
                            .when(clip_risk, |el| {
                                el.child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_3()
                                        .child(meta(format!("Boosted bands can clip. A preamp of {:.1} dB keeps the loudest band safe.", dsp.suggested_preamp()), cx))
                                        .child(small_button("eq-auto-preamp", "Use it").on_click(cx.listener(|this, _, window, cx| {
                                            let preamp = this.settings.dsp.suggested_preamp();
                                            this.sound.preamp.update(cx, |s, cx| s.set_value(preamp, window, cx));
                                            this.edit_dsp(|d| d.preamp_db = preamp);
                                            cx.notify();
                                        }))),
                                )
                            }),
                    )
                    .child(self.effects_card(cx))
                    // Other tools
                    .child(div().mt_6().child(heading("Listening tools")))
                    .child(setting_row(
                        "Balance",
                        match dsp.balance {
                            b if b < 0. => "Leaning left.",
                            b if b > 0. => "Leaning right.",
                            _ => "Centred.",
                        },
                        div().w(px(220.)).flex().items_center().gap_2().child(faint("L", cx)).child(Slider::new(&self.sound.balance).flex_1()).child(faint("R", cx)),
                        cx,
                    ))
                    .child(setting_row(
                        "Mono",
                        "Plays both channels in both ears. Useful with one earbud, or for hearing loss in one ear.",
                        Switch::new("mono").checked(dsp.mono).on_click(cx.listener(|this, checked: &bool, _, cx| {
                            let on = *checked;
                            this.edit_dsp(|d| d.mono = on);
                            cx.notify();
                        })),
                        cx,
                    ))
                    .child(setting_row(
                        "Headphone crossfeed",
                        "Blends a little of each side into the other, the way speakers reach both ears. Makes hard-panned older recordings less tiring on headphones.",
                        Switch::new("crossfeed").checked(dsp.crossfeed).on_click(cx.listener(|this, checked: &bool, _, cx| {
                            let on = *checked;
                            this.edit_dsp(|d| d.crossfeed = on);
                            cx.notify();
                        })),
                        cx,
                    ))
                    .child(faint("ReplayGain, album gain, and loudness measurement are in Settings › Playback.", cx).mt_4()),
            )
    }

    fn chip(
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        active: bool,
        cx: &App,
    ) -> Stateful<Div> {
        let p = pal(cx);
        div()
            .id(id)
            .px(px(10.))
            .py(px(5.))
            .rounded_full()
            .border_1()
            .text_size(px(12.5))
            .cursor_pointer()
            .when(active, |el| {
                el.bg(p.accent_soft)
                    .border_color(p.accent.opacity(0.5))
                    .text_color(p.accent)
            })
            .when(!active, |el| {
                el.border_color(p.line)
                    .text_color(p.ink_2)
                    .hover(|s| s.text_color(p.ink).border_color(p.ink_3))
            })
            .child(label.into())
    }

    /// Graphic or parametric, and loading or saving Equalizer APO / AutoEq files.
    fn eq_mode_row(&self, dsp: &Dsp, cx: &mut Context<Self>) -> Div {
        let parametric = dsp.parametric_mode();
        div()
            .flex()
            .items_center()
            .gap(px(6.))
            .child(Self::chip("eq-graphic", "Graphic", !parametric, cx).on_click(cx.listener(|this, _, _, cx| {
                this.edit_dsp(|d| d.mode = "graphic".into());
                cx.notify();
            })))
            .child(Self::chip("eq-parametric", "Parametric", parametric, cx).on_click(cx.listener(|this, _, _, cx| {
                this.edit_dsp(|d| {
                    d.mode = "parametric".into();
                    if d.parametric.is_empty() {
                        // Start from the graphic bands, so switching keeps the sound.
                        d.parametric = BANDS
                            .iter()
                            .zip(d.bands)
                            .map(|(f, g)| ParamBand { frequency: *f as f32, gain: g, q: 1.41, ..Default::default() })
                            .collect();
                    }
                });
                cx.notify();
            })))
            .child(div().flex_1())
            .child(small_button("eq-import", "Load EQ file…").ghost().on_click(cx.listener(|this, _, window, cx| {
                if !this.can_pick(cx) {
                    return;
                }
                let Some(path) = rfd::FileDialog::new()
                    .set_title("Load a ParametricEQ.txt from AutoEq, or an Equalizer APO configuration")
                    .add_filter("Equalizer settings", &["txt"])
                    .pick_file()
                else {
                    return;
                };
                let loaded = std::fs::read_to_string(&path)
                    .map_err(anyhow::Error::from)
                    .and_then(|text| needle_core::dsp::parse_parametric(&text));
                match loaded {
                    Ok((preamp, bands)) => {
                        let name = path.file_stem().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                        let count = bands.len();
                        let dsp = Dsp { eq: true, mode: "parametric".into(), preamp_db: preamp, parametric: bands, preset: name, ..this.settings.dsp.clone() };
                        this.apply_dsp(dsp, window, cx);
                        this.notify(format!("Loaded {count} filters."));
                    }
                    Err(error) => this.fail(format!("{error:#}")),
                }
            })))
            .child(small_button("eq-export", "Save EQ file…").ghost().on_click(cx.listener(|this, _, _, cx| {
                if !this.can_pick(cx) {
                    return;
                }
                let dsp = this.settings.dsp.clone();
                let bands: Vec<ParamBand> = if dsp.parametric_mode() {
                    dsp.parametric.clone()
                } else {
                    BANDS
                        .iter()
                        .zip(dsp.bands)
                        .filter(|(_, g)| *g != 0.)
                        .map(|(f, g)| ParamBand { frequency: *f as f32, gain: g, q: 1.41, ..Default::default() })
                        .collect()
                };
                let Some(path) = rfd::FileDialog::new().set_file_name("ParametricEQ.txt").add_filter("Equalizer settings", &["txt"]).save_file() else {
                    return;
                };
                match std::fs::write(&path, needle_core::dsp::format_parametric(dsp.preamp_db, &bands)) {
                    Ok(()) => this.notify("Saved in the Equalizer APO format."),
                    Err(error) => this.fail(format!("Could not save: {error}")),
                }
            })))
    }

    /// The listener's own presets, and saving the current sound as one.
    fn user_preset_rows(&self, dsp: &Dsp, cx: &mut Context<Self>) -> Vec<Div> {
        let p = pal(cx);
        let mut rows: Vec<Div> = vec![];
        for (i, preset) in self.settings.eq_presets.iter().enumerate() {
            if i % 4 == 0 {
                rows.push(div().w_full().flex().gap(px(6.)));
            }
            let (apply, remove) = (preset.clone(), preset.name.clone());
            let chip = div()
                .flex()
                .items_center()
                .child(
                    Self::chip(
                        ("user-preset", i),
                        preset.name.clone(),
                        dsp.preset == preset.name,
                        cx,
                    )
                    .on_click(cx.listener(move |this, _, window, cx| {
                        let dsp = apply.apply(&this.settings.dsp);
                        this.apply_dsp(dsp, window, cx);
                    })),
                )
                .child(
                    div()
                        .id(("user-preset-remove", i))
                        .px(px(5.))
                        .text_size(px(12.))
                        .text_color(p.ink_3)
                        .cursor_pointer()
                        .hover(|s| s.text_color(p.danger))
                        .child("×")
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.settings.eq_presets.retain(|p| p.name != remove);
                            this.persist_settings();
                            cx.notify();
                        })),
                );
            let row = rows.pop().unwrap_or_else(div);
            rows.push(row.child(chip));
        }
        rows.push(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .w(px(220.))
                        .child(Input::new(&self.sound.preset_name).small()),
                )
                .child(
                    small_button("eq-save-preset", "Save as preset")
                        .ghost()
                        .on_click(cx.listener(|this, _, window, cx| {
                            let typed = this.sound.preset_name.read(cx).value().trim().to_string();
                            let name = if typed.is_empty() {
                                format!("My sound {}", this.settings.eq_presets.len() + 1)
                            } else {
                                typed
                            };
                            let preset = UserPreset::from_dsp(&name, &this.settings.dsp);
                            match this.settings.eq_presets.iter_mut().find(|p| p.name == name) {
                                Some(existing) => *existing = preset,
                                None => this.settings.eq_presets.push(preset),
                            }
                            this.settings.dsp.preset = name.clone();
                            this.player.send(Command::Dsp(this.settings.dsp.clone()));
                            this.persist_settings();
                            this.sound
                                .preset_name
                                .update(cx, |input, cx| input.set_value("", window, cx));
                            this.notify(format!("Saved \"{name}\"."));
                            cx.notify();
                        })),
                ),
        );
        rows
    }

    /// The parametric equalizer: a row per band with its kind, frequency, gain, and width.
    fn parametric_view(&self, dsp: &Dsp, cx: &mut Context<Self>) -> Div {
        let p = pal(cx);
        let count = dsp.parametric.len();
        div()
            .flex()
            .flex_col()
            .gap_3()
            .when(!dsp.eq, |el| el.opacity(0.45))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(div().w(px(96.)).text_size(px(13.)).text_color(p.ink_2).child("Preamp"))
                    .child(Slider::new(&self.sound.preamp).flex_1())
                    .child(div().w(px(64.)).text_size(px(12.5)).child(format!("{:+.1} dB", dsp.preamp_db))),
            )
            .children(dsp.parametric.iter().enumerate().map(|(i, band)| {
                let kind_name = PARAMETRIC_KINDS.iter().find(|(k, _)| *k == band.kind).map_or("Peak", |(_, n)| *n);
                let has_gain = matches!(band.kind.as_str(), "peak" | "lowshelf" | "highshelf");
                let slider = |which: &'static str| self.sound.band_sliders.get(&(band.uid.clone(), which)).map(|(s, _)| s.clone());
                let column = |label: String, which: &'static str| {
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(div().text_size(px(12.)).text_color(p.ink_2).child(label))
                        .children(slider(which).map(|s| Slider::new(&s)))
                };
                let (cycle, toggle, remove) = (band.uid.clone(), band.uid.clone(), band.uid.clone());
                div()
                    .id(("band", i))
                    .p_3()
                    .rounded(px(8.))
                    .bg(p.canvas)
                    .border_1()
                    .border_color(p.line_soft)
                    .flex()
                    .items_center()
                    .gap_3()
                    .when(!band.on, |el| el.opacity(0.5))
                    .child(
                        Self::chip(("band-kind", i), kind_name, false, cx)
                            .w(px(96.))
                            .flex()
                            .justify_center()
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.edit_dsp(|d| {
                                    if let Some(band) = d.parametric.iter_mut().find(|b| b.uid == cycle) {
                                        let at = PARAMETRIC_KINDS.iter().position(|(k, _)| *k == band.kind).unwrap_or(0);
                                        band.kind = PARAMETRIC_KINDS[(at + 1) % PARAMETRIC_KINDS.len()].0.into();
                                        d.preset = "Custom".into();
                                    }
                                });
                                cx.notify();
                            })),
                    )
                    .child(column(hertz(band.frequency), "frequency"))
                    .child(if has_gain {
                        column(format!("{:+.1} dB", band.gain), "gain").into_any_element()
                    } else {
                        div().flex_1().into_any_element()
                    })
                    .child(column(format!("Q {:.2}", band.q), "q"))
                    .child(Switch::new(("band-on", i)).checked(band.on).on_click(cx.listener(move |this, checked: &bool, _, cx| {
                        let on = *checked;
                        this.edit_dsp(|d| {
                            if let Some(band) = d.parametric.iter_mut().find(|b| b.uid == toggle) {
                                band.on = on;
                            }
                        });
                        cx.notify();
                    })))
                    .child(
                        div()
                            .id(("band-remove", i))
                            .px_1()
                            .text_color(p.ink_3)
                            .cursor_pointer()
                            .hover(|s| s.text_color(p.danger))
                            .child("×")
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.edit_dsp(|d| {
                                    d.parametric.retain(|b| b.uid != remove);
                                    d.preset = "Custom".into();
                                });
                                cx.notify();
                            })),
                    )
            }))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .when(count < MAX_PARAMETRIC, |el| {
                        el.child(small_button("band-add", "Add band").on_click(cx.listener(|this, _, _, cx| {
                            this.edit_dsp(|d| {
                                d.parametric.push(ParamBand::default());
                                d.preset = "Custom".into();
                            });
                            cx.notify();
                        })))
                    })
                    .child(faint("Click a band's kind to change it. Peak, shelf, and notch bands follow Q; a higher Q is narrower.", cx)),
            )
    }

    fn effects_card(&self, cx: &mut Context<Self>) -> Div {
        let p = pal(cx);
        let registry = self.player.effects().clone();
        let available = registry.all();
        let slots = self.settings.dsp.effects.clone();
        let count = slots.len();
        div()
            .mt_6()
            .p_5()
            .rounded(px(10.))
            .bg(p.chrome)
            .border_1()
            .border_color(p.line_soft)
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(heading("Effects"))
                    .child(meta("Effects come from plugins. They play in this order, after the equalizer.", cx)),
            )
            .children(slots.into_iter().enumerate().map(|(i, slot)| {
                let def = registry.find(&slot.plugin, &slot.effect);
                let failure = registry.failure(&slot.plugin, &slot.effect);
                let uid = slot.uid.clone();
                let title = def.as_ref().map_or(slot.effect.clone(), |d| d.name.clone());
                let source = def.as_ref().map_or(slot.plugin.clone(), |d| {
                    format!("{}{}", d.plugin_name, if d.is_wasm() { " · own DSP code" } else { "" })
                });
                let (up, down, remove, toggle) = (uid.clone(), uid.clone(), uid.clone(), uid.clone());
                div()
                    .id(("effect", i))
                    .p_4()
                    .rounded(px(8.))
                    .bg(p.canvas)
                    .border_1()
                    .border_color(if failure.is_some() { p.danger.opacity(0.4) } else { p.line_soft })
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .flex()
                                    .flex_col()
                                    .child(strong(title))
                                    .child(faint(source, cx)),
                            )
                            .when(i > 0, |el| {
                                el.child(small_button(SharedString::from(format!("effect-up-{i}")), "Up").ghost().on_click(cx.listener(move |this, _, _, cx| {
                                    this.edit_dsp(|d| {
                                        if let Some(at) = d.effects.iter().position(|s| s.uid == up) && at > 0 {
                                            d.effects.swap(at, at - 1);
                                        }
                                    });
                                    cx.notify();
                                })))
                            })
                            .when(i + 1 < count, |el| {
                                el.child(small_button(SharedString::from(format!("effect-down-{i}")), "Down").ghost().on_click(cx.listener(move |this, _, _, cx| {
                                    this.edit_dsp(|d| {
                                        if let Some(at) = d.effects.iter().position(|s| s.uid == down) && at + 1 < d.effects.len() {
                                            d.effects.swap(at, at + 1);
                                        }
                                    });
                                    cx.notify();
                                })))
                            })
                            .child(small_button(SharedString::from(format!("effect-remove-{i}")), "Remove").ghost().on_click(cx.listener(move |this, _, _, cx| {
                                this.edit_dsp(|d| d.effects.retain(|s| s.uid != remove));
                                cx.notify();
                            })))
                            .child(Switch::new(("effect-on", i)).checked(slot.on).on_click(cx.listener(move |this, checked: &bool, _, cx| {
                                let on = *checked;
                                this.edit_dsp(|d| {
                                    if let Some(slot) = d.effects.iter_mut().find(|s| s.uid == toggle) {
                                        slot.on = on;
                                    }
                                });
                                cx.notify();
                            }))),
                    )
                    .when(def.is_none(), |el| el.child(faint("Not available now. Turn on its plugin in Settings › Plugins.", cx)))
                    .when_some(failure, |el, failure| el.child(meta(format!("Stopped: {failure}"), cx).text_color(p.danger)))
                    .when_some(def.as_ref().filter(|d| !d.description.is_empty()), |el, d| el.child(meta(d.description.clone(), cx)))
                    .when_some(def, |el, def| {
                        let values = def.values(&slot);
                        el.children(def.params.iter().zip(values).filter_map(|(param, value)| {
                            let (slider, _) = self.sound.effect_sliders.get(&(uid.clone(), param.id.clone()))?;
                            Some(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_3()
                                    .when(!slot.on, |el| el.opacity(0.45))
                                    .child(div().w(px(120.)).text_size(px(13.)).text_color(p.ink_2).child(param.name.clone()))
                                    .child(Slider::new(slider).flex_1())
                                    .child(div().w(px(72.)).text_size(px(12.5)).text_color(p.ink).child(param.display(value))),
                            )
                        }))
                    })
            }))
            .child(if available.is_empty() {
                meta(
                    "No effects yet. Turn on a plugin that adds effects in Settings › Plugins. Add the example plugins there to try Studio effects and Bitcrusher.",
                    cx,
                )
                .into_any_element()
            } else {
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(faint("Add an effect", cx))
                    // One chip per line would report a wrong height with flex_wrap, so rows of four.
                    .children(available.chunks(4).enumerate().map(|(row, defs)| {
                        div().w_full().flex().gap(px(6.)).children(defs.iter().enumerate().map(|(j, def)| {
                            let (plugin, effect) = (def.plugin.clone(), def.id.clone());
                            div()
                                .id(("effect-add", row * 4 + j))
                                .px(px(10.))
                                .py(px(5.))
                                .rounded_full()
                                .border_1()
                                .border_color(p.line)
                                .text_size(px(12.5))
                                .text_color(p.ink_2)
                                .cursor_pointer()
                                .hover(|s| s.text_color(p.ink).border_color(p.ink_3))
                                .child(format!("+ {}", def.name))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.edit_dsp(|d| d.effects.push(EffectSlot::new(&plugin, &effect)));
                                    cx.notify();
                                }))
                        }))
                    }))
                    .into_any_element()
            })
    }

    fn band_column(
        &self,
        label: &str,
        slider: &Entity<SliderState>,
        value: f32,
        preamp: bool,
        cx: &App,
    ) -> Div {
        let p = pal(cx);
        div()
            .w(px(if preamp { 56. } else { 44. }))
            .flex()
            .flex_col()
            .items_center()
            .gap_2()
            .child(
                div()
                    .text_size(px(11.5))
                    .text_color(if value == 0. { p.ink_3 } else { p.ink })
                    .child(if value == 0. {
                        "0".to_string()
                    } else {
                        format!("{value:+.1}")
                    }),
            )
            .child(
                div()
                    .h(px(170.))
                    .w(px(24.))
                    .overflow_hidden()
                    .child(Slider::new(slider).vertical().h(px(170.))),
            )
            .child(if preamp {
                strong(label.to_string()).text_size(px(11.5))
            } else {
                faint(label.to_string(), cx).text_size(px(11.5))
            })
    }
}
