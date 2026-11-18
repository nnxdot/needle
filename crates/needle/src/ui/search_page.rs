//! Search: what a search finds across the whole library (`Library::find`). Before typing:
//! the recent searches, genres to browse, and rules. While typing: chips (All, Songs, Albums,
//! Artists, Lyrics) with their counts, then on All the best match on a card tinted with its
//! cover's colour, songs, albums, artists, and the lines of lyrics that hold the words.
//! Typing on a page that is not a list opens it; on a list page, typing filters the list.
use super::{
    AppView, Event, Page, QueueItem,
    library::{Group, album_groups, artist_groups},
    motion, pal, quote,
    theme::display_font,
    widgets::{
        SidewaysOnly as _, artwork, count, cover, display, faint, glyph, icon, meta, seed_color,
    },
};
use gpui::{prelude::*, *};
use gpui_component::button::{Button, ButtonVariants};
use needle_core::{
    browse::{AlbumSummary, ArtistSummary},
    database::Library,
    find::{Best, Found, LyricMatch, RULE_EXAMPLES, Recent},
    model::{Track, format_duration},
};
use rand::seq::SliceRandom;
use std::ops::Range;

/// The chip Search shows.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum SearchTab {
    #[default]
    All,
    Songs,
    Albums,
    Artists,
    Lyrics,
}

const CHIPS: [(SearchTab, &str); 5] = [
    (SearchTab::All, "All"),
    (SearchTab::Songs, "Songs"),
    (SearchTab::Albums, "Albums"),
    (SearchTab::Artists, "Artists"),
    (SearchTab::Lyrics, "Lyrics"),
];

/// A genre to browse: its name, how many songs it has, and one of its covers.
#[derive(Clone, Debug)]
pub struct GenreTile {
    pub name: String,
    pub songs: usize,
    pub artwork: Option<String>,
}

/// How many of each All asks for and shows.
const EACH: usize = 8;
const SONGS: usize = 5;
const LYRICS: usize = 3;
/// How many a chip page asks for: lyrics, and songs, albums, or artists.
const LYRICS_ALL: usize = 50;
const ON_A_CHIP: usize = 200;
/// How many recent searches are read (a wide page shows 4 in one line), and genres.
const RECENT: usize = 8;
const RECENT_WIDE: usize = 4;
const GENRES: usize = 8;
/// Content narrower than this is one column.
const WIDE: f32 = 640.;
/// The top result card's width (wide), and its cover, wide and narrow.
const CARD: f32 = 292.;
const CARD_COVER: f32 = 116.;
const CARD_COVER_NARROW: f32 = 96.;
/// A shelf or grid tile.
const TILE: f32 = 150.;
const ARTIST_TILE: f32 = 104.;
const GAP: f32 = 20.;
const FROM_SEARCH: &str = "From your search";

/// The kind and key `remember_search` keeps for a recent search.
fn recent_key(recent: &Recent) -> (&'static str, String) {
    match recent {
        Recent::Words(words) => ("words", words.clone()),
        Recent::Song(t) => ("song", t.id.clone()),
        Recent::Album(a) => ("album", a.key.clone()),
        Recent::Artist(a) => ("artist", a.name.clone()),
    }
}

fn album_page_of(a: &AlbumSummary) -> Page {
    Page::Album {
        album: a.album.clone(),
        artist: a.artist.clone(),
        query: a.query.clone(),
    }
}

fn plural(n: usize, one: &str, many: &str) -> String {
    if n == 1 {
        format!("1 {one}")
    } else {
        format!("{} {many}", count(n))
    }
}

/// The words whose letters are lit in names: none for a rule.
fn lit_words(text: &str) -> Vec<String> {
    if text.contains(['=', '<', '>', '(', ')', '"']) {
        return vec![];
    }
    text.split_whitespace().map(|w| w.to_lowercase()).collect()
}

/// Where in `text` the `words` (lowercase) are, whatever the case: sorted, joined where they
/// touch or overlap.
pub(crate) fn lit_ranges(text: &str, words: &[String]) -> Vec<Range<usize>> {
    let mut marks: Vec<Range<usize>> = vec![];
    for word in words.iter().filter(|w| !w.is_empty()) {
        for (start, _) in text.char_indices() {
            let mut rest = text[start..].char_indices();
            let mut end = start;
            let matched = word.chars().all(|w| match rest.next() {
                Some((at, c)) if c.to_lowercase().eq(w.to_lowercase()) => {
                    end = start + at + c.len_utf8();
                    true
                }
                _ => false,
            });
            if matched {
                marks.push(start..end);
            }
        }
    }
    marks.sort_by_key(|r| r.start);
    let mut joined: Vec<Range<usize>> = vec![];
    for mark in marks {
        match joined.last_mut() {
            Some(last) if mark.start <= last.end => last.end = last.end.max(mark.end),
            _ => joined.push(mark),
        }
    }
    joined
}

/// A name with the letters that match the search lit: accent colour, bold.
fn lit(text: &str, words: &[String], accent: Hsla) -> StyledText {
    let style = HighlightStyle {
        color: Some(accent),
        font_weight: Some(FontWeight::BOLD),
        ..Default::default()
    };
    StyledText::new(text.to_string())
        .with_highlights(lit_ranges(text, words).into_iter().map(|r| (r, style)))
}

/// The genres with the most songs, each with one of its covers.
fn genre_tiles(library: &Library) -> Vec<GenreTile> {
    let mut genres = library.genres().unwrap_or_default();
    genres.sort_by_key(|(_, songs)| std::cmp::Reverse(*songs));
    genres.truncate(GENRES);
    genres
        .into_iter()
        .map(|(name, songs)| {
            let artwork = library
                .search(&format!("genre = {} limit 40", quote(&name)))
                .unwrap_or_default()
                .into_iter()
                .filter(|t| !t.missing)
                .find_map(|t| t.artwork);
            GenreTile {
                name,
                songs,
                artwork,
            }
        })
        .collect()
}

