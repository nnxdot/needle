//! "Play on": choose this computer or a speaker on the network.
use super::{AppView, Event, menus::Entry, motion, pal};
use gpui::{prelude::*, *};
use gpui_component::ActiveTheme;
use gpui_component::slider::{Slider, SliderEvent, SliderState};
use needle_core::{
    audio::Command,
    cast::{self, Group, Speaker},
};
use std::time::Duration;

pub struct SpeakerMenu {
    pub position: Point<Pixels>,
    /// Speakers found, once the search is done.
    pub found: Option<Vec<Speaker>>,
    pub serial: usize,
    /// Choosing several outputs to play together.
    pub picking: Option<Group>,
}

impl AppView {
    /// The speaker Needle plays on, if not this computer.
    pub(super) fn current_speaker(&self) -> Option<Speaker> {
        self.settings
            .output_device
            .as_deref()
            .and_then(Speaker::from_device_name)
    }

    pub(super) fn open_speaker_menu(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        self.menu = None;
        self.menu_serial += 1;
        self.speaker_menu = Some(SpeakerMenu {
            position,
            found: None,
            serial: self.menu_serial,
            picking: None,
        });
        let sender = self.sender.clone();
        std::thread::spawn(move || {
            let found = cast::discover::discover(Duration::from_secs(3));
            let _ = sender.send(Event::Speakers(found));
        });
        cx.notify();
    }

    pub(super) fn speakers_found(&mut self, found: Vec<Speaker>) {
        if let Some(menu) = &mut self.speaker_menu {
            menu.found = Some(found);
        }
    }

    /// The group Needle plays on, if several outputs play at once.
    pub(super) fn current_group(&self) -> Option<Group> {
        self.settings
            .output_device
            .as_deref()
            .and_then(Group::from_device_name)
    }

    fn play_on_group(&mut self, group: Group) {
        if group.speakers.is_empty() {
            return self.play_on(None);
        }
        if group.speakers.len() == 1 && !group.this_computer {
            return self.play_on(group.speakers.into_iter().next());
        }
        let name = group.name();
        self.settings.output_device = Some(group.device_name());
        self.settings.exclusive = false;
        self.configure();
        self.notify(format!(
            "Playing on {name}. It may take a few seconds to start. Line them up in Settings › Playback."
        ));
    }

    fn play_on(&mut self, speaker: Option<Speaker>) {
        let name = speaker.as_ref().map(|s| s.name.clone());
        self.settings.output_device = speaker.map(|s| s.device_name());
        if name.is_some() {
            // Speakers take a shared stream; exclusive output is for this computer's devices.
            self.settings.exclusive = false;
        }
        self.configure();
        self.notify(match name {
            Some(name) => format!("Playing on {name}. It may take a few seconds to start."),
            None => "Playing on this computer.".into(),
        });
    }

    /// A speaker that failed was dropped by the player; follow it.
    pub(super) fn follow_output(&mut self) {
        if (self.current_speaker().is_some() || self.current_group().is_some())
            && self.playback.output_device.is_none()
            && !self.playback.output.is_empty()
        {
            self.settings.output_device = None;
        }
    }

    pub(super) fn speaker_menu_view(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        let menu = self.speaker_menu.as_ref()?;
        let current = self.current_speaker();
        let group = self.current_group();
        let mut listed: Vec<Speaker> = current.iter().cloned().collect();
        listed.extend(group.iter().flat_map(|g| g.speakers.clone()));
        for speaker in menu.found.iter().flatten() {
            if !listed
                .iter()
                .any(|s| s.address == speaker.address && s.kind == speaker.kind)
            {
                listed.push(speaker.clone());
            }
        }
        if let Some(picking) = &menu.picking {
            let mut entries = vec![
                Entry::Label("Play together on".into()),
                Entry::item_keep(
                    if picking.this_computer {
                        "check"
                    } else {
                        "blank"
                    },
                    "This computer",
                    None,
                    |this, _, cx| {
                        if let Some(p) = this.speaker_menu.as_mut().and_then(|m| m.picking.as_mut())
                        {
                            p.this_computer = !p.this_computer;
                        }
                        cx.notify();
                    },
                ),
            ];
            for speaker in listed {
                let chosen = picking
                    .speakers
                    .iter()
                    .any(|s| s.address == speaker.address);
                let label = format!("{} · {}", speaker.name, speaker.kind.label());
                entries.push(Entry::item_keep(
                    if chosen { "check" } else { "blank" },
                    label,
                    None,
                    move |this, _, cx| {
                        if let Some(p) = this.speaker_menu.as_mut().and_then(|m| m.picking.as_mut())
                        {
                            if let Some(at) =
                                p.speakers.iter().position(|s| s.address == speaker.address)
                            {
                                p.speakers.remove(at);
                            } else {
                                p.speakers.push(speaker.clone());
                            }
                        }
                        cx.notify();
                    },
                ));
            }
            if menu.found.is_none() {
                entries.push(Entry::Label("Looking for speakers…".into()));
            }
            let count = picking.len();
            let chosen = picking.clone();
            if count >= 2 {
                entries.push(Entry::item(
                    "speaker",
                    format!("Play on these {count}"),
                    None,
                    move |this, _, _| {
                        this.speaker_menu = None;
                        this.play_on_group(chosen.clone());
                    },
                ));
            } else {
                entries.push(Entry::Label("Choose two or more.".into()));
            }
            entries.push(Entry::item_keep(
                "chevron-left",
                "Back",
                None,
                |this, _, cx| {
                    if let Some(menu) = this.speaker_menu.as_mut() {
                        menu.picking = None;
                    }
                    cx.notify();
                },
            ));
            return Some(
                self.speaker_menu_body(menu.position, menu.serial, entries, cx)
                    .into_any_element(),
            );
        }
        let mut entries = vec![
            Entry::Label("Play on".into()),
            Entry::item(
                if current.is_none() && group.is_none() {
                    "check"
                } else {
                    "blank"
                },
                "This computer",
                None,
                |this, _, _| {
                    this.speaker_menu = None;
                    this.play_on(None);
                },
            ),
        ];
        if let Some(group) = &group {
            entries.push(Entry::item("check", group.name(), None, |_, _, _| {}));
        }
        for speaker in listed {
            let on = current
                .as_ref()
                .is_some_and(|c| c.address == speaker.address && c.kind == speaker.kind);
            let label = format!("{} · {}", speaker.name, speaker.kind.label());
            entries.push(Entry::item(
                if on { "check" } else { "blank" },
                label,
                None,
                move |this, _, _| {
                    this.speaker_menu = None;
                    this.play_on(Some(speaker.clone()));
                },
            ));
        }
        match &menu.found {
            None => entries.push(Entry::Label("Looking for speakers…".into())),
            Some(found) if found.is_empty() => entries.push(Entry::Label(
                "No speakers answered. Needle looks for Chromecast, AirPlay, and DLNA speakers on this network.".into(),
            )),
            Some(_) => {}
        }
        let start = group.clone().unwrap_or_else(|| Group {
            speakers: current.iter().cloned().collect(),
            this_computer: current.is_none(),
            delays: Default::default(),
        });
        entries.push(Entry::item_keep(
            "speaker",
            "Play on several at once…",
            None,
            move |this, _, cx| {
                if let Some(menu) = this.speaker_menu.as_mut() {
                    menu.picking = Some(start.clone());
                }
                cx.notify();
            },
        ));
        Some(
            self.speaker_menu_body(menu.position, menu.serial, entries, cx)
                .into_any_element(),
        )
    }

