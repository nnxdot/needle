use super::{
    AppView, PAGE_SIZE, Page, Sort, motion, pal,
    widgets::{
        artwork, count as thousands, cover, faint, glyph, icon, icon_button, meta, page_title,
        quality,
    },
};
use gpui::{prelude::*, *};
use gpui_component::{
    Disableable, Sizable,
    button::{Button, ButtonVariants},
};
use needle_core::browse::{AlbumSummary, ArtistSummary};

/// A way out of an empty page.
#[derive(Clone, Copy)]
pub enum Step {
    ClearSearch,
    Palette,
    Songs,
    AddFolder,
    Import,
}
impl Step {
    fn label(self) -> (&'static str, &'static str) {
        match self {
            Self::ClearSearch => ("Clear search", "close"),
            Self::Palette => ("Search everything", "command"),
            Self::Songs => ("Browse songs", "songs"),
            Self::AddFolder => ("Add music folder", "folder"),
            Self::Import => ("Import a library", "import"),
        }
    }
    fn run(self, this: &mut AppView, window: &mut Window, cx: &mut Context<AppView>) {
        match self {
            Self::ClearSearch => {
                this.search.update(cx, |s, cx| s.set_value("", window, cx));
                window.focus(&this.focus);
                this.refresh(cx);
            }
            Self::Palette => {
                let text = this.search_text(cx);
                this.open_palette(window, cx);
                this.palette
                    .input
                    .update(cx, |s, cx| s.set_value(text, window, cx));
                this.palette.active = 0;
                this.palette_search(cx);
            }
            Self::Songs => this.navigate(Page::Songs, window, cx),
            Self::AddFolder => this.import_folder(window, cx),
            Self::Import => this.navigate(Page::Import, window, cx),
        }
    }
}

/// One album or artist tile.
#[derive(Clone)]
pub struct Group {
    pub title: String,
    pub subtitle: String,
    pub artwork: Option<String>,
    pub seed: String,
    pub tracks: usize,
    pub page: Page,
}

pub fn album_groups(albums: Vec<AlbumSummary>) -> Vec<Group> {
    albums
        .into_iter()
        .map(|album| Group {
            title: if album.album.is_empty() {
                "Unknown album".into()
            } else {
                album.album.clone()
            },
            subtitle: if album.year > 0 {
                format!("{} · {}", album.artist, album.year)
            } else {
                album.artist.clone()
            },
            artwork: album.artwork,
            seed: format!("{}{}", album.album, album.artist),
            tracks: album.tracks,
            page: Page::Album {
                album: album.album,
                artist: album.artist,
                query: album.query,
            },
        })
        .collect()
}

pub fn artist_groups(artists: Vec<ArtistSummary>) -> Vec<Group> {
    artists
        .into_iter()
        .map(|artist| Group {
            title: if artist.name.is_empty() {
                "Unknown artist".into()
            } else {
                artist.name.clone()
            },
            subtitle: match (artist.albums, artist.tracks) {
                (1, 1) => "1 track".into(),
                (1, t) => format!("{t} tracks"),
                (a, t) => format!("{a} albums · {t} tracks"),
            },
            artwork: artist.artwork,
            seed: artist.name.clone(),
            tracks: artist.tracks,
            page: Page::Artist(artist.name),
        })
        .collect()
}

fn initials(name: &str) -> String {
    name.split_whitespace()
        .filter(|w| !w.eq_ignore_ascii_case("the"))
        .filter_map(|w| w.chars().next())
        .take(2)
        .collect::<String>()
        .to_uppercase()
}

impl AppView {
    pub(super) fn main(
        &self,
        width: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let body = match self.page {
            Page::Settings => self.settings_view(width, cx).into_any_element(),
            Page::Sound => self.sound_view(width, cx).into_any_element(),
            Page::Import => self.import_view(width, cx).into_any_element(),
            Page::Folders => self.folders_view(cx).into_any_element(),
            Page::History => self.history_view(width, cx).into_any_element(),
            Page::Timing => self.timing_view(cx),
            Page::Wrapped(year) => self.wrapped_view(year, width, cx),
            Page::Doctor => self.doctor_view(width, cx),
            _ if self.total == 0 && self.scan.is_none() && !self.loading => {
                self.onboarding(cx).into_any_element()
            }
            Page::Home => self.home_view(cx).into_any_element(),
            _ => self.collection(width, window, cx).into_any_element(),
        };
        let page = div()
            .flex_1()
            .min_w_0()
            .h_full()
            .flex()
            .flex_col()
            .child(body);
        // A short fade each time the page changes.
        div()
            .flex_1()
            .min_w_0()
            .h_full()
            .flex()
            .child(motion::animate(
                page,
                ("page", self.page_serial),
                160,
                cx,
                |el, t| el.opacity(t),
            ))
    }

