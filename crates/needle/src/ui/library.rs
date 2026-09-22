use super::{
    AppView, PAGE_SIZE, Page, Sort, pal,
    widgets::{artwork, count as thousands, cover, faint, glyph, icon, icon_button, meta, page_title, quality},
};
use gpui::{prelude::*, *};
use gpui_component::{
    Disableable, Sizable,
    button::{Button, ButtonVariants},
    input::Input,
};
use needle_core::{
    browse::{AlbumSummary, ArtistSummary},
    model::format_duration,
};

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
            title: if album.album.is_empty() { "Unknown album".into() } else { album.album.clone() },
            subtitle: if album.year > 0 { format!("{} · {}", album.artist, album.year) } else { album.artist.clone() },
            artwork: album.artwork,
            seed: format!("{}{}", album.album, album.artist),
            tracks: album.tracks,
            page: Page::Album { album: album.album, artist: album.artist, query: album.query },
        })
        .collect()
}

pub fn artist_groups(artists: Vec<ArtistSummary>) -> Vec<Group> {
    artists
        .into_iter()
        .map(|artist| Group {
            title: if artist.name.is_empty() { "Unknown artist".into() } else { artist.name.clone() },
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
    pub(super) fn main(&self, width: f32, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let body = match self.page {
            Page::Settings => self.settings_view(cx).into_any_element(),
            Page::History => self.history_view(width, cx).into_any_element(),
            _ if self.total == 0 && self.scan.is_none() && !self.loading => self.onboarding(cx).into_any_element(),
            _ => self.collection(width, window, cx).into_any_element(),
        };
        div().flex_1().min_w_0().h_full().flex().flex_col().child(body)
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
                            .on_click(cx.listener(|this, _, _, cx| this.import_folder(cx))),
                    )
                    .child(
                        Button::new("empty-demo")
                            .ghost()
                            .label("Try three demo recordings")
                            .on_click(cx.listener(|this, _, _, cx| this.demo(cx))),
                    ),
            )
            .child(faint("Ctrl+O adds a folder at any time. No account needed.", cx))
    }

    fn header(&self, cx: &mut Context<Self>) -> impl IntoElement {
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
        let shown = if self.page.is_grid() { self.groups.len() } else { count };
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
        let can_play = (!self.tracks.is_empty() || matches!(self.page, Page::Artist(_)) && !self.groups.is_empty()) && self.query_error.is_none();
        let album_art = if let Page::Album { .. } = &self.page { self.tracks.first().cloned() } else { None };
        let artist = if let Page::Artist(name) = &self.page { Some(name.clone()) } else { None };
        div()
            .flex_shrink_0()
            .px_6()
            .pt_6()
            .pb_4()
            .flex()
            .items_end()
            .gap_5()
            .when_some(album_art.clone(), |el, track| el.child(artwork(Some(&track), 148., cx)))
            .when_some(artist.clone(), |el, name| {
                el.child(
                    div()
                        .size(px(120.))
                        .flex_shrink_0()
                        .rounded_full()
                        .bg(p.raised)
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_size(px(38.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(p.ink_2)
                        .child(initials(&name)),
                )
            })
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .when(album_art.is_some(), |el| el.child(faint("Album", cx)))
                    .when(artist.is_some(), |el| el.child(faint("Artist", cx)))
                    .child(page_title(title))
                    .when_some(album_art.clone(), |el, track| {
                        let artist_name = if track.album_artist.is_empty() { track.artist.clone() } else { track.album_artist.clone() };
                        let page = Page::Artist(artist_name.clone());
                        el.child(
                            div()
                                .id("album-artist")
                                .text_size(px(14.))
                                .font_weight(FontWeight::MEDIUM)
                                .cursor_pointer()
                                .hover(|s| s.underline())
                                .child(if artist_name.is_empty() { "Unknown artist".into() } else { artist_name })
                                .on_click(cx.listener(move |this, _, window, cx| this.navigate(page.clone(), window, cx))),
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
                    .when_some(rule, |el, rule| el.child(meta(rule, cx).text_color(p.accent).truncate())),
            )
            .when(!matches!(self.page, Page::Albums | Page::Artists), |el| {
                el.child(
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
                                .label("Shuffle")
                                .disabled(!can_play)
                                .on_click(cx.listener(|this, _, _, cx| this.play_view(0, true, cx))),
                        )
                        .when(playlist.is_none(), |el| {
                            el.child(
                                icon_button("save-view", "plus", "Save as a playlist")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.show_save = !this.show_save;
                                        if this.show_save {
                                            let name = this.search_text(cx);
                                            this.playlist_name.update(cx, |s, cx| {
                                                s.set_value(name, window, cx);
                                                s.focus(window, cx);
                                            });
                                        }
                                        cx.notify();
                                    })),
                            )
                        }),
                )
            })
    }

    fn playlist_tools(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        let Page::Playlist(id) = &self.page else {
            return None;
        };
        let playlist = self.playlists.iter().find(|p| &p.id == id)?.clone();
        let (rename, export, delete) = (playlist.clone(), playlist.clone(), playlist.clone());
        Some(
            div()
                .px_6()
                .pb_3()
                .flex()
                .gap_2()
                .items_center()
                .child(Input::new(&self.playlist_name).small().w(px(260.)))
                .child(
                    Button::new("rename-playlist")
                        .small()
                        .label("Rename")
                        .on_click(cx.listener(move |this, _, _, cx| {
                            let mut playlist = rename.clone();
                            playlist.name = this.playlist_name.read(cx).value().trim().to_string();
                            if playlist.name.is_empty() {
                                return this.fail("A playlist needs a name.");
                            }
                            playlist.updated_at = chrono::Utc::now().timestamp();
                            match this.library.save_playlist(&playlist) {
                                Ok(()) => {
                                    this.playlists = this.library.playlists().unwrap_or_default();
                                    this.notify("Playlist renamed.");
                                }
                                Err(e) => this.fail(e.to_string()),
                            }
                            cx.notify();
                        })),
                )
                .child(
                    Button::new("export-playlist")
                        .small()
                        .ghost()
                        .label("Export M3U8")
                        .on_click(cx.listener(move |this, _, _, _| {
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
                        .label(if self.confirm_delete { "Click again to delete" } else { "Delete playlist" })
                        .on_click(cx.listener(move |this, _, window, cx| {
                            if this.confirm_delete {
                                match this.library.delete_playlist(&delete.id) {
                                    Ok(()) => {
                                        this.playlists = this.library.playlists().unwrap_or_default();
                                        this.back.clear();
                                        this.open(Page::Songs, window, cx);
                                        this.notify("Playlist deleted. Your music files are untouched.");
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

    fn save_form(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = pal(cx);
        let has_rule = !self.expression(cx).trim().is_empty();
        let selected = self.selection.ids.len();
        div()
            .mx_6()
            .mb_4()
            .p_3()
            .rounded(px(8.))
            .bg(p.raised)
            .flex()
            .items_center()
            .gap_2()
            .child(Input::new(&self.playlist_name).small().flex_1())
            .child(
                Button::new("save-smart")
                    .small()
                    .primary()
                    .icon(icon("smart"))
                    .label("Smart playlist")
                    .disabled(!has_rule)
                    .tooltip("Keeps the rule and updates itself as your library changes")
                    .on_click(cx.listener(|this, _, _, cx| this.save_playlist(true, cx))),
            )
            .child(
                Button::new("save-static")
                    .small()
                    .label(if selected > 1 { format!("Save {selected} selected") } else { "Save these tracks".into() })
                    .on_click(cx.listener(|this, _, _, cx| this.save_playlist(false, cx))),
            )
            .child(
                icon_button("cancel-save", "close", "Cancel")
                    .small()
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.show_save = false;
                        cx.notify();
                    })),
            )
    }

    fn collection(&self, width: f32, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = pal(cx);
        let body = if let Some(error) = &self.query_error {
            self.problem("That rule needs a fix", error.clone(), cx).into_any_element()
        } else if self.loading && self.tracks.is_empty() && !self.page.is_grid() {
            self.skeleton(cx).into_any_element()
        } else if self.page.is_grid() && self.groups.is_empty() && self.loading {
            self.skeleton(cx).into_any_element()
        } else if self.page.is_grid() && !self.groups.is_empty() {
            self.grid(width, cx).into_any_element()
        } else if self.tracks.is_empty() {
            let search = self.search_text(cx);
            let (title, detail) = match &self.page {
                Page::Favorites => ("No favorites yet".to_string(), "Press the heart on any track, or Ctrl+D with tracks selected.".to_string()),
                Page::Recent => ("Nothing added in the last 30 days".into(), "New files in your music folders appear here automatically.".into()),
                Page::Playlist(_) if search.is_empty() => ("This playlist is empty".into(), "Right-click tracks anywhere and choose the playlist to add them.".into()),
                _ if !search.is_empty() => (format!("Nothing matches “{search}”"), "Try fewer words, or a rule such as  artist contains \"Nick\".".into()),
                _ => ("Nothing to show".into(), "Try another view.".into()),
            };
            self.problem(title, detail, cx).into_any_element()
        } else {
            self.table(width, window, cx).into_any_element()
        };
        div()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .child(self.header(cx))
            .children(self.playlist_tools(cx))
            .when(self.show_save, |el| el.child(self.save_form(cx)))
            .child(body)
            .when(self.page.is_tracks() && !self.page.is_grid() && self.matched_total > PAGE_SIZE, |el| {
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
                                    icon_button("previous-page", "chevron-left", "Previous 1,000")
                                        .small()
                                        .disabled(self.page_offset == 0)
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.page_offset = this.page_offset.saturating_sub(PAGE_SIZE);
                                            this.refresh(cx);
                                        })),
                                )
                                .child(
                                    icon_button("next-page", "chevron-right", "Next 1,000")
                                        .small()
                                        .disabled(self.page_offset + self.tracks.len() >= self.matched_total)
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.page_offset += PAGE_SIZE;
                                            this.refresh(cx);
                                        })),
                                ),
                        ),
                )
            })
    }

    fn problem(&self, title: impl Into<SharedString>, detail: impl Into<SharedString>, cx: &App) -> impl IntoElement {
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
            .child(div().text_size(px(15.)).font_weight(FontWeight::MEDIUM).text_center().child(title.into()))
            .child(meta(detail, cx).max_w(px(460.)).text_center().line_height(relative(1.5)))
    }

    fn skeleton(&self, cx: &App) -> impl IntoElement {
        let p = pal(cx);
        div().flex_1().px_6().pt_2().flex().flex_col().children((0..9).map(|i| {
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
                        .child(div().h(px(10.)).w(relative(0.25 + (i % 3) as f32 * 0.08)).rounded(px(3.)).bg(p.raised))
                        .child(div().h(px(8.)).w(relative(0.16 + (i % 4) as f32 * 0.05)).rounded(px(3.)).bg(p.raised.opacity(0.7))),
                )
        }))
    }

    fn sort_label(&self, id: &'static str, label: &'static str, field: &'static str, cx: &mut Context<Self>) -> Stateful<Div> {
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
            .when_some(state, |el, g| el.child(glyph(g).size(px(12.)).text_color(p.ink)))
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
        let show_album = width > 820. && !album_view;
        let show_quality = width > 680.;
        div()
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
                    .when(!album_view, |el| el.child(div().w(px(36.))))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .gap_1()
                            .child(self.sort_label("sort-title", "Title", "title", cx))
                            .when(!album_view, |el| {
                                el.child("·").child(self.sort_label("sort-artist", "Artist", "artist", cx))
                            }),
                    )
                    .when(show_album, |el| {
                        el.child(div().w(relative(0.3)).flex_shrink_0().child(self.sort_label("sort-album", "Album", "album", cx)))
                    })
                    .when(show_quality, |el| el.child(div().w(px(92.)).child(self.sort_label("sort-format", "Quality", "format", cx))))
                    .child(div().w(px(28.)))
                    .child(div().w(px(46.)).flex().justify_end().child(self.sort_label("sort-time", "Time", "duration", cx)))
                    .child(div().w(px(28.))),
            )
            .child(
                uniform_list(
                    "track-list",
                    self.tracks.len(),
                    cx.processor(move |this, range: std::ops::Range<usize>, _, cx| {
                        range
                            .map(|index| this.track_row(index, album_view, show_album, show_quality, cx))
                            .collect::<Vec<_>>()
                    }),
                )
                .track_scroll(self.list_scroll.clone())
                .flex_1()
                .pb_4(),
            )
    }

    fn track_row(&self, index: usize, album_view: bool, show_album: bool, show_quality: bool, cx: &mut Context<Self>) -> Div {
        let p = pal(cx);
        let track = &self.tracks[index];
        let selected = self.selection.ids.contains(&track.id);
        let cursor = self.selection.cursor == Some(index);
        let playing = self.playback.current.as_ref().is_some_and(|i| i.track.id == track.id);
        let favorite = track.rating >= 4;
        let id = track.id.clone();
        let height = self.settings.layout.row_height.clamp(44., 76.);
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
            .when(cursor && selected, |el| el.border_1().border_color(p.accent.opacity(0.28)))
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
                        glyph(if self.playback.playing { "volume" } else { "pause" })
                            .size(px(15.))
                            .text_color(p.accent)
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
            .when(!album_view, |el| el.child(artwork(Some(track), 36., cx)))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
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
                    .when(!album_view || track.artist != track.album_artist && !track.album_artist.is_empty(), |el| {
                        el.child(
                            div()
                                .truncate()
                                .text_size(px(12.))
                                .text_color(p.ink_2)
                                .child(if show_album || album_view {
                                    track.display_artist().to_string()
                                } else {
                                    format!("{} · {}", track.display_artist(), track.display_album())
                                }),
                        )
                    }),
            )
            .when(show_album, |el| {
                el.child(
                    div()
                        .w(relative(0.3))
                        .flex_shrink_0()
                        .min_w_0()
                        .truncate()
                        .text_size(px(12.5))
                        .text_color(p.ink_2)
                        .child(track.display_album().to_string()),
                )
            })
            .when(show_quality, |el| {
                el.child(div().w(px(92.)).flex_shrink_0().truncate().text_size(px(11.5)).text_color(p.ink_3).child(quality(track)))
            })
            .child(
                div()
                    .id(("row-fav", index))
                    .w(px(28.))
                    .flex_shrink_0()
                    .flex()
                    .justify_center()
                    .cursor_pointer()
                    .when(!favorite, |el| el.opacity(0.).group_hover("row", |s| s.opacity(1.)))
                    .text_color(if favorite { p.accent } else { p.ink_3 })
                    .hover(|s| s.text_color(p.accent))
                    .child(glyph(if favorite { "heart-fill" } else { "heart" }).size(px(16.)).text_color(if favorite { p.accent } else { p.ink_2 }))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        this.set_rating(&[id.clone()], if favorite { 0 } else { 5 });
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .w(px(46.))
                    .flex_shrink_0()
                    .text_right()
                    .text_size(px(12.5))
                    .text_color(p.ink_2)
                    .child(format_duration(track.duration)),
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
            .on_click(cx.listener(move |this, event: &ClickEvent, window, cx| this.click_track(index, event, window, cx)))
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, event: &MouseDownEvent, _, cx| this.open_menu(index, event.position, cx)),
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
        uniform_list(
            "group-grid",
            rows,
            cx.processor(move |this, range: std::ops::Range<usize>, _, cx| {
                range
                    .map(|row| {
                        div()
                            .px_6()
                            .pb(px(26.))
                            .flex()
                            .gap(px(gap))
                            .children((row * columns..((row + 1) * columns).min(this.groups.len())).map(|i| this.tile(i, tile, artists, cx)))
                    })
                    .collect::<Vec<_>>()
            }),
        )
        .flex_1()
    }

    fn tile(&self, index: usize, size: f32, round: bool, cx: &mut Context<Self>) -> Stateful<Div> {
        let p = pal(cx);
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
                    .child(if round {
                        div()
                            .size(px(size))
                            .rounded_full()
                            .bg(p.raised)
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_size(px(size * 0.24))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(p.ink_3)
                            .group_hover("tile", |s| s.bg(p.raised_hover).text_color(p.ink_2))
                            .child(initials(&group.title))
                            .into_any_element()
                    } else {
                        div()
                            .rounded(px(8.))
                            .group_hover("tile", |s| s.opacity(0.86))
                            .child(cover(group.artwork.as_deref(), &group.seed, size, cx))
                            .into_any_element()
                    }),
            )
            .child(
                div()
                    .text_size(px(13.5))
                    .font_weight(FontWeight::MEDIUM)
                    .truncate()
                    .when(round, |el| el.text_center())
                    .child(group.title.clone()),
            )
            .child(meta(group.subtitle.clone(), cx).truncate().when(round, |el| el.text_center()))
            .on_click(cx.listener(move |this, _, window, cx| this.navigate(page.clone(), window, cx)))
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
