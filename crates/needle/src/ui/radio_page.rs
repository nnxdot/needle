//! The Radio page, as the iPhone's: your station (from the song played most lately), quick
//! mixes of the whole library, and a station from each song played most. The tiles take their
//! colour from covers (the music is the colour); they are plain fills, not glass.
use super::{
    AppView, Event, Page, pal,
    widgets::{cover, display, faint, glyph, meta, page_title, small_button, strong},
};
use gpui::{prelude::*, *};
use needle_core::{audio::QueueItem, database::Library, model::Track};
use rand::seq::SliceRandom;

/// How many songs the station shelf shows.
const STATIONS: usize = 12;
/// A station card's cover.
const CARD: f32 = 164.;
/// The rule behind Favorites.
const FAVORITES: &str = "rating >= 4";

/// What the page shows, read on another thread.
pub struct RadioData {
    /// The songs played most in the last 90 days that can start a station, most first.
    pub seeds: Vec<Track>,
    pub songs: usize,
    pub favorites: usize,
    /// A cover for each quick mix, for its colour: Shuffle all, Favorites, New to you, Your
    /// listening, Genre map.
    pub art: [Option<String>; 5],
}

/// Read what the page shows.
pub fn read(library: &Library) -> anyhow::Result<RadioData> {
    let since = chrono::Utc::now().timestamp() - 90 * 86_400;
    let stats = library.history_stats(Some(since))?;
    let seeds: Vec<Track> = stats
        .top_tracks
        .iter()
        .filter_map(|t| t.track_id.as_deref())
        .filter_map(|id| library.track(id).ok().flatten())
        .filter(|t| !t.missing)
        .take(STATIONS)
        .collect();
    let songs = library.count()?;
    let favorites = library.search_page(FAVORITES, 0, 20)?;
    let home = library.home(12)?;
    let album_art = |list: &[needle_core::browse::AlbumSummary], skip: usize| {
        list.iter().filter_map(|a| a.artwork.clone()).nth(skip)
    };
    let favorite_art = favorites.tracks.iter().find_map(|t| t.artwork.clone());
    let seed_art = |n: usize| seeds.iter().filter_map(|t| t.artwork.clone()).nth(n);
    let art = [
        album_art(&home.most_played, 0)
            .or_else(|| album_art(&home.recent, 0))
            .or_else(|| album_art(&home.added, 1)),
        favorite_art,
        album_art(&home.added, 0),
        seed_art(0).or_else(|| album_art(&home.recent, 1)),
        seed_art(1)
            .or_else(|| album_art(&home.most_played, 1))
            .or_else(|| album_art(&home.added, 2)),
    ];
    Ok(RadioData {
        seeds,
        songs,
        favorites: favorites.total,
        art,
    })
}

/// Songs as a queue, with why they play.
fn items(tracks: Vec<Track>, reason: &str) -> Vec<QueueItem> {
    tracks
        .into_iter()
        .filter(|t| !t.missing)
        .map(|track| QueueItem {
            track,
            reason: reason.to_string(),
        })
        .collect()
}

/// "1 song", "1,204 songs".
fn songs(n: usize) -> String {
    if n == 1 {
        "1 song".into()
    } else {
        format!("{} songs", super::widgets::count(n))
    }
}

impl AppView {
    /// Read the page's songs and covers in the background (the page shows what it had
    /// meanwhile, or tiles that breathe the first time).
    pub(super) fn load_radio(&mut self) {
        let (library, sender) = (self.library.clone(), self.sender.clone());
        std::thread::spawn(move || {
            let _ = sender.send(Event::Radio(
                read(&library).map(Box::new).map_err(|e| format!("{e:#}")),
            ));
        });
    }

    /// The page's songs came: their covers' colours are measured next.
    pub(super) fn radio_read(&mut self, data: Result<Box<RadioData>, String>) {
        if let Ok(data) = &data {
            let covers: Vec<String> = data
                .art
                .iter()
                .flatten()
                .cloned()
                .chain(data.seeds.iter().filter_map(|t| t.artwork.clone()))
                .collect();
            for path in covers {
                let _ = self.look(&path);
            }
        }
        self.radio = Some(data);
    }

    /// The colour of a cover once it is measured.
    fn cover_color(&self, art: Option<&str>) -> Option<Hsla> {
        art.and_then(|path| self.cached_look(path)).map(|l| l.vivid)
    }