    fn onboarding(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = pal(cx);
        div()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_4()
            .px_10()
            .pb_10()
            .child(glyph("logo").size(px(56.)).text_color(p.accent))
            .child(div().mt_2().text_size(px(26.)).font_weight(FontWeight::SEMIBOLD).child("Bring your music home."))
            .child(
                div()
                    .max_w(px(440.))
                    .text_center()
                    .text_size(px(14.))
                    .line_height(relative(1.6))
                    .text_color(p.ink_2)
                    .child("Point Needle at the folders where your music lives. It reads your files in place, keeps its own index, and never uploads anything."),
            )
            .child(
                div()
                    .mt_2()
                    .flex()
                    .gap_2()
                    .child(
                        Button::new("empty-add")
                            .primary()
                            .icon(icon("folder"))
                            .label("Add a music folder")
                            .on_click(cx.listener(|this, _, window, cx| this.import_folder(window, cx))),
                    )
                    .child(
                        Button::new("empty-demo")
                            .ghost()
                            .label("Try three demo recordings")
                            .on_click(cx.listener(|this, _, _, cx| this.demo(cx))),
                    ),
            )
            .child(faint("Ctrl+O adds a folder at any time. No account needed.", cx))
            .child(
                Button::new("empty-import")
                    .ghost()
                    .small()
                    .icon(icon("import"))
                    .label("Coming from iTunes, Spotify, or Last.fm? Bring your history")
                    .on_click(cx.listener(|this, _, window, cx| this.navigate(Page::Import, window, cx))),
            )
    }

