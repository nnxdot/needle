//! Home: a greeting, quick starts (Start fast), and shelves of covers.
use super::widgets::SidewaysOnly as _;
use super::{
    AppView, Event, Page,
    library::{Group, album_groups, artist_groups},
    pal,
    widgets::{cover, display, faint, generated_cover, glyph, meta},
};
use chrono::Timelike;
use gpui::{prelude::*, *};
use needle_core::browse::{AlbumSummary, Home};
use needle_core::model::Playlist;

/// Covers per shelf.
const SHELF: usize = 14;
const TILE: f32 = 164.;

impl AppView {
    pub(super) fn load_home(&mut self) {
        let library = self.library.clone();
        let sender = self.sender.clone();
        std::thread::spawn(move || {
            let event = match library.home(SHELF) {
                Ok(home) => Event::Home(Box::new(home)),
                Err(e) => Event::Error(format!("{e:#}")),
            };
            let _ = sender.send(event);
            // On this day: what played one and five years ago today.
            let now = chrono::Local::now();
            let offset = now.offset().local_minus_utc();
            if let Ok(days) = library.on_this_day(now.timestamp(), offset, &[1, 5], SHELF) {
                let _ = sender.send(Event::OnThisDay(days));
            }
            // December: the year in music.
            let year = needle_core::wrapped::home_year(now.date_naive()).filter(|y| {
                library
                    .listening_years()
                    .is_ok_and(|years| years.contains(y))
            });
            let _ = sender.send(Event::HomeYear(year));
        });
    }

    pub(super) fn home_view(&self, width: f32, cx: &mut Context<Self>) -> impl IntoElement {
        let p = pal(cx);
        let hour = chrono::Local::now().hour();
        let greeting = match hour {
            5..=11 => "Good morning",
            12..=17 => "Good afternoon",
            _ => "Good evening",
        };
        let empty = Home::default();
        let home = self.home.as_deref().unwrap_or(&empty);
        let days: Vec<(String, String, Vec<Group>, bool, Page)> = self
            .on_this_day
            .iter()
            .map(|day| {
                let date = chrono::DateTime::from_timestamp(day.day, 0)
                    .map(|d| {
                        d.with_timezone(&chrono::Local)
                            .format("%-d %B %Y")
                            .to_string()
                    })
                    .unwrap_or_default();
                (
                    match day.years_ago {
                        1 => "One year ago today".to_string(),
                        n => format!("{n} years ago today"),
                    },
                    date,
                    album_groups(day.albums.clone()),
                    false,
                    Page::History,
                )
            })
            .collect();
        // The Mac's order: Jump back in, New in your library, On repeat, Favorites, then On
        // this day, then the artists.
        let shelves: Vec<(String, String, Vec<Group>, bool, Page)> = [
            (
                "Jump back in",
                "Albums you played last",
                album_groups(home.recent.clone()),
                false,
                Page::History,
            ),
            (
                "New in your library",
                "Added most recently",
                album_groups(home.added.clone()),
                false,
                Page::Recent,
            ),
            (
                "On repeat",
                "Your most played albums",
                album_groups(home.most_played.clone()),
                false,
                Page::History,
            ),
            (
                "Favorites",
                "Albums with the songs you love",
                album_groups(home.favorites.clone()),
                false,
                Page::Favorites,
            ),
            (
                "Artists you play most",
                "",
                artist_groups(home.artists.clone()),
                true,
                Page::Artists,
            ),
        ]
        .into_iter()
        .map(|(title, note, groups, round, page)| {
            (title.to_string(), note.to_string(), groups, round, page)
        })
        .enumerate()
        .flat_map(|(i, shelf)| {
            // On this day comes right after Favorites, as on the Mac.
            let mut out = vec![shelf];
            if i == 3 {
                out.extend(days.clone());
            }
            out
        })
        .filter(|s| !s.2.is_empty())
        .collect();
        div()
            .id("home")
            .size_full()
            .overflow_y_scroll()
            .track_scroll(&self.page_scroll)
            .pb(px(40.) + self.foot_room())
            .child(
                div()
                    .px_8()
                    .pt_10()
                    .pb_6()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(display(greeting, 44.))
                    .child(
                        meta(
                            format!(
                                "{} songs in your library{}",
                                super::widgets::count(self.total),
                                if self.recent.is_empty() {
                                    String::new()
                                } else {
                                    format!(" · {} listens lately", self.recent.len())
                                }
                            ),
                            cx,
                        )
                        .text_size(px(13.)),
                    ),
            )
            .child(self.start_fast(width, cx))
            .children(self.home_year_card(cx))
            .when(self.home.is_none(), |el| el.child(Self::shelf_skeleton(cx)))
            .children(shelves.into_iter().enumerate().map(
                |(i, (title, note, groups, round, all))| {
                    let tiles: Vec<AnyElement> = groups
                        .iter()
                        .enumerate()
                        .map(|(j, group)| {
                            self.shelf_tile(i * 100 + j, group, round, cx)
                                .into_any_element()
                        })
                        .collect();
                    div()
                        .pt_6()
                        .child(
                            div()
                                .px_8()
                                .pb_3()
                                .flex()
                                .items_end()
                                .gap_3()
                                .child(display(title, 22.))
                                .when(!note.is_empty(), |el| el.child(faint(note, cx).pb(px(3.))))
                                .child(div().flex_1())
                                .child(
                                    div()
                                        .id(("shelf-all", i))
                                        .text_size(px(12.5))
                                        .text_color(p.ink_2)
                                        .cursor_pointer()
                                        .hover(|s| s.text_color(p.ink).underline())
                                        .child("See all")
                                        .on_click(cx.listener(move |this, _, window, cx| {
                                            this.navigate(all.clone(), window, cx)
                                        })),
                                ),
                        )
                        .child(self.shelf_row(i, tiles, cx))
                },
            ))
    }

