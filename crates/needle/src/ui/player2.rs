//! Needle 2.0's player, after the Mac's: one rounded bar floating over the foot of the page,
//! with the transport at its left, a small screen in its middle (the cover, the song, "artist —
//! album", and where it is), and the panels' buttons and the volume at its right. Which
//! buttons show, and in what order, is the listener's choice (Settings › Appearance ›
//! Player buttons).
use super::{
    AppView, Page, Panel, pal,
    widgets::{artwork, glyph, icon, icon_button},
};
use gpui::{prelude::*, *};
use gpui_component::{
    Disableable, Selectable, Sizable,
    button::{Button, ButtonVariants},
};
use needle_core::{audio::Command, audio::Repeat};

/// The bar's height and its widest.
pub(super) const HEIGHT: f32 = 58.;
pub(super) const MAX_WIDTH: f32 = 860.;
/// The buttons that sit left of the screen, with the transport.
const TRANSPORT: [&str; 5] = ["shuffle", "previous", "play", "next", "repeat"];
const ICON: f32 = 26.;

/// A floating pane's fill on Windows and Linux, where nothing blurs behind it: the look's
/// raised colour, nearly solid, with a thin line and a soft shadow.
pub(super) fn frosted<E: Styled>(el: E, radius: f32, cx: &App) -> E {
    let p = pal(cx);
    el.rounded(px(radius))
        .bg(p.raised.opacity(0.95))
        .border_1()
        .border_color(if p.dark {
            hsla(0., 0., 1., 0.08)
        } else {
            hsla(0., 0., 0., 0.08)
        })
        .shadow(vec![BoxShadow {
            color: hsla(0., 0., 0., if p.dark { 0.45 } else { 0.18 }),
            offset: point(px(0.), px(10.)),
            blur_radius: px(30.),
            spread_radius: px(0.),
        }])
}

