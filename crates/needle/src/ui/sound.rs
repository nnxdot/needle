use super::{
    AppView, pal,
    widgets::{faint, heading, meta, page_title, setting_row, small_button, strong},
};
use gpui::{prelude::*, *};
use gpui_component::{
    button::ButtonVariants,
    slider::{Slider, SliderEvent, SliderState},
    switch::Switch,
};
use needle_core::{
    audio::Command,
    dsp::{BANDS, Dsp, MAX_GAIN_DB, PRESETS},
    effects::EffectSlot,
};
use std::collections::HashMap;

pub struct SoundControls {
    preamp: Entity<SliderState>,
    balance: Entity<SliderState>,
    bands: Vec<Entity<SliderState>>,
    /// Effect sliders by (slot uid, parameter id), made when an effect first shows.
    effect_sliders: HashMap<(String, String), (Entity<SliderState>, Subscription)>,
    _subscriptions: Vec<Subscription>,
}

fn band_label(frequency: f64) -> String {
    if frequency >= 1000. {
        format!("{}k", frequency / 1000.)
    } else {
        format!("{frequency}")
    }
}

impl SoundControls {
    pub fn new(dsp: &Dsp, cx: &mut Context<AppView>) -> Self {
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
        Self {
            preamp,
            balance,
            bands,
            effect_sliders: HashMap::new(),
            _subscriptions: subscriptions,
        }
    }
    pub fn forget_slider(&mut self, uid: &str, param: &str) {
        self.effect_sliders
            .remove(&(uid.to_string(), param.to_string()));
    }
}

impl AppView {
    /// Make sliders for effects that do not have them yet, and drop those of removed effects.
    pub(super) fn sync_effect_sliders(&mut self, cx: &mut Context<Self>) {
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
                                        let dsp = Dsp { eq: this.settings.dsp.eq, ..this.settings.dsp.clone() };
                                        let dsp = Dsp { preamp_db: 0., bands: [0.; 10], preset: "Flat".into(), ..dsp };
                                        this.apply_dsp(dsp, window, cx);
                                    })))
                                    .child(Switch::new("eq-on").checked(dsp.eq).on_click(cx.listener(|this, checked: &bool, _, cx| {
                                        let on = *checked;
                                        this.edit_dsp(|d| d.eq = on);
                                        cx.notify();
                                    }))),
                            )
                            // Rows of five: GPUI's flex_wrap inside a column reports the height of one chip per line.
                            .children(preset_rows)
                            .child(
                                div()
                                    .flex()
                                    .gap_2()
                                    .when(!dsp.eq, |el| el.opacity(0.45))
                                    .child(self.band_column("Preamp", &self.sound.preamp, dsp.preamp_db, true, cx))
                                    .child(div().w(px(1.)).h(px(210.)).bg(p.line).mx_2())
                                    .children(BANDS.iter().enumerate().map(|(i, f)| self.band_column(&band_label(*f), &self.sound.bands[i], dsp.bands[i], false, cx))),
                            )
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
