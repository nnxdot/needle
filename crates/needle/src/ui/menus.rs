//! One menu style for the whole app: icon, label, shortcut hint, and "›" submenus that open
//! in place. Menus work with the mouse and with ↑ ↓ Enter → ← Esc.
use super::{AppView, Page, motion, pal, widgets::glyph};
use gpui::{prelude::*, *};
use gpui_component::ActiveTheme;
use std::rc::Rc;

type Action = Rc<dyn Fn(&mut AppView, &mut Window, &mut Context<AppView>)>;

#[derive(Clone)]
pub enum Entry {
    Item {
        icon: &'static str,
        label: SharedString,
        hint: Option<&'static str>,
        action: Action,
    },
    Sub {
        icon: &'static str,
        label: SharedString,
        key: &'static str,
    },
    Label(SharedString),
    Separator,
}
impl Entry {
    fn item(
        icon: &'static str,
        label: impl Into<SharedString>,
        hint: Option<&'static str>,
        action: impl Fn(&mut AppView, &mut Window, &mut Context<AppView>) + 'static,
    ) -> Self {
        Self::Item {
            icon,
            label: label.into(),
            hint,
            action: Rc::new(action),
        }
    }
    fn selectable(&self) -> bool {
        matches!(self, Self::Item { .. } | Self::Sub { .. })
    }
}

pub struct TrackMenu {
    pub position: Point<Pixels>,
    pub index: usize,
    /// The submenu being shown, if any.
    pub sub: Option<&'static str>,
    /// Keyboard highlight among selectable entries.
    pub highlight: Option<usize>,
    /// Changes whenever the menu or submenu opens, to replay the entrance animation.
    pub serial: usize,
}

impl AppView {
    pub(super) fn open_menu(
        &mut self,
        index: usize,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        if let Some(track) = self.tracks.get(index)
            && !self.selection.ids.contains(&track.id)
        {
            self.select_single(index, cx);
        }
        self.menu_serial += 1;
        self.menu = Some(TrackMenu {
            position,
            index,
            sub: None,
            highlight: None,
            serial: self.menu_serial,
        });
        cx.notify();
    }

