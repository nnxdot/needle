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
};

pub struct SoundControls {
    preamp: Entity<SliderState>,
    balance: Entity<SliderState>,
    bands: Vec<Entity<SliderState>>,
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
            _subscriptions: subscriptions,
        }
    }
}

impl AppView {
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
        let p = pal(cx);
        let dsp = self.settings.dsp.clone();
        let exclusive = self.settings.exclusive;
        let clip_risk = dsp.eq && dsp.preamp_db > dsp.suggested_preamp() + 0.01;
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
                            // A single row: GPUI's flex_wrap inside a column reports the height of one chip per line.
                            .child(div().w_full().flex().gap(px(6.)).children(PRESETS.iter().enumerate().map(|(i, (name, preamp, bands))| {
                                let active = dsp.preset == *name;
                                let (preamp, bands) = (*preamp, *bands);
                                div()
                                    .id(("preset", i))
                                    .px(px(10.))
                                    .py(px(5.))
                                    .rounded_full()
                                    .border_1()
                                    .text_size(px(12.5))
                                    .cursor_pointer()
                                    .when(active, |el| el.bg(p.accent_soft).border_color(p.accent.opacity(0.5)).text_color(p.accent))
                                    .when(!active, |el| el.border_color(p.line).text_color(p.ink_2).hover(|s| s.text_color(p.ink).border_color(p.ink_3)))
                                    .child(*name)
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        let dsp = Dsp { eq: true, preamp_db: preamp, bands, preset: name.to_string(), ..this.settings.dsp.clone() };
                                        this.apply_dsp(dsp, window, cx);
                                    }))
                            })))
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
