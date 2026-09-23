//! "Your year in music": a year of listening told back, from the listening history.
use super::{
    AppView, Event, Page, album_page,
    library::human_duration,
    pal,
    widgets::{count, cover, display, faint, meta, small_button},
};
use gpui::{prelude::*, *};
use gpui_component::button::ButtonVariants;
use needle_core::{
    history::HistoryTop,
    model::{Playlist, Track},
    wrapped::Wrapped,
};

/// Text with some parts picked out: `(text, emphasised)`.
fn rich(parts: &[(&str, bool)], emphasis: Hsla) -> StyledText {
    let mut text = String::new();
    let mut highlights = vec![];
    for (part, strong) in parts {
        let start = text.len();
        text.push_str(part);
        if *strong {
            highlights.push((
                start..text.len(),
                HighlightStyle {
                    color: Some(emphasis),
                    font_weight: Some(FontWeight::BOLD),
                    ..Default::default()
                },
            ));
        }
    }
    StyledText::new(text).with_highlights(highlights)
}

fn plays(n: usize) -> String {
    if n == 1 {
        "1 play".into()
    } else {
        format!("{} plays", count(n))
    }
}

/// "11 PM", "6 AM".
fn hour_name(hour: u8) -> String {
    match hour {
        0 => "midnight".into(),
        12 => "noon".into(),
        h if h < 12 => format!("{h} AM"),
        h => format!("{} PM", h - 12),
    }
}

/// What the busiest hour says about someone.
fn listener_kind(hour: u8) -> &'static str {
    match hour {
        5..=11 => "An early riser.",
        12..=17 => "An afternoon listener.",
        18..=21 => "An evening listener.",
        _ => "A night owl.",
    }
}

/// The year to open first: last year during January, when this one has barely begun.
pub fn default_year(years: &[i32]) -> i32 {
    use chrono::Datelike;
    let today = chrono::Local::now().date_naive();
    let this = today.year();
    if today.month() == 1 && years.contains(&(this - 1)) {
        this - 1
    } else {
        this
    }
}

impl AppView {
    pub(super) fn load_wrapped(&mut self, year: i32) {
        if self.wrapped.as_ref().is_some_and(|w| w.year == year) {
            return;
        }
        self.wrapped = None;
        let library = self.library.clone();
        let sender = self.sender.clone();
        std::thread::spawn(move || {
            let result = (|| -> anyhow::Result<_> {
                Ok((library.wrapped(year)?, library.listening_years()?))
            })();
            let _ = sender.send(match result {
                Ok((wrapped, years)) => Event::Wrapped(Box::new(wrapped), years),
                Err(e) => Event::Error(format!("{e:#}")),
            });
        });
    }

    pub(super) fn wrapped_loaded(&mut self, wrapped: Wrapped, years: Vec<i32>) {
        for artist in wrapped.stats.top_artists.iter().take(5) {
            self.request_artist_image(&artist.artist, true);
        }
        self.wrapped_years = years;
        self.wrapped = Some(wrapped);
    }

    /// Open the year in music, the one that makes most sense right now.
    pub(super) fn open_wrapped(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let years = self.library.listening_years().unwrap_or_default();
        self.navigate(Page::Wrapped(default_year(&years)), window, cx);
    }

    fn wrapped_tracks(wrapped: &Wrapped) -> Vec<Track> {
        wrapped
            .top_track_ids
            .iter()
            .filter_map(|id| wrapped.tracks.get(id).cloned())
            .collect()
    }