impl AppView {
    /// Looks the search up again, on another thread; an older answer is dropped. Until the
    /// answer comes, the old one stays.
    pub(super) fn run_find(&mut self, cx: &mut Context<Self>) {
        self.find_generation += 1;
        let generation = self.find_generation;
        let text = self.search_text(cx);
        if text.is_empty() {
            self.found = None;
            self.found_error = None;
            self.load_search_home();
            cx.notify();
            return;
        }
        if self.search_genres.is_none() {
            self.load_search_home();
        }
        let tab = self.search_tab;
        let limit = match tab {
            SearchTab::All => EACH,
            SearchTab::Lyrics => LYRICS_ALL,
            _ => ON_A_CHIP,
        };
        let (library, sender) = (self.library.clone(), self.sender.clone());
        std::thread::spawn(move || {
            let found = library
                .find(&text, limit)
                .map(Box::new)
                .map_err(|e| format!("{e:#}"));
            let _ = sender.send(Event::Found(generation, tab, found));
        });
    }

    /// An answer came: kept when it is the newest.
    pub(super) fn found(
        &mut self,
        generation: u64,
        tab: SearchTab,
        found: Result<Box<Found>, String>,
    ) {
        if generation != self.find_generation {
            return;
        }
        match found {
            Ok(found) => {
                let names: Vec<String> = found
                    .artists
                    .iter()
                    .map(|a| a.name.clone())
                    .chain(match &found.best {
                        Some(Best::Artist(a)) => Some(a.name.clone()),
                        _ => None,
                    })
                    .collect();
                for name in names {
                    self.request_artist_image(&name, false);
                }
                // The card's colour: measured once, in the background.
                if let Some(path) = found.best.as_ref().and_then(Self::best_art) {
                    let _ = self.look(&path);
                }
                self.found = Some(found);
                self.found_for = tab;
                self.found_shown = generation;
                self.found_error = None;
            }
            Err(error) => {
                self.found = None;
                self.found_error = Some(error);
            }
        }
    }

    /// Reads the recent searches, and the genres the first time.
    pub(super) fn load_search_home(&mut self) {
        let genres = self.search_genres.is_none();
        if genres {
            self.search_genres = Some(Default::default());
        }
        let (library, sender) = (self.library.clone(), self.sender.clone());
        std::thread::spawn(move || {
            let recent = library.recent_searches(RECENT).unwrap_or_default();
            let genres = genres.then(|| genre_tiles(&library));
            let _ = sender.send(Event::SearchHome(recent, genres));
        });
    }

    pub(super) fn search_home(&mut self, recent: Vec<Recent>, genres: Option<Vec<GenreTile>>) {
        for item in &recent {
            if let Recent::Artist(a) = item {
                self.request_artist_image(&a.name.clone(), false);
            }
        }
        self.search_recent = recent;
        if let Some(genres) = genres {
            self.search_genres = Some(std::rc::Rc::new(genres));
        }
    }

    /// Keeps something looked for (see `Library::remember_search`).
    pub(super) fn remember_search(&mut self, kind: &'static str, key: &str) {
        let (library, key) = (self.library.clone(), key.to_string());
        std::thread::spawn(move || {
            let _ = library.remember_search(kind, &key);
        });
    }

    /// The ×: drops one recent search; the next one moves up.
    fn forget_search(&mut self, kind: &'static str, key: String, cx: &mut Context<Self>) {
        self.search_recent
            .retain(|r| recent_key(r) != (kind, key.clone()));
        let (library, sender) = (self.library.clone(), self.sender.clone());
        std::thread::spawn(move || {
            let _ = library.forget_search(kind, &key);
            let recent = library.recent_searches(RECENT).unwrap_or_default();
            let _ = sender.send(Event::SearchHome(recent, None));
        });
        cx.notify();
    }

    /// Clear: every recent search goes, with no question.
    fn clear_searches(&mut self, cx: &mut Context<Self>) {
        self.search_recent.clear();
        let library = self.library.clone();
        std::thread::spawn(move || {
            let _ = library.clear_searches();
        });
        cx.notify();
    }

    /// The search, on another page that shows it (Songs, Albums, Artists), as typing there
    /// does: the text stays.
    fn search_on(&mut self, page: Page, cx: &mut Context<Self>) {
        self.back.push(self.page.clone());
        self.forward.clear();
        self.page = page;
        self.refresh(cx);
        cx.notify();
    }

    /// Puts `text` in the search field and looks for it, on All.
    fn search_for(&mut self, text: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.search
            .update(cx, |s, cx| s.set_value(text.to_string(), window, cx));
        self.search_tab = SearchTab::All;
        self.page_scroll.set_offset(point(px(0.), px(0.)));
        self.run_find(cx);
    }

    fn choose_tab(&mut self, tab: SearchTab, cx: &mut Context<Self>) {
        if self.search_tab == tab {
            return;
        }
        self.search_tab = tab;
        self.page_scroll.set_offset(point(px(0.), px(0.)));
        self.run_find(cx);
        cx.notify();
    }

