//! Two pages about the library as a whole: Your library (what it is made of) and the genre map
//! (every genre a bubble, as big as its songs, where its songs sound; click one to play it).
use super::{
    AppView, Event, Page, pal,
    widgets::{display, faint, meta, page_title, small_button},
};
use gpui::{prelude::*, *};
use needle_core::radio::GenreSpot;

/// The genres drawn as bubbles; the rest are chips under the map.
const BUBBLES: usize = 40;

impl AppView {
    /// Load what the page needs, when it opens.
    pub(super) fn load_insights(&mut self) {
        let (library, sender) = (self.library.clone(), self.sender.clone());
        match self.page {
            Page::Numbers => {
                std::thread::spawn(move || {
                    let _ = sender.send(match library.numbers() {
                        Ok(n) => Event::Numbers(Box::new(n)),
                        Err(e) => Event::Error(format!("{e:#}")),
                    });
                });
            }
            Page::MoodMap => {
                std::thread::spawn(move || {
                    let _ = sender.send(match library.genre_map() {
                        Ok(spots) => Event::GenreMap(spots),
                        Err(e) => Event::Error(format!("{e:#}")),
                    });
                });
                self.load_genre_art();
            }
            _ => {}
        }
    }

    /// The cloud or bubbles, laid out by the core for the map's shape (when one shows).
    fn load_genre_art(&mut self) {
        let (view, plays) = (self.genre_view, self.genre_by_plays);
        if view == 0 {
            return;
        }
        let (library, sender) = (self.library.clone(), self.sender.clone());
        std::thread::spawn(move || {
            use needle_core::genre_art::ArtKind;
            let (kind, most) = if view == 1 {
                (ArtKind::Cloud, 120)
            } else {
                (ArtKind::Bubbles, 160)
            };
            let _ = sender.send(match library.genre_art(kind, plays, 1.6, most) {
                Ok(art) => Event::GenreArt(view, plays, Box::new(art)),
                Err(e) => Event::Error(format!("{e:#}")),
            });
        });
    }

    /// Plays a genre written any of these ways, shuffled.
    fn play_genres(&mut self, spellings: &[String]) {
        match self.library.songs_of_genres(spellings, 500) {
            Ok(tracks) if !tracks.is_empty() => self.play_tracks(tracks, "From the genre map"),
            Ok(_) => {}
            Err(e) => self.fail(format!("{e:#}")),
        }
    }