    /// December's card: the year in music is ready.
    fn home_year_card(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        let year = self.home_year?;
        let p = pal(cx);
        Some(
            div().px_8().pt_6().child(
                div()
                    .id("home-year")
                    .rounded(px(16.))
                    .p(px(20.))
                    .bg(p.accent.opacity(0.12))
                    .border_1()
                    .border_color(p.accent.opacity(0.25))
                    .flex()
                    .items_center()
                    .gap_4()
                    .cursor_pointer()
                    .hover(|s| s.bg(p.accent.opacity(0.18)))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .font_family(super::theme::MONO_FONT)
                                    .text_size(px(11.))
                                    .text_color(p.accent)
                                    .child(format!("YOUR {year} IN MUSIC")),
                            )
                            .child(display(format!("Your {year} in music"), 26.))
                            .child(meta(
                                "Your hours, your top artist, and your top songs of the year.",
                                cx,
                            )),
                    )
                    .child(glyph("chevron-right").size(px(20.)).text_color(p.accent))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.navigate(Page::Wrapped(year), window, cx)
                    })),
            ),
        )
    }

    /// One Home row, scrolled sideways, with an arrow at each end (while the pointer is over
    /// it, and only toward more) that moves it a page of tiles.
    pub(super) fn shelf_row(
        &self,
        i: usize,
        tiles: Vec<AnyElement>,
        cx: &mut Context<Self>,
    ) -> Div {
        let p = pal(cx);
        let Some(scroll) = self.shelf_scrolls.get(i).cloned() else {
            return div().child(
                div()
                    .id(("shelf", i))
                    .overflow_x_scroll()
                    .sideways_only()
                    .child(div().px_8().flex().gap(px(20.)).children(tiles)),
            );
        };
        let group = SharedString::from(format!("shelf-{i}"));
        let (offset, most) = (scroll.offset().x, -scroll.max_offset().width);
        let arrow = |back: bool| {
            let scroll = scroll.clone();
            div()
                .id(("shelf-arrow", i * 2 + back as usize))
                .absolute()
                .top(px((TILE - 56.) / 2.))
                .when(back, |el| el.left(px(6.)))
                .when(!back, |el| el.right(px(6.)))
                .w(px(30.))
                .h(px(56.))
                .rounded_full()
                .flex()
                .items_center()
                .justify_center()
                .bg(p.raised.opacity(0.92))
                .border_1()
                .border_color(p.line_soft)
                .shadow_md()
                .cursor_pointer()
                .opacity(0.)
                .group_hover(group.clone(), |s| s.opacity(1.))
                .hover(|s| s.bg(p.raised))
                .child(
                    glyph(if back {
                        "chevron-left"
                    } else {
                        "chevron-right"
                    })
                    .size(px(16.))
                    .text_color(p.ink),
                )
                .on_click(cx.listener(move |_, _, _, cx| {
                    cx.stop_propagation();
                    // A page: the tiles that fit in the row's width, at least one.
                    let step = TILE + 20.;
                    let shown = f32::from(scroll.bounds().size.width) - 64.;
                    let page = ((shown / step).floor().max(1.)) * step;
                    let now = scroll.offset();
                    let most = -f32::from(scroll.max_offset().width);
                    let x = (f32::from(now.x) + if back { page } else { -page }).clamp(most, 0.);
                    scroll.set_offset(point(px(x), now.y));
                    cx.notify();
                }))
        };
        div()
            .relative()
            .group(group.clone())
            .child(
                // A row of its own width (wider than the window), so it can scroll.
                div()
                    .id(("shelf", i))
                    .flex()
                    .overflow_x_scroll()
                    .sideways_only()
                    .track_scroll(&scroll)
                    .child(div().flex_none().px_8().flex().gap(px(20.)).children(tiles)),
            )
            .when(offset < px(-1.), |el| el.child(arrow(true)))
            .when(offset > most + px(1.), |el| el.child(arrow(false)))
    }

    /// Start fast: up to six quick starts under the greeting, three across in a wide window
    /// and two in a narrow one. The grid stays whether music plays or not; while Home loads,
    /// plain tiles hold its place.
    fn start_fast(&self, width: f32, cx: &mut Context<Self>) -> Div {
        let p = pal(cx);
        let content = (width - 64.).max(200.);
        let columns = if content >= QUICK_WIDE { 3 } else { 2 };
        let w = (content - QUICK_GAP * (columns as f32 - 1.)) / columns as f32;
        let tiles: Vec<AnyElement> = match self.home.as_deref() {
            Some(home) => quick_starts(home, &self.playlists, self.total)
                .iter()
                .enumerate()
                .map(|(i, quick)| self.quick_tile(i, quick, w, cx).into_any_element())
                .collect(),
            None => (0..QUICK_MOST)
                .map(|_| {
                    div()
                        .w(px(w))
                        .h(px(QUICK_H))
                        .flex_shrink_0()
                        .rounded(px(QUICK_R))
                        .bg(p.raised.opacity(0.5))
                        .into_any_element()
                })
                .collect(),
        };
        if tiles.is_empty() {
            return div();
        }
        // Rows of their own: a wrapped row in a column takes too much height (GPUI).
        let mut rows = vec![];
        let mut tiles = tiles.into_iter().peekable();
        while tiles.peek().is_some() {
            rows.push(
                div()
                    .flex()
                    .gap(px(QUICK_GAP))
                    .children(tiles.by_ref().take(columns)),
            );
        }
        div()
            .px_8()
            .pb_2()
            .flex()
            .flex_col()
            .gap(px(QUICK_GAP))
            .children(rows)
    }

    /// One quick start: its picture at the left, its name, and the playing mark when it plays
    /// now. A press plays it; a right click opens the menu of an album or a playlist.
    fn quick_tile(&self, i: usize, quick: &Quick, w: f32, cx: &mut Context<Self>) -> Stateful<Div> {
        let p = pal(cx);
        let lit = self
            .playback
            .current
            .as_ref()
            .is_some_and(|now| quick.plays(&now.reason, &now.track.album));
        let square = |color: Hsla, name: &str, ink: Hsla| {
            div()
                .size(px(QUICK_H))
                .flex_shrink_0()
                .rounded_l(px(QUICK_R))
                .bg(color)
                .flex()
                .items_center()
                .justify_center()
                .child(glyph(name).size(px(24.)).text_color(ink))
                .into_any_element()
        };
        let picture = match quick {
            Quick::Favorites => square(p.accent, "heart-fill", p.accent_ink),
            Quick::Station => square(p.accent_soft, "radio", p.accent),
            Quick::Album(album) => quick_cover(
                album.artwork.as_deref(),
                &format!("{}{}", album.album, album.artist),
                cx,
            ),
            Quick::Playlist(playlist) => self
                .quick_playlist_cover(&playlist.id, cx)
                .unwrap_or_else(|| square(p.raised_hover, "playlist", p.ink_2)),
        };
        let name = quick.name();
        let play = quick.clone();
        let menu = quick.clone();
        div()
            .id(("home-quick", i))
            .w(px(w))
            .h(px(QUICK_H))
            .flex_shrink_0()
            .rounded(px(QUICK_R))
            .bg(p.raised)
            .flex()
            .items_center()
            .gap(px(12.))
            .pr(px(12.))
            .cursor_pointer()
            .hover(|s| s.bg(p.raised_hover))
            .active(|s| s.opacity(0.85))
            .child(picture)
            .child(
                div().flex_1().min_w_0().child(
                    div()
                        .w_full()
                        .text_size(px(13.5))
                        .line_height(px(17.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .line_clamp(2)
                        .text_color(if lit { p.accent } else { p.ink })
                        .child(name),
                ),
            )
            .when(lit, |el| {
                el.child(super::motion::equalizer(
                    p.accent,
                    self.playback.playing,
                    cx,
                ))
            })
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, event: &MouseDownEvent, _, cx| match &menu {
                    Quick::Album(album) => {
                        this.open_group_menu(album_of(album), event.position, cx)
                    }
                    Quick::Playlist(playlist) => {
                        this.open_playlist_menu(playlist.id.clone(), event.position, cx)
                    }
                    Quick::Favorites | Quick::Station => {}
                }),
            )
            .on_click(cx.listener(move |this, _, _, _| match &play {
                Quick::Favorites => this.play_shuffled(
                    super::radio_page::FAVORITES,
                    super::radio_page::FAVORITES_SHUFFLED,
                ),
                Quick::Station => this.your_station(),
                Quick::Album(album) => {
                    this.play_rule(album_of(album).base(), quick_album_reason(&album.album))
                }
                Quick::Playlist(playlist) => this.play_playlist((**playlist).clone(), false, false),
            }))
    }

    /// A playlist's picture for a quick start: its own picture, one cover, or four.
    fn quick_playlist_cover(&self, id: &str, cx: &App) -> Option<AnyElement> {
        let covers = self.playlist_art.get(id)?;
        let half = QUICK_H / 2.;
        match covers.len() {
            0 => None,
            1..=3 => Some(quick_cover(Some(&covers[0]), "", cx)),
            _ => Some(
                div()
                    .size(px(QUICK_H))
                    .flex_shrink_0()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .flex()
                            .child(quick_image(&covers[0], half, (true, false), cx))
                            .child(quick_image(&covers[1], half, (false, false), cx)),
                    )
                    .child(
                        div()
                            .flex()
                            .child(quick_image(&covers[2], half, (false, true), cx))
                            .child(quick_image(&covers[3], half, (false, false), cx)),
                    )
                    .into_any_element(),
            ),
        }
    }

    pub(super) fn shelf_tile(
        &self,
        index: usize,
        group: &Group,
        round: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let page = group.page.clone();
        let open = page.clone();
        div()
            .id(("home-tile", index))
            .group("tile")
            .w(px(TILE))
            .flex_shrink_0()
            .flex()
            .flex_col()
            .gap_1()
            .cursor_pointer()
            .child(
                div()
                    .relative()
                    .mb_2()
                    .rounded(if round { px(TILE) } else { px(8.) })
                    .shadow_md()
                    .when(super::motion::enabled(cx), |el| {
                        let glow = self.cover_glow(group.artwork.as_deref(), cx);
                        el.group_hover("tile", move |s| s.top(px(-2.)).shadow(glow.clone()))
                    })
                    .child(if round {
                        self.artist_photo(&group.title, TILE, cx)
                    } else {
                        cover(group.artwork.as_deref(), &group.seed, TILE, cx)
                    })
                    .child(self.cover_play(index, page, TILE, cx)),
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
            .on_mouse_down(MouseButton::Right, {
                let page = open.clone();
                cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                    this.open_group_menu(page.clone(), event.position, cx)
                })
            })
            .on_click(
                cx.listener(move |this, _, window, cx| this.navigate(open.clone(), window, cx)),
            )
    }

    /// Grey shelves that breathe while Home is loading.
    fn shelf_skeleton(cx: &App) -> AnyElement {
        let p = pal(cx);
        let shelf = |i: usize| {
            div()
                .pt_6()
                .px_8()
                .flex()
                .flex_col()
                .gap_3()
                .child(
                    div()
                        .h(px(20.))
                        .w(px(180. - i as f32 * 30.))
                        .rounded(px(4.))
                        .bg(p.raised),
                )
                .child(div().flex().gap(px(20.)).children((0..6).map(|_| {
                    div()
                        .size(px(TILE))
                        .flex_shrink_0()
                        .rounded(px(8.))
                        .bg(p.raised)
                })))
        };
        super::motion::repeat(
            div().child(shelf(0)).child(shelf(1)),
            "home-skeleton",
            1400,
            1.,
            cx,
            |el, t| el.opacity(0.45 + 0.55 * t),
        )
    }
}