    pub(super) fn search_view(&self, width: f32, cx: &mut Context<Self>) -> AnyElement {
        let p = pal(cx);
        let text = self.search_text(cx);
        let typing = !text.is_empty();
        let content = (width - 64.).max(200.);
        let head = div()
            .px_8()
            .pt_10()
            .pb_4()
            .flex()
            .flex_col()
            .gap_4()
            .child(display("Search", 44.))
            .when(typing, |el| el.child(self.chip_row(cx)));
        let body: AnyElement = if !typing {
            self.before_typing(content, cx)
        } else if let Some(error) = &self.found_error {
            div()
                .flex()
                .flex_col()
                .gap_6()
                .child(
                    div()
                        .px_8()
                        .child(meta(error.clone(), cx).text_color(p.danger)),
                )
                .child(self.rules_section(content, cx))
                .into_any_element()
        } else if let Some(found) = &self.found {
            let words = lit_words(&text);
            let shown = if Self::nothing(found) {
                self.nothing_found(&text, content, cx)
            } else {
                match self.search_tab {
                    SearchTab::All => self.everything_found(&text, &words, found, content, cx),
                    SearchTab::Songs => self.songs_found(&words, found, cx),
                    SearchTab::Albums => self.groups_found(&words, found, false, content, cx),
                    SearchTab::Artists => self.groups_found(&words, found, true, content, cx),
                    SearchTab::Lyrics => self.lyrics_found(&text, &found.lyrics, cx),
                }
            };
            // New results fade in; the old ones stayed until now.
            motion::fade(
                div().child(shown),
                ("search-found", self.found_shown as usize),
                180,
                cx,
                |el, t, _| el.opacity(0.25 + 0.75 * t),
            )
        } else {
            div().into_any_element()
        };
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(head)
            .child(
                div()
                    .id("search-page")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .track_scroll(&self.page_scroll)
                    .pb(px(40.) + self.foot_room())
                    .child(body),
            )
            .into_any_element()
    }

    fn nothing(found: &Found) -> bool {
        found.best.is_none()
            && found.song_count == 0
            && found.album_count == 0
            && found.artist_count == 0
            && found.lyric_count == 0
    }

    /// The chips, each but All with its count; one with none is dimmed and does nothing.
    fn chip_row(&self, cx: &mut Context<Self>) -> Div {
        let p = pal(cx);
        let found = self.found.as_deref();
        div()
            .flex()
            .gap_2()
            .children(CHIPS.iter().enumerate().map(|(i, (tab, name))| {
                let tab = *tab;
                let n = found.map(|f| match tab {
                    SearchTab::All => 0,
                    SearchTab::Songs => f.song_count,
                    SearchTab::Albums => f.album_count,
                    SearchTab::Artists => f.artist_count,
                    SearchTab::Lyrics => f.lyric_count,
                });
                let on = tab == self.search_tab;
                let none = tab != SearchTab::All && n == Some(0) && !on;
                let chip = div()
                    .id(("search-chip", i))
                    .h(px(30.))
                    .px(px(13.))
                    .rounded_full()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .text_size(px(13.))
                    .when(on, |el| {
                        el.bg(p.accent)
                            .text_color(p.accent_ink)
                            .font_weight(FontWeight::SEMIBOLD)
                    })
                    .when(!on, |el| {
                        el.bg(p.raised)
                            .text_color(p.ink)
                            .font_weight(FontWeight::MEDIUM)
                    })
                    .child(*name)
                    .when_some(n.filter(|_| tab != SearchTab::All), |el, n| {
                        el.child(div().text_size(px(11.5)).opacity(0.62).child(count(n)))
                    });
                if none {
                    chip.opacity(0.4)
                } else if on {
                    chip
                } else {
                    chip.cursor_pointer()
                        .hover(|s| s.bg(p.raised_hover))
                        .on_click(cx.listener(move |this, _, _, cx| this.choose_tab(tab, cx)))
                }
            }))
    }

    /// A part's title, with "All N ›" at the end when it has a chip of its own.
    fn part_head(
        &self,
        title: &'static str,
        all: Option<(usize, SearchTab)>,
        cx: &mut Context<Self>,
    ) -> Div {
        let p = pal(cx);
        div()
            .pb_2()
            .flex()
            .items_end()
            .justify_between()
            .gap_4()
            .child(display(title, 22.))
            .when_some(all, |el, (n, tab)| {
                el.child(
                    div()
                        .id(SharedString::from(format!("search-all-{title}")))
                        .text_size(px(12.5))
                        .text_color(p.ink_2)
                        .cursor_pointer()
                        .hover(|s| s.text_color(p.ink))
                        .child(format!("All {} ›", count(n)))
                        .on_click(cx.listener(move |this, _, _, cx| this.choose_tab(tab, cx))),
                )
            })
    }

