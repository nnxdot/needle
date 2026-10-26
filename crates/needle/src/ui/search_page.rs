//! Search: what a search finds across the whole library (`Library::find`), the best match
//! large, then songs, albums, artists, the lines of lyrics that hold the words, and rules to
//! try. Typing on a page that is not a list opens it; on a list page, typing filters the list.
use super::{
    AppView, Event, Page, album_page,
    library::{album_groups, artist_groups},
    pal, quote,
    widgets::{artwork, display, faint, meta, segmented},
};
use gpui::{prelude::*, *};
use gpui_component::button::{Button, ButtonVariants};
use needle_core::{
    find::{Best, Found, LyricMatch, RULE_EXAMPLES},
    model::{Track, format_duration},
};

/// What the Search page shows: everything, or every line of lyrics that holds the words.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum SearchTab {
    #[default]
    All,
    Lyrics,
}

/// How many of each All shows; Lyrics shows up to `LYRICS_ALL`.
const EACH: usize = 8;
const SONGS: usize = 5;
const LYRICS: usize = 3;
const LYRICS_ALL: usize = 50;

impl AppView {
    /// Looks the search up again, on another thread; an older answer is dropped.
    pub(super) fn run_find(&mut self, cx: &mut Context<Self>) {
        self.find_generation += 1;
        let generation = self.find_generation;
        let text = self.search_text(cx);
        if text.is_empty() {
            self.found = None;
            self.found_error = None;
            self.found_lyrics.clear();
            cx.notify();
            return;
        }
        let (library, sender, tab) = (self.library.clone(), self.sender.clone(), self.search_tab);
        std::thread::spawn(move || {
            let event = match tab {
                SearchTab::All => Event::Found(
                    generation,
                    library
                        .find(&text, EACH)
                        .map(Box::new)
                        .map_err(|e| format!("{e:#}")),
                ),
                SearchTab::Lyrics => Event::FoundLyrics(
                    generation,
                    library.lyric_matches(&text, LYRICS_ALL).unwrap_or_default(),
                ),
            };
            let _ = sender.send(event);
        });
    }