    fn save_wrapped_playlist(&mut self, cx: &mut Context<Self>) {
        let Some(wrapped) = &self.wrapped else { return };
        let name = format!("Top songs of {}", wrapped.year);
        let ids: Vec<String> = Self::wrapped_tracks(wrapped)
            .into_iter()
            .map(|t| t.id)
            .collect();
        // Saving again refreshes the same playlist instead of making another.
        let existing = self
            .playlists
            .iter()
            .find(|p| p.name == name && p.query.is_none())
            .map(|p| p.id.clone());
        let playlist = Playlist {
            id: existing.unwrap_or_else(crate::uuid_string),
            name: name.clone(),
            query: None,
            track_ids: ids,
            updated_at: chrono::Utc::now().timestamp(),
        };
        match self.library.save_playlist(&playlist) {
            Ok(()) => {
                self.playlists = self.library.playlists().unwrap_or_default();
                self.notify(format!("Saved “{name}”."));
            }
            Err(e) => self.fail(e.to_string()),
        }
        cx.notify();
    }

    fn wrapped_section(&self, title: &str, cx: &App) -> Div {
        div()
            .mt(px(56.))
            .flex()
            .flex_col()
            .gap_4()
            .child(display(title.to_string(), 28.).text_color(pal(cx).ink))
    }