    fn header(&self, width: f32, cx: &mut Context<Self>) -> impl IntoElement {
        let p = pal(cx);
        let count = self.matched_total;
        let duration: f64 = self.tracks.iter().map(|t| t.duration).sum();
        let search = self.search_text(cx);
        let playlist = if let Page::Playlist(id) = &self.page {
            self.playlists.iter().find(|p| &p.id == id).cloned()
        } else {
            None
        };
        let title = match (&self.page, &playlist) {
            (_, Some(list)) => list.name.clone(),
            (Page::Songs, _) if !search.is_empty() => "Results".into(),
            (page, _) => page.title(),
        };
        let noun = match self.page {
            Page::Albums | Page::Artist(_) => "albums",
            Page::Artists => "artists",
            _ => "tracks",
        };
        let shown = if self.page.is_grid() {
            self.groups.len()
        } else {
            count
        };
        let mut summary = if self.loading && self.tracks.is_empty() {
            "Loading…".to_string()
        } else if shown == 1 {
            format!("1 {}", noun.trim_end_matches('s'))
        } else {
            format!("{} {noun}", thousands(shown))
        };
        if noun == "tracks" && count > 0 && count <= self.tracks.len() {
            summary.push_str(&format!(" · {}", human_duration(duration)));
        }
        if matches!(self.page, Page::Artist(_)) {
            let tracks: usize = self.groups.iter().map(|g| g.tracks).sum();
            summary.push_str(&format!(" · {} tracks", thousands(tracks)));
        }
        let rule = playlist
            .as_ref()
            .and_then(|p| p.query.clone())
            .map(|rule| format!("Smart playlist · {rule}"))
            .or_else(|| {
                self.explanation
                    .clone()
                    .filter(|e| e.starts_with("Matches rule") && !search.is_empty())
                    .map(|e| e.replacen("Matches rule: ", "Rule · ", 1))
            });
        let can_play = (!self.tracks.is_empty()
            || matches!(self.page, Page::Artist(_)) && !self.groups.is_empty())
            && self.query_error.is_none();
        let album_art = if let Page::Album { .. } = &self.page {
            self.tracks.first().cloned()
        } else {
            None
        };
        let artist = if let Page::Artist(name) = &self.page {
            Some(name.clone())
        } else {
            None
        };
        // Pages with a cover (album, artist, playlist); smaller covers and icon-only buttons
        // when the page is narrow.
        let big = album_art.is_some() || artist.is_some() || playlist.is_some();
        let narrow = width < 760.;
        let tight = width < 620.;
        let cover_size = if narrow { 132. } else { 196. };
        // Play and the rest: under the title beside a cover, else beside the title.
        let buttons = (!matches!(self.page, Page::Albums | Page::Artists)).then(|| {
            div()
                .flex()
                .flex_shrink_0()
                .gap_2()
                .child(
                    Button::new("play-view")
                        .primary()
                        .icon(icon("play"))
                        .label("Play")
                        .disabled(!can_play)
                        .on_click(cx.listener(|this, _, _, cx| this.play_view(0, false, cx))),
                )
                .child(
                    Button::new("shuffle-view")
                        .icon(icon("shuffle"))
                        .when(!tight, |b| b.label("Shuffle"))
                        .disabled(!can_play)
                        .on_click(cx.listener(|this, _, _, cx| this.play_view(0, true, cx))),
                )
                .when(
                    matches!(self.page, Page::Album { .. })
                        && !self.tracks.is_empty()
                        && self.tracks.iter().all(|t| t.is_streamed()),
                    |el| {
                        let kept = self.tracks.iter().all(needle_core::sources::is_kept);
                        let ids: Vec<String> = self.tracks.iter().map(|t| t.id.clone()).collect();
                        el.child(
                            icon_button(
                                "album-keep",
                                if kept { "pin-fill" } else { "pin" },
                                if kept {
                                    "Stop keeping this album on this computer"
                                } else {
                                    "Keep this album on this computer, to play without the network"
                                },
                            )
                            .on_click(cx.listener(
                                move |this, _, _, cx| {
                                    this.keep_streamed(&ids, !kept);
                                    cx.notify();
                                },
                            )),
                        )
                    },
                )
                .when_some(
                    match &self.page {
                        Page::Artist(name) => Some(name.clone()),
                        _ => None,
                    },
                    |el, artist| {
                        let blend = artist.clone();
                        el.child(
                            Button::new("artist-radio")
                                .icon(icon("radio"))
                                .when(!tight, |b| b.label("Radio"))
                                .on_click(cx.listener(move |this, _, _, _| {
                                    this.start_artist_radio(artist.clone())
                                })),
                        )
                        .child(
                            icon_button("artist-blend", "blend", "Blend with another artist")
                                .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                                    // Below the button, clear of its tooltip.
                                    let position = window.mouse_position() + point(px(0.), px(22.));
                                    this.open_blend_menu(blend.clone(), position, cx)
                                })),
                        )
                    },
                )
                .when_some(
                    playlist.clone().filter(|l| l.query.is_none()),
                    |el, list| {
                        el.child(
                            Button::new("add-songs")
                                .icon(icon("plus"))
                                .when(!tight, |b| b.label("Add songs"))
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.open_add_songs(&list, window, cx)
                                })),
                        )
                    },
                )
                .when_some(playlist.clone(), |el, list| {
                    el.child(
                        Button::new("edit-playlist")
                            .icon(icon("edit"))
                            .when(!tight, |b| b.label("Edit"))
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.open_playlist_editor(
                                    Some(list.clone()),
                                    vec![],
                                    None,
                                    window,
                                    cx,
                                )
                            })),
                    )
                })
                .when(playlist.is_none(), |el| {
                    el.child(
                        icon_button("save-view", "plus", "Save as a playlist").on_click(
                            cx.listener(|this, _, window, cx| {
                                this.save_view_as_playlist(window, cx)
                            }),
                        ),
                    )
                })
        });
        let (beside, under) = if big {
            (None, buttons)
        } else {
            (buttons, None)
        };
        div()
            .flex_shrink_0()
            .px_6()
            .pt_6()
            .pb_4()
            .flex()
            .items_end()
            .gap_5()
            .when(
                album_art.is_some() || artist.is_some() || playlist.is_some(),
                |el| el.pt(px(34.)).pb_6(),
            )
            .when_some(playlist.clone(), |el, list| {
                el.child(div().rounded(px(8.)).shadow_lg().child(self.playlist_cover(
                    list.cover.as_deref(),
                    &self.tracks,
                    cover_size,
                    cx,
                )))
            })
            .when_some(album_art.clone(), |el, track| {
                el.child(div().rounded(px(8.)).shadow_lg().child(artwork(
                    Some(&track),
                    cover_size,
                    cx,
                )))
            })
            .when_some(artist.clone(), |el, name| {
                el.child(div().rounded_full().shadow_lg().child(self.artist_photo(
                    &name,
                    cover_size * 0.9,
                    cx,
                )))
            })
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .children(self.breadcrumbs(cx))
                    .child(
                        if album_art.is_some() || artist.is_some() || playlist.is_some() {
                            super::widgets::display(title, if narrow { 34. } else { 46. })
                                .truncate()
                        } else {
                            page_title(title)
                        },
                    )
                    .when_some(album_art.clone(), |el, track| {
                        let artist_name = if track.album_artist.is_empty() {
                            track.artist.clone()
                        } else {
                            track.album_artist.clone()
                        };
                        let page = Page::Artist(artist_name.clone());
                        el.child(
                            div()
                                .id("album-artist")
                                .text_size(px(14.))
                                .font_weight(FontWeight::MEDIUM)
                                .cursor_pointer()
                                .hover(|s| s.underline())
                                .child(if artist_name.is_empty() {
                                    "Unknown artist".into()
                                } else {
                                    artist_name
                                })
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.navigate(page.clone(), window, cx)
                                })),
                        )
                    })
                    .child(
                        meta(
                            match &album_art {
                                Some(track) => [
                                    (track.year > 0).then(|| track.year.to_string()),
                                    Some(summary.clone()),
                                    Some(quality(track)),
                                ]
                                .into_iter()
                                .flatten()
                                .collect::<Vec<_>>()
                                .join(" · "),
                                None => summary,
                            },
                            cx,
                        )
                        .mt_1(),
                    )
                    .when_some(
                        playlist
                            .as_ref()
                            .map(|l| l.description.clone())
                            .filter(|d| !d.is_empty()),
                        |el, description| {
                            el.child(meta(description, cx).text_color(p.ink_2).truncate())
                        },
                    )
                    .when_some(rule, |el, rule| {
                        el.child(meta(rule, cx).text_color(p.accent).truncate())
                    })
                    .when_some(under, |el, buttons| el.child(buttons.mt_4())),
            )
            .children(beside)
    }

    fn playlist_tools(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        let Page::Playlist(id) = &self.page else {
            return None;
        };
        let playlist = self.playlists.iter().find(|p| &p.id == id)?.clone();
        let (export, delete) = (playlist.clone(), playlist.clone());
        Some(
            div()
                .px_6()
                .pb_3()
                .flex()
                .gap_2()
                .items_center()
                .child(
                    Button::new("export-playlist")
                        .small()
                        .ghost()
                        .label("Export M3U8")
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if !this.can_pick(cx) {
                                return;
                            }
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
                        })),
                )
                .child(div().flex_1())
                .child(
                    Button::new("delete-playlist")
                        .small()
                        .ghost()
                        .when(self.confirm_delete, |b| b.danger())
                        .label(if self.confirm_delete {
                            "Click again to delete"
                        } else {
                            "Delete playlist"
                        })
                        .on_click(cx.listener(move |this, _, window, cx| {
                            if this.confirm_delete {
                                match this.library.delete_playlist(&delete.id) {
                                    Ok(()) => {
                                        this.playlists =
                                            this.library.playlists().unwrap_or_default();
                                        this.back.clear();
                                        this.open(Page::Songs, window, cx);
                                        this.notify(
                                            "Playlist deleted. Your music files are untouched.",
                                        );
                                    }
                                    Err(e) => this.fail(e.to_string()),
                                }
                            } else {
                                this.confirm_delete = true;
                            }
                            cx.notify();
                        })),
                ),
        )
    }

    /// Save what the page shows: a smart playlist of its rule, or the songs shown (or
    /// selected).
    fn save_view_as_playlist(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let rule = self.expression(cx);
        let name = self.search_text(cx);
        let rule = (!rule.trim().is_empty()).then_some(rule);
        let ids = if rule.is_some() {
            vec![]
        } else {
            let selected = self.selected_tracks();
            if selected.len() > 1 {
                selected
            } else {
                self.tracks.clone()
            }
            .into_iter()
            .map(|t| t.id)
            .collect()
        };
        self.open_playlist_editor(None, ids, rule, window, cx);
        if let Some(editor) = &self.editor
            && !name.is_empty()
        {
            editor.set_name(name, window, cx);
        }
    }

    fn collection(
        &self,
        width: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let p = pal(cx);
        let body = if let Some(error) = self
            .query_error
            .as_ref()
            .filter(|_| self.tracks.is_empty() && self.groups.is_empty())
        {
            self.problem(
                "That rule needs a fix",
                error.clone(),
                &[Step::ClearSearch],
                cx,
            )
            .into_any_element()
        } else if self.loading
            && if self.page.is_grid() {
                self.groups.is_empty()
            } else {
                self.tracks.is_empty()
            }
        {
            self.skeleton(cx).into_any_element()
        } else if self.page.is_grid() && !self.groups.is_empty() {
            self.grid(width, cx).into_any_element()
        } else if self.tracks.is_empty() {
            let search = self.search_text(cx);
            let (title, detail, steps): (String, String, &[Step]) = match &self.page {
                _ if !search.is_empty() => (
                    format!("Nothing matches “{search}”"),
                    "Try fewer words, or a rule such as  artist contains \"Nick\".".into(),
                    &[Step::ClearSearch, Step::Palette],
                ),
                Page::Favorites => (
                    "No favorites yet".to_string(),
                    "Press the heart on any track, or drag songs onto Favorites.".to_string(),
                    &[Step::Songs],
                ),
                Page::Folder(_) => (
                    "No songs in this folder".into(),
                    "Needle lists music it has scanned. Check for changes in Settings › Library if you added files here.".into(),
                    &[Step::Songs],
                ),
                Page::Recent => (
                    "Nothing added in the last 30 days".into(),
                    "New files in your music folders appear here automatically.".into(),
                    &[Step::AddFolder],
                ),
                Page::Source { name, .. } => (
                    format!("No songs from {name} yet"),
                    "Needle is getting the list of songs, or the server has none. Check Settings › Plugins.".into(),
                    &[Step::Songs],
                ),
                Page::Playlist(_) => (
                    "This playlist is empty".into(),
                    "Drag songs onto the playlist in the sidebar, or right-click them and choose Add to playlist.".into(),
                    &[Step::Songs],
                ),
                Page::Songs | Page::Albums | Page::Artists => (
                    "Your library is empty".into(),
                    "Add a folder with music, or bring your library over from iTunes, Spotify, or Last.fm.".into(),
                    &[Step::AddFolder, Step::Import],
                ),
                _ => ("Nothing to show yet".into(), "Your songs are one click away.".into(), &[Step::Songs]),
            };
            self.problem(title, detail, steps, cx).into_any_element()
        } else {
            self.table(width, window, cx).into_any_element()
        };
        div()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .child(self.header(width, cx))
            .children(self.folder_strip(cx))
            .children(self.playlist_tools(cx))
            .when_some(
                self.query_error
                    .clone()
                    .filter(|_| !self.tracks.is_empty() || !self.groups.is_empty()),
                |el, error| {
                    el.child(
                        div()
                            .mx_6()
                            .mb_2()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(glyph("alert").size(px(14.)).text_color(p.danger))
                            .child(
                                meta(format!("{error} · showing the last results"), cx)
                                    .text_color(p.danger),
                            ),
                    )
                },
            )
            .child(body)
            .when(
                self.page.is_tracks() && !self.page.is_grid() && self.matched_total > PAGE_SIZE,
                |el| {
                    el.child(
                        div()
                            .h(px(44.))
                            .px_6()
                            .flex_shrink_0()
                            .flex()
                            .items_center()
                            .justify_between()
                            .border_t_1()
                            .border_color(p.line_soft)
                            .child(meta(
                                format!(
                                    "{}–{} of {} tracks",
                                    thousands(self.page_offset + 1),
                                    thousands(self.page_offset + self.tracks.len()),
                                    thousands(self.matched_total)
                                ),
                                cx,
                            ))
                            .child(
                                div()
                                    .flex()
                                    .gap_1()
                                    .child(
                                        icon_button(
                                            "previous-page",
                                            "chevron-left",
                                            "Previous 1,000",
                                        )
                                        .small()
                                        .disabled(self.page_offset == 0)
                                        .on_click(
                                            cx.listener(|this, _, _, cx| {
                                                this.page_offset =
                                                    this.page_offset.saturating_sub(PAGE_SIZE);
                                                this.refresh(cx);
                                            }),
                                        ),
                                    )
                                    .child(
                                        icon_button("next-page", "chevron-right", "Next 1,000")
                                            .small()
                                            .disabled(
                                                self.page_offset + self.tracks.len()
                                                    >= self.matched_total,
                                            )
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.page_offset += PAGE_SIZE;
                                                this.refresh(cx);
                                            })),
                                    ),
                            ),
                    )
                },
            )
    }

    fn problem(
        &self,
        title: impl Into<SharedString>,
        detail: impl Into<SharedString>,
        steps: &[Step],
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let p = pal(cx);
        div()
            .flex_1()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_2()
            .pb_20()
            .px_10()
            .child(glyph("search").size(px(28.)).text_color(p.ink_3).mb_2())
            .child(
                div()
                    .text_size(px(15.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_center()
                    .child(title.into()),
            )
            .child(
                meta(detail, cx)
                    .max_w(px(460.))
                    .text_center()
                    .line_height(relative(1.5)),
            )
            .child(
                div()
                    .mt_3()
                    .flex()
                    .gap_2()
                    .children(steps.iter().enumerate().map(|(i, step)| {
                        let step = *step;
                        let (label, name) = step.label();
                        let button = Button::new(("next-step", i)).icon(icon(name)).label(label);
                        if i == 0 {
                            button.primary()
                        } else {
                            button.ghost()
                        }
                        .on_click(
                            cx.listener(move |this, _, window, cx| step.run(this, window, cx)),
                        )
                    })),
            )
    }

    fn skeleton(&self, cx: &App) -> AnyElement {
        let p = pal(cx);
        let rows = div()
            .flex_1()
            .px_6()
            .pt_2()
            .flex()
            .flex_col()
            .children((0..9).map(|i| {
                div()
                    .h(px(52.))
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(div().size(px(36.)).rounded(px(4.)).bg(p.raised))
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(
                                div()
                                    .h(px(10.))
                                    .w(relative(0.25 + (i % 3) as f32 * 0.08))
                                    .rounded(px(3.))
                                    .bg(p.raised),
                            )
                            .child(
                                div()
                                    .h(px(8.))
                                    .w(relative(0.16 + (i % 4) as f32 * 0.05))
                                    .rounded(px(3.))
                                    .bg(p.raised.opacity(0.7)),
                            ),
                    )
            }));
        // A slow breathing shimmer so a long load reads as "working", not "stuck".
        motion::repeat(rows, "skeleton", 1400, 1., cx, |el, t| {
            el.opacity(0.45 + 0.55 * t)
        })
    }

    fn sort_label(
        &self,
        id: &'static str,
        label: &'static str,
        field: &'static str,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let p = pal(cx);
        let state = match self.sort {
            Sort::Asc(f) if f == field => Some("arrow-up"),
            Sort::Desc(f) if f == field => Some("arrow-down"),
            _ => None,
        };
        div()
            .id(id)
            .flex()
            .items_center()
            .gap_1()
            .cursor_pointer()
            .text_color(if state.is_some() { p.ink } else { p.ink_3 })
            .hover(|s| s.text_color(p.ink))
            .child(label)
            .when_some(state, |el, g| {
                el.child(glyph(g).size(px(12.)).text_color(p.ink))
            })
            .on_click(cx.listener(move |this, _, _, cx| {
                this.sort = match this.sort {
                    Sort::Asc(f) if f == field => Sort::Desc(field),
                    Sort::Desc(f) if f == field => Sort::Default,
                    _ => Sort::Asc(field),
                };
                this.page_offset = 0;
                this.refresh(cx);
            }))
    }

    fn table(&self, width: f32, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = pal(cx);
        let album_view = matches!(self.page, Page::Album { .. });
        let columns = self.visible_columns(width, album_view);
        let spare = super::columns::spare(width, &columns);
        // The favorite heart sits just before Time.
        let (time, before_heart): (Vec<_>, Vec<_>) =
            columns.iter().cloned().partition(|c| c.key == "time");
        let show_album = columns.iter().any(|c| c.key == "album");
        let row_columns = (before_heart.clone(), time.clone());
        div()
            .on_drop(
                cx.listener(|this, _: &super::columns::ResizingColumn, _, cx| {
                    this.finish_column_resize();
                    cx.notify();
                }),
            )
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(32.))
                    .mx_4()
                    .px_2()
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .gap_3()
                    .border_b_1()
                    .border_color(p.line_soft)
                    .text_size(px(12.))
                    .text_color(p.ink_3)
                    .child(
                        div().w(px(28.)).text_right().child(
                            div()
                                .id("sort-reset")
                                .cursor_pointer()
                                .hover(|s| s.text_color(p.ink))
                                .child("#")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.sort = Sort::Default;
                                    this.refresh(cx);
                                })),
                        ),
                    )
                    .when(!album_view, |el| el.child(div().w(px(38.))))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .gap_1()
                            .child(self.sort_label("sort-title", "Title", "title", cx))
                            .when(!album_view, |el| {
                                el.child("·").child(self.sort_label(
                                    "sort-artist",
                                    "Artist",
                                    "artist",
                                    cx,
                                ))
                            }),
                    )
                    .children(
                        before_heart
                            .iter()
                            .map(|c| self.column_header(c, spare, cx))
                            .collect::<Vec<_>>(),
                    )
                    .child(div().w(px(28.)))
                    .children(
                        time.iter()
                            .map(|c| self.column_header(c, spare, cx))
                            .collect::<Vec<_>>(),
                    )
                    .child(div().w(px(28.)))
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(|this, event: &MouseDownEvent, _, cx| {
                            this.open_header_menu(event.position, cx)
                        }),
                    )
                    .on_drag_move(cx.listener(
                        |this, event: &DragMoveEvent<super::columns::ResizingColumn>, _, cx| {
                            let drag = event.drag(cx).clone();
                            this.resize_column(&drag, f32::from(event.event.position.x));
                            cx.notify();
                        },
                    ))
                    .on_drop(
                        cx.listener(|this, _: &super::columns::ResizingColumn, _, cx| {
                            this.finish_column_resize();
                            cx.notify();
                        }),
                    ),
            )
            .child(with_scrollbar(
                uniform_list(
                    "track-list",
                    self.tracks.len(),
                    cx.processor(move |this, range: std::ops::Range<usize>, _, cx| {
                        range
                            .map(|index| {
                                this.track_row(index, album_view, show_album, &row_columns, cx)
                            })
                            .collect::<Vec<_>>()
                    }),
                )
                .track_scroll(self.list_scroll.clone())
                .flex_1()
                .pb_4(),
                &self.list_scroll,
            ))
    }

    fn track_row(
        &self,
        index: usize,
        album_view: bool,
        show_album: bool,
        columns: &(
            Vec<needle_core::model::ColumnSetting>,
            Vec<needle_core::model::ColumnSetting>,
        ),
        cx: &mut Context<Self>,
    ) -> Div {
        // Songs of a playlist of picked songs can be dragged into another order.
        let arrange = self.arrangeable_playlist(cx).is_some();
        let p = pal(cx);
        let track = &self.tracks[index];
        let selected = self.selection.ids.contains(&track.id);
        let cursor = self.selection.cursor == Some(index);
        let playing = self
            .playback
            .current
            .as_ref()
            .is_some_and(|i| i.track.id == track.id);
        let favorite = track.rating >= 4;
        let id = track.id.clone();
        let height = self.settings.layout.row_height.clamp(32., 76.);
        // Compact rows hold one line: a small cover, then the title and artist side by side.
        let compact = height < 50.;
        let number = if album_view && track.track_number > 0 {
            track.track_number.to_string()
        } else {
            thousands(self.page_offset + index + 1)
        };
        let row = div()
            .id(("row", index))
            .group("row")
            .h(px(height))
            .w_full()
            .px_2()
            .rounded(px(6.))
            .flex()
            .items_center()
            .gap_3()
            .cursor_default()
            .when(selected, |el| el.bg(p.selection))
            .when(!selected, |el| el.hover(|s| s.bg(p.raised.opacity(0.55))))
            .when(cursor && selected, |el| {
                el.border_1().border_color(p.accent.opacity(0.28))
            })
            .child(
                div()
                    .w(px(28.))
                    .flex_shrink_0()
                    .relative()
                    .flex()
                    .justify_end()
                    .text_size(px(12.))
                    .text_color(p.ink_3)
                    .child(if playing {
                        motion::equalizer(
                            format!("eq-{index}"),
                            p.accent,
                            self.playback.playing,
                            cx,
                        )
                        .into_any_element()
                    } else {
                        div()
                            .group_hover("row", |s| s.opacity(0.))
                            .child(number)
                            .into_any_element()
                    })
                    .when(!playing, |el| {
                        el.child(
                            div()
                                .id(("row-play", index))
                                .absolute()
                                .right_0()
                                .top(px(-1.))
                                .opacity(0.)
                                .group_hover("row", |s| s.opacity(1.))
                                .cursor_pointer()
                                .text_color(p.ink)
                                .child(glyph("play").size(px(15.)).text_color(p.ink))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    cx.stop_propagation();
                                    this.select_single(index, cx);
                                    this.play_view(index, false, cx);
                                })),
                        )
                    }),
            )
            .when(!album_view, |el| {
                el.child(artwork(Some(track), if compact { 24. } else { 38. }, cx))
            })
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .when(compact, |el| el.flex_row().items_center().gap_2())
                    .child(
                        div()
                            .min_w_0()
                            .flex_shrink()
                            .overflow_hidden()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .min_w_0()
                                    .truncate()
                                    .text_size(px(13.5))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(if playing {
                                        p.accent
                                    } else if track.missing {
                                        p.ink_3
                                    } else {
                                        p.ink
                                    })
                                    .child(track.title.clone()),
                            )
                            .when(track.missing, |el| {
                                el.child(
                                    div()
                                        .flex_shrink_0()
                                        .px(px(5.))
                                        .rounded(px(3.))
                                        .bg(p.danger.opacity(0.14))
                                        .text_color(p.danger)
                                        .text_size(px(11.))
                                        .child("Missing file"),
                                )
                            }),
                    )
                    .when(
                        !album_view
                            || track.artist != track.album_artist && !track.album_artist.is_empty(),
                        |el| {
                            el.child(
                                div()
                                    .min_w_0()
                                    .flex_shrink()
                                    .truncate()
                                    .text_size(px(12.))
                                    .text_color(p.ink_2)
                                    .child(if show_album || album_view {
                                        track.display_artist().to_string()
                                    } else {
                                        format!(
                                            "{} · {}",
                                            track.display_artist(),
                                            track.display_album()
                                        )
                                    }),
                            )
                        },
                    ),
            )
            .children(
                columns
                    .0
                    .iter()
                    .map(|c| self.column_cell(c, track, cx))
                    .collect::<Vec<_>>(),
            )
            .child(
                div()
                    .id(("row-fav", index))
                    .w(px(28.))
                    .flex_shrink_0()
                    .flex()
                    .justify_center()
                    .cursor_pointer()
                    .when(!favorite, |el| {
                        el.opacity(0.).group_hover("row", |s| s.opacity(1.))
                    })
                    .text_color(if favorite { p.accent } else { p.ink_3 })
                    .hover(|s| s.text_color(p.accent))
                    .child(self.heart(
                        &id,
                        favorite,
                        16.,
                        if favorite { p.accent } else { p.ink_2 },
                        cx,
                    ))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        this.set_rating(std::slice::from_ref(&id), if favorite { 0 } else { 5 });
                        cx.notify();
                    })),
            )
            .children(
                columns
                    .1
                    .iter()
                    .map(|c| self.column_cell(c, track, cx))
                    .collect::<Vec<_>>(),
            )
            .child(
                div()
                    .id(("row-more", index))
                    .w(px(28.))
                    .h(px(28.))
                    .flex_shrink_0()
                    .rounded(px(5.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .opacity(0.)
                    .group_hover("row", |s| s.opacity(1.))
                    .text_color(p.ink_2)
                    .hover(|s| s.bg(p.raised_hover).text_color(p.ink))
                    .child(glyph("more").size(px(16.)).text_color(p.ink_2))
                    .on_click(cx.listener(move |this, event: &ClickEvent, _, cx| {
                        cx.stop_propagation();
                        this.open_menu(index, event.position(), cx);
                    })),
            )
            .on_click(cx.listener(move |this, event: &ClickEvent, window, cx| {
                this.click_track(index, event, window, cx)
            }))
            .when(arrange, |el| {
                let line = p.accent;
                el.drag_over::<super::flow::DraggedTracks>(move |s, _, _, _| {
                    s.border_t_2().border_color(line)
                })
                .on_drop(cx.listener(
                    move |this, dragged: &super::flow::DraggedTracks, _, cx| {
                        this.move_in_playlist(&dragged.ids, index, cx)
                    },
                ))
            })
            .on_drag(
                {
                    let ids: Vec<String> = if selected {
                        self.selection.ids.iter().cloned().collect()
                    } else {
                        vec![track.id.clone()]
                    };
                    super::flow::DraggedTracks {
                        label: if ids.len() > 1 {
                            format!("{} songs", ids.len()).into()
                        } else {
                            track.title.clone().into()
                        },
                        ids,
                    }
                },
                |value, _, _, cx| super::flow::drag_preview(value, cx),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                    this.open_menu(index, event.position, cx)
                }),
            );
        div().w_full().px_4().child(row)
    }

    fn grid(&self, width: f32, cx: &mut Context<Self>) -> impl IntoElement {
        let artists = self.page == Page::Artists;
        let gap = 22.;
        let inner = (width - 48.).max(200.);
        let columns = ((inner + gap) / (172. + gap)).floor().max(1.) as usize;
        let tile = (inner - gap * (columns as f32 - 1.)) / columns as f32;
        let rows = self.groups.len().div_ceil(columns);
        let list = uniform_list(
            "group-grid",
            rows,
            cx.processor(move |this, range: std::ops::Range<usize>, _, cx| {
                range
                    .map(|row| {
                        div().px_6().pb(px(26.)).flex().gap(px(gap)).children(
                            (row * columns..((row + 1) * columns).min(this.groups.len()))
                                .map(|i| this.tile(i, tile, artists, cx)),
                        )
                    })
                    .collect::<Vec<_>>()
            }),
        )
        .track_scroll(self.grid_scroll.clone())
        .flex_1();
        with_scrollbar(list, &self.grid_scroll)
    }

    fn tile(&self, index: usize, size: f32, round: bool, cx: &mut Context<Self>) -> Stateful<Div> {
        let group = &self.groups[index];
        let page = group.page.clone();
        div()
            .id(("tile", index))
            .group("tile")
            .w(px(size))
            .flex()
            .flex_col()
            .gap_1()
            .cursor_pointer()
            .child(
                div()
                    .relative()
                    .mb_2()
                    .rounded(if round { px(size) } else { px(8.) })
                    .shadow_md()
                    .child(if round {
                        self.artist_photo(&group.title, size, cx)
                    } else {
                        div()
                            .rounded(px(8.))
                            .group_hover("tile", |s| s.opacity(0.86))
                            .child(cover(group.artwork.as_deref(), &group.seed, size, cx))
                            .into_any_element()
                    })
                    .child(self.cover_play(index, page.clone(), size, cx)),
            )
            .child(
                div()
                    .text_size(px(13.5))
                    .font_weight(FontWeight::MEDIUM)
                    .truncate()
                    .when(round, |el| el.text_center())
                    .child(group.title.clone()),
            )
            .child(
                meta(group.subtitle.clone(), cx)
                    .truncate()
                    .when(round, |el| el.text_center()),
            )
            .on_click(
                cx.listener(move |this, _, window, cx| this.navigate(page.clone(), window, cx)),
            )
    }
}