impl AppView {
    /// The floating player, `width` wide at most.
    pub(super) fn floating_player(&self, width: f32, cx: &mut Context<Self>) -> Stateful<Div> {
        let width = width.min(MAX_WIDTH);
        let buttons = self.settings.player_buttons();
        let narrow = width < 640.;
        let (left, right): (Vec<&str>, Vec<&str>) =
            buttons.iter().partition(|b| TRANSPORT.contains(b));
        div()
            .id("floating-player")
            .occlude()
            .w(px(width))
            .h(px(HEIGHT))
            .flex_shrink_0()
            .map(|el| frosted(el, HEIGHT / 2., cx))
            .flex()
            .items_center()
            .gap(px(if narrow { 10. } else { 16. }))
            .pl(px(if narrow { 14. } else { 20. }))
            .pr(px(if narrow { 12. } else { 18. }))
            .child(
                div()
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .gap(px(if narrow { 6. } else { 10. }))
                    .children(left.iter().filter_map(|b| self.player_button(b, cx))),
            )
            .child(self.player_screen(narrow, cx))
            .child(
                div()
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .gap(px(4.))
                    .children(right.iter().filter_map(|b| self.player_button(b, cx)))
                    .when(!narrow, |el| el.child(self.player_volume(64., cx)))
                    .child(self.player_more(cx)),
            )
    }
    /// One of the player's buttons by its settings name.
    fn player_button(&self, name: &str, cx: &mut Context<Self>) -> Option<AnyElement> {
        let p = pal(cx);
        let current = self.playback.current.as_ref().map(|i| i.track.clone());
        let element = match name {
            "shuffle" => icon_button("shuffle", "shuffle", "Shuffle this view")
                .small()
                .disabled(self.tracks.is_empty())
                .on_click(cx.listener(|this, _, _, cx| this.play_view(0, true, cx)))
                .into_any_element(),
            "previous" => icon_button("previous", "previous", "Previous · Ctrl+Left")
                .on_click(cx.listener(|this, _, _, _| this.player.send(Command::Previous)))
                .into_any_element(),
            "next" => icon_button("next", "next", "Next · Ctrl+Right")
                .on_click(cx.listener(|this, _, _, _| this.player.send(Command::Next)))
                .into_any_element(),
            "play" => {
                let playing = self.playback.playing;
                div()
                    .id("play-pause")
                    .size(px(34.))
                    .rounded_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .hover(|s| s.bg(p.raised_hover))
                    .child(
                        glyph(if playing { "pause" } else { "play" })
                            .size(px(22.))
                            .text_color(p.ink),
                    )
                    .tooltip(move |window, cx| {
                        gpui_component::tooltip::Tooltip::new(if playing {
                            "Pause · Space"
                        } else {
                            "Play · Space"
                        })
                        .build(window, cx)
                    })
                    .on_click(cx.listener(|this, _, _, cx| this.toggle_playback(cx)))
                    .into_any_element()
            }
            "repeat" => {
                let repeat = self.playback.repeat;
                icon_button(
                    "repeat",
                    if repeat == Repeat::One {
                        "repeat-one"
                    } else {
                        "repeat"
                    },
                    match repeat {
                        Repeat::Off => "Repeat is off",
                        Repeat::All => "Repeating the queue",
                        Repeat::One => "Repeating this track",
                    },
                )
                .small()
                .when(repeat != Repeat::Off, |b| b.text_color(p.accent))
                .on_click(cx.listener(|this, _, _, _| {
                    let next = match this.playback.repeat {
                        Repeat::Off => Repeat::All,
                        Repeat::All => Repeat::One,
                        Repeat::One => Repeat::Off,
                    };
                    this.player.send(Command::Repeat(next));
                }))
                .into_any_element()
            }
            "favorite" => match current {
                Some(track) => self.favorite_button(track, cx).into_any_element(),
                None => return None,
            },
            "lyrics" => Button::new("lyrics-toggle")
                .ghost()
                .with_size(px(ICON))
                .icon(icon("lyrics"))
                .selected(self.settings.show_inspector && self.panel == Panel::Lyrics)
                .tooltip(super::widgets::keys("Lyrics · Ctrl+L"))
                .on_click(cx.listener(|this, _, _, cx| {
                    this.toggle_panel(Panel::Lyrics);
                    cx.notify();
                }))
                .into_any_element(),
            "queue" => self.queue_button(cx).into_any_element(),
            "speakers" => self.speakers_button(cx).into_any_element(),
            _ => return None,
        };
        Some(element)
    }
    /// The small screen in the bar's middle: the cover (it opens the big player), the song and
    /// "artist — album" (they open the album and the artist), and where the song is, along its
    /// foot, with the signal path at its right.
    fn player_screen(&self, narrow: bool, cx: &mut Context<Self>) -> Div {
        let p = pal(cx);
        let current = self.playback.current.as_ref().map(|i| i.track.clone());
        let (path, path_detail, exclusive) = self.signal_path(cx);
        div()
            .flex_1()
            .min_w_0()
            .h(px(44.))
            .rounded(px(10.))
            .bg(if p.dark {
                hsla(0., 0., 1., 0.045)
            } else {
                hsla(0., 0., 0., 0.035)
            })
            .border_1()
            .border_color(if p.dark {
                hsla(0., 0., 1., 0.05)
            } else {
                hsla(0., 0., 0., 0.05)
            })
            .flex()
            .items_center()
            .gap(px(10.))
            .pl(px(4.))
            .pr(px(10.))
            .overflow_hidden()
            .child(
                div()
                    .id("open-big")
                    .flex_shrink_0()
                    .cursor_pointer()
                    .hover(|s| s.opacity(0.85))
                    .when_some(current.clone(), |el, track| {
                        el.on_mouse_down(
                            MouseButton::Right,
                            cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                                this.open_playing_menu(track.clone(), event.position, cx)
                            }),
                        )
                    })
                    .child(artwork(current.as_ref(), 36., cx))
                    .tooltip(|window, cx| {
                        gpui_component::tooltip::Tooltip::new(super::widgets::keys(
                            "Open the big player · Ctrl+P",
                        ))
                        .build(window, cx)
                    })
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.big = true;
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .justify_center()
                    .child(match &current {
                        None => div()
                            .text_size(px(12.5))
                            .text_color(p.ink_2)
                            .text_center()
                            .child("Nothing playing")
                            .into_any_element(),
                        Some(track) => {
                            let album = super::album_page(track);
                            let artist = Page::Artist(track.artist.clone());
                            let second = if track.album.is_empty() {
                                track.display_artist().to_string()
                            } else {
                                format!("{} — {}", track.display_artist(), track.album)
                            };
                            div()
                                .flex()
                                .flex_col()
                                .items_center()
                                .min_w_0()
                                .child(
                                    div()
                                        .id("now-title")
                                        .max_w_full()
                                        .text_size(px(12.5))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .truncate()
                                        .cursor_pointer()
                                        .hover(|s| s.underline())
                                        .child(track.title.clone())
                                        .on_click(cx.listener(move |this, _, window, cx| {
                                            this.navigate(album.clone(), window, cx)
                                        })),
                                )
                                .child(
                                    div()
                                        .id("now-artist")
                                        .max_w_full()
                                        .text_size(px(11.5))
                                        .text_color(p.ink_2)
                                        .truncate()
                                        .cursor_pointer()
                                        .hover(|s| s.underline().text_color(p.ink))
                                        .child(second)
                                        .on_click(cx.listener(move |this, _, window, cx| {
                                            this.navigate(artist.clone(), window, cx)
                                        })),
                                )
                                .into_any_element()
                        }
                    })
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .h(px(10.))
                            .px(px(8.))
                            .child(self.thin_seek_bar(current.is_none(), cx)),
                    ),
            )
            .when(!narrow && !path.is_empty(), |el| {
                el.child(
                    div()
                        .id("signal-path")
                        .flex_shrink_0()
                        .px(px(6.))
                        .rounded(px(6.))
                        .border_1()
                        .border_color(if exclusive { p.accent } else { p.line })
                        .text_size(px(10.5))
                        .text_color(if exclusive { p.accent } else { p.ink_3 })
                        .cursor_pointer()
                        .child(short_path(&path))
                        .tooltip(move |window, cx| {
                            gpui_component::tooltip::Tooltip::new(path_detail.clone())
                                .build(window, cx)
                        })
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.navigate(Page::Settings, window, cx)
                        })),
                )
            })
    }
    /// Mute and a short volume slider.
    fn player_volume(&self, slider: f32, cx: &mut Context<Self>) -> Stateful<Div> {
        self.volume_control(slider, cx)
    }
    /// The rest, in one button: the A–B loop, the equalizer, and the mini player.
    fn player_more(&self, cx: &mut Context<Self>) -> Div {
        div()
            .flex()
            .items_center()
            .gap(px(2.))
            .child(self.loop_button(cx))
            .child(self.sound_button(cx))
            .child(self.mini_button(cx))
    }
}

/// The signal path's first part, for the chip: "FLAC 24/96 → 48 kHz (resampled)" → "FLAC 24/96".
fn short_path(path: &str) -> String {
    path.split(" → ").next().unwrap_or(path).trim().to_string()
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_chip_shows_the_file_part() {
        assert_eq!(
            super::short_path("FLAC 24/96 → 48 kHz (resampled)"),
            "FLAC 24/96"
        );
        assert_eq!(super::short_path("MP3 320"), "MP3 320");
    }
}