    fn speaker_menu_body(
        &self,
        position: Point<Pixels>,
        serial: usize,
        entries: Vec<Entry>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let p = pal(cx);
        let rows = Self::menu_rows(&entries, None, cx);
        let body = div()
            .id("speaker-menu")
            .occlude()
            .w(px(280.))
            .p_1()
            .rounded(px(9.))
            .bg(cx.theme().popover)
            .border_1()
            .border_color(p.line)
            .shadow_lg()
            .flex()
            .flex_col()
            .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                this.speaker_menu = None;
                cx.notify();
            }))
            .children(rows);
        let body = motion::animate(body, ("speaker-menu-in", serial), 140, cx, |el, t| {
            el.opacity(t).mt(px(-6. * (1. - t)))
        });
        deferred(
            anchored()
                .position(position)
                .anchor(Corner::BottomRight)
                .snap_to_window_with_margin(px(8.))
                .child(body),
        )
        .with_priority(2)
    }

    /// Make timing sliders for the members of the group playing now.
    pub(super) fn sync_timing_sliders(&mut self, cx: &mut Context<Self>) {
        let Some(group) = self.current_group() else {
            self.speaker_timing.clear();
            return;
        };
        let members = group.members();
        self.speaker_timing
            .retain(|key, _| members.iter().any(|(k, _)| k == key));
        for (key, _) in members {
            if self.speaker_timing.contains_key(&key) {
                continue;
            }
            let value = *group.delays.get(&key).unwrap_or(&0) as f32;
            let slider = cx.new(|_| {
                SliderState::new()
                    .min(-1000.)
                    .max(1000.)
                    .step(10.)
                    .default_value(value)
            });
            let address = key.clone();
            let subscription = cx.subscribe(&slider, move |this, _, event, cx| {
                let SliderEvent::Change(value) = event;
                let Some(mut group) = this.current_group() else {
                    return;
                };
                group
                    .delays
                    .insert(address.clone(), value.start().round() as i32);
                group.delays.retain(|_, ms| *ms != 0);
                this.settings.output_device = Some(group.device_name());
                this.player
                    .send(Command::SpeakerDelays(group.delays.clone()));
                cx.notify();
            });
            self.speaker_timing.insert(key, (slider, subscription));
        }
    }

    /// Settings › Playback: line up the members of a group by ear.
    pub(super) fn speaker_timing_view(&self, cx: &mut Context<Self>) -> Option<Div> {
        let group = self.current_group()?;
        let p = pal(cx);
        Some(
            div()
                .mt_4()
                .flex()
                .flex_col()
                .gap_2()
                .child(super::widgets::strong("Speaker timing"))
                .child(super::widgets::meta(
                    "Needle lines the speakers up for you. If one still sounds early or late, move its slider: to the right plays it later.",
                    cx,
                ))
                .children(group.members().into_iter().filter_map(|(key, name)| {
                    let (slider, _) = self.speaker_timing.get(&key)?;
                    let ms = *group.delays.get(&key).unwrap_or(&0);
                    Some(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(div().w(px(160.)).text_size(px(13.)).text_color(p.ink_2).child(name))
                            .child(Slider::new(slider).flex_1())
                            .child(div().w(px(72.)).text_size(px(12.5)).child(format!("{ms:+} ms"))),
                    )
                })),
        )
    }
}