/// Quick starts at most.
const QUICK_MOST: usize = 6;
/// A quick start's height, its corners, and the room between two.
const QUICK_H: f32 = 58.;
const QUICK_R: f32 = 12.;
const QUICK_GAP: f32 = 10.;
/// The content width from which three quick starts go across.
const QUICK_WIDE: f32 = 560.;

/// One quick start on Home.
#[derive(Clone, Debug)]
enum Quick {
    /// The favorite songs, shuffled.
    Favorites,
    /// The Radio page's station.
    Station,
    Album(Box<AlbumSummary>),
    Playlist(Box<Playlist>),
}

impl Quick {
    fn name(&self) -> String {
        match self {
            Self::Favorites => "Favorites".into(),
            Self::Station => "Your station".into(),
            Self::Album(album) if album.album.is_empty() => "Unknown album".into(),
            Self::Album(album) => album.album.clone(),
            Self::Playlist(playlist) => playlist.name.clone(),
        }
    }

    /// Whether the song playing now came from this quick start: by why it plays, and an
    /// album's song by its album too.
    fn plays(&self, reason: &str, album_now: &str) -> bool {
        match self {
            Self::Favorites => reason == super::radio_page::FAVORITES_SHUFFLED,
            Self::Station => reason.starts_with("Radio: like"),
            Self::Album(album) => {
                album_now == album.album && reason == quick_album_reason(&album.album)
            }
            Self::Playlist(playlist) => reason == format!("From the playlist {}", playlist.name),
        }
    }
}