    /// Your station: from the songs played most in the last 30 days, the first that can
    /// start one; any song when nothing has played yet.
    fn your_station(&mut self) {
        self.play_station(|library| {
            let since = chrono::Utc::now().timestamp() - 30 * 86_400;
            let mut seed = library
                .history_stats(Some(since))?
                .top_tracks
                .iter()
                .filter_map(|t| t.track_id.as_deref())
                .filter_map(|id| library.track(id).ok().flatten())
                .find(|t| !t.missing);
            if seed.is_none() {
                let total = library.count()?;
                if total > 0 {
                    let at = rand::random::<u32>() as usize % total;
                    seed = library
                        .search_page("", at, 1)?
                        .tracks
                        .into_iter()
                        .find(|t| !t.missing);
                }
            }
            let Some(seed) = seed else {
                anyhow::bail!("Add music first")
            };
            super::radio::song_station(library, seed)
        });
    }

    /// Play `rule`'s songs shuffled.
    fn play_shuffled(&mut self, rule: &'static str, reason: &'static str) {
        let (library, sender) = (self.library.clone(), self.sender.clone());
        std::thread::spawn(move || {
            let event = match library.search_page(rule, 0, super::PLAY_LIMIT) {
                Ok(page) => {
                    let mut tracks = page.tracks;
                    tracks.shuffle(&mut rand::thread_rng());
                    Event::Play(items(tracks, reason), None)
                }
                Err(e) => Event::Error(format!("{e:#}")),
            };
            let _ = sender.send(event);
        });
    }

    /// New to you: the albums added lately, shuffled together.
    fn play_new(&mut self) {
        let (library, sender) = (self.library.clone(), self.sender.clone());
        std::thread::spawn(move || {
            let event = (|| -> anyhow::Result<Vec<Track>> {
                let mut tracks = vec![];
                for album in library.home(12)?.added {
                    tracks.extend(library.album_tracks(&album.key)?);
                }
                tracks.shuffle(&mut rand::thread_rng());
                Ok(tracks)
            })();
            let _ = sender.send(match event {
                Ok(tracks) if tracks.is_empty() => Event::Error("Add music first.".into()),
                Ok(tracks) => Event::Play(items(tracks, "New to you"), None),
                Err(e) => Event::Error(format!("{e:#}")),
            });
        });
    }

