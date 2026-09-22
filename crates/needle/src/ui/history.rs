use super::{
    AppView, Event, album_page,
    library::human_duration,
    pal,
    widgets::{count, faint, glyph, meta, page_title, segmented, strong},
};
use gpui::{prelude::*, *};
use needle_core::{
    history::{HistoryStats, HistoryTop},
    model::format_duration,
};

const RANGES: [(&str, Option<i64>); 4] = [
    ("7 days", Some(7)),
    ("30 days", Some(30)),
    ("12 months", Some(365)),
    ("All time", None),
];
const HISTORY_PAGE: usize = 500;

impl AppView {
    /// Reload statistics for the chosen range and the newest page of listens.
    pub(super) fn load_history(&mut self) {
        let library = self.library.clone();
        let sender = self.sender.clone();
        let days = RANGES[self.history_range].1;
        self.history_loading = true;
        std::thread::spawn(move || {
            let since = days.map(|d| {
                let start = chrono::Local::now().date_naive() - chrono::Duration::days(d - 1);
                start
                    .and_hms_opt(0, 0, 0)
                    .and_then(|t| t.and_local_timezone(chrono::Local).earliest())
                    .map(|t| t.timestamp())
                    .unwrap_or_default()
            });
            let result = (|| -> anyhow::Result<_> {
                Ok((
                    library.history_stats(since)?,
                    library.history_page(0, HISTORY_PAGE)?,
                    library.history_count()?,
                ))
            })();
            let _ = sender.send(match result {
                Ok((stats, listens, total)) => Event::History(Box::new(stats), listens, total),
                Err(e) => Event::Error(format!("{e:#}")),
            });
        });
    }

    fn load_more_history(&mut self) {
        if self.history_loading || self.history.len() >= self.history_total {
            return;
        }
        self.history_loading = true;
        let library = self.library.clone();
        let sender = self.sender.clone();
        let offset = self.history.len();
        std::thread::spawn(move || {
            let _ = sender.send(match library.history_page(offset, HISTORY_PAGE) {
                Ok(listens) => Event::MoreHistory(offset, listens),
                Err(e) => Event::Error(format!("{e:#}")),
            });
        });
    }

    fn bars(&self, id: &'static str, values: Vec<(String, f64)>, height: f32, cx: &App) -> Div {
        let p = pal(cx);
        let peak = values.iter().map(|v| v.1).fold(0., f64::max).max(1.);
        div().h(px(height)).flex().items_end().gap(px(2.)).children(
            values.into_iter().enumerate().map(|(i, (label, value))| {
                let h = if value > 0. {
                    (value / peak * height as f64).max(3.)
                } else {
                    1.
                };
                div()
                    .id((id, i))
                    .flex_1()
                    .h(px(h as f32))
                    .rounded_t(px(2.))
                    .bg(if value > 0. { p.accent } else { p.line })
                    .hover(|s| s.opacity(0.7))
                    .tooltip(move |window, cx| {
                        gpui_component::tooltip::Tooltip::new(format!(
                            "{label} · {}",
                            human_duration(value)
                        ))
                        .build(window, cx)
                    })
            }),
        )
    }