    /// The cloud (words) or bubbles (circles by family), drawn where the core placed them, as
    /// wide as `side` (or as tall as `most_height`, when the picture is the taller).
    fn genre_art_view(&self, side: f32, most_height: f32, cx: &mut Context<Self>) -> Div {
        let p = pal(cx);
        let Some((view, plays, art)) = self.genre_art.clone() else {
            return div().child(meta("Laying it out…", cx));
        };
        if view != self.genre_view || plays != self.genre_by_plays {
            return div().child(meta("Laying it out…", cx));
        }
        if art.items.is_empty() {
            return div().child(meta(
                if plays {
                    "Nothing played yet."
                } else {
                    "Your songs have no genres in their tags yet."
                },
                cx,
            ));
        }
        let scale = side.min(most_height / art.height.max(0.01));
        let cloud = view == 1;
        let items = art.items.iter().enumerate().map(|(i, item)| {
            let tone = hsla(
                item.hue / 360.,
                if p.dark { 0.5 } else { 0.6 },
                if p.dark { 0.68 } else { 0.4 },
                1.,
            );
            let spellings = item.spellings.clone();
            let (x, y, size) = (item.x * scale, item.y * scale, item.size * scale);
            let tip = format!(
                "{}: {} songs, {} plays · {}",
                item.genre, item.songs, item.plays, item.family
            );
            let el = if cloud {
                // A box as wide as the core reckoned the word, the word centred in it.
                let w = needle_core::genre_art::word_width(&item.genre) * size;
                let h = size * 1.3;
                div()
                    .id(("genre-word", i))
                    .absolute()
                    .left(px(x - w / 2.))
                    .top(px(y - h / 2.))
                    .w(px(w))
                    .h(px(h))
                    .flex()
                    .items_center()
                    .justify_center()
                    .whitespace_nowrap()
                    .text_size(px(size.max(6.)))
                    .line_height(px(h))
                    .font_weight(FontWeight::BOLD)
                    .text_color(tone)
                    .cursor_pointer()
                    .hover(move |s| s.text_color(tone.opacity(0.75)))
                    .child(item.genre.clone())
            } else {
                div()
                    .id(("genre-circle", i))
                    .absolute()
                    .left(px(x - size))
                    .top(px(y - size))
                    .size(px(size * 2.))
                    .rounded_full()
                    .bg(tone.opacity(0.9))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .hover(move |s| s.bg(tone))
                    // As big as fits, its longest word on one line; no name when too small.
                    .when_some(
                        Some((size * 0.32).clamp(8., 20.).min(size * item.fit))
                            .filter(|f| *f >= 7.),
                        |el, font| {
                            el.child(
                                div()
                                    .w(px(size * 1.6))
                                    .text_center()
                                    .overflow_hidden()
                                    .text_size(px(font))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(if p.dark {
                                        hsla(0., 0., 0., 0.85)
                                    } else {
                                        hsla(0., 0., 1., 1.)
                                    })
                                    .child(item.genre.clone()),
                            )
                        },
                    )
            };
            el.tooltip(move |window, cx| {
                gpui_component::tooltip::Tooltip::new(tip.clone()).build(window, cx)
            })
            .on_click(cx.listener(move |this, _, _, _| this.play_genres(&spellings)))
        });
        div()
            .relative()
            .w(px(scale))
            .h(px(art.height * scale))
            .children(items)
    }