    fn menu_entries(&self) -> Vec<Entry> {
        let Some(menu) = &self.menu else {
            return vec![];
        };
        let Some(track) = self.tracks.get(menu.index).cloned() else {
            return vec![];
        };
        let selected = self.selected_tracks();
        let count = selected.len();
        let many = count > 1;
        let index = menu.index;
        match menu.sub {
            Some("playlists") => {
                let mut entries = vec![Entry::item(
                    "plus",
                    "New playlist from selection…",
                    None,
                    |this, window, cx| {
                        this.show_save = true;
                        this.playlist_name.update(cx, |s, cx| {
                            s.set_value("", window, cx);
                            s.focus(window, cx);
                        });
                    },
                )];
                let playlists: Vec<_> = self
                    .playlists
                    .iter()
                    .filter(|p| p.query.is_none())
                    .cloned()
                    .collect();
                if !playlists.is_empty() {
                    entries.push(Entry::Separator);
                }
                entries.extend(playlists.into_iter().map(|playlist| {
                    let id = playlist.id.clone();
                    Entry::item("playlist", playlist.name, None, move |this, _, _| {
                        let selected = this.selected_tracks();
                        this.add_to_playlist(&id, selected);
                    })
                }));
                entries
            }
            Some("plugins") => self
                .plugins
                .commands()
                .into_iter()
                .filter(|c| c.for_tracks)
                .map(|command| {
                    let (plugin, id) = (command.plugin.clone(), command.id.clone());
                    Entry::item("plugin", command.title, None, move |this, _, _| {
                        let ids = this.selected_tracks().into_iter().map(|t| t.id).collect();
                        this.run_plugin_command(plugin.clone(), id.clone(), ids);
                    })
                })
                .collect(),
            _ => {
                let favorite = selected.iter().all(|t| t.rating >= 4);
                let mut entries = vec![
                    Entry::item(
                        "play",
                        if many {
                            format!("Play {count} tracks")
                        } else {
                            "Play".into()
                        },
                        Some("Enter"),
                        move |this, _, cx| {
                            let selected = this.selected_tracks();
                            if selected.len() > 1 {
                                let reason = this.reason(cx);
                                this.play_tracks(selected, &reason);
                            } else {
                                this.play_view(index, false, cx);
                            }
                        },
                    ),
                    Entry::item(
                        "next",
                        if many {
                            format!("Play {count} next")
                        } else {
                            "Play next".into()
                        },
                        Some("Shift+Enter"),
                        |this, _, _| {
                            let selected = this.selected_tracks();
                            this.play_next(selected);
                        },
                    ),
                    Entry::item(
                        "queue",
                        if many {
                            format!("Add {count} to queue")
                        } else {
                            "Add to queue".into()
                        },
                        Some("Ctrl+Enter"),
                        |this, _, _| {
                            let selected = this.selected_tracks();
                            this.enqueue(selected);
                        },
                    ),
                    Entry::Separator,
                    Entry::item(
                        if favorite { "heart-fill" } else { "heart" },
                        if favorite {
                            "Remove from favorites"
                        } else {
                            "Add to favorites"
                        },
                        Some("Ctrl+D"),
                        move |this, _, _| {
                            let ids: Vec<String> =
                                this.selected_tracks().into_iter().map(|t| t.id).collect();
                            this.set_rating(&ids, if favorite { 0 } else { 5 });
                        },
                    ),
                    Entry::Sub {
                        icon: "playlist",
                        label: "Add to playlist".into(),
                        key: "playlists",
                    },
                ];
                if self.plugins.commands().iter().any(|c| c.for_tracks) {
                    entries.push(Entry::Sub {
                        icon: "plugin",
                        label: "Plugins".into(),
                        key: "plugins",
                    });
                }
                if !many {
                    let album = super::album_page(&track);
                    let artist = Page::Artist(track.artist.clone());
                    let stem_track = track.clone();
                    let path = track.path.trim_start_matches("\\\\?\\").to_string();
                    let copy = path.clone();
                    entries.extend([
                        Entry::Separator,
                        Entry::item("albums", "Go to album", None, move |this, window, cx| {
                            this.navigate(album.clone(), window, cx)
                        }),
                        Entry::item("artists", "Go to artist", None, move |this, window, cx| {
                            this.navigate(artist.clone(), window, cx)
                        }),
                        Entry::item(
                            "edit",
                            "Edit tags…",
                            Some("Ctrl+E"),
                            |this, window, cx| this.edit_tags(window, cx),
                        ),
                        Entry::item("stems", "Split into stems", None, move |this, _, cx| {
                            this.settings.show_inspector = true;
                            this.panel = super::Panel::Details;
                            this.separate(stem_track.clone(), cx);
                        }),
                        Entry::Separator,
                        Entry::item("folder", "Show in File Explorer", None, move |_, _, _| {
                            let _ = std::process::Command::new("explorer")
                                .arg(format!("/select,{path}"))
                                .spawn();
                        }),
                        Entry::item("copy", "Copy file path", None, move |_, _, cx| {
                            cx.write_to_clipboard(ClipboardItem::new_string(copy.clone()))
                        }),
                    ]);
                } else {
                    entries.extend([
                        Entry::Separator,
                        Entry::Label(format!("{count} tracks selected").into()),
                    ]);
                }
                entries
            }
        }
    }

    fn activate(&mut self, entry: Entry, window: &mut Window, cx: &mut Context<Self>) {
        match entry {
            Entry::Item { action, .. } => {
                self.menu = None;
                self.playlist_menu = None;
                action(self, window, cx);
            }
            Entry::Sub { key, .. } => {
                self.menu_serial += 1;
                if let Some(menu) = self.menu.as_mut() {
                    menu.sub = Some(key);
                    menu.highlight = Some(0);
                    menu.serial = self.menu_serial;
                }
            }
            _ => {}
        }
        cx.notify();
    }

    /// Keyboard handling while a menu is open. Returns false when no menu is open.
    pub(super) fn menu_key(
        &mut self,
        key: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.menu.is_none() {
            return false;
        }
        let entries = self.menu_entries();
        let selectable: Vec<usize> = entries
            .iter()
            .enumerate()
            .filter(|(_, e)| e.selectable())
            .map(|(i, _)| i)
            .collect();
        let Some(menu) = self.menu.as_mut() else {
            return false;
        };
        let n = selectable.len();
        match key {
            "down" if n > 0 => menu.highlight = Some(menu.highlight.map_or(0, |h| (h + 1) % n)),
            "up" if n > 0 => {
                menu.highlight = Some(menu.highlight.map_or(n - 1, |h| (h + n - 1) % n))
            }
            "left" if menu.sub.is_some() => {
                menu.sub = None;
                menu.highlight = Some(0);
            }
            "enter" | "right" => {
                if let Some(entry) = menu
                    .highlight
                    .and_then(|h| selectable.get(h))
                    .map(|i| entries[*i].clone())
                    && (key == "enter" || matches!(entry, Entry::Sub { .. }))
                {
                    self.activate(entry, window, cx);
                }
            }
            _ => {}
        }
        cx.notify();
        true
    }