    fn top_list(
        &self,
        title: &'static str,
        rows: &[HistoryTop],
        kind: u8,
        cx: &mut Context<Self>,
    ) -> Div {
        let p = pal(cx);
        let peak = rows.first().map(|r| r.seconds).unwrap_or(1.).max(1.);
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(faint(title, cx))
            .when(rows.is_empty(), |el| el.child(meta("Nothing yet.", cx)))
            .children(rows.iter().take(5).enumerate().map(|(i, row)| {
                let (name, sub) = match kind {
                    0 => (row.artist.clone(), String::new()),
                    1 => (row.album.clone(), row.artist.clone()),
                    _ => (row.title.clone(), row.artist.clone()),
                };
                let target = match kind {
                    0 => Some(super::Page::Artist(row.artist.clone())),
                    1 => Some(album_page(&needle_core::model::Track {
                        album: row.album.clone(),
                        album_artist: row.artist.clone(),
                        ..Default::default()
                    })),
                    _ => None,
                };
                let track_id = row.track_id.clone();
                div()
                    .id((title, i))
                    .flex()
                    .flex_col()
                    .gap(px(3.))
                    .cursor_pointer()
                    .child(
                        div()
                            .flex()
                            .justify_between()
                            .gap_2()
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .flex()
                                    .gap_1()
                                    .child(div().text_size(px(12.5)).truncate().child(
                                        if name.is_empty() {
                                            "Unknown".into()
                                        } else {
                                            name
                                        },
                                    ))
                                    .when(!sub.is_empty(), |el| {
                                        el.child(faint(format!("· {sub}"), cx).truncate())
                                    }),
                            )
                            .child(
                                faint(
                                    if row.plays == 1 {
                                        "1 play".into()
                                    } else {
                                        format!("{} plays", row.plays)
                                    },
                                    cx,
                                )
                                .flex_shrink_0(),
                            ),
                    )
                    .child(
                        div().h(px(3.)).rounded_full().bg(p.line).child(
                            div()
                                .h_full()
                                .rounded_full()
                                .bg(p.accent.opacity(0.8))
                                .w(relative((row.seconds / peak) as f32)),
                        ),
                    )
                    .hover(|s| s.opacity(0.8))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        if let Some(page) = target.clone() {
                            this.navigate(page, window, cx);
                        } else if let Some(id) = &track_id
                            && let Ok(Some(track)) = this.library.track(id)
                        {
                            this.play_tracks(vec![track], "One of your most played");
                        }
                    }))
            }))
    }

    fn insights(&self, stats: &HistoryStats, cx: &mut Context<Self>) -> Div {
        // At most ~60 bars: longer ranges are grouped into weeks or months of days.
        let per = stats.days.len().div_ceil(60).max(1);
        let days: Vec<(String, f64)> = stats
            .days
            .chunks(per)
            .map(|chunk| {
                let date = chrono::NaiveDate::parse_from_str(&chunk[0].date, "%Y-%m-%d")
                    .map(|d| {
                        d.format(if per == 1 {
                            "%a %-d %b"
                        } else {
                            "from %-d %b %Y"
                        })
                        .to_string()
                    })
                    .unwrap_or_default();
                (date, chunk.iter().map(|d| d.seconds).sum())
            })
            .collect();
        let first = stats
            .days
            .first()
            .map(|d| d.date.clone())
            .unwrap_or_default();
        let first = chrono::NaiveDate::parse_from_str(&first, "%Y-%m-%d")
            .map(|d| d.format("%-d %b %Y").to_string())
            .unwrap_or_default();
        let hours: Vec<(String, f64)> = stats
            .hours
            .iter()
            .map(|h| (format!("{:02}:00", h.hour), h.seconds))
            .collect();
        div()
            .flex()
            .flex_col()
            .gap_6()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(faint(
                        if per == 1 {
                            "Listening per day".to_string()
                        } else {
                            format!("Listening per {per} days")
                        },
                        cx,
                    ))
                    .child(self.bars("day", days, 84., cx))
                    .child(
                        div()
                            .flex()
                            .justify_between()
                            .child(faint(first, cx))
                            .child(faint("Today", cx)),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(faint("Time of day", cx))
                    .child(self.bars("hour", hours, 44., cx))
                    .child(
                        div()
                            .flex()
                            .justify_between()
                            .child(faint("00:00", cx))
                            .child(faint("12:00", cx))
                            .child(faint("23:00", cx)),
                    ),
            )
            .child(self.top_list("Top artists", &stats.top_artists, 0, cx))
            .child(self.top_list("Top albums", &stats.top_albums, 1, cx))
            .child(self.top_list("Top tracks", &stats.top_tracks, 2, cx))
    }

    pub(super) fn history_view(&self, width: f32, cx: &mut Context<Self>) -> impl IntoElement {
        let p = pal(cx);
        let weak = cx.entity().downgrade();
        let stats = self.history_stats.clone();
        let wide = width > 900.;
        let summary = match &stats {
            None => "Loading…".to_string(),
            Some(s) if s.listens == 0 => "No listening in this period.".into(),
            Some(s) => format!(
                "{} · {} · {} · {}",
                plural(s.plays, "play"),
                human_duration(s.seconds),
                plural(s.distinct_artists, "artist"),
                plural(s.distinct_tracks, "different track")
            ),
        };
        let list = uniform_list(
            "history-list",
            self.history_runs.len(),
            cx.processor(|this, range: std::ops::Range<usize>, _, cx| {
                if range.end + 100 > this.history_runs.len() {
                    this.load_more_history();
                }
                let p = pal(cx);
                range
                    .map(|index| {
                        let (first, times) = this.history_runs[index];
                        let listen = &this.history[first];
                        let track_id = listen.track_id.clone();
                        let track = this.history_tracks.get(&listen.track_id);
                        let seed = track
                            .map(super::widgets::track_seed)
                            .unwrap_or_else(|| listen.album.clone());
                        let byline = [listen.artist.as_str(), listen.album.as_str()]
                            .into_iter()
                            .filter(|s| !s.is_empty())
                            .collect::<Vec<_>>()
                            .join(" · ");
                        let row = div()
                            .id(("listen", index))
                            .group("listen")
                            .h(px(54.))
                            .w_full()
                            .px_2()
                            .rounded(px(6.))
                            .flex()
                            .items_center()
                            .gap_3()
                            .hover(|s| s.bg(p.raised.opacity(0.55)))
                            .child(super::widgets::cover(
                                track.and_then(|t| t.artwork.as_deref()),
                                &seed,
                                38.,
                                cx,
                            ))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap_2()
                                            .child(
                                                div()
                                                    .min_w_0()
                                                    .text_size(px(13.5))
                                                    .font_weight(FontWeight::MEDIUM)
                                                    .truncate()
                                                    .child(listen.title.clone()),
                                            )
                                            .when(times > 1, |el| {
                                                el.child(
                                                    div()
                                                        .flex_shrink_0()
                                                        .px(px(6.))
                                                        .rounded_full()
                                                        .bg(p.raised)
                                                        .text_size(px(11.))
                                                        .text_color(p.ink_2)
                                                        .child(format!("×{times}")),
                                                )
                                            }),
                                    )
                                    .child(meta(byline, cx).truncate()),
                            )
                            .when(!listen.qualified, |el| {
                                el.child(
                                    faint(
                                        format!(
                                            "skipped at {}",
                                            format_duration(listen.listened_seconds)
                                        ),
                                        cx,
                                    )
                                    .flex_shrink_0(),
                                )
                            })
                            .child(
                                meta(listened_when(listen.started_at), cx)
                                    .w(px(150.))
                                    .flex_shrink_0()
                                    .text_right(),
                            )
                            .child(
                                div()
                                    .id(("history-play", index))
                                    .w(px(28.))
                                    .flex()
                                    .justify_end()
                                    .opacity(0.)
                                    .group_hover("listen", |s| s.opacity(1.))
                                    .cursor_pointer()
                                    .child(glyph("play").size(px(15.)).text_color(p.ink))
                                    .on_click(cx.listener(move |this, _, _, _| {
                                        if let Ok(Some(track)) = this.library.track(&track_id) {
                                            this.play_tracks(
                                                vec![track],
                                                "Played again from your history",
                                            );
                                        }
                                    })),
                            );
                        div().w_full().px_4().child(row)
                    })
                    .collect::<Vec<_>>()
            }),
        )
        .flex_1()
        .pb_4();
        div()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .child(
                div()
                    .px_6()
                    .pt_6()
                    .pb_4()
                    .flex()
                    .items_end()
                    .justify_between()
                    .gap_4()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(page_title("Listening history"))
                            .child(meta(summary, cx)),
                    )
                    .child(segmented(
                        "history-range",
                        &RANGES.map(|r| r.0),
                        self.history_range,
                        cx,
                        move |index, _, cx| {
                            let _ = weak.update(cx, |this, cx| {
                                this.history_range = index;
                                this.history_stats = None;
                                this.load_history();
                                cx.notify();
                            });
                        },
                    )),
            )
            .child(if self.history.is_empty() && !self.history_loading {
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap_2()
                    .pb_20()
                    .child(glyph("history").size(px(28.)).text_color(p.ink_3).mb_2())
                    .child(strong("Your listening story starts here"))
                    .child(meta(
                        "Play something. Every listen is recorded on this device, even offline.",
                        cx,
                    ))
                    .into_any_element()
            } else {
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .when(!wide, |el| el.flex_col())
                    .child(
                        div()
                            .flex_1()
                            .min_h_0()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .child(
                                faint(
                                    format!(
                                        "Every listen, newest first · {}",
                                        count(self.history_total)
                                    ),
                                    cx,
                                )
                                .px_6()
                                .pb_2(),
                            )
                            .child(list),
                    )
                    .when_some(stats.filter(|_| wide), |el, stats| {
                        el.child(
                            div()
                                .id("insights")
                                .w(px(330.))
                                .flex_shrink_0()
                                .h_full()
                                .overflow_y_scroll()
                                .pl_2()
                                .pr_6()
                                .pb_6()
                                .child(self.insights(&stats, cx)),
                        )
                    })
                    .into_any_element()
            })
    }
}