/// Why an album's songs play (as everywhere an album plays).
fn quick_album_reason(album: &str) -> String {
    format!("From the album {album}")
}

fn album_of(album: &AlbumSummary) -> Page {
    Page::Album {
        album: album.album.clone(),
        artist: album.artist.clone(),
        query: album.query.clone(),
    }
}

/// The quick starts, in this order: Favorites (with one favorite or more), your station (with
/// any song), the two albums played most, the first two playlists. The albums played last
/// fill the rest. An even number, six at most; none when fewer than two.
fn quick_starts(home: &Home, playlists: &[Playlist], songs: usize) -> Vec<Quick> {
    let mut out = vec![];
    if !home.favorites.is_empty() {
        out.push(Quick::Favorites);
    }
    if songs > 0 {
        out.push(Quick::Station);
    }
    out.extend(
        home.most_played
            .iter()
            .take(2)
            .cloned()
            .map(|a| Quick::Album(Box::new(a))),
    );
    out.extend(
        playlists
            .iter()
            .take(2)
            .cloned()
            .map(|p| Quick::Playlist(Box::new(p))),
    );
    for album in &home.recent {
        if out.len() >= QUICK_MOST {
            break;
        }
        let shown = out
            .iter()
            .any(|q| matches!(q, Quick::Album(a) if a.key == album.key));
        if !shown {
            out.push(Quick::Album(Box::new(album.clone())));
        }
    }
    out.truncate(QUICK_MOST);
    if out.len() % 2 == 1 {
        out.pop();
    }
    if out.len() < 2 {
        out.clear();
    }
    out
}

