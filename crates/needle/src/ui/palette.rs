//! The command palette (Ctrl+K): one place to go anywhere and do anything.
use super::{
    AppView, Event, Page, Panel, motion,
    pages::SETTINGS_TABS,
    pal,
    widgets::{artwork, glyph},
};
use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme, Sizable,
    input::{Enter, Escape, Input, InputEvent, InputState, MoveDown, MoveUp},
};
use needle_core::{audio::Command, browse::AlbumSummary, model::Track};
use std::rc::Rc;

type Run = Rc<dyn Fn(&mut AppView, &mut Window, &mut Context<AppView>)>;

#[derive(Clone)]
pub struct Item {
    pub group: &'static str,
    pub icon: &'static str,
    pub title: SharedString,
    pub detail: SharedString,
    pub hint: Option<&'static str>,
    pub track: Option<Track>,
    pub run: Run,
}

fn item(
    group: &'static str,
    icon: &'static str,
    title: impl Into<SharedString>,
    detail: impl Into<SharedString>,
    hint: Option<&'static str>,
    run: impl Fn(&mut AppView, &mut Window, &mut Context<AppView>) + 'static,
) -> Item {
    Item {
        group,
        icon,
        title: title.into(),
        detail: detail.into(),
        hint,
        track: None,
        run: Rc::new(run),
    }
}

pub struct PaletteState {
    pub open: bool,
    pub input: Entity<InputState>,
    pub active: usize,
    pub generation: u64,
    pub found: Vec<Item>,
    pub serial: usize,
    _subscription: Subscription,
}

impl PaletteState {
    pub fn new(window: &mut Window, cx: &mut Context<AppView>) -> Self {
        let input = cx
            .new(|cx| InputState::new(window, cx).placeholder("Go to, play, or change anything…"));
        let subscription = cx.subscribe(&input, |this: &mut AppView, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                this.palette.active = 0;
                this.palette_search(cx);
                cx.notify();
            }
        });
        Self {
            open: false,
            input,
            active: 0,
            generation: 0,
            found: vec![],
            serial: 0,
            _subscription: subscription,
        }
    }
}

fn matches(query: &[String], title: &str, detail: &str) -> Option<usize> {
    let haystack = format!("{} {}", title, detail).to_lowercase();
    if !query.iter().all(|w| haystack.contains(w.as_str())) {
        return None;
    }
    let title = title.to_lowercase();
    Some(match query.first() {
        Some(first) if title.starts_with(first.as_str()) => 0,
        Some(first)
            if title
                .split_whitespace()
                .any(|w| w.starts_with(first.as_str())) =>
        {
            1
        }
        _ => 2,
    })
}