impl AppView {
    /// A round artist photo, or the artist's initials when no photo is known.
    pub(super) fn artist_photo(&self, name: &str, size: f32, cx: &App) -> AnyElement {
        let p = pal(cx);
        if let Some(Some(path)) = self.artist_images.get(name) {
            // The picture is cut to its circle, whatever its shape.
            return div()
                .size(px(size))
                .flex_shrink_0()
                .rounded_full()
                .overflow_hidden()
                .child(
                    img(std::path::PathBuf::from(path))
                        .size_full()
                        .object_fit(ObjectFit::Cover),
                )
                .into_any_element();
        }
        super::widgets::generated_cover(&initials(name), size, true, &p).into_any_element()
    }
}

pub fn human_duration(seconds: f64) -> String {
    let minutes = (seconds / 60.).round() as u64;
    if minutes >= 60 {
        format!("{} h {} min", minutes / 60, minutes % 60)
    } else {
        format!("{minutes} min")
    }
}

/// A long list with a scrollbar on its right that can be dragged (the wheel still works).
fn with_scrollbar(list: impl IntoElement, handle: &UniformListScrollHandle) -> Div {
    use gpui_component::scroll::{Scrollbar, ScrollbarShow};
    div()
        .relative()
        .flex_1()
        .min_h_0()
        .flex()
        .flex_col()
        .child(list)
        .child(Scrollbar::vertical(handle).scrollbar_show(ScrollbarShow::Always))
}