    /// An answer came: kept when it is the newest.
    pub(super) fn found(&mut self, generation: u64, found: Result<Box<Found>, String>) {
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
                self.found = Some(found);
                self.found_error = None;
            }
            Err(error) => {
                self.found = None;
                self.found_error = Some(error);
            }
        }
    }

    /// The search, on another page that shows it (Songs, Albums, Artists), as typing there
    /// does: the text stays.
    fn search_on(&mut self, page: Page, cx: &mut Context<Self>) {
        self.back.push(self.page.clone());
        self.forward.clear();
        self.page = page;
        self.page_offset = 0;
        self.refresh(cx);
        cx.notify();
    }

    pub(super) fn search_view(&self, width: f32, cx: &mut Context<Self>) -> AnyElement {
        let p = pal(cx);
        let text = self.search_text(cx);
        let weak = cx.entity().downgrade();
        let tab = self.search_tab;
        let chips = segmented(
            "search-tabs",
            &["All", "Songs", "Albums", "Artists", "Lyrics"],
            if tab == SearchTab::Lyrics { 4 } else { 0 },
            cx,
            move |index, _, cx| {
                let _ = weak.update(cx, |this, cx| match index {
                    1 => this.search_on(Page::Songs, cx),
                    2 => this.search_on(Page::Albums, cx),
                    3 => this.search_on(Page::Artists, cx),
                    _ => {
                        this.search_tab = if index == 4 {
                            SearchTab::Lyrics
                        } else {
                            SearchTab::All
                        };
                        this.run_find(cx);
                    }
                });
            },
        );
        let head = div()
            .px_8()
            .pt_10()
            .pb_4()
            .flex()
            .flex_col()
            .gap_4()
            .child(display("Search", 44.))
            .child(div().flex().child(chips));
        let body: AnyElement = if text.is_empty() {
            div()
                .px_8()
                .flex()
                .flex_col()
                .gap_3()
                .child(meta(
                    "Search your songs, albums, and artists, and the words of their lyrics.",
                    cx,
                ))
                .child(self.rule_chips(cx))
                .into_any_element()
        } else if let Some(error) = &self.found_error {
            div()
                .px_8()
                .child(meta(error.clone(), cx).text_color(p.danger))
                .into_any_element()
        } else if tab == SearchTab::Lyrics {
            self.lyrics_found(&text, &self.found_lyrics, cx)
        } else if let Some(found) = &self.found {
            self.everything_found(&text, found, width, cx)
        } else {
            div().into_any_element()
        };
        div()
            .id("search-page")
            .size_full()
            .overflow_y_scroll()
            .pb(px(40.) + self.foot_room())
            .child(head)
            .child(body)
            .into_any_element()
    }

    fn found_section(title: &'static str, cx: &App) -> Div {
        let _ = cx;
        div().px_8().pt_6().pb_3().child(display(title, 22.))
    }

    fn everything_found(
        &self,
        text: &str,
        found: &Found,
        width: f32,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if found.best.is_none()
            && found.songs.is_empty()
            && found.albums.is_empty()
            && found.artists.is_empty()
            && found.lyrics.is_empty()
        {
            return div()
                .px_8()
                .flex()
                .flex_col()
                .gap_3()
                .child(meta(format!("Nothing matches “{text}”."), cx))
                .child(self.rule_chips(cx))
                .into_any_element();
        }
        let wide = width > 820.;
        let best = found.best.as_ref().map(|b| self.best_card(b, cx));
        let songs = (!found.songs.is_empty()).then(|| {
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .child(display("Songs", 22.).pb_2())
                .children(
                    found
                        .songs
                        .iter()
                        .take(SONGS)
                        .enumerate()
                        .map(|(i, t)| self.song_row(i, t, &found.songs[i..], cx)),
                )
        });
        let top = div()
            .px_8()
            .flex()
            .when(wide, |el| el.flex_row().gap_8())
            .when(!wide, |el| el.flex_col().gap_6())
            .children(best)
            .children(songs);
        let albums = (!found.albums.is_empty()).then(|| {
            let tiles: Vec<AnyElement> = album_groups(found.albums.clone())
                .iter()
                .enumerate()
                .map(|(j, g)| self.shelf_tile(9_000 + j, g, false, cx).into_any_element())
                .collect();
            div()
                .child(Self::found_section("Albums", cx))
                .child(self.shelf_row(90, tiles, cx))
        });
        let artists = (!found.artists.is_empty()).then(|| {
            let tiles: Vec<AnyElement> = artist_groups(found.artists.clone())
                .iter()
                .enumerate()
                .map(|(j, g)| self.shelf_tile(9_500 + j, g, true, cx).into_any_element())
                .collect();
            div()
                .child(Self::found_section("Artists", cx))
                .child(self.shelf_row(91, tiles, cx))
        });
        let lyrics = (!found.lyrics.is_empty()).then(|| {
            div().child(Self::found_section("In the lyrics", cx)).child(
                div().px_8().flex().flex_col().children(
                    found
                        .lyrics
                        .iter()
                        .take(LYRICS)
                        .enumerate()
                        .map(|(i, m)| self.lyric_row(i, text, m, cx)),
                ),
            )
        });
        div()
            .flex()
            .flex_col()
            .child(top)
            .children(albums)
            .children(artists)
            .children(lyrics)
            .child(Self::found_section("Try a rule", cx))
            .child(div().px_8().child(self.rule_chips(cx)))
            .into_any_element()
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

    /// The best match, large: its cover (or the artist's photo), what it is, its name, Play.
    fn best_card(&self, best: &Best, cx: &mut Context<Self>) -> AnyElement {
        let p = pal(cx);
        let (kind, name, under, picture, page): (&str, String, String, AnyElement, Page) =
            match best {
                Best::Song(t) => (
                    "Song",
                    t.title.clone(),
                    t.artist.clone(),
                    artwork(Some(&**t), 112., cx),
                    album_page(t),
                ),
                Best::Album(a) => (
                    "Album",
                    a.album.clone(),
                    a.artist.clone(),
                    super::widgets::cover(a.artwork.as_deref(), &a.key, 112., cx),
                    Page::Album {
                        album: a.album.clone(),
                        artist: a.artist.clone(),
                        query: a.query.clone(),
                    },
                ),
                Best::Artist(a) => (
                    "Artist",
                    a.name.clone(),
                    format!("{} songs", a.tracks),
                    div()
                        .rounded_full()
                        .child(self.artist_photo(&a.name, 112., cx))
                        .into_any_element(),
                    Page::Artist(a.name.clone()),
                ),
            };
        let play = best.clone();
        let open = page.clone();
        div()
            .id("search-best")
            .w(px(300.))
            .flex_shrink_0()
            .p_5()
            .rounded(px(14.))
            .bg(p.accent.opacity(0.1))
            .border_1()
            .border_color(p.accent.opacity(0.18))
            .cursor_pointer()
            .hover(|s| s.bg(p.accent.opacity(0.15)))
            .flex()
            .flex_col()
            .gap_1()
            .child(
                div()
                    .pb_3()
                    .child(div().rounded(px(8.)).shadow_lg().child(picture)),
            )
            .child(
                faint(kind, cx)
                    .text_color(p.accent)
                    .font_weight(FontWeight::SEMIBOLD),
            )
            .child(display(name, 26.).truncate())
            .child(meta(under, cx).truncate())
            .child(
                div().pt_3().child(
                    Button::new("search-best-play")
                        .primary()
                        .label("Play")
                        .on_click(cx.listener(move |this, _, _, cx| {
                            cx.stop_propagation();
                            this.play_best(&play);
                        })),
                ),
            )
            .on_click(
                cx.listener(move |this, _, window, cx| this.navigate(open.clone(), window, cx)),
            )
            .into_any_element()
    }

    fn play_best(&mut self, best: &Best) {
        let tracks = match best {
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
        self.play_tracks(tracks, "From your search");
    }

    /// A song: click plays it and the songs found after it.
    fn song_row(&self, i: usize, track: &Track, from: &[Track], cx: &mut Context<Self>) -> Div {
        let p = pal(cx);
        let playing = self
            .playback
            .current
            .as_ref()
            .is_some_and(|c| c.track.id == track.id);
        let list = from.to_vec();
        div().child(
            div()
                .id(("search-song", i))
                .flex()
                .items_center()
                .gap_3()
                .px_2()
                .py(px(5.))
                .rounded(px(8.))
                .cursor_pointer()
                .hover(|s| s.bg(p.raised.opacity(0.6)))
                .child(artwork(Some(track), 40., cx))
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
                                .child(track.title.clone()),
                        )
                        .child(meta(format!("{} · {}", track.artist, track.album), cx).truncate()),
                )
                .child(faint(format_duration(track.duration), cx))
                .on_click(cx.listener(move |this, _, _, _| {
                    this.play_tracks(list.clone(), "From your search")
                })),
        )
    }

    /// A line of lyrics with the words lit, and its song under it; click plays the song.
    fn lyric_row(&self, i: usize, text: &str, found: &LyricMatch, cx: &mut Context<Self>) -> Div {
        let p = pal(cx);
        let line = found.line.clone();
        let (before, lit, after) = match line.to_lowercase().find(&text.to_lowercase()) {
            Some(at) if line.is_char_boundary(at) && line.is_char_boundary(at + text.len()) => (
                line[..at].to_string(),
                line[at..at + text.len()].to_string(),
                line[at + text.len()..].to_string(),
            ),
            _ => (line.clone(), String::new(), String::new()),
        };
        let track = found.track.clone();
        div().child(
            div()
                .id(("search-lyric", i))
                .flex()
                .items_center()
                .gap_3()
                .px_2()
                .py(px(6.))
                .rounded(px(8.))
                .cursor_pointer()
                .hover(|s| s.bg(p.raised.opacity(0.6)))
                .child(artwork(Some(&found.track), 40., cx))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .child(
                            div()
                                .flex()
                                .text_size(px(14.))
                                .truncate()
                                .child(format!("“{before}"))
                                .child(
                                    div()
                                        .text_color(p.accent)
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .child(lit),
                                )
                                .child(format!("{after}”")),
                        )
                        .child(
                            meta(
                                format!("{} · {}", found.track.title, found.track.artist),
                                cx,
                            )
                            .truncate(),
                        ),
                )
                .on_click(cx.listener(move |this, _, _, _| {
                    this.play_tracks(vec![track.clone()], "From your search")
                })),
        )
    }

    /// Rules to try: each puts its rule in the search field.
    fn rule_chips(&self, cx: &mut Context<Self>) -> Div {
        let p = pal(cx);
        div()
            .flex()
            .flex_wrap()
            .gap_2()
            .children(RULE_EXAMPLES.iter().enumerate().map(|(i, (name, rule))| {
                let rule = *rule;
                div()
                    .id(("search-rule", i))
                    .px_3()
                    .py(px(6.))
                    .rounded_full()
                    .bg(p.raised)
                    .border_1()
                    .border_color(p.line_soft)
                    .text_size(px(12.5))
                    .cursor_pointer()
                    .hover(|s| {
                        s.bg(p.raised.opacity(0.7))
                            .border_color(p.accent.opacity(0.4))
                    })
                    .flex()
                    .gap_2()
                    .child(div().child(*name))
                    .child(faint(rule, cx).font_family("Consolas"))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.search
                            .update(cx, |s, cx| s.set_value(rule, window, cx));
                        this.search_tab = SearchTab::All;
                        this.run_find(cx);
                    }))
            }))
    }
}