/// A cover for a quick start: the tile's full height, round only on the tile's left side.
fn quick_cover(path: Option<&str>, seed: &str, cx: &App) -> AnyElement {
    let p = pal(cx);
    match path {
        Some(path) => quick_image(path, QUICK_H, (true, true), cx).into_any_element(),
        None => generated_cover(seed, QUICK_H, false, &p)
            .rounded_l(px(QUICK_R))
            .into_any_element(),
    }
}

/// A square of a cover `size` wide, round at the top left and the bottom left as asked (the
/// corners at the tile's edge). A plain square while the small copy is being made.
fn quick_image(path: &str, size: f32, (top, bottom): (bool, bool), cx: &App) -> Div {
    let p = pal(cx);
    let r = px(QUICK_R);
    let corners = |el: Div| {
        el.when(top, |el| el.rounded_tl(r))
            .when(bottom, |el| el.rounded_bl(r))
    };
    let square = corners(div().size(px(size)).flex_shrink_0().bg(p.raised_hover));
    match super::thumbs::for_size(path, size) {
        Some(file) => {
            let mut picture = img(file).size_full().object_fit(ObjectFit::Cover);
            if top {
                picture = picture.rounded_tl(r);
            }
            if bottom {
                picture = picture.rounded_bl(r);
            }
            square.child(picture)
        }
        None => square,
    }
}

