//! Offline radio in the interface: measuring songs in the background, stations from a song or
//! an artist, and blends of two artists.
use super::{AppView, Event, menus::Entry, motion, pal};
use gpui::{prelude::*, *};
use gpui_component::ActiveTheme;
use needle_core::{
    audio::QueueItem,
    radio::{self, Features},
};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

/// How many songs a station queues.
const STATION: usize = 50;

pub struct BlendMenu {
    pub position: Point<Pixels>,
    pub artist: String,
    /// The nearest-sounding artists, once known.
    pub choices: Option<Vec<String>>,
    pub serial: usize,
}

impl AppView {
    /// Measure unmeasured songs, one at a time and gently, while the setting is on.
    pub(super) fn start_measuring(&mut self) {
        let library = self.library.clone();
        let sender = self.sender.clone();
        let on = self.measuring.clone();
        std::thread::spawn(move || {
            loop {
                if !on.load(Ordering::Relaxed) {
                    std::thread::sleep(Duration::from_secs(5));
                    continue;
                }
                let batch = library.unmeasured(8).unwrap_or_default();
                if batch.is_empty() {
                    let _ = sender.send(Event::Measured(
                        library.measured_count().unwrap_or_default(),
                    ));
                    std::thread::sleep(Duration::from_secs(60));
                    continue;
                }
                for track in batch {
                    if !on.load(Ordering::Relaxed) {
                        break;
                    }
                    let features = radio::analyze(&track).unwrap_or(Features {
                        version: radio::VERSION,
                        ..Default::default()
                    });
                    let _ = library.save_features(&track.id, &features);
                    std::thread::sleep(Duration::from_millis(40));
                }
                let _ = sender.send(Event::Measured(
                    library.measured_count().unwrap_or_default(),
                ));
            }
        });
    }

    pub(super) fn set_measuring(&mut self, on: bool) {
        self.settings.sound_analysis = on;
        self.measuring.store(on, Ordering::Relaxed);
        self.persist_settings();
    }

    /// A song's sound, from the library (cached for drawing).
    pub(super) fn sound_of(&self, id: &str) -> Option<Features> {
        if let Some(found) = self.sound_cache.borrow().get(id) {
            return found.clone();
        }
        let found = self
            .library
            .features(id)
            .ok()
            .flatten()
            .filter(|f| f.usable());
        self.sound_cache
            .borrow_mut()
            .insert(id.to_string(), found.clone());
        found
    }

    fn play_station(
        &mut self,
        work: impl FnOnce(&needle_core::database::Library) -> anyhow::Result<(Vec<QueueItem>, String)>
        + Send
        + 'static,
    ) {
        let library = self.library.clone();
        let sender = self.sender.clone();
        self.notify("Tuning in…");
        std::thread::spawn(move || {
            let _ = sender.send(match work(&library) {
                Ok((items, _)) if items.len() < 2 => Event::Error(
                    "Not enough songs are measured yet to make a station. Needle measures them in the background; try again soon.".into(),
                ),
                Ok((items, notice)) => Event::Play(items, Some(notice)),
                Err(e) => Event::Error(format!("{e:#}")),
            });
        });
    }

    /// Play this song, then songs that sound like it.
    pub(super) fn start_radio(&mut self, track: needle_core::model::Track) {
        self.play_station(move |library| {
            let station = library.radio(std::slice::from_ref(&track), STATION - 1)?;
            let reason = format!("Radio: like “{}”", track.title);
            let mut items = vec![QueueItem {
                track: track.clone(),
                reason: reason.clone(),
            }];
            items.extend(station.into_iter().map(|t| QueueItem {
                track: t,
                reason: reason.clone(),
            }));
            let count = items.len();
            Ok((
                items,
                format!("Radio from “{}”: {count} songs.", track.title),
            ))
        });
    }

    pub(super) fn start_artist_radio(&mut self, artist: String) {
        self.play_station(move |library| {
            let station = library.artist_radio(&artist, STATION)?;
            let reason = format!("Radio: {artist}");
            let count = station.len();
            let items = station
                .into_iter()
                .map(|t| QueueItem {
                    track: t,
                    reason: reason.clone(),
                })
                .collect();
            Ok((items, format!("{artist} radio: {count} songs.")))
        });
    }

    fn start_blend(&mut self, first: String, second: String) {
        self.play_station(move |library| {
            let station = library.blend(&first, &second, STATION)?;
            let reason = format!("Blend: {first} × {second}");
            let count = station.len();
            let items = station
                .into_iter()
                .map(|t| QueueItem {
                    track: t,
                    reason: reason.clone(),
                })
                .collect();
            Ok((items, format!("{first} × {second}: {count} songs.")))
        });
    }

    /// "Blend with…": the artists that sound nearest, found in the background.
    pub(super) fn open_blend_menu(
        &mut self,
        artist: String,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        self.menu = None;
        self.menu_serial += 1;
        self.blend_menu = Some(BlendMenu {
            position,
            artist: artist.clone(),
            choices: None,
            serial: self.menu_serial,
        });
        let library = self.library.clone();
        let sender = self.sender.clone();
        std::thread::spawn(move || {
            let choices = library.similar_artists(&artist, 10).unwrap_or_default();
            let _ = sender.send(Event::BlendChoices(artist, choices));
        });
        cx.notify();
    }

    pub(super) fn blend_choices(&mut self, artist: String, choices: Vec<String>) {
        if let Some(menu) = &mut self.blend_menu
            && menu.artist == artist
        {
            menu.choices = Some(choices);
        }
    }

    pub(super) fn blend_menu_view(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        let menu = self.blend_menu.as_ref()?;
        let p = pal(cx);
        let mut entries = vec![Entry::Label(format!("Blend {} with", menu.artist).into())];
        match &menu.choices {
            None => entries.push(Entry::Label("Listening for similar artists…".into())),
            Some(list) if list.is_empty() => {
                entries.push(Entry::Label("No other artists are measured yet.".into()))
            }
            Some(list) => entries.extend(list.iter().map(|other| {
                let (first, second) = (menu.artist.clone(), other.clone());
                Entry::item("artists", other.clone(), None, move |this, _, _| {
                    this.blend_menu = None;
                    this.start_blend(first.clone(), second.clone());
                })
            })),
        }
        let rows = Self::menu_rows(&entries, None, cx);
        let body = div()
            .id("blend-menu")
            .occlude()
            .w(px(240.))
            .p_1()
            .rounded(px(9.))
            .bg(cx.theme().popover)
            .border_1()
            .border_color(p.line)
            .shadow_lg()
            .flex()
            .flex_col()
            .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                this.blend_menu = None;
                cx.notify();
            }))
            .children(rows);
        let body = motion::animate(body, ("blend-menu-in", menu.serial), 140, cx, |el, t| {
            el.opacity(t).mt(px(6. * (1. - t)))
        });
        Some(
            deferred(
                anchored()
                    .position(menu.position)
                    .snap_to_window_with_margin(px(8.))
                    .child(body),
            )
            .with_priority(2),
        )
    }
}

/// Shared switch for the measuring thread.
pub fn switch(on: bool) -> Arc<AtomicBool> {
    Arc::new(AtomicBool::new(on))
}