impl AppView {
    pub(super) fn open_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.palette.open = true;
        self.palette.active = 0;
        self.palette.found.clear();
        self.palette.serial += 1;
        self.menu = None;
        self.palette.input.update(cx, |s, cx| {
            s.set_value("", window, cx);
            s.focus(window, cx);
        });
        cx.notify();
    }
    pub(super) fn close_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.palette.open = false;
        window.focus(&self.focus);
        cx.notify();
    }

    /// Everything the palette can do without searching the library.
    fn palette_commands(&self) -> Vec<Item> {
        let page =
            |icon: &'static str, title: &'static str, target: Page, hint: Option<&'static str>| {
                item("Go to", icon, title, "", hint, move |this, window, cx| {
                    this.navigate(target.clone(), window, cx)
                })
            };
        let mut items = vec![
            page("home", "Home", Page::Home, Some("Ctrl+1")),
            page("songs", "Songs", Page::Songs, Some("Ctrl+2")),
            page("albums", "Albums", Page::Albums, Some("Ctrl+3")),
            page("artists", "Artists", Page::Artists, Some("Ctrl+4")),
            page("folder", "Folders", Page::Folders, None),
            page("heart", "Favorites", Page::Favorites, Some("Ctrl+5")),
            page("recent", "Recently added", Page::Recent, Some("Ctrl+6")),
            page(
                "history",
                "Listening history",
                Page::History,
                Some("Ctrl+7"),
            ),
            page("eq", "Sound and equalizer", Page::Sound, None),
            page("import", "Import from other apps", Page::Import, None),
            page("wrench", "Fix my library", Page::Doctor, None),
        ];
        items.extend(self.playlists.iter().map(|p| {
            let target = Page::Playlist(p.id.clone());
            item(
                "Playlists",
                if p.query.is_some() {
                    "smart"
                } else {
                    "playlist"
                },
                p.name.clone(),
                if p.query.is_some() {
                    "Smart playlist"
                } else {
                    "Playlist"
                },
                None,
                move |this, window, cx| this.navigate(target.clone(), window, cx),
            )
        }));
        items.extend(SETTINGS_TABS.iter().enumerate().map(|(i, (name, icon))| {
            item(
                "Settings",
                icon,
                format!("Settings › {name}"),
                "",
                if i == 0 { Some("Ctrl+,") } else { None },
                move |this, window, cx| {
                    this.settings_tab = i;
                    this.navigate(Page::Settings, window, cx);
                },
            )
        }));
        let playing = self.playback.playing;
        if self.tray.is_some() {
            items.push(item(
                "Actions",
                "mini",
                "Hide to tray",
                "The music plays on",
                None,
                |this, window, _| this.hide_to_tray(window),
            ));
        }
        items.extend([
            item(
                "Actions",
                if playing { "pause" } else { "play" },
                if playing { "Pause" } else { "Play" },
                "",
                Some("Space"),
                |this, _, cx| this.toggle_playback(cx),
            ),
            item(
                "Actions",
                "next",
                "Next track",
                "",
                Some("Ctrl+→"),
                |this, _, _| this.player.send(Command::Next),
            ),
            item(
                "Actions",
                "previous",
                "Previous track",
                "",
                Some("Ctrl+←"),
                |this, _, _| this.player.send(Command::Previous),
            ),
            item(
                "Actions",
                "shuffle",
                "Shuffle this view",
                "",
                None,
                |this, _, cx| this.play_view(0, true, cx),
            ),
            item(
                "Actions",
                "expand",
                "Open the big player",
                "",
                Some("Ctrl+P"),
                |this, _, cx| {
                    this.big = true;
                    cx.notify();
                },
            ),
            item(
                "Go to",
                "history",
                "Your year in music",
                "Your top artists, songs, and albums of the year",
                None,
                |this, window, cx| this.open_wrapped(window, cx),
            ),
            item(
                "Actions",
                "radio",
                "Start radio from this song",
                "Songs that sound like the one playing",
                None,
                |this, _, _| {
                    if let Some(item) = this.playback.current.clone() {
                        this.start_radio(item.track);
                    }
                },
            ),
            item(
                "Actions",
                "lyrics",
                "Time the lyrics of this song",
                "Tap along to time lines or words",
                None,
                |this, window, cx| this.open_timing(window, cx),
            ),
            item(
                "Actions",
                "mini",
                "Switch to the mini player",
                "",
                Some("Ctrl+M"),
                |this, window, cx| this.open_mini(window, cx),
            ),
            item(
                "Actions",
                "queue",
                "Show the queue",
                "",
                Some("Ctrl+J"),
                |this, _, _| {
                    this.settings.show_inspector = true;
                    this.panel = Panel::Queue;
                },
            ),
            item(
                "Actions",
                "close",
                "Clear the queue",
                "",
                None,
                |this, _, _| this.player.send(Command::ClearQueue),
            ),
            item(
                "Actions",
                "folder",
                "Add a music folder",
                "",
                Some("Ctrl+O"),
                |this, _, cx| this.import_folder(cx),
            ),
            item(
                "Actions",
                "loop",
                "Check folders for changes",
                "",
                None,
                |this, _, cx| this.rescan(cx),
            ),
            item(
                "Actions",
                "palette",
                "Switch the look: Night, Midnight, Day, and your themes",
                "",
                None,
                |this, window, cx| {
                    let modes = this.look_modes(cx);
                    let at = modes.iter().position(|m| *m == this.settings.theme);
                    let next = modes[at.map_or(0, |i| (i + 1) % modes.len())].clone();
                    this.choose_look(&next, window, cx);
                },
            ),
            item(
                "Actions",
                "eq",
                if self.settings.dsp.eq {
                    "Turn the equalizer off"
                } else {
                    "Turn the equalizer on"
                },
                "",
                None,
                |this, _, _| {
                    this.settings.dsp.eq = !this.settings.dsp.eq;
                    this.player.send(Command::Dsp(this.settings.dsp.clone()));
                },
            ),
        ]);
        if let Some(current) = self.playback.current.as_ref().map(|c| c.track.clone()) {
            let (album, artist, stems) = (
                super::album_page(&current),
                Page::Artist(current.artist.clone()),
                current.clone(),
            );
            items.extend([
                item(
                    "Now playing",
                    "albums",
                    "Go to the album playing",
                    current.album.clone(),
                    None,
                    move |this, window, cx| this.navigate(album.clone(), window, cx),
                ),
                item(
                    "Now playing",
                    "artists",
                    "Go to the artist playing",
                    current.artist.clone(),
                    None,
                    move |this, window, cx| this.navigate(artist.clone(), window, cx),
                ),
                item(
                    "Now playing",
                    "stems",
                    "Split the song playing into stems",
                    current.title.clone(),
                    None,
                    move |this, _, cx| {
                        this.big = true;
                        this.big_side = super::now_playing::Side::Stems;
                        this.separate(stems.clone(), cx);
                    },
                ),
            ]);
        }
        items.extend(
            self.plugins
                .commands()
                .into_iter()
                .filter(|c| !c.for_tracks)
                .map(|c| {
                    let (plugin, id) = (c.plugin.clone(), c.id.clone());
                    item("Plugins", "plugin", c.title, "", None, move |this, _, _| {
                        this.run_plugin_command(plugin.clone(), id.clone(), vec![])
                    })
                }),
        );
        items
    }

    /// Look up songs, albums, and artists in the background for the current text.
    pub(super) fn palette_search(&mut self, cx: &mut Context<Self>) {
        let text = self.palette.input.read(cx).value().trim().to_string();
        self.palette.generation += 1;
        self.palette.found.clear();
        if text.chars().count() < 2 {
            return;
        }
        let generation = self.palette.generation;
        let library = self.library.clone();
        let sender = self.sender.clone();
        std::thread::spawn(move || {
            let quoted = needle_core::query::quote(&text);
            let songs = library
                .search_page(&text, 0, 6)
                .map(|p| p.tracks)
                .unwrap_or_default();
            let albums = library
                .albums(&format!("album contains {quoted}"))
                .unwrap_or_default()
                .into_iter()
                .take(4)
                .collect();
            let artists = library
                .artists(&format!("artist contains {quoted}"))
                .unwrap_or_default()
                .into_iter()
                .take(4)
                .map(|a| a.name)
                .collect();
            let _ = sender.send(Event::PaletteFound(generation, songs, albums, artists));
        });
    }

    pub(super) fn palette_found(
        &mut self,
        generation: u64,
        songs: Vec<Track>,
        albums: Vec<AlbumSummary>,
        artists: Vec<String>,
    ) {
        if generation != self.palette.generation {
            return;
        }
        let mut found: Vec<Item> = songs
            .into_iter()
            .map(|track| {
                let play = track.clone();
                let mut entry = item(
                    "Songs",
                    "songs",
                    track.title.clone(),
                    format!("{} · {}", track.display_artist(), track.display_album()),
                    Some("Enter plays"),
                    move |this, _, _| {
                        this.play_tracks(vec![play.clone()], "Chosen from the command palette")
                    },
                );
                entry.track = Some(track);
                entry
            })
            .collect();
        found.extend(albums.into_iter().map(|album| {
            let target = Page::Album {
                album: album.album.clone(),
                artist: album.artist.clone(),
                query: album.query.clone(),
            };
            item(
                "Albums",
                "albums",
                if album.album.is_empty() {
                    "Unknown album".to_string()
                } else {
                    album.album.clone()
                },
                album.artist.clone(),
                None,
                move |this, window, cx| this.navigate(target.clone(), window, cx),
            )
        }));
        found.extend(artists.into_iter().map(|name| {
            let target = Page::Artist(name.clone());
            item(
                "Artists",
                "artists",
                name,
                "",
                None,
                move |this, window, cx| this.navigate(target.clone(), window, cx),
            )
        }));
        self.palette.found = found;
    }

    fn palette_items(&self, cx: &App) -> Vec<Item> {
        let text = self.palette.input.read(cx).value().to_lowercase();
        let words: Vec<String> = text.split_whitespace().map(String::from).collect();
        let commands = self.palette_commands();
        if words.is_empty() {
            // A short, useful start: pages, then the most common actions.
            return commands
                .into_iter()
                .filter(|i| matches!(i.group, "Go to" | "Actions" | "Now playing"))
                .take(14)
                .collect();
        }
        let mut scored: Vec<(usize, Item)> = commands
            .into_iter()
            .filter_map(|i| matches(&words, &i.title, &i.detail).map(|s| (s, i)))
            .collect();
        scored.sort_by_key(|(s, _)| *s);
        let mut items: Vec<Item> = scored.into_iter().map(|(_, i)| i).take(12).collect();
        items.extend(self.palette.found.iter().cloned());
        items
    }

    pub(super) fn palette_keys(
        &self,
        field: Stateful<Div>,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        field
            .capture_action(cx.listener(|this, _: &MoveDown, _, cx| {
                let n = this.palette_items(cx).len().max(1);
                this.palette.active = (this.palette.active + 1) % n;
                cx.stop_propagation();
                cx.notify();
            }))
            .capture_action(cx.listener(|this, _: &MoveUp, _, cx| {
                let n = this.palette_items(cx).len().max(1);
                this.palette.active = (this.palette.active + n - 1) % n;
                cx.stop_propagation();
                cx.notify();
            }))
            .capture_action(cx.listener(|this, _: &Enter, window, cx| {
                cx.stop_propagation();
                let items = this.palette_items(cx);
                if let Some(chosen) = items
                    .get(this.palette.active.min(items.len().saturating_sub(1)))
                    .cloned()
                {
                    this.close_palette(window, cx);
                    (chosen.run)(this, window, cx);
                    cx.notify();
                }
            }))
            .capture_action(cx.listener(|this, _: &Escape, window, cx| {
                cx.stop_propagation();
                this.close_palette(window, cx);
            }))
    }

    pub(super) fn palette_view(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        if !self.palette.open {
            return None;
        }
        let p = pal(cx);
        let items = self.palette_items(cx);
        let active = self.palette.active.min(items.len().saturating_sub(1));
        let mut last_group = "";
        let mut rows: Vec<AnyElement> = vec![];
        for (i, entry) in items.iter().enumerate() {
            if entry.group != last_group {
                last_group = entry.group;
                rows.push(
                    div()
                        .px_3()
                        .pt_2()
                        .pb_1()
                        .text_size(px(11.5))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(p.ink_3)
                        .child(entry.group)
                        .into_any_element(),
                );
            }
            let chosen = entry.clone();
            let is_active = i == active;
            rows.push(
                div()
                    .id(("palette-row", i))
                    .h(px(40.))
                    .mx_1()
                    .px_2()
                    .rounded(px(7.))
                    .flex()
                    .items_center()
                    .gap_3()
                    .cursor_pointer()
                    .when(is_active, |el| el.bg(p.raised_hover))
                    .hover(|s| s.bg(p.raised))
                    .child(match &entry.track {
                        Some(track) => artwork(Some(track), 28., cx),
                        None => div()
                            .size(px(28.))
                            .rounded(px(6.))
                            .bg(p.raised)
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(glyph(entry.icon).size(px(15.)).text_color(if is_active {
                                p.accent
                            } else {
                                p.ink_2
                            }))
                            .into_any_element(),
                    })
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .text_size(px(13.5))
                                    .truncate()
                                    .child(entry.title.clone()),
                            )
                            .when(!entry.detail.is_empty(), |el| {
                                el.child(
                                    div()
                                        .text_size(px(12.))
                                        .text_color(p.ink_3)
                                        .truncate()
                                        .child(entry.detail.clone()),
                                )
                            }),
                    )
                    .when_some(entry.hint, |el, hint| {
                        el.child(div().text_size(px(11.)).text_color(p.ink_3).child(hint))
                    })
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.close_palette(window, cx);
                        (chosen.run)(this, window, cx);
                        cx.notify();
                    }))
                    .into_any_element(),
            );
        }
        let searching = self.palette.input.read(cx).value().trim().chars().count() >= 2;
        let panel = div()
            .id("palette")
            .occlude()
            .w(px(640.))
            .rounded(px(12.))
            .bg(cx.theme().popover)
            .border_1()
            .border_color(p.line)
            .shadow_lg()
            .flex()
            .flex_col()
            .overflow_hidden()
            .child(
                self.palette_keys(div().id("palette-field"), cx)
                    .h(px(52.))
                    .px_4()
                    .flex()
                    .items_center()
                    .gap_3()
                    .border_b_1()
                    .border_color(p.line)
                    .child(glyph("search").size(px(17.)).text_color(p.ink_3))
                    .child(
                        Input::new(&self.palette.input)
                            .appearance(false)
                            .flex_1()
                            .large(),
                    ),
            )
            .child(
                div()
                    .id("palette-list")
                    .max_h(px(432.))
                    .overflow_y_scroll()
                    .py_1()
                    .children(rows)
                    .when(items.is_empty(), |el| {
                        el.child(div().p_4().text_size(px(13.)).text_color(p.ink_2).child(
                            if searching {
                                "Nothing found yet…"
                            } else {
                                "Type to search."
                            },
                        ))
                    }),
            )
            .child(
                div()
                    .h(px(34.))
                    .px_4()
                    .flex()
                    .items_center()
                    .gap_4()
                    .border_t_1()
                    .border_color(p.line_soft)
                    .text_size(px(11.5))
                    .text_color(p.ink_3)
                    .child("↑ ↓ move")
                    .child("Enter open")
                    .child("Esc close"),
            );
        let panel = motion::animate(
            panel,
            ("palette-in", self.palette.serial),
            160,
            cx,
            |el, t| el.opacity(t).mt(px(10. * (1. - t))),
        );
        Some(
            deferred(
                div()
                    .id("palette-backdrop")
                    .absolute()
                    .inset_0()
                    .occlude()
                    .bg(gpui::black().opacity(if p.dark { 0.45 } else { 0.25 }))
                    .flex()
                    .justify_center()
                    // Don't stretch the panel to the window's height; it is as tall as its list.
                    .items_start()
                    .pt(px(84.))
                    .child(panel)
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, window, cx| this.close_palette(window, cx)),
                    ),
            )
            .with_priority(3),
        )
    }
}