#[cfg(test)]
mod tests {
    use super::{Quick, quick_starts};
    use needle_core::browse::{AlbumSummary, Home};
    use needle_core::model::Playlist;

    fn album(key: &str) -> AlbumSummary {
        AlbumSummary {
            key: key.into(),
            album: key.into(),
            ..Default::default()
        }
    }

    fn playlist(name: &str) -> Playlist {
        Playlist {
            id: name.into(),
            name: name.into(),
            ..Default::default()
        }
    }

    #[test]
    fn six_in_order_then_the_albums_played_last_fill() {
        let home = Home {
            favorites: vec![album("f")],
            most_played: vec![album("a"), album("b"), album("c")],
            recent: vec![album("a"), album("r")],
            ..Default::default()
        };
        let names: Vec<String> =
            quick_starts(&home, &[playlist("p1"), playlist("p2"), playlist("p3")], 10)
                .iter()
                .map(Quick::name)
                .collect();
        assert_eq!(names, ["Favorites", "Your station", "a", "b", "p1", "p2"]);
        // No favorites and no playlists: the albums played last fill, each once.
        let home = Home {
            most_played: vec![album("a")],
            recent: vec![album("a"), album("r"), album("s"), album("t")],
            ..Default::default()
        };
        let names: Vec<String> = quick_starts(&home, &[], 10)
            .iter()
            .map(Quick::name)
            .collect();
        assert_eq!(names, ["Your station", "a", "r", "s"]);
    }

    #[test]
    fn an_even_number_and_none_under_two() {
        let two = Home {
            most_played: vec![album("a"), album("b")],
            ..Default::default()
        };
        assert_eq!(quick_starts(&two, &[], 10).len(), 2);
        assert_eq!(quick_starts(&two, &[], 0).len(), 2);
        let one = Home {
            most_played: vec![album("a")],
            ..Default::default()
        };
        assert!(quick_starts(&one, &[], 0).is_empty());
        assert!(quick_starts(&Home::default(), &[], 10).is_empty());
    }

    #[test]
    fn lights_by_why_it_plays() {
        let a = Quick::Album(Box::new(album("a")));
        assert!(a.plays("From the album a", "a"));
        assert!(!a.plays("From the playlist a", "a"));
        assert!(Quick::Playlist(Box::new(playlist("p"))).plays("From the playlist p", "x"));
        assert!(Quick::Station.plays("Radio: like “x”", "x"));
        assert!(!Quick::Station.plays("Radio: Someone", "x"));
    }
}