    /// One artist row: `rank` shown, `id` unique on the page.
    fn wrapped_artist(
        &self,
        artist: &HistoryTop,
        rank: usize,
        id: usize,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let page = Page::Artist(artist.artist.clone());
        div()
            .id(("wrapped-artist", id))
            .flex()
            .items_center()
            .gap_3()
            .cursor_pointer()
            .hover(|s| s.opacity(0.85))
            .child(faint(rank.to_string(), cx).w(px(16.)).text_right())
            .child(self.artist_photo(&artist.artist, 44., cx))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .child(
                        div()
                            .text_size(px(14.))
                            .font_weight(FontWeight::MEDIUM)
                            .truncate()
                            .child(artist.artist.clone()),
                    )
                    .child(faint(
                        format!(
                            "{} · {}",
                            plays(artist.plays),
                            human_duration(artist.seconds)
                        ),
                        cx,
                    )),
            )
            .on_click(
                cx.listener(move |this, _, window, cx| this.navigate(page.clone(), window, cx)),
            )
            .into_any_element()
    }

    pub(super) fn wrapped_view(&self, year: i32, width: f32, cx: &mut Context<Self>) -> AnyElement {
        let p = pal(cx);
        let years = self.wrapped_years.clone();
        let header = div()
            .flex()
            .items_end()
            .gap_4()
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(faint("Your year in music", cx))
                    .child(display(year.to_string(), 64.)),
            )
            .when(years.len() > 1, |el| {
                // One chip per year, newest first.
                el.child(
                    div()
                        .flex()
                        .p(px(3.))
                        .gap(px(2.))
                        .rounded(px(8.))
                        .bg(p.raised)
                        .children(years.iter().take(6).map(|&y| {
                            let active = y == year;
                            div()
                                .id(("wrapped-year", y as usize))
                                .px_3()
                                .py(px(5.))
                                .rounded(px(6.))
                                .text_size(px(13.))
                                .cursor_pointer()
                                .when(active, |el| {
                                    el.bg(p.canvas)
                                        .text_color(p.ink)
                                        .font_weight(FontWeight::MEDIUM)
                                })
                                .when(!active, |el| {
                                    el.text_color(p.ink_2).hover(|s| s.text_color(p.ink))
                                })
                                .child(y.to_string())
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.navigate(Page::Wrapped(y), window, cx)
                                }))
                        })),
                )
            });
        let page = |body: Div| {
            div()
                .id("wrapped")
                .size_full()
                .overflow_y_scroll()
                .child(
                    div()
                        .px_8()
                        .pt_6()
                        .pb(px(72.))
                        .flex()
                        .flex_col()
                        .child(header)
                        .child(body),
                )
                .into_any_element()
        };
        let Some(w) = self.wrapped.as_ref().filter(|w| w.year == year) else {
            return page(div().mt_6().child(meta("Gathering your year…", cx)));
        };
        let s = &w.stats;
        if s.listens == 0 {
            return page(div().mt_6().flex().flex_col().gap_2().child(meta(
                format!("No listening in {year} yet. Play some music and come back."),
                cx,
            )));
        }
        let hours = (s.seconds / 3600.).round() as usize;
        let minutes = (s.seconds / 60.).round() as usize;
        let time = if hours >= 2 {
            format!("{} hours", count(hours))
        } else {
            format!("{} minutes", count(minutes))
        };
        let (plays_text, songs_text, artists_text) = (
            plays(s.plays),
            count(s.distinct_tracks),
            count(s.distinct_artists),
        );
        let opening = div()
            .mt_4()
            .max_w(px(760.))
            .text_size(px(24.))
            .line_height(relative(1.4))
            .text_color(p.ink_2)
            .child(rich(
                &[
                    ("You spent ", false),
                    (&time, true),
                    (" with your music: ", false),
                    (&plays_text, true),
                    (" of ", false),
                    (&songs_text, true),
                    (
                        if s.distinct_tracks == 1 {
                            " song by "
                        } else {
                            " songs by "
                        },
                        false,
                    ),
                    (&artists_text, true),
                    (
                        if s.distinct_artists == 1 {
                            " artist."
                        } else {
                            " artists."
                        },
                        false,
                    ),
                ],
                p.ink,
            ));
        // The year's albums, as a row of covers.
        let covers: Vec<AnyElement> = s
            .top_albums
            .iter()
            .zip(&w.album_tracks)
            .take(5)
            .enumerate()
            .map(|(i, (album, track))| {
                let track = track.as_ref().and_then(|id| w.tracks.get(id));
                let size = if i == 0 { 150. } else { 118. };
                let target = album_page(&Track {
                    album: album.album.clone(),
                    album_artist: album.artist.clone(),
                    ..Default::default()
                });
                div()
                    .id(("wrapped-album", i))
                    .w(px(size))
                    .flex_shrink_0()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .cursor_pointer()
                    .hover(|s| s.opacity(0.85))
                    .child(cover(
                        track.and_then(|t| t.artwork.as_deref()),
                        &album.album,
                        size,
                        cx,
                    ))
                    .child(
                        div()
                            .mt_1()
                            .text_size(px(13.))
                            .font_weight(FontWeight::MEDIUM)
                            .truncate()
                            .child(album.album.clone()),
                    )
                    .child(
                        faint(format!("{} · {}", album.artist, plays(album.plays)), cx).truncate(),
                    )
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.navigate(target.clone(), window, cx)
                    }))
                    .into_any_element()
            })
            .collect();

        let mut body = div().flex().flex_col().child(opening);

        if let Some(top) = s.top_artists.first() {
            let name = top.artist.clone();
            let photo = if width > 640. { 168. } else { 112. };
            body = body.child(
                self.wrapped_section("Your artist of the year", cx).child(
                    div()
                        .flex()
                        .gap_8()
                        .items_center()
                        .child(
                            div()
                                .id("wrapped-top-artist")
                                .cursor_pointer()
                                .child(self.artist_photo(&name, photo, cx))
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.navigate(Page::Artist(name.clone()), window, cx)
                                })),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .flex_col()
                                .gap_2()
                                .child(display(top.artist.clone(), 40.))
                                .child(meta(
                                    format!(
                                        "{} and {} together.",
                                        plays(top.plays),
                                        human_duration(top.seconds)
                                    ),
                                    cx,
                                ))
                                .when(s.top_artists.len() > 1, |el| {
                                    el.child(
                                        div().mt_4().flex().flex_col().gap_3().children(
                                            s.top_artists
                                                .iter()
                                                .enumerate()
                                                .skip(1)
                                                .take(4)
                                                .map(|(i, a)| self.wrapped_artist(a, i + 1, i, cx))
                                                .collect::<Vec<_>>(),
                                        ),
                                    )
                                }),
                        ),
                ),
            );
        }

        let songs: Vec<AnyElement> = w
            .top_track_ids
            .iter()
            .take(5)
            .enumerate()
            .map(|(i, id)| {
                let track = w.tracks.get(id);
                let top = s
                    .top_tracks
                    .iter()
                    .find(|t| t.track_id.as_deref() == Some(id.as_str()));
                let title = track
                    .map(|t| t.title.clone())
                    .or_else(|| top.map(|t| t.title.clone()))
                    .unwrap_or_default();
                let artist = track
                    .map(|t| t.display_artist().to_string())
                    .or_else(|| top.map(|t| t.artist.clone()))
                    .unwrap_or_default();
                let count = top.map(|t| t.plays).unwrap_or_default();
                let clicked = track.cloned();
                div()
                    .id(("wrapped-song", i))
                    .flex()
                    .items_center()
                    .gap_4()
                    .px_2()
                    .py_1()
                    .rounded(px(8.))
                    .cursor_pointer()
                    .hover(|s| s.bg(p.raised.opacity(0.6)))
                    .child(
                        div()
                            .w(px(28.))
                            .text_right()
                            .text_size(px(20.))
                            .font_weight(FontWeight::BOLD)
                            .text_color(if i == 0 { p.accent } else { p.ink_3 })
                            .child(format!("{}", i + 1)),
                    )
                    .child(match track {
                        Some(t) => cover(
                            t.artwork.as_deref(),
                            &super::widgets::track_seed(t),
                            52.,
                            cx,
                        ),
                        None => cover(None, &title, 52., cx),
                    })
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(
                                div()
                                    .text_size(px(15.))
                                    .font_weight(FontWeight::MEDIUM)
                                    .truncate()
                                    .child(title),
                            )
                            .child(meta(artist, cx).truncate()),
                    )
                    .when(count > 0, |el| {
                        el.child(faint(plays(count), cx).flex_shrink_0())
                    })
                    .on_click(cx.listener(move |this, _, _, _| {
                        if let Some(t) = clicked.clone() {
                            this.play_tracks(vec![t], "One of your songs of the year");
                        }
                    }))
                    .into_any_element()
            })
            .collect();
        let top_count = w.top_track_ids.len();
        body = body.child(
            self.wrapped_section("Songs on repeat", cx)
                .child(
                    div()
                        .max_w(px(640.))
                        .flex()
                        .flex_col()
                        .gap_1()
                        .children(songs),
                )
                .child(
                    div()
                        .flex()
                        .gap_2()
                        .child(
                            small_button("wrapped-play", format!("Play your top {top_count}"))
                                .primary()
                                .on_click(cx.listener(|this, _, _, _| {
                                    if let Some(w) = &this.wrapped {
                                        let tracks = Self::wrapped_tracks(w);
                                        let reason = format!("Your {} in music", w.year);
                                        this.play_tracks(tracks, &reason);
                                    }
                                })),
                        )
                        .child(
                            small_button("wrapped-save", "Save as a playlist")
                                .ghost()
                                .on_click(
                                    cx.listener(|this, _, _, cx| this.save_wrapped_playlist(cx)),
                                ),
                        ),
                ),
        );

        if !covers.is_empty() {
            body = body.child(
                self.wrapped_section("Albums you lived in", cx).child(
                    div()
                        .id("wrapped-albums")
                        .overflow_x_scroll()
                        .child(div().flex().items_end().gap_5().children(covers)),
                ),
            );
        }

        // The rhythm: when, how much, how steadily.
        let mut rhythm = self.wrapped_section("Your rhythm", cx);
        if let Some(hour) = w.peak_hour {
            let hour_text = hour_name(hour);
            rhythm = rhythm.child(
                div()
                    .text_size(px(20.))
                    .line_height(relative(1.4))
                    .text_color(p.ink_2)
                    .child(rich(
                        &[
                            ("You listen most around ", false),
                            (&hour_text, true),
                            (". ", false),
                            (listener_kind(hour), false),
                        ],
                        p.ink,
                    )),
            );
            let bars: Vec<(String, f64)> = s
                .hours
                .iter()
                .map(|h| (hour_name(h.hour), h.seconds))
                .collect();
            rhythm = rhythm.child(
                div()
                    .max_w(px(640.))
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(self.bars("wrapped-hour", bars, 64., cx))
                    .child(
                        div()
                            .flex()
                            .justify_between()
                            .child(faint("midnight", cx))
                            .child(faint("noon", cx))
                            .child(faint("11 PM", cx)),
                    ),
            );
        }
        let mut facts: Vec<(String, String)> = vec![];
        if let Some(day) = &w.busiest_day
            && let Ok(date) = chrono::NaiveDate::parse_from_str(&day.date, "%Y-%m-%d")
        {
            facts.push((
                "Your biggest day".into(),
                format!(
                    "{}: {}",
                    date.format("%A %-d %B"),
                    human_duration(day.seconds)
                ),
            ));
        }
        if w.streak > 1 {
            facts.push((
                "Your longest streak".into(),
                format!("{} days in a row", w.streak),
            ));
        }
        let active = s.days.iter().filter(|d| d.listens > 0).count();
        facts.push((
            "Days with music".into(),
            format!("{active} of {}", s.days.len()),
        ));
        rhythm = rhythm.child(div().mt_2().flex().gap_10().children(facts.into_iter().map(
            |(label, value)| {
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(faint(label, cx))
                    .child(
                        div()
                            .text_size(px(17.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(value),
                    )
            },
        )));
        body = body.child(rhythm);

        if !w.genres.is_empty() {
            let peak = w.genres[0].1.max(1.);
            let total: f64 = w.genres.iter().map(|g| g.1).sum::<f64>().max(1.);
            // Pixel widths: percentages do not resolve inside these columns.
            let track = (width - 120.).clamp(160., 560.);
            body = body.child(
                self.wrapped_section("What it sounded like", cx).child(
                    div()
                        .w(px(track))
                        .flex()
                        .flex_col()
                        .gap_3()
                        .children(w.genres.iter().map(|(name, seconds)| {
                            div()
                                .flex()
                                .flex_col()
                                .gap_1()
                                .child(
                                    div()
                                        .flex()
                                        .justify_between()
                                        .child(
                                            div()
                                                .text_size(px(14.))
                                                .font_weight(FontWeight::MEDIUM)
                                                .child(name.clone()),
                                        )
                                        .child(faint(
                                            format!("{:.0}%", seconds / total * 100.),
                                            cx,
                                        )),
                                )
                                .child(
                                    div()
                                        .w(px(track))
                                        .h(px(6.))
                                        .rounded_full()
                                        .bg(p.line)
                                        .child(
                                            div()
                                                .h_full()
                                                .rounded_full()
                                                .bg(p.accent)
                                                .w(px(track * (seconds / peak) as f32)),
                                        ),
                                )
                        })),
                ),
            );
        }

        if !w.new_artists.is_empty() {
            body = body.child(
                self.wrapped_section("New to you", cx)
                    .child(meta(format!("Artists you first played in {year}."), cx))
                    .child(
                        div().flex().flex_col().gap_3().children(
                            w.new_artists
                                .iter()
                                .enumerate()
                                .map(|(i, a)| self.wrapped_artist(a, i + 1, 10 + i, cx))
                                .collect::<Vec<_>>(),
                        ),
                    ),
            );
        }
        page(body)
    }
}

#[cfg(test)]
mod tests {
    use super::{hour_name, listener_kind};

    #[test]
    fn hours_read_naturally() {
        assert_eq!(hour_name(0), "midnight");
        assert_eq!(hour_name(9), "9 AM");
        assert_eq!(hour_name(12), "noon");
        assert_eq!(hour_name(23), "11 PM");
        assert_eq!(listener_kind(23), "A night owl.");
        assert_eq!(listener_kind(7), "An early riser.");
    }
}