fn plural(n: usize, noun: &str) -> String {
    if n == 1 {
        format!("1 {noun}")
    } else {
        format!("{} {noun}s", count(n))
    }
}

impl AppView {
    /// Regroup the history into runs of back-to-back plays and look up covers for new songs.
    pub(super) fn history_changed(&mut self) {
        self.history_runs.clear();
        for (i, listen) in self.history.iter().enumerate() {
            match self.history_runs.last_mut() {
                Some((first, n)) if self.history[*first].track_id == listen.track_id => *n += 1,
                _ => self.history_runs.push((i, 1)),
            }
        }
        let missing: Vec<String> = self
            .history
            .iter()
            .map(|l| l.track_id.clone())
            .filter(|id| !self.history_tracks.contains_key(id))
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        if !missing.is_empty() {
            for track in self.library.tracks_by_ids(&missing).unwrap_or_default() {
                self.history_tracks.insert(track.id.clone(), track);
            }
        }
    }

    /// Reload the latest listens and the tracks behind them.
    pub(super) fn refresh_recent(&mut self) {
        self.recent = self.library.history(50).unwrap_or_default();
        let ids: Vec<String> = self.recent.iter().map(|l| l.track_id.clone()).collect();
        self.recent_tracks = self
            .library
            .tracks_by_ids(&ids)
            .unwrap_or_default()
            .into_iter()
            .map(|t| (t.id.clone(), t))
            .collect();
    }

