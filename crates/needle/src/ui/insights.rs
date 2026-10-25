//! Two pages about the library as a whole: Your library (what it is made of) and the mood map
//! (every measured song placed by how it sounds; click a spot to play the songs around it).
use super::{
    AppView, Event, Page, pal,
    widgets::{display, faint, meta, page_title, small_button},
};
use gpui::{prelude::*, *};
use gpui_component::Disableable;
use needle_core::radio::MoodPoint;
use std::{cell::Cell, rc::Rc};

/// How far around a clicked spot the mood map plays, in the map's 0-to-1 places.
const AREA: f32 = 0.12;

/// The mood map's colours: one for each feel.
fn mood_color(mood: &str) -> Hsla {
    match mood {
        "Upbeat" => hsla(36. / 360., 0.82, 0.62, 1.),
        "Intense" => hsla(4. / 360., 0.72, 0.6, 1.),
        "Mellow" => hsla(168. / 360., 0.5, 0.52, 1.),
        _ => hsla(250. / 360., 0.5, 0.66, 1.),
    }
}

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
                    let _ = sender.send(match library.mood_map() {
                        Ok(points) => Event::MoodMap(points),
                        Err(e) => Event::Error(format!("{e:#}")),
                    });
                });
            }
            _ => {}
        }
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

    pub(super) fn mood_view(&self, width: f32, cx: &mut Context<Self>) -> AnyElement {
        let p = pal(cx);
        let points: Rc<Vec<MoodPoint>> = self.mood_points.clone();
        let picked = self.mood_pick;
        let bounds = self.mood_bounds.clone();
        let inside = picked.map(|(x, y)| {
            points
                .iter()
                .filter(|q| ((q.x - x).powi(2) + (q.y - y).powi(2)).sqrt() <= AREA)
                .count()
        });
        let side = (width - 64.).clamp(280., 900.);
        let map_height = (side * 0.62).clamp(260., 560.);
        let accent = p.accent;
        let line = p.line_soft;
        let count = points.len();
        let map = canvas(
            {
                let bounds = bounds.clone();
                move |b, _, _| bounds.set(Some(b))
            },
            move |b, _, window, _| {
                // The axes' middle lines, then every song, then the circle picked.
                let mid_x = b.left() + b.size.width / 2.;
                let mid_y = b.top() + b.size.height / 2.;
                window.paint_quad(fill(
                    Bounds::new(point(mid_x, b.top()), size(px(1.), b.size.height)),
                    line,
                ));
                window.paint_quad(fill(
                    Bounds::new(point(b.left(), mid_y), size(b.size.width, px(1.))),
                    line,
                ));
                for q in points.iter() {
                    let at = point(
                        b.left() + b.size.width * (0.02 + q.x * 0.96) - px(2.5),
                        b.top() + b.size.height * (0.98 - q.y * 0.96) - px(2.5),
                    );
                    window.paint_quad(
                        fill(
                            Bounds::new(at, size(px(5.), px(5.))),
                            mood_color(q.mood).opacity(0.85),
                        )
                        .corner_radii(px(2.5)),
                    );
                }
                if let Some((x, y)) = picked {
                    let r = b.size.width.min(b.size.height) * AREA;
                    let centre = point(
                        b.left() + b.size.width * (0.02 + x * 0.96),
                        b.top() + b.size.height * (0.98 - y * 0.96),
                    );
                    window.paint_quad(
                        outline(
                            Bounds::new(centre - point(r, r), size(r * 2., r * 2.)),
                            accent,
                            BorderStyle::Dashed,
                        )
                        .corner_radii(r),
                    );
                }
            },
        )
        .size_full();
        div()
            .id("mood-map")
            .size_full()
            .overflow_y_scroll()
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
                    .child(page_title("Mood map"))
                    .child(meta(
                        if count == 0 {
                            "Needle measures each song's sound in the background (Settings › Playback). Measured songs show here.".to_string()
                        } else {
                            format!("{} songs, by how they sound. Click a spot to play the songs around it.", super::widgets::count(count))
                        },
                        cx,
                    )),
            )
            .child(
                div()
                    .relative()
                    .w(px(side))
                    .h(px(map_height))
                    .rounded(px(14.))
                    .bg(p.raised)
                    .border_1()
                    .border_color(p.line_soft)
                    .overflow_hidden()
                    .cursor_crosshair()
                    .child(map)
                    .child(div().absolute().left(px(12.)).top(px(8.)).child(faint("Lively", cx)))
                    .child(div().absolute().left(px(12.)).bottom(px(8.)).child(faint("Calm", cx)))
                    .child(div().absolute().right(px(12.)).bottom(px(8.)).child(faint("Bright", cx)))
                    .child(div().absolute().left(px(70.)).bottom(px(8.)).child(faint("Dark", cx)))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                            if let Some(b) = this.mood_bounds.get() {
                                let x = ((f32::from(event.position.x - b.left()) / f32::from(b.size.width)) - 0.02) / 0.96;
                                let y = (0.98 - f32::from(event.position.y - b.top()) / f32::from(b.size.height)) / 0.96;
                                this.mood_pick = Some((x.clamp(0., 1.), y.clamp(0., 1.)));
                                cx.notify();
                            }
                        }),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .children(["Upbeat", "Intense", "Mellow", "Melancholy"].map(|mood| {
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .child(div().size(px(9.)).rounded_full().bg(mood_color(mood)))
                            .child(faint(mood, cx))
                    }))
                    .child(div().flex_1())
                    .when_some(inside, |el, count| {
                        el.child(meta(format!("{} songs in the circle", super::widgets::count(count)), cx))
                            .child(
                                small_button("mood-play", "Play this area")
                                    .disabled(count == 0)
                                    .on_click(cx.listener(|this, _, _, _| {
                                        if let Some((x, y)) = this.mood_pick {
                                            match this.library.mood_area(x, y, AREA, 200) {
                                                Ok(tracks) if !tracks.is_empty() => {
                                                    this.play_tracks(tracks, "From the mood map")
                                                }
                                                Ok(_) => {}
                                                Err(e) => this.fail(format!("{e:#}")),
                                            }
                                        }
                                    })),
                            )
                    }),
            )
            .into_any_element()
    }
}

/// Where the mood map was drawn, to place clicks on it.
pub(super) type MapBounds = Rc<Cell<Option<Bounds<Pixels>>>>;