    pub(super) fn numbers_view(&self, width: f32, cx: &mut Context<Self>) -> AnyElement {
        let p = pal(cx);
        let Some(n) = self.numbers.as_deref() else {
            return div()
                .p_8()
                .child(page_title("Your library"))
                .child(meta("Counting…", cx))
                .into_any_element();
        };
        let percent = |part: usize| {
            if n.songs == 0 {
                0.
            } else {
                part as f32 / n.songs as f32 * 100.
            }
        };
        let column = (width - 64. - 24.).max(320.) / 2.;
        let bars = |title: &'static str, rows: Vec<(String, usize)>, cx: &App| -> Div {
            let p = pal(cx);
            let top = rows.iter().map(|r| r.1).max().unwrap_or(1).max(1);
            div()
                .w(px(column))
                .flex()
                .flex_col()
                .gap(px(8.))
                .child(display(title, 20.))
                .children(rows.into_iter().map(move |(label, count)| {
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(3.))
                        .child(
                            div()
                                .flex()
                                .justify_between()
                                .text_size(px(12.5))
                                .child(label)
                                .child(
                                    div()
                                        .text_color(p.ink_3)
                                        .child(super::widgets::count(count)),
                                ),
                        )
                        .child(
                            div().h(px(8.)).rounded(px(4.)).bg(p.raised).child(
                                div()
                                    .h_full()
                                    .rounded(px(4.))
                                    .bg(p.accent)
                                    .w(relative(count as f32 / top as f32)),
                            ),
                        )
                }))
        };
        let figure = |value: String, label: &'static str, cx: &App| {
            div()
                .flex()
                .flex_col()
                .child(display(value, 30.))
                .child(faint(label, cx))
        };
        let hours = n.duration / 3600.;
        let size = if n.size > 1 << 30 {
            format!("{:.1} GB", n.size as f64 / (1u64 << 30) as f64)
        } else {
            format!("{:.0} MB", n.size as f64 / (1u64 << 20) as f64)
        };
        let fix = |title: &'static str,
                   count: usize,
                   albums: &[needle_core::browse::AlbumSummary],
                   cx: &App| {
            let p = pal(cx);
            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(display(title, 20.))
                .child(meta(
                    if count == 0 {
                        "None.".to_string()
                    } else {
                        format!("{} albums", super::widgets::count(count))
                    },
                    cx,
                ))
                .children(albums.iter().take(8).map(|a| {
                    div()
                        .text_size(px(12.5))
                        .text_color(p.ink_2)
                        .truncate()
                        .child(format!("{} · {}", a.album, a.artist))
                }))
        };
        div()
            .id("numbers")
            .size_full()
            .overflow_y_scroll()
            .track_scroll(&self.page_scroll)
            .px_8()
            .pt_8()
            .pb(px(40.) + self.foot_room())
            .flex()
            .flex_col()
            .gap(px(28.))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(page_title("Your library"))
                    .child(meta("What your music is made of.", cx)),
            )
            .child(
                div()
                    .flex()
                    .gap(px(36.))
                    .child(figure(super::widgets::count(n.songs), "songs", cx))
                    .child(figure(super::widgets::count(n.albums), "albums", cx))
                    .child(figure(super::widgets::count(n.artists), "artists", cx))
                    .child(figure(format!("{hours:.0}"), "hours", cx))
                    .when(n.size > 0, |el| {
                        el.child(figure(size, "on this computer", cx))
                    }),
            )
            .child(
                div()
                    .flex()
                    .gap(px(36.))
                    .child(figure(
                        format!("{:.0}%", percent(n.lossless)),
                        "lossless",
                        cx,
                    ))
                    .child(figure(
                        format!("{:.0}%", percent(n.hires)),
                        "hi-res (above 16 bits or 48 kHz)",
                        cx,
                    ))
                    // With lossless, the whole library; low quality is a part of it.
                    .child(figure(format!("{:.0}%", percent(n.lossy)), "lossy", cx))
                    .child(figure(
                        format!("{:.0}%", percent(n.low_quality_songs)),
                        "low quality (under 192 kbit/s)",
                        cx,
                    ))
                    .when(n.server_songs > 0, |el| {
                        el.child(figure(
                            super::widgets::count(n.server_songs),
                            "from your server",
                            cx,
                        ))
                    }),
            )
            .child(
                div()
                    .flex()
                    .gap(px(24.))
                    .items_start()
                    .child(bars(
                        "Formats",
                        n.formats.iter().take(8).cloned().collect(),
                        cx,
                    ))
                    .child(bars("Decades", n.decades.clone(), cx)),
            )
            .child(
                div()
                    .flex()
                    .gap(px(24.))
                    .items_start()
                    .child(bars("Genres", n.genres.clone(), cx))
                    .child(bars(
                        "Artists with the most songs",
                        n.artists_by_songs.clone(),
                        cx,
                    )),
            )
            .child(
                div()
                    .flex()
                    .gap(px(24.))
                    .items_start()
                    .child(
                        div()
                            .w(px(column))
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(fix(
                                "Albums without a cover",
                                n.albums_without_cover,
                                &n.without_cover,
                                cx,
                            ))
                            .when(
                                n.albums_without_cover > 0 && self.settings.online_media,
                                |el| {
                                    el.child(
                                        small_button("numbers-find-covers", "Find covers")
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                this.settings_tab = 2;
                                                this.navigate(Page::Settings, window, cx);
                                            })),
                                    )
                                },
                            ),
                    )
                    .child(
                        div()
                            .w(px(column))
                            .child(fix(
                                "Low quality albums",
                                n.low_quality_albums,
                                &n.low_quality,
                                cx,
                            ))
                            .child(faint("Every song lossy and under 192 kbit/s.", cx)),
                    ),
            )
            .child(div().h(px(1.)).bg(p.line_soft))
            .into_any_element()
    }

    /// Plays a genre's songs, shuffled.
    fn play_genre(&mut self, genre: &str) {
        match self.library.genre_songs(genre, 500) {
            Ok(tracks) if !tracks.is_empty() => self.play_tracks(tracks, "From the genre map"),
            Ok(_) => {}
            Err(e) => self.fail(format!("{e:#}")),
        }
    }

    pub(super) fn mood_view(&self, width: f32, cx: &mut Context<Self>) -> AnyElement {
        let p = pal(cx);
        let spots = self.genre_spots.clone();
        let side = (width - 64.).clamp(280., 980.);
        let map_height = (side * 0.62).clamp(280., 600.);
        let biggest = spots.first().map_or(1, |g| g.songs.max(1)) as f32;
        let (placed, rest): (Vec<&GenreSpot>, Vec<&GenreSpot>) = {
            let mut placed = vec![];
            let mut rest = vec![];
            for (i, spot) in spots.iter().enumerate() {
                if i < BUBBLES && spot.place.is_some() {
                    placed.push(spot);
                } else {
                    rest.push(spot);
                }
            }
            (placed, rest)
        };
        // Small bubbles last, so they sit on top of the large ones and stay clickable.
        let mut drawn = placed.clone();
        drawn.sort_by_key(|g| std::cmp::Reverse(g.songs));
        // Each bubble's centre and size, then nudged apart where they overlap, inside the map.
        let mut laid: Vec<(f32, f32, f32)> = drawn
            .iter()
            .map(|spot| {
                let (x, y) = spot.place.unwrap_or((0.5, 0.5));
                let d = 40. + 120. * (spot.songs as f32 / biggest).sqrt();
                (side * (0.06 + x * 0.88), map_height * (0.94 - y * 0.88), d)
            })
            .collect();
        for _ in 0..200 {
            for a in 0..laid.len() {
                for b in a + 1..laid.len() {
                    let (mut dx, mut dy) = (laid[b].0 - laid[a].0, laid[b].1 - laid[a].1);
                    // In one row or column (genres that sound alike): both ways, in turn.
                    if dy.abs() < 1. {
                        dy = if (a + b) % 2 == 0 { 6. } else { -6. };
                    }
                    if dx.abs() < 1. {
                        dx = if b % 2 == 0 { 6. } else { -6. };
                    }
                    let gap = (laid[a].2 + laid[b].2) / 2. + 4.;
                    let far = (dx * dx + dy * dy).sqrt();
                    if far < gap {
                        // Apart along the line between them (or sideways when on top).
                        let (ux, uy) = if far > 0.01 {
                            (dx / far, dy / far)
                        } else {
                            (1., 0.)
                        };
                        let push = (gap - far) / 2.;
                        laid[a].0 -= ux * push;
                        laid[a].1 -= uy * push;
                        laid[b].0 += ux * push;
                        laid[b].1 += uy * push;
                    }
                }
            }
            // Inside the map, clear of the words in its top and bottom corners.
            for spot in laid.iter_mut() {
                let r = spot.2 / 2.;
                spot.0 = spot.0.clamp(r + 4., side - r - 4.);
                spot.1 = spot.1.clamp(r + 28., map_height - r - 28.);
            }
        }
        let bubbles = drawn
            .into_iter()
            .zip(laid)
            .enumerate()
            .map(|(i, (spot, (cx_, cy_, d)))| {
                let color = super::widgets::seed_color(&spot.genre.to_lowercase());
                let tone = hsla(color.h, 0.55, if p.dark { 0.6 } else { 0.42 }, 1.);
                let genre = spot.genre.clone();
                div()
                    .id(("genre-bubble", i))
                    .absolute()
                    .left(px(cx_ - d / 2.))
                    .top(px(cy_ - d / 2.))
                    .size(px(d))
                    .rounded_full()
                    .bg(tone.opacity(0.22))
                    .border_1()
                    .border_color(tone.opacity(0.65))
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .px_2()
                    .cursor_pointer()
                    .hover(move |s| s.bg(tone.opacity(0.4)))
                    .child(
                        div()
                            .max_w_full()
                            .truncate()
                            .text_center()
                            .text_size(px((d / 7.).clamp(11., 17.)))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(p.ink)
                            .child(spot.genre.clone()),
                    )
                    .when(d > 74., |el| {
                        el.child(faint(
                            format!("{} songs", super::widgets::count(spot.songs)),
                            cx,
                        ))
                    })
                    .on_click(cx.listener(move |this, _, _, _| this.play_genre(&genre)))
            })
            .collect::<Vec<_>>();
        let chips = rest
            .into_iter()
            .enumerate()
            .map(|(i, spot)| {
                let genre = spot.genre.clone();
                div()
                    .id(("genre-chip", i))
                    .px_3()
                    .py(px(5.))
                    .rounded_full()
                    .bg(p.raised)
                    .border_1()
                    .border_color(p.line_soft)
                    .text_size(px(12.5))
                    .cursor_pointer()
                    .hover(|s| s.bg(p.raised.opacity(0.7)))
                    .flex()
                    .gap_2()
                    .child(div().child(spot.genre.clone()))
                    .child(faint(super::widgets::count(spot.songs), cx))
                    .on_click(cx.listener(move |this, _, _, _| this.play_genre(&genre)))
            })
            .collect::<Vec<_>>();
        let total = spots.len();
        let measured = spots.iter().any(|g| g.place.is_some());
        div()
            .id("genre-map")
            .size_full()
            .overflow_y_scroll()
            .track_scroll(&self.page_scroll)
            .px_8()
            .pt_8()
            .pb(px(40.) + self.foot_room())
            .flex()
            .flex_col()
            .gap_5()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(page_title("Genre map"))
                    .child(meta(
                        if self.genre_view != 0 {
                            format!(
                                "Every genre a {}, as big as its {}. Click one to play it.",
                                if self.genre_view == 1 { "word" } else { "circle, grouped by family" },
                                if self.genre_by_plays { "plays" } else { "songs" }
                            )
                        } else if total == 0 {
                            "Your songs have no genres in their tags yet.".to_string()
                        } else if !measured {
                            "Needle measures each song's sound in the background (Settings › Playback). Genres take their place as their songs are measured.".to_string()
                        } else {
                            format!("{} genres, each where its songs sound. Click one to play it.", super::widgets::count(total))
                        },
                        cx,
                    )),
            )
            // Map, Cloud, or Bubbles; the last two by songs or plays.
            .child({
                let weak = cx.entity().downgrade();
                let by = cx.entity().downgrade();
                div()
                    .flex()
                    .gap_3()
                    .child(super::widgets::segmented("genre-view", &["Map", "Cloud", "Bubbles"], self.genre_view, cx, move |i, _, cx| {
                        let _ = weak.update(cx, |this, cx| {
                            this.genre_view = i;
                            this.load_genre_art();
                            cx.notify();
                        });
                    }))
                    .when(self.genre_view != 0, |el| {
                        el.child(super::widgets::segmented("genre-by", &["By songs", "By plays"], usize::from(self.genre_by_plays), cx, move |i, _, cx| {
                            let _ = by.update(cx, |this, cx| {
                                this.genre_by_plays = i == 1;
                                this.load_genre_art();
                                cx.notify();
                            });
                        }))
                    })
            })
            .when(self.genre_view != 0, |el| el.child(self.genre_art_view(side, 640., cx)))
            .when(measured && self.genre_view == 0, |el| {
                el.child(
                    div()
                        .relative()
                        .w(px(side))
                        .h(px(map_height))
                        .rounded(px(14.))
                        .bg(p.raised)
                        .border_1()
                        .border_color(p.line_soft)
                        .overflow_hidden()
                        .children(bubbles)
                        .child(div().absolute().left(px(12.)).top(px(8.)).child(faint("Lively", cx)))
                        .child(div().absolute().left(px(12.)).bottom(px(8.)).child(faint("Calm", cx)))
                        .child(div().absolute().right(px(12.)).bottom(px(8.)).child(faint("Bright", cx)))
                        .child(div().absolute().left(px(70.)).bottom(px(8.)).child(faint("Dark", cx))),
                )
            })
            .when(!chips.is_empty() && self.genre_view == 0, |el| {
                el.child(display("More genres", 18.))
                    .child(div().flex().flex_wrap().gap_2().children(chips))
            })
            .into_any_element()
    }
}
