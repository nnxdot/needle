//! "Play on": choose this computer or a speaker on the network.
use super::{AppView, Event, menus::Entry, motion, pal};
use gpui::{prelude::*, *};
use gpui_component::ActiveTheme;
use needle_core::cast::{self, Speaker};
use std::time::Duration;

pub struct SpeakerMenu {
    pub position: Point<Pixels>,
    /// Speakers found, once the search is done.
    pub found: Option<Vec<Speaker>>,
    pub serial: usize,
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
        if self.current_speaker().is_some()
            && self.playback.output_device.is_none()
            && !self.playback.output.is_empty()
        {
            self.settings.output_device = None;
        }
    }

    pub(super) fn speaker_menu_view(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        let menu = self.speaker_menu.as_ref()?;
        let p = pal(cx);
        let current = self.current_speaker();
        let mut entries = vec![
            Entry::Label("Play on".into()),
            Entry::item(
                if current.is_none() { "check" } else { "blank" },
                "This computer",
                None,
                |this, _, _| {
                    this.speaker_menu = None;
                    this.play_on(None);
                },
            ),
        ];
        let mut listed: Vec<Speaker> = current.iter().cloned().collect();
        for speaker in menu.found.iter().flatten() {
            if !listed
                .iter()
                .any(|s| s.address == speaker.address && s.kind == speaker.kind)
            {
                listed.push(speaker.clone());
            }
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
        let body = motion::animate(body, ("speaker-menu-in", menu.serial), 140, cx, |el, t| {
            el.opacity(t).mt(px(-6. * (1. - t)))
        });
        Some(
            deferred(
                anchored()
                    .position(menu.position)
                    .anchor(Corner::BottomRight)
                    .snap_to_window_with_margin(px(8.))
                    .child(body),
            )
            .with_priority(2),
        )
    }
}