    pub(super) fn radio_view(&self, width: f32, cx: &mut Context<Self>) -> AnyElement {
        let body = match &self.radio {
            None => self.radio_loading(cx).into_any_element(),
            Some(Err(error)) => div()
                .flex()
                .flex_col()
                .gap_3()
                .items_start()
                .child(strong("The radio could not be read"))
                .child(meta(error.clone(), cx))
                .child(
                    small_button("radio-again", "Try again").on_click(cx.listener(
                        |this, _, _, cx| {
                            this.radio = None;
                            this.load_radio();
                            cx.notify();
                        },
                    )),
                )
                .into_any_element(),
            Some(Ok(data)) => self.radio_body(data, width, cx).into_any_element(),
        };
        div()
            .id("radio")
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
                    .child(page_title("Radio"))
                    .child(meta(
                        "Stations Needle makes from your music, on this computer.",
                        cx,
                    )),
            )
            .child(body)
            .into_any_element()
    }

    /// Grey tiles that breathe while the page reads.
    fn radio_loading(&self, cx: &App) -> Div {
        let p = pal(cx);
        let tile = |w: f32, h: f32| {
            div()
                .w(px(w))
                .h(px(h))
                .flex_shrink_0()
                .rounded(px(super::glass::MEDIUM))
                .bg(p.raised)
        };
        div()
            .flex()
            .flex_col()
            .gap(px(28.))
            .child(tile(560., 200.))
            .child(div().flex().gap_3().children((0..4).map(|_| tile(180., 92.))))
            .child(
                div()
                    .flex()
                    .gap(px(20.))
                    .children((0..5).map(|_| tile(CARD, CARD))),
            )
    }

    fn radio_body(&self, data: &RadioData, width: f32, cx: &mut Context<Self>) -> Div {
        let inner = (width - 64.).max(320.);
        let mut body = div()
            .flex()
            .flex_col()
            .gap(px(28.))
            .child(self.your_station_tile(data.seeds.first(), inner, cx))
            .child(self.quick_mixes(data, inner, cx));
        if data.seeds.is_empty() {
            body = body.child(self.radio_empty(cx));
        } else {
            body = body.child(self.stations(&data.seeds, inner, cx));
        }
        body
    }

    /// Your station: the song it starts from, on a fill in its cover's colour.
    fn your_station_tile(&self, seed: Option<&Track>, width: f32, cx: &mut Context<Self>) -> Stateful<Div> {
        let p = pal(cx);
        let art = seed.and_then(|t| t.artwork.clone());
        let color = self.cover_color(art.as_deref()).unwrap_or(p.accent);
        let mix = super::motion::mix;
        let strength = if p.dark { 0.42 } else { 0.26 };
        let from = mix(p.raised, color, strength);
        let to = mix(p.raised, color, strength * 0.35);
        let size = 168.;
        let (title, artist) = match seed {
            Some(t) => (format!("From {}", t.title), t.display_artist().to_string()),
            None => (
                "From anything in your library".to_string(),
                "Needle picks a song to start".to_string(),
            ),
        };
        let glow = self.cover_glow(art.as_deref(), cx);
        div()
            .id("your-station")
            .group("your-station")
            .w(px(width.min(760.)))
            .p(px(16.))
            .rounded(px(super::glass::LARGE))
            .bg(linear_gradient(
                90.,
                linear_color_stop(from, 0.),
                linear_color_stop(to, 1.),
            ))
            .flex()
            .items_center()
            .gap(px(22.))
            .cursor_pointer()
            .when(super::motion::enabled(cx), |el| {
                el.relative()
                    .hover(move |s| s.top(px(-2.)).shadow(glow.clone()))
            })
            .child(
                div()
                    .flex_shrink_0()
                    .rounded(px(super::glass::SMALL))
                    .shadow_md()
                    .child(cover(
                        art.as_deref(),
                        &seed.map(super::widgets::track_seed).unwrap_or_default(),
                        size,
                        cx,
                    )),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .text_size(px(11.))
                            .font_weight(FontWeight::BOLD)
                            .text_color(p.ink_2)
                            .child("YOUR STATION"),
                    )
                    .child(display(title, 26.).w_full().line_clamp(2))
                    .child(meta(artist, cx).truncate()),
            )
            .child(
                div()
                    .flex_shrink_0()
                    .mr_2()
                    .size(px(52.))
                    .rounded_full()
                    .bg(p.accent)
                    .group_hover("your-station", |s| s.bg(p.accent.opacity(0.9)))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(glyph("play").size(px(22.)).text_color(p.accent_ink)),
            )
            .on_click(cx.listener(|this, _, _, _| this.your_station()))
    }

    /// Mixes of the whole library, each tile in the colour of one of its covers.
    fn quick_mixes(&self, data: &RadioData, width: f32, cx: &mut Context<Self>) -> Div {
        let favorites = data.favorites;
        let tiles: Vec<(&'static str, String, &'static str, Option<String>, bool)> = vec![
            ("Shuffle all", songs(data.songs), "shuffle", data.art[0].clone(), data.songs > 0),
            (
                "Favorites",
                if favorites > 0 {
                    format!("{}, shuffled", songs(favorites))
                } else {
                    "Star songs to fill it".into()
                },
                "heart",
                data.art[1].clone(),
                favorites > 0,
            ),
            ("New to you", "Added lately, shuffled".into(), "recent", data.art[2].clone(), data.songs > 0),
            ("Your listening", "Top songs and artists".into(), "history", data.art[3].clone(), true),
            ("Genre map", "Click a genre to play it".into(), "radio", data.art[4].clone(), true),
        ];
        // As many in a row as fit, at least 180 px each; rows of their own (no wrapping, see
        // gpui-gotchas).
        let per_row = (((width + 12.) / (180. + 12.)).floor() as usize).clamp(2, 5);
        let tile_w = (width - 12. * (per_row - 1) as f32) / per_row as f32;
        let mut rows = vec![];
        let mut row = vec![];
        for (i, (title, detail, icon, art, on)) in tiles.into_iter().enumerate() {
            row.push(self.mix_tile(i, title, detail, icon, art, on, tile_w, cx).into_any_element());
            if row.len() == per_row {
                rows.push(std::mem::take(&mut row));
            }
        }
        if !row.is_empty() {
            rows.push(row);
        }
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(display("Quick mixes", 20.))
            .children(rows.into_iter().map(|row| div().flex().gap_3().children(row)))
    }

    #[allow(clippy::too_many_arguments)]
    fn mix_tile(
        &self,
        index: usize,
        title: &'static str,
        detail: String,
        icon: &'static str,
        art: Option<String>,
        on: bool,
        width: f32,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let p = pal(cx);
        let color = self.cover_color(art.as_deref());
        let mix = super::motion::mix;
        // A plain fill in the cover's colour; the page's raised colour until it is known.
        let fill = color.map_or(p.raised, |c| mix(p.raised, c, if p.dark { 0.24 } else { 0.14 }));
        // No cover: a quiet grey square (amber is only for what plays and the main button).
        let chip = color.map_or(mix(p.raised, p.ink, 0.3), |c| {
            // The icon's square: the cover's colour, deep enough for white on it.
            let c = c.to_rgb();
            let c: Hsla = c.into();
            hsla(c.h, c.s.min(0.75), c.l.clamp(0.3, 0.46), 1.)
        });
        let glow = self.cover_glow(art.as_deref(), cx);
        div()
            .id(("radio-mix", index))
            .w(px(width))
            .flex_shrink_0()
            .p(px(14.))
            .rounded(px(super::glass::MEDIUM))
            .bg(fill)
            .flex()
            .flex_col()
            .gap(px(10.))
            .when(!on, |el| el.opacity(0.55))
            .when(on, |el| {
                el.cursor_pointer()
                    .when(super::motion::enabled(cx), |el| {
                        el.relative().hover(move |s| s.top(px(-2.)).shadow(glow.clone()))
                    })
                    .on_click(cx.listener(move |this, _, window, cx| match index {
                        0 => this.play_shuffled("", "Shuffle all"),
                        1 => this.play_shuffled(FAVORITES, "Favorites, shuffled"),
                        2 => this.play_new(),
                        3 => this.navigate(Page::History, window, cx),
                        _ => this.navigate(Page::MoodMap, window, cx),
                    }))
            })
            .child(
                div()
                    .size(px(40.))
                    .rounded(px(11.))
                    .bg(chip)
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(glyph(icon).size(px(20.)).text_color(gpui::white())),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .text_size(px(13.5))
                            .font_weight(FontWeight::SEMIBOLD)
                            .truncate()
                            .child(title),
                    )
                    .child(meta(detail, cx).truncate()),
            )
    }

    /// A station from each song played most: its cover, and a click starts it.
    fn stations(&self, seeds: &[Track], width: f32, cx: &mut Context<Self>) -> Div {
        let gap = 20.;
        let per_row = (((width + gap) / (CARD + gap)).floor() as usize).max(1);
        let cards: Vec<AnyElement> = seeds
            .iter()
            .enumerate()
            .map(|(i, seed)| self.station_card(i, seed, cx).into_any_element())
            .collect();
        let mut rows: Vec<Vec<AnyElement>> = vec![];
        for card in cards {
            match rows.last_mut() {
                Some(row) if row.len() < per_row => row.push(card),
                _ => rows.push(vec![card]),
            }
        }
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(display("Stations from your favorites", 20.))
                    .child(faint(
                        "Each starts with the song, then plays songs that flow from it.",
                        cx,
                    )),
            )
            .children(
                rows.into_iter()
                    .map(|row| div().flex().gap(px(gap)).children(row)),
            )
    }

    fn station_card(&self, index: usize, seed: &Track, cx: &mut Context<Self>) -> Stateful<Div> {
        let p = pal(cx);
        let track = seed.clone();
        let group = SharedString::from(format!("station-{index}"));
        div()
            .id(("radio-station", index))
            .group(group.clone())
            .w(px(CARD))
            .flex_shrink_0()
            .flex()
            .flex_col()
            .gap_1()
            .cursor_pointer()
            .child(
                div()
                    .relative()
                    .mb_2()
                    .rounded(px(super::glass::SMALL))
                    .shadow_md()
                    .when(super::motion::enabled(cx), |el| {
                        let glow = self.cover_glow(seed.artwork.as_deref(), cx);
                        el.group_hover(group.clone(), move |s| s.top(px(-2.)).shadow(glow.clone()))
                    })
                    .child(cover(
                        seed.artwork.as_deref(),
                        &super::widgets::track_seed(seed),
                        CARD,
                        cx,
                    ))
                    // "Radio", on a dark chip over the cover's foot (content, not glass).
                    .child(
                        div()
                            .absolute()
                            .left(px(8.))
                            .bottom(px(8.))
                            .px(px(8.))
                            .py(px(3.))
                            .rounded_full()
                            .bg(hsla(0., 0., 0., 0.62))
                            .flex()
                            .items_center()
                            .gap_1()
                            .text_size(px(11.))
                            .font_weight(FontWeight::BOLD)
                            .text_color(gpui::white())
                            .child(glyph("radio").size(px(12.)).text_color(gpui::white()))
                            .child("Radio"),
                    ),
            )
            .child(
                div()
                    .text_size(px(13.5))
                    .font_weight(FontWeight::MEDIUM)
                    .truncate()
                    .child(seed.title.clone()),
            )
            .child(
                div()
                    .text_size(px(12.5))
                    .text_color(p.ink_2)
                    .truncate()
                    .child(seed.display_artist().to_string()),
            )
            .on_click(cx.listener(move |this, _, _, _| this.start_radio(track.clone())))
    }

    /// No songs played yet: where stations come from.
    fn radio_empty(&self, cx: &App) -> Div {
        let p = pal(cx);
        div()
            .max_w(px(520.))
            .flex()
            .flex_col()
            .gap_2()
            .child(glyph("radio").size(px(28.)).text_color(p.ink_3))
            .child(strong("Stations come from what you play"))
            .child(
                meta(
                    "Play some music, and a station for each song you play most shows here. Any song's menu can start one too: Start radio.",
                    cx,
                )
                .w_full(),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::songs;

    #[test]
    fn counts_read_as_words() {
        assert_eq!(songs(1), "1 song");
        assert_eq!(songs(2), "2 songs");
        assert_eq!(songs(12_000), "12,000 songs");
    }
}
