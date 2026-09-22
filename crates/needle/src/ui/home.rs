//! Home: a greeting, what you were listening to, and shelves of covers.
use super::{
    AppView, Event, Page, album_page,
    library::{Group, album_groups, artist_groups},
    pal,
    widgets::{artwork, cover, display, faint, icon, meta},
};
use chrono::Timelike;
use gpui::{prelude::*, *};
use gpui_component::button::{Button, ButtonVariants};
use needle_core::browse::Home;

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
        });
    }

    pub(super) fn home_view(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = pal(cx);
        let hour = chrono::Local::now().hour();
        let greeting = match hour {
            5..=11 => "Good morning",
            12..=17 => "Good afternoon",
            _ => "Good evening",
        };
        let empty = Home::default();
        let home = self.home.as_deref().unwrap_or(&empty);
        let shelves: Vec<(&str, &str, Vec<Group>, bool, Page)> = [
            (
                "Jump back in",
                "Albums you played last",
                album_groups(home.recent.clone()),
                false,
                Page::History,
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
                "New in your library",
                "Added most recently",
                album_groups(home.added.clone()),
                false,
                Page::Recent,
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
        .filter(|s| !s.2.is_empty())
        .collect();
        div()
            .id("home")
            .size_full()
            .overflow_y_scroll()
            .pb_10()
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
            .children(self.home_now(cx))
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
                        .child(
                            div()
                                .id(("shelf", i))
                                .overflow_x_scroll()
                                .child(div().px_8().flex().gap(px(20.)).children(tiles)),
                        )
                },
            ))
    }

    /// The song playing now (or the last one heard), large, with a way to carry on.
    fn home_now(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        let p = pal(cx);
        let playing = self.playback.current.as_ref().map(|c| c.track.clone());
        let recent = self.home.as_ref().and_then(|h| h.recent.first().cloned());
        let (label, title, subtitle, art, page): (&str, String, String, AnyElement, Page) =
            match (&playing, recent) {
                (Some(track), _) => (
                    if self.playback.playing {
                        "Playing now"
                    } else {
                        "Paused"
                    },
                    track.title.clone(),
                    format!("{} · {}", track.display_artist(), track.album),
                    artwork(Some(track), 132., cx),
                    album_page(track),
                ),
                (None, Some(album)) => (
                    "Last played",
                    album.album.clone(),
                    album.artist.clone(),
                    cover(
                        album.artwork.as_deref(),
                        &format!("{}{}", album.album, album.artist),
                        132.,
                        cx,
                    ),
                    Page::Album {
                        album: album.album,
                        artist: album.artist,
                        query: album.query,
                    },
                ),
                _ => return None,
            };
        let resume = playing.is_some();
        let open = page.clone();
        Some(
            div().px_8().child(
                div()
                    .id("home-now")
                    .p_4()
                    .rounded(px(14.))
                    .bg(p.raised.opacity(0.55))
                    .border_1()
                    .border_color(p.line_soft)
                    .flex()
                    .items_center()
                    .gap_5()
                    .cursor_pointer()
                    .hover(|s| s.bg(p.raised.opacity(0.8)))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.navigate(open.clone(), window, cx)
                    }))
                    .child(div().rounded(px(8.)).shadow_lg().child(art))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                faint(label, cx)
                                    .text_color(p.accent)
                                    .font_weight(FontWeight::MEDIUM),
                            )
                            .child(display(title, 28.).truncate())
                            .child(meta(subtitle, cx).text_size(px(13.)).truncate())
                            .child(
                                div().mt_3().flex().gap_2().child(
                                    Button::new("home-play")
                                        .primary()
                                        .icon(icon(if resume && self.playback.playing {
                                            "pause"
                                        } else {
                                            "play"
                                        }))
                                        .label(if resume {
                                            if self.playback.playing {
                                                "Pause"
                                            } else {
                                                "Resume"
                                            }
                                        } else {
                                            "Play album"
                                        })
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            cx.stop_propagation();
                                            if resume {
                                                this.toggle_playback(cx);
                                            } else {
                                                this.play_rule(page.base(), "From Home".into());
                                            }
                                        })),
                                ),
                            ),
                    ),
            ),
        )
    }

    fn shelf_tile(
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