    /// A part's title with an action at the end ("Clear").
    fn part_head_with(
        &self,
        title: &'static str,
        action: &'static str,
        on_click: impl Fn(&mut Self, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) -> Div {
        let p = pal(cx);
        div()
            .pb_2()
            .flex()
            .items_end()
            .justify_between()
            .child(display(title, 22.))
            .child(
                div()
                    .id(SharedString::from(format!("search-head-{title}")))
                    .text_size(px(12.5))
                    .text_color(p.ink_2)
                    .cursor_pointer()
                    .hover(|s| s.text_color(p.ink))
                    .child(action)
                    .on_click(cx.listener(move |this, _, _, cx| on_click(this, cx))),
            )
    }

    // ---- Before typing ----

    fn before_typing(&self, content: f32, cx: &mut Context<Self>) -> AnyElement {
        div()
            .flex()
            .flex_col()
            .gap_6()
            .children(self.recent_section(content, cx))
            .children(self.browse_section(content, cx))
            .child(self.rules_section(content, cx))
            .into_any_element()
    }

    /// Recent: one line of up to 4 tiles on a wide page, rows on a narrow one.
    fn recent_section(&self, content: f32, cx: &mut Context<Self>) -> Option<Div> {
        if self.search_recent.is_empty() {
            return None;
        }
        let wide = content >= WIDE;
        let head = self.part_head_with("Recent", "Clear", |this, cx| this.clear_searches(cx), cx);
        let list = if wide {
            let gap = 12.;
            let w = (content - gap * (RECENT_WIDE as f32 - 1.)) / RECENT_WIDE as f32;
            div().flex().gap(px(gap)).children(
                self.search_recent
                    .iter()
                    .take(RECENT_WIDE)
                    .enumerate()
                    .map(|(i, r)| self.recent_item(i, r, Some(w), cx)),
            )
        } else {
            div().flex().flex_col().children(
                self.search_recent
                    .iter()
                    .enumerate()
                    .map(|(i, r)| self.recent_item(i, r, None, cx)),
            )
        };
        Some(div().px_8().flex().flex_col().child(head).child(list))
    }

    /// One recent search: a tile `width` wide, or a row. Its × shows under the pointer.
    fn recent_item(
        &self,
        i: usize,
        recent: &Recent,
        width: Option<f32>,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let p = pal(cx);
        let size = 40.;
        let (picture, name, second): (AnyElement, String, String) = match recent {
            Recent::Words(words) => (
                div()
                    .size(px(size))
                    .flex_shrink_0()
                    .rounded(px(6.))
                    .bg(p.raised_hover)
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(glyph("recent").text_color(p.ink_2))
                    .into_any_element(),
                words.clone(),
                "You searched for this".into(),
            ),
            Recent::Song(t) => (
                artwork(Some(&**t), size, cx),
                t.title.clone(),
                format!("Song · {}", t.artist),
            ),
            Recent::Album(a) => (
                cover(a.artwork.as_deref(), &a.key, size, cx),
                a.album.clone(),
                format!("Album · {}", a.artist),
            ),
            Recent::Artist(a) => (
                self.artist_photo(&a.name, size, cx),
                a.name.clone(),
                "Artist".into(),
            ),
        };
        let (kind, key) = recent_key(recent);
        let group = SharedString::from(format!("search-recent-{i}"));
        let press = recent.clone();
        div()
            .id(("search-recent", i))
            .group(group.clone())
            .flex()
            .items_center()
            .gap_3()
            .p(px(8.))
            .rounded(px(12.))
            .cursor_pointer()
            .when_some(width, |el, w| {
                el.w(px(w)).flex_shrink_0().bg(p.raised.opacity(0.7))
            })
            .hover(|s| s.bg(p.raised_hover))
            .child(picture)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .child(
                        div()
                            .text_size(px(13.5))
                            .font_weight(FontWeight::MEDIUM)
                            .truncate()
                            .child(name),
                    )
                    .child(meta(second, cx).truncate()),
            )
            .child(
                div()
                    .id(("search-recent-x", i))
                    .size(px(24.))
                    .flex_shrink_0()
                    .rounded_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .opacity(0.)
                    .group_hover(group, |s| s.opacity(1.))
                    .hover(|s| s.bg(p.raised))
                    .child(glyph("close").size(px(14.)).text_color(p.ink_2))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        this.forget_search(kind, key.clone(), cx);
                    })),
            )
            .on_click(cx.listener(move |this, _, window, cx| this.open_recent(&press, window, cx)))
    }

    /// A press on a recent search: words go in the field, a song plays, an album or artist
    /// opens.
    fn open_recent(&mut self, recent: &Recent, window: &mut Window, cx: &mut Context<Self>) {
        let (kind, key) = recent_key(recent);
        match recent {
            Recent::Words(words) => {
                self.search_for(words, window, cx);
                return;
            }
            Recent::Song(t) => self.play_tracks(vec![(**t).clone()], FROM_SEARCH),
            Recent::Album(a) => self.navigate(album_page_of(a), window, cx),
            Recent::Artist(a) => self.navigate(Page::Artist(a.name.clone()), window, cx),
        }
        self.remember_search(kind, &key);
    }

    /// Browse: the genres with the most songs, as colour tiles with a cover in the corner.
    fn browse_section(&self, content: f32, cx: &mut Context<Self>) -> Option<Div> {
        let genres = self.search_genres.clone()?;
        if genres.is_empty() {
            return None;
        }
        let columns = if content >= WIDE { 4 } else { 2 };
        let gap = 12.;
        let w = (content - gap * (columns as f32 - 1.)) / columns as f32;
        // Rows of their own: a wrapped row in a column takes too much height (GPUI).
        let head = self.part_head("Browse", None, cx);
        let rows: Vec<Div> = genres
            .chunks(columns)
            .enumerate()
            .map(|(r, row)| {
                div().flex().gap(px(gap)).children(
                    row.iter()
                        .enumerate()
                        .map(|(c, g)| self.genre_tile(r * columns + c, g, w, cx)),
                )
            })
            .collect();
        Some(
            div()
                .px_8()
                .flex()
                .flex_col()
                .child(head)
                .child(div().flex().flex_col().gap(px(gap)).children(rows)),
        )
    }

    fn genre_tile(
        &self,
        i: usize,
        genre: &GenreTile,
        w: f32,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let base = seed_color(&genre.name.to_lowercase());
        let (light, deep) = (hsla(base.h, 0.42, 0.44, 1.), hsla(base.h, 0.5, 0.22, 1.));
        let rule = format!("genre = {}", quote(&genre.name));
        div()
            .id(("search-genre", i))
            .relative()
            .overflow_hidden()
            .w(px(w))
            .h(px(72.))
            .flex_shrink_0()
            .rounded(px(12.))
            .bg(linear_gradient(
                135.,
                linear_color_stop(light, 0.),
                linear_color_stop(deep, 1.),
            ))
            .p(px(12.))
            .pr(px(70.))
            .cursor_pointer()
            .hover(|s| s.opacity(0.88))
            .child(
                div()
                    .text_size(px(15.))
                    .font_weight(FontWeight::BOLD)
                    .text_color(white())
                    .truncate()
                    .child(genre.name.clone()),
            )
            .child(
                div()
                    .text_size(px(11.5))
                    .text_color(white().opacity(0.7))
                    .child(plural(genre.songs, "song", "songs")),
            )
            .child(
                div()
                    .absolute()
                    .right(px(12.))
                    .bottom(px(-12.))
                    .rounded(px(6.))
                    .shadow_lg()
                    .child(cover(genre.artwork.as_deref(), &genre.name, 52., cx)),
            )
            .on_click(cx.listener(move |this, _, window, cx| {
                // The genre's songs: the Songs page with the genre's rule.
                this.search
                    .update(cx, |s, cx| s.set_value(rule.clone(), window, cx));
                this.search_on(Page::Songs, cx);
            }))
    }

    /// Rules: each chip puts its rule in the search field; a wide page shows the rule too.
    fn rules_section(&self, content: f32, cx: &mut Context<Self>) -> Div {
        let p = pal(cx);
        let wide = content >= WIDE;
        div()
            .px_8()
            .flex()
            .flex_col()
            .child(self.part_head("Rules", None, cx))
            .child(
                div()
                    .id("search-rules")
                    .overflow_x_scroll()
                    .flex()
                    .gap_2()
                    .children(RULE_EXAMPLES.iter().enumerate().map(|(i, (name, rule))| {
                        let rule = *rule;
                        div()
                            .id(("search-rule", i))
                            .flex_shrink_0()
                            .h(px(32.))
                            .px(px(13.))
                            .rounded_full()
                            .border_1()
                            .border_color(p.line)
                            .text_size(px(13.))
                            .cursor_pointer()
                            .hover(|s| s.bg(p.raised).border_color(p.accent.opacity(0.4)))
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(div().child(*name))
                            .when(wide, |el| {
                                el.child(
                                    faint(rule, cx).font_family("Consolas").text_size(px(11.5)),
                                )
                            })
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.search_for(rule, window, cx)
                            }))
                    })),
            )
    }

    // ---- While typing ----

    /// Nothing matches: a round icon, what to try, then Rules and Recent.
    fn nothing_found(&self, text: &str, content: f32, cx: &mut Context<Self>) -> AnyElement {
        let p = pal(cx);
        div()
            .flex()
            .flex_col()
            .gap_6()
            .child(
                div()
                    .px_8()
                    .pt_8()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap_1()
                    .child(
                        div()
                            .size(px(64.))
                            .mb_3()
                            .rounded_full()
                            .bg(p.raised)
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(glyph("search").size(px(28.)).text_color(p.ink_2)),
                    )
                    .child(
                        div()
                            .w_full()
                            .text_center()
                            .text_size(px(18.))
                            .font_weight(FontWeight::BOLD)
                            .child(format!("Nothing for “{text}”")),
                    )
                    .child(
                        meta("Look at the spelling. Or search with a rule.", cx)
                            .w_full()
                            .text_center(),
                    ),
            )
            .child(self.rules_section(content, cx))
            .children(self.recent_section(content, cx))
            .into_any_element()
    }

    fn everything_found(
        &self,
        text: &str,
        words: &[String],
        found: &Found,
        content: f32,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let wide = content >= WIDE;
        let best = found
            .best
            .as_ref()
            .map(|b| self.best_card(b, words, wide, cx));
        let songs = (!found.songs.is_empty()).then(|| {
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .child(self.part_head("Songs", Some((found.song_count, SearchTab::Songs)), cx))
                .children(
                    found
                        .songs
                        .iter()
                        .take(SONGS)
                        .enumerate()
                        .map(|(i, t)| self.song_row(i, t, &found.songs[i..], words, false, cx)),
                )
        });
        let top = div()
            .px_8()
            .flex()
            .when(wide, |el| el.flex_row().items_start().gap(px(28.)))
            .when(!wide, |el| el.flex_col().gap_6())
            .children(best)
            .children(songs);
        let albums = (!found.albums.is_empty()).then(|| {
            let tiles: Vec<AnyElement> = album_groups(found.albums.clone())
                .iter()
                .zip(&found.albums)
                .enumerate()
                .map(|(j, (g, a))| {
                    self.search_tile(
                        9_000 + j,
                        g,
                        false,
                        words,
                        ("album", a.key.clone()),
                        TILE,
                        cx,
                    )
                    .into_any_element()
                })
                .collect();
            div()
                .pt_6()
                .child(div().px_8().child(self.part_head(
                    "Albums",
                    Some((found.album_count, SearchTab::Albums)),
                    cx,
                )))
                .child(self.shelf_row(90, tiles, cx))
        });
        let artist_tiles = |size: f32, cx: &mut Context<Self>| -> Vec<AnyElement> {
            artist_groups(found.artists.clone())
                .iter()
                .zip(&found.artists)
                .enumerate()
                .map(|(j, (g, a))| {
                    self.search_tile(
                        9_500 + j,
                        g,
                        true,
                        words,
                        ("artist", a.name.clone()),
                        size,
                        cx,
                    )
                    .into_any_element()
                })
                .collect()
        };
        let lyric_rows = |cx: &mut Context<Self>| {
            div()
                .flex()
                .flex_col()
                .child(self.part_head(
                    "In the lyrics",
                    Some((found.lyric_count, SearchTab::Lyrics)),
                    cx,
                ))
                .children(
                    found
                        .lyrics
                        .iter()
                        .take(LYRICS)
                        .enumerate()
                        .map(|(i, m)| self.lyric_row(i, text, m, cx)),
                )
        };
        let (has_artists, has_lyrics) = (!found.artists.is_empty(), !found.lyrics.is_empty());
        // Artists and the lyrics share one line when there is room for both.
        let last: Option<AnyElement> = if wide && has_artists && has_lyrics {
            let tiles = artist_tiles(ARTIST_TILE, cx);
            Some(
                div()
                    .px_8()
                    .pt_6()
                    .flex()
                    .items_start()
                    .gap(px(28.))
                    .child(
                        div()
                            .w(px(CARD))
                            .flex_shrink_0()
                            .flex()
                            .flex_col()
                            .child(self.part_head("Artists", None, cx))
                            .child(
                                div()
                                    .id("search-artists-line")
                                    .overflow_x_scroll()
                                    .sideways_only()
                                    .child(div().flex().gap(px(16.)).children(tiles)),
                            ),
                    )
                    .child(div().flex_1().min_w_0().child(lyric_rows(cx)))
                    .into_any_element(),
            )
        } else {
            let artists = has_artists.then(|| {
                let tiles = artist_tiles(TILE, cx);
                div()
                    .pt_6()
                    .child(div().px_8().child(self.part_head("Artists", None, cx)))
                    .child(self.shelf_row(91, tiles, cx))
            });
            let lyrics = has_lyrics.then(|| div().px_8().pt_6().child(lyric_rows(cx)));
            Some(
                div()
                    .flex()
                    .flex_col()
                    .children(artists)
                    .children(lyrics)
                    .into_any_element(),
            )
        };
        div()
            .flex()
            .flex_col()
            .child(top)
            .children(albums)
            .children(last)
            .into_any_element()
    }

    /// The Songs chip: how many, Shuffle (all that match), the rows, and "Show all".
    fn songs_found(&self, words: &[String], found: &Found, cx: &mut Context<Self>) -> AnyElement {
        if found.songs.is_empty() {
            return div()
                .px_8()
                .child(meta("No songs match.", cx))
                .into_any_element();
        }
        div()
            .px_8()
            .flex()
            .flex_col()
            .child(
                div()
                    .pb_3()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(meta(plural(found.song_count, "song", "songs"), cx))
                    .child(
                        Button::new("search-shuffle")
                            .primary()
                            .icon(icon("shuffle"))
                            .label("Shuffle")
                            .on_click(cx.listener(|this, _, _, cx| this.shuffle_found(cx))),
                    ),
            )
            .children(
                found
                    .songs
                    .iter()
                    .enumerate()
                    .map(|(i, t)| self.song_row(i, t, &found.songs[i..], words, true, cx)),
            )
            .children(self.show_all(SearchTab::Songs, found.song_count, found.songs.len(), cx))
            .into_any_element()
    }

    /// The Albums chip (a grid) or the Artists chip (a grid of round photos).
    fn groups_found(
        &self,
        words: &[String],
        found: &Found,
        round: bool,
        content: f32,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let (groups, keys, total, tab, base): (Vec<Group>, Vec<(&'static str, String)>, _, _, _) =
            if round {
                (
                    artist_groups(found.artists.clone()),
                    found
                        .artists
                        .iter()
                        .map(|a: &ArtistSummary| ("artist", a.name.clone()))
                        .collect(),
                    found.artist_count,
                    SearchTab::Artists,
                    9_500,
                )
            } else {
                (
                    album_groups(found.albums.clone()),
                    found
                        .albums
                        .iter()
                        .map(|a| ("album", a.key.clone()))
                        .collect(),
                    found.album_count,
                    SearchTab::Albums,
                    9_000,
                )
            };
        if groups.is_empty() {
            let none = if round {
                "No artists match."
            } else {
                "No albums match."
            };
            return div().px_8().child(meta(none, cx)).into_any_element();
        }
        let columns = ((content + GAP) / (TILE + GAP)).floor().max(1.) as usize;
        let shown = groups.len();
        let tiles: Vec<AnyElement> = groups
            .iter()
            .zip(keys)
            .enumerate()
            .map(|(j, (g, key))| {
                self.search_tile(base + j, g, round, words, key, TILE, cx)
                    .into_any_element()
            })
            .collect();
        let mut rows = vec![];
        let mut tiles = tiles.into_iter().peekable();
        while tiles.peek().is_some() {
            let row: Vec<AnyElement> = tiles.by_ref().take(columns).collect();
            rows.push(div().flex().gap(px(GAP)).children(row));
        }
        div()
            .px_8()
            .flex()
            .flex_col()
            .gap_6()
            .children(rows)
            .children(self.show_all(tab, total, shown, cx))
            .into_any_element()
    }

    /// The last row of a chip page that shows fewer than all: it opens that page with the
    /// search, as the chip did before.
    fn show_all(
        &self,
        tab: SearchTab,
        total: usize,
        shown: usize,
        cx: &mut Context<Self>,
    ) -> Option<Stateful<Div>> {
        if self.found_for != tab || total <= shown {
            return None;
        }
        let p = pal(cx);
        let (name, page) = match tab {
            SearchTab::Albums => ("Albums", Page::Albums),
            SearchTab::Artists => ("Artists", Page::Artists),
            _ => ("Songs", Page::Songs),
        };
        Some(
            div()
                .id("search-show-all")
                .mt_2()
                .px_2()
                .py(px(10.))
                .rounded(px(8.))
                .text_size(px(13.5))
                .font_weight(FontWeight::MEDIUM)
                .text_color(p.accent)
                .cursor_pointer()
                .hover(|s| s.bg(p.raised.opacity(0.6)))
                .child(format!("Show all {} in {name}", count(total)))
                .on_click(cx.listener(move |this, _, _, cx| this.search_on(page.clone(), cx))),
        )
    }

    fn lyrics_found(&self, text: &str, found: &[LyricMatch], cx: &mut Context<Self>) -> AnyElement {
        if found.is_empty() {
            return div()
                .px_8()
                .child(meta(
                    if text.chars().count() < 3 {
                        "Type three letters or more to look in the lyrics.".to_string()
                    } else {
                        format!("No lyrics hold “{text}”.")
                    },
                    cx,
                ))
                .into_any_element();
        }
        div()
            .px_8()
            .flex()
            .flex_col()
            .children(
                found
                    .iter()
                    .enumerate()
                    .map(|(i, m)| self.lyric_row(i, text, m, cx)),
            )
            .into_any_element()
    }

    /// The best match's cover file (an artist's: a cover of theirs), for the card's colour.
    fn best_art(best: &Best) -> Option<String> {
        match best {
            Best::Song(t) => t.artwork.clone(),
            Best::Album(a) => a.artwork.clone(),
            Best::Artist(a) => a.artwork.clone(),
        }
    }

    /// The top result: its cover (an artist's photo is round), what it is, its name, a detail
    /// line, and two buttons. The card takes the cover's colour under a veil: GPUI cannot blur
    /// it, so the colour is measured from the cover. Wide: the cover above the text.
    fn best_card(
        &self,
        best: &Best,
        words: &[String],
        wide: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let p = pal(cx);
        let size = if wide { CARD_COVER } else { CARD_COVER_NARROW };
        let (kind, name, detail, picture): (&str, String, Vec<String>, AnyElement) = match best {
            Best::Song(t) => (
                "SONG",
                t.title.clone(),
                vec![t.artist.clone(), t.album.clone()],
                artwork(Some(&**t), size, cx),
            ),
            Best::Album(a) => (
                "ALBUM",
                a.album.clone(),
                vec![
                    a.artist.clone(),
                    if a.year > 0 {
                        a.year.to_string()
                    } else {
                        String::new()
                    },
                    plural(a.tracks, "song", "songs"),
                ],
                cover(a.artwork.as_deref(), &a.key, size, cx),
            ),
            Best::Artist(a) => (
                "ARTIST",
                a.name.clone(),
                vec![
                    plural(a.tracks, "song", "songs"),
                    plural(a.albums, "album", "albums"),
                ],
                self.artist_photo(&a.name, size, cx),
            ),
        };
        let detail = detail
            .into_iter()
            .filter(|d| !d.trim().is_empty())
            .collect::<Vec<_>>()
            .join(" · ");
        let round = matches!(best, Best::Artist(_));
        // The cover's colour (or the accent until it is measured), under a veil: dark in the
        // dark look, light in the day look, so the text reads on any cover.
        let tint = Self::best_art(best)
            .and_then(|path| self.cached_look(&path))
            .map_or(p.accent, |l| l.vivid);
        let (from, to, edge) = if p.dark {
            (
                hsla(tint.h, tint.s.min(0.55), 0.25, 1.),
                hsla(tint.h, tint.s.min(0.4), 0.14, 1.),
                white().opacity(0.1),
            )
        } else {
            (
                hsla(tint.h, tint.s.min(0.55), 0.85, 1.),
                hsla(tint.h, tint.s.min(0.4), 0.93, 1.),
                white().opacity(0.75),
            )
        };
        let (play, second) = (best.clone(), best.clone());
        let open = best.clone();
        let buttons = div()
            .flex()
            .gap_2()
            .pt(px(if wide { 14. } else { 10. }))
            .child(
                Button::new("search-best-play")
                    .primary()
                    .icon(icon("play"))
                    .label("Play")
                    .on_click(cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        this.play_best(&play, false);
                    })),
            )
            .child(match best {
                Best::Song(_) => Button::new("search-best-next")
                    .icon(icon("queue"))
                    .label("Play next")
                    .on_click(cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        if let Best::Song(t) = &second {
                            this.remember_search("song", &t.id);
                            this.play_next(vec![(**t).clone()]);
                        }
                    })),
                _ => Button::new("search-best-shuffle")
                    .icon(icon("shuffle"))
                    .label("Shuffle")
                    .on_click(cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        this.play_best(&second, true);
                    })),
            });
        let words_col = div()
            .flex_1()
            .min_w_0()
            .flex()
            .flex_col()
            .child(
                div()
                    .text_size(px(10.5))
                    .font_weight(FontWeight::BOLD)
                    .text_color(p.accent)
                    .child(format!("TOP RESULT · {kind}")),
            )
            .child(
                div()
                    .w_full()
                    .pt(px(2.))
                    .font_family(display_font())
                    .text_size(px(if wide { 23. } else { 20. }))
                    .line_height(px(if wide { 27. } else { 24. }))
                    .font_weight(FontWeight::EXTRA_BOLD)
                    .text_color(p.ink)
                    .line_clamp(2)
                    .child(lit(&name, words, p.accent)),
            )
            .child(
                div()
                    .w_full()
                    .pt(px(2.))
                    .text_size(px(12.5))
                    .text_color(p.ink)
                    .opacity(0.78)
                    .truncate()
                    .child(detail),
            )
            .child(buttons);
        let picture = div()
            .flex_shrink_0()
            .rounded(if round { px(size) } else { px(10.) })
            .shadow_lg()
            .child(picture);
        div()
            .id("search-best")
            .flex_shrink_0()
            .rounded(px(22.))
            .bg(linear_gradient(
                180.,
                linear_color_stop(from, 0.),
                linear_color_stop(to, 1.),
            ))
            .border_1()
            .border_color(edge)
            .shadow_lg()
            .cursor_pointer()
            .hover(|s| s.border_color(p.accent.opacity(0.45)))
            .map(|el| {
                if wide {
                    el.w(px(CARD))
                        .p(px(18.))
                        .flex()
                        .flex_col()
                        .child(div().pb(px(14.)).child(picture))
                        .child(words_col)
                } else {
                    el.w_full()
                        .p(px(14.))
                        .flex()
                        .items_center()
                        .gap(px(14.))
                        .child(picture)
                        .child(words_col)
                }
            })
            .on_click(cx.listener(move |this, _, window, cx| match &open {
                Best::Song(_) => this.play_best(&open, false),
                Best::Album(a) => {
                    this.remember_search("album", &a.key);
                    this.navigate(album_page_of(a), window, cx);
                }
                Best::Artist(a) => {
                    this.remember_search("artist", &a.name);
                    this.navigate(Page::Artist(a.name.clone()), window, cx);
                }
            }))
            .into_any_element()
    }

    fn play_best(&mut self, best: &Best, shuffle: bool) {
        let (kind, key) = match best {
            Best::Song(t) => ("song", t.id.clone()),
            Best::Album(a) => ("album", a.key.clone()),
            Best::Artist(a) => ("artist", a.name.clone()),
        };
        self.remember_search(kind, &key);
        let mut tracks = match best {
            Best::Song(t) => vec![(**t).clone()],
            Best::Album(a) => self.library.album_tracks(&a.key).unwrap_or_default(),
            Best::Artist(a) => self
                .library
                .search(&format!(
                    "artist = {0} or album_artist = {0}",
                    quote(&a.name)
                ))
                .unwrap_or_default(),
        };
        if shuffle {
            tracks.shuffle(&mut rand::thread_rng());
            self.play_tracks(tracks, &format!("Shuffled · {FROM_SEARCH}"));
        } else {
            self.play_tracks(tracks, FROM_SEARCH);
        }
    }

    /// The Songs chip's Shuffle: every song that matches, shuffled.
    fn shuffle_found(&mut self, cx: &mut Context<Self>) {
        let text = self.search_text(cx);
        let (library, sender) = (self.library.clone(), self.sender.clone());
        std::thread::spawn(move || {
            let mut tracks = library.search(&text).unwrap_or_default();
            tracks.retain(|t| !t.missing);
            if tracks.is_empty() {
                return;
            }
            tracks.shuffle(&mut rand::thread_rng());
            let items = tracks
                .into_iter()
                .map(|track| QueueItem {
                    track,
                    reason: format!("Shuffled · {FROM_SEARCH}"),
                })
                .collect();
            let _ = sender.send(Event::Play(items, None));
        });
    }

    /// An album or artist found: its cover or round photo, the name with the matching letters
    /// lit, and the line under it. A press opens it.
    #[allow(clippy::too_many_arguments)]
    fn search_tile(
        &self,
        index: usize,
        group: &Group,
        round: bool,
        words: &[String],
        remember: (&'static str, String),
        size: f32,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let p = pal(cx);
        let open = group.page.clone();
        div()
            .id(("search-tile", index))
            .group("tile")
            .w(px(size))
            .flex_shrink_0()
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
                        cover(group.artwork.as_deref(), &group.seed, size, cx)
                    })
                    .when(size >= TILE, |el| {
                        el.child(self.cover_play(index, group.page.clone(), size, cx))
                    }),
            )
            .child(
                div()
                    .text_size(px(13.5))
                    .font_weight(FontWeight::MEDIUM)
                    .truncate()
                    .when(round, |el| el.text_center())
                    .child(lit(&group.title, words, p.accent)),
            )
            .when(!round || size >= TILE, |el| {
                el.child(
                    meta(group.subtitle.clone(), cx)
                        .truncate()
                        .when(round, |el| el.text_center()),
                )
            })
            .on_click(cx.listener(move |this, _, window, cx| {
                this.remember_search(remember.0, &remember.1);
                this.navigate(open.clone(), window, cx);
            }))
    }

    /// A song: cover, the title (lit) above the artist (and the album on the Songs chip), and
    /// its length. A press plays it and the songs found after it.
    fn song_row(
        &self,
        i: usize,
        track: &Track,
        from: &[Track],
        words: &[String],
        with_album: bool,
        cx: &mut Context<Self>,
    ) -> Div {
        let p = pal(cx);
        let playing = self
            .playback
            .current
            .as_ref()
            .is_some_and(|c| c.track.id == track.id);
        let list = from.to_vec();
        let id = track.id.clone();
        let second = if with_album && !track.album.is_empty() {
            format!("{} · {}", track.artist, track.album)
        } else {
            track.artist.clone()
        };
        div().child(
            div()
                .id(("search-song", i))
                .flex()
                .items_center()
                .gap_3()
                .px_2()
                .py(px(5.))
                .rounded(px(9.))
                .cursor_pointer()
                .hover(|s| s.bg(p.raised.opacity(0.6)))
                .active(|s| s.bg(p.raised))
                .child(artwork(Some(track), 38., cx))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .child(
                            div()
                                .text_size(px(13.5))
                                .font_weight(FontWeight::MEDIUM)
                                .truncate()
                                .when(playing, |el| el.text_color(p.accent))
                                .child(lit(&track.title, words, p.accent)),
                        )
                        .child(
                            div()
                                .text_size(px(12.))
                                .text_color(p.ink_2)
                                .truncate()
                                .child(lit(&second, words, p.accent)),
                        ),
                )
                .child(faint(format_duration(track.duration), cx))
                .on_click(cx.listener(move |this, _, _, _| {
                    this.remember_search("song", &id);
                    this.play_tracks(list.clone(), FROM_SEARCH)
                })),
        )
    }

    /// A line of lyrics with the words lit, and its song under it; a press plays the song.
    fn lyric_row(&self, i: usize, text: &str, found: &LyricMatch, cx: &mut Context<Self>) -> Div {
        let p = pal(cx);
        let line = format!("“{}”", found.line);
        let track = found.track.clone();
        div().child(
            div()
                .id(("search-lyric", i))
                .flex()
                .items_center()
                .gap_3()
                .px_2()
                .py(px(6.))
                .rounded(px(9.))
                .cursor_pointer()
                .hover(|s| s.bg(p.raised.opacity(0.6)))
                .active(|s| s.bg(p.raised))
                .child(artwork(Some(&found.track), 38., cx))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .child(div().text_size(px(14.)).truncate().child(lit(
                            &line,
                            &[text.to_lowercase()],
                            p.accent,
                        )))
                        .child(
                            meta(
                                format!("{} · {}", found.track.title, found.track.artist),
                                cx,
                            )
                            .truncate(),
                        ),
                )
                .on_click(cx.listener(move |this, _, _, _| {
                    this.remember_search("song", &track.id);
                    this.play_tracks(vec![track.clone()], FROM_SEARCH)
                })),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{lit_ranges, lit_words};

    fn words(text: &str) -> Vec<String> {
        lit_words(text)
    }

    #[test]
    fn lights_every_match_whatever_the_case() {
        assert_eq!(lit_ranges("Half Moon Diner", &words("moon")), vec![5..9]);
        assert_eq!(
            lit_ranges("Moonrise moon", &words("MOON")),
            vec![0..4, 9..13]
        );
        assert_eq!(
            lit_ranges("Blue Moon Motel", &words("moon mo")),
            vec![5..9, 10..12]
        );
    }

    #[test]
    fn joins_overlaps_and_keeps_char_bounds() {
        assert_eq!(lit_ranges("aaaa", &words("aa")), vec![0..4]);
        assert_eq!(lit_ranges("Café Über", &words("über")), vec![6..11]);
        assert!(lit_ranges("Moon", &words("rating >= 4")).is_empty());
    }
}