    /// The latest listens, newest first, for the mini player: one row per run of plays, with
    /// the cover, the artist, when it was, and how many times in a row.
    pub(super) fn recent_listens(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = pal(cx);
        let now = chrono::Utc::now().timestamp();
        let mut runs: Vec<(&needle_core::model::Listen, usize)> = vec![];
        for listen in &self.recent {
            match runs.last_mut() {
                Some((last, n)) if last.track_id == listen.track_id => *n += 1,
                _ => runs.push((listen, 1)),
            }
        }
        div()
            .id("recent-listens")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .when(self.recent.is_empty(), |el| {
                el.child(meta("No listening history yet.", cx))
            })
            .children(runs.into_iter().enumerate().map(|(i, (listen, times))| {
                let id = listen.track_id.clone();
                let track = self.recent_tracks.get(&listen.track_id);
                let seed = track
                    .map(super::widgets::track_seed)
                    .unwrap_or_else(|| listen.album.clone());
                let art =
                    super::widgets::cover(track.and_then(|t| t.artwork.as_deref()), &seed, 36., cx);
                let when = ago(now - listen.started_at);
                let byline = if listen.artist.is_empty() {
                    when
                } else {
                    format!("{} · {when}", listen.artist)
                };
                div()
                    .id(("recent", i))
                    .h(px(50.))
                    .px_2()
                    .rounded(px(6.))
                    .flex()
                    .items_center()
                    .gap_3()
                    .cursor_pointer()
                    .hover(|s| s.bg(p.ink.opacity(0.06)))
                    .child(art)
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(
                                div()
                                    .text_size(px(13.))
                                    .truncate()
                                    .child(listen.title.clone()),
                            )
                            .child(meta(byline, cx).truncate()),
                    )
                    .when(times > 1, |el| {
                        el.child(
                            div()
                                .flex_shrink_0()
                                .px(px(6.))
                                .rounded_full()
                                .bg(p.raised)
                                .text_size(px(11.))
                                .text_color(p.ink_2)
                                .child(format!("×{times}")),
                        )
                    })
                    .on_click(cx.listener(move |this, event: &ClickEvent, _, _| {
                        if event.click_count() == 2
                            && let Ok(Some(track)) = this.library.track(&id)
                        {
                            this.play_tracks(vec![track], "Played again from your history");
                        }
                    }))
            }))
    }
}

/// "just now", "5 min ago", "3 h ago", "yesterday", "4 days ago".
fn ago(seconds: i64) -> String {
    match seconds.max(0) {
        s if s < 60 => "just now".into(),
        s if s < 3600 => format!("{} min ago", s / 60),
        s if s < 86_400 => format!("{} h ago", s / 3600),
        s if s < 172_800 => "yesterday".into(),
        s => format!("{} days ago", s / 86_400),
    }
}

/// "Today · 18:02", "Yesterday · 21:10", "Tue 22 Sep · 18:02", or with the year when it is not
/// this year.
fn listened_when(timestamp: i64) -> String {
    use chrono::Datelike;
    let Some(at) =
        chrono::DateTime::from_timestamp(timestamp, 0).map(|d| d.with_timezone(&chrono::Local))
    else {
        return String::new();
    };
    let today = chrono::Local::now().date_naive();
    let day = at.date_naive();
    let time = at.format("%H:%M");
    if day == today {
        format!("Today · {time}")
    } else if Some(day) == today.pred_opt() {
        format!("Yesterday · {time}")
    } else if day.year() == today.year() {
        format!("{} · {time}", at.format("%a %-d %b"))
    } else {
        format!("{} · {time}", at.format("%-d %b %Y"))
    }
}