    /// The rows of a menu, in the one menu style: icon, label, shortcut hint, and "›".
    fn menu_rows(
        entries: &[Entry],
        highlight: Option<usize>,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let p = pal(cx);
        let mut position_in_selectable = 0usize;
        entries
            .iter()
            .enumerate()
            .map(|(i, entry)| match entry {
                Entry::Separator => div().my_1().mx_1().h(px(1.)).bg(p.line).into_any_element(),
                Entry::Label(text) => div()
                    .px_2()
                    .py_1()
                    .text_size(px(12.))
                    .text_color(p.ink_3)
                    .child(text.clone())
                    .into_any_element(),
                entry => {
                    let (icon, label, hint, sub) = match entry {
                        Entry::Item {
                            icon, label, hint, ..
                        } => (*icon, label.clone(), *hint, false),
                        Entry::Sub { icon, label, .. } => (*icon, label.clone(), None, true),
                        _ => unreachable!(),
                    };
                    let active = highlight == Some(position_in_selectable);
                    position_in_selectable += 1;
                    let entry = entry.clone();
                    div()
                        .id(("menu-row", i))
                        .h(px(30.))
                        .px_2()
                        .rounded(px(5.))
                        .flex()
                        .items_center()
                        .gap_2()
                        .cursor_pointer()
                        .text_size(px(13.))
                        .when(active, |el| el.bg(p.raised_hover))
                        .hover(|s| s.bg(p.raised_hover))
                        .child(glyph(icon).size(px(15.)).text_color(if active {
                            p.ink
                        } else {
                            p.ink_2
                        }))
                        .child(div().flex_1().truncate().child(label))
                        .when_some(hint, |el, hint| {
                            el.child(div().text_size(px(11.)).text_color(p.ink_3).child(hint))
                        })
                        .when(sub, |el| {
                            el.child(glyph("chevron-right").size(px(14.)).text_color(p.ink_3))
                        })
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.activate(entry.clone(), window, cx)
                        }))
                        .into_any_element()
                }
            })
            .collect()
    }

    pub(super) fn track_menu(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        let menu = self.menu.as_ref()?;
        self.tracks.get(menu.index)?;
        let p = pal(cx);
        let entries = self.menu_entries();
        let highlight = menu.highlight;
        let rows = Self::menu_rows(&entries, highlight, cx);
        let title = match menu.sub {
            Some("playlists") => Some("Add to playlist"),
            Some("plugins") => Some("Plugins"),
            _ => None,
        };
        let body = div()
            .id("track-menu")
            .occlude()
            .w(px(252.))
            .p_1()
            .rounded(px(9.))
            .bg(cx.theme().popover)
            .border_1()
            .border_color(p.line)
            .shadow_lg()
            .flex()
            .flex_col()
            .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                this.menu = None;
                cx.notify();
            }))
            .when_some(title, |el, title| {
                el.child(
                    div()
                        .id("menu-back")
                        .h(px(30.))
                        .px_2()
                        .rounded(px(5.))
                        .flex()
                        .items_center()
                        .gap_2()
                        .cursor_pointer()
                        .text_size(px(12.5))
                        .font_weight(FontWeight::MEDIUM)
                        .hover(|s| s.bg(p.raised_hover))
                        .child(glyph("chevron-left").size(px(14.)).text_color(p.ink_2))
                        .child(title)
                        .on_click(cx.listener(|this, _, _, cx| {
                            if let Some(menu) = this.menu.as_mut() {
                                menu.sub = None;
                                menu.highlight = None;
                            }
                            cx.notify();
                        })),
                )
                .child(div().my_1().mx_1().h(px(1.)).bg(p.line))
            })
            .children(rows);
        let body = motion::animate(body, ("menu-in", menu.serial), 140, cx, |el, t| {
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

/// The right-click menu of a playlist in the sidebar.
pub struct PlaylistMenu {
    pub position: Point<Pixels>,
    pub id: String,
    pub serial: usize,
    /// Delete was chosen once; the next choice deletes.
    pub confirm: bool,
}

impl AppView {
    pub(super) fn open_playlist_menu(
        &mut self,
        id: String,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        self.menu = None;
        self.menu_serial += 1;
        self.playlist_menu = Some(PlaylistMenu {
            position,
            id,
            serial: self.menu_serial,
            confirm: false,
        });
        cx.notify();
    }

    /// Play a playlist's songs (a smart playlist's current matches), loaded in the background.
    fn play_playlist(&mut self, playlist: needle_core::model::Playlist, shuffle: bool, next: bool) {
        let library = self.library.clone();
        let sender = self.sender.clone();
        std::thread::spawn(move || {
            let event = match library.playlist_tracks(&playlist) {
                Ok(mut tracks) => {
                    if shuffle {
                        use rand::seq::SliceRandom;
                        tracks.shuffle(&mut rand::thread_rng());
                    }
                    let reason = format!("From the playlist {}", playlist.name);
                    let items: Vec<_> = tracks
                        .into_iter()
                        .filter(|t| !t.missing)
                        .map(|track| needle_core::audio::QueueItem {
                            track,
                            reason: reason.clone(),
                        })
                        .collect();
                    if next {
                        super::Event::Enqueue(
                            items,
                            format!("Added {} to the queue.", playlist.name),
                        )
                    } else {
                        super::Event::Play(items, None)
                    }
                }
                Err(e) => super::Event::Error(format!("{e:#}")),
            };
            let _ = sender.send(event);
        });
    }

    fn playlist_menu_entries(&self) -> Vec<Entry> {
        let Some(menu) = &self.playlist_menu else {
            return vec![];
        };
        let Some(playlist) = self.playlists.iter().find(|p| p.id == menu.id).cloned() else {
            return vec![];
        };
        let smart = playlist.query.is_some();
        let (play, shuffle, queue, open, export, delete) = (
            playlist.clone(),
            playlist.clone(),
            playlist.clone(),
            playlist.clone(),
            playlist.clone(),
            playlist.clone(),
        );
        let confirm = menu.confirm;
        let position = menu.position;
        vec![
            Entry::item("play", "Play", None, move |this, _, _| {
                this.play_playlist(play.clone(), false, false)
            }),
            Entry::item("shuffle", "Shuffle", None, move |this, _, _| {
                this.play_playlist(shuffle.clone(), true, false)
            }),
            Entry::item("queue", "Add to queue", None, move |this, _, _| {
                this.play_playlist(queue.clone(), false, true)
            }),
            Entry::Separator,
            Entry::item(
                "edit",
                if smart {
                    "Rename or edit rule…"
                } else {
                    "Rename…"
                },
                None,
                move |this, window, cx| {
                    this.navigate(Page::Playlist(open.id.clone()), window, cx);
                    this.playlist_name.update(cx, |s, cx| s.focus(window, cx));
                },
            ),
            Entry::item("external", "Export as M3U8…", None, move |this, _, _| {
                let library = this.library.clone();
                let playlist = export.clone();
                this.background(move || {
                    let Some(path) = rfd::FileDialog::new()
                        .set_file_name(format!("{}.m3u8", playlist.name))
                        .save_file()
                    else {
                        return Ok(String::new());
                    };
                    library.export_playlist(&playlist, &path)?;
                    Ok("Playlist exported.".into())
                });
            }),
            Entry::Separator,
            Entry::item(
                "trash",
                if confirm {
                    "Click again to delete"
                } else {
                    "Delete playlist"
                },
                None,
                move |this, window, cx| {
                    if !confirm {
                        // Keep the menu open and ask once more.
                        this.menu_serial += 1;
                        this.playlist_menu = Some(PlaylistMenu {
                            position,
                            id: delete.id.clone(),
                            serial: this.menu_serial,
                            confirm: true,
                        });
                        return;
                    }
                    match this.library.delete_playlist(&delete.id) {
                        Ok(()) => {
                            this.playlists = this.library.playlists().unwrap_or_default();
                            if this.page == Page::Playlist(delete.id.clone()) {
                                this.back.clear();
                                this.open(Page::Home, window, cx);
                            }
                            this.notify("Playlist deleted. Your music files are untouched.");
                        }
                        Err(e) => this.fail(e.to_string()),
                    }
                },
            ),
        ]
    }

    pub(super) fn playlist_menu_view(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        let menu = self.playlist_menu.as_ref()?;
        let p = pal(cx);
        let entries = self.playlist_menu_entries();
        if entries.is_empty() {
            return None;
        }
        let rows = Self::menu_rows(&entries, None, cx);
        let body = div()
            .id("playlist-menu")
            .occlude()
            .w(px(220.))
            .p_1()
            .rounded(px(9.))
            .bg(cx.theme().popover)
            .border_1()
            .border_color(p.line)
            .shadow_lg()
            .flex()
            .flex_col()
            .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                this.playlist_menu = None;
                cx.notify();
            }))
            .children(rows);
        let body = motion::animate(body, ("playlist-menu-in", menu.serial), 140, cx, |el, t| {
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
