//! Needle 2.0's player, after the Mac's: one rounded bar floating over the foot of the page,
//! with the transport at its left, a small screen in its middle (the cover, the song, "artist —
//! album", and where it is), and the panels' buttons and the volume at its right. Which
//! buttons show, and in what order, is the listener's choice (Settings › Appearance ›
//! Player buttons); the rest wait in the ••• menu at the bar's end.
use super::{
    AppView, Page, Panel,
    menus::Entry,
    motion, pal,
    player_tools::Tool,
    widgets::{artwork, glyph, icon, icon_button},
};
use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme, Disableable, Selectable, Sizable,
    button::{Button, ButtonVariants},
};
use needle_core::{audio::Command, audio::Repeat};

/// The bar's height and its widest.
pub(super) const HEIGHT: f32 = 58.;
pub(super) const MAX_WIDTH: f32 = 860.;
/// The buttons that sit left of the screen, with the transport.
const TRANSPORT: [&str; 5] = ["shuffle", "previous", "play", "next", "repeat"];
const ICON: f32 = 26.;
/// The buttons out of the box: the rest are in the ••• menu.
const DEFAULT: [&str; 7] = [
    "previous", "play", "next", "favorite", "lyrics", "queue", "volume",
];

/// The buttons chosen, in order. A list never changed (the core's whole list, which every
/// Needle before 2.0 kept) is the short one out of the box.
fn chosen_buttons(settings: &needle_core::model::Settings) -> Vec<&'static str> {
    let list = settings.player_buttons();
    if list == needle_core::model::PLAYER_BUTTONS {
        DEFAULT.to_vec()
    } else {
        list
    }
}

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
        let narrow = width < 640.;
        let buttons = self.bar_buttons(narrow);
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
                    .when(buttons.len() < BUTTONS.len(), |el| {
                        el.child(self.player_more(narrow, cx))
                    }),
            )
    }
    /// The buttons in the bar now: the chosen ones, without the volume in a narrow window.
    fn bar_buttons(&self, narrow: bool) -> Vec<&'static str> {
        chosen_buttons(&self.settings)
            .into_iter()
            .filter(|b| !(narrow && *b == "volume"))
            .collect()
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
            "favorite" => self.favorite_button(current?, cx).into_any_element(),
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
            "volume" => self.volume_control(64., cx).into_any_element(),
            "sleep" => self.sleep_button(cx),
            "speed" => self.speed_button(cx),
            "loop" => self.loop_button(cx).into_any_element(),
            "sound" => self.sound_button(cx).into_any_element(),
            "mini" => self.mini_button(cx).into_any_element(),
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
    /// ••• : the buttons not in the bar, as a menu.
    fn player_more(&self, narrow: bool, cx: &mut Context<Self>) -> Button {
        icon_button("player-more", "more", "More")
            .with_size(px(ICON))
            .selected(self.player_menu.is_some())
            .on_click(cx.listener(move |this, _, window, cx| {
                if this.player_menu.take().is_none() {
                    this.menu = None;
                    this.menu_serial += 1;
                    this.player_menu = Some((window.mouse_position(), this.menu_serial, narrow));
                }
                cx.notify();
            }))
    }

    /// The ••• menu's entries: each button not in the bar, as an item that does what it does.
    fn player_menu_entries(&self, position: Point<Pixels>, narrow: bool) -> Vec<Entry> {
        let shown = self.bar_buttons(narrow);
        let playing = self.playback.current.is_some();
        let mut entries = vec![];
        for &(name, glyph_name, label) in BUTTONS.iter().filter(|b| !shown.contains(&b.0)) {
            let entry = match name {
                "shuffle" => Entry::item(glyph_name, "Shuffle this view", None, |this, _, cx| {
                    this.play_view(0, true, cx)
                }),
                "previous" => Entry::item(glyph_name, label, Some("Ctrl+Left"), |this, _, _| {
                    this.player.send(Command::Previous)
                }),
                "next" => Entry::item(glyph_name, label, Some("Ctrl+Right"), |this, _, _| {
                    this.player.send(Command::Next)
                }),
                "repeat" => {
                    let repeat = self.playback.repeat;
                    Entry::item(
                        if repeat == Repeat::One {
                            "repeat-one"
                        } else {
                            "repeat"
                        },
                        match repeat {
                            Repeat::Off => "Repeat",
                            Repeat::All => "Repeating the queue",
                            Repeat::One => "Repeating this song",
                        },
                        None,
                        move |this, _, _| {
                            this.player.send(Command::Repeat(match repeat {
                                Repeat::Off => Repeat::All,
                                Repeat::All => Repeat::One,
                                Repeat::One => Repeat::Off,
                            }))
                        },
                    )
                }
                "favorite" if playing => {
                    let favorite = self
                        .playback
                        .current
                        .as_ref()
                        .is_some_and(|i| i.track.rating >= 4);
                    Entry::item(
                        if favorite { "heart-fill" } else { "heart" },
                        if favorite {
                            "Remove from favorites"
                        } else {
                            "Add to favorites"
                        },
                        None,
                        |this, _, _| this.toggle_favorite_playing(),
                    )
                }
                "lyrics" => Entry::item(glyph_name, label, Some("Ctrl+L"), |this, _, _| {
                    this.toggle_panel(Panel::Lyrics)
                }),
                "queue" => Entry::item(glyph_name, label, Some("Ctrl+J"), |this, _, _| {
                    this.toggle_panel(Panel::Queue)
                }),
                "speakers" => Entry::item(
                    glyph_name,
                    match self.current_speaker() {
                        Some(s) => format!("Playing on {}", s.name),
                        None => "Play on another speaker".into(),
                    },
                    None,
                    move |this, _, cx| this.open_speaker_menu(position, cx),
                ),
                "volume" if !self.playback.exclusive => {
                    let muted = self.muted_volume.is_some();
                    Entry::item(
                        if muted { "volume" } else { "volume-off" },
                        if muted { "Unmute" } else { "Mute" },
                        None,
                        |this, window, cx| this.toggle_mute(window, cx),
                    )
                }
                "sleep" => Entry::item(
                    glyph_name,
                    match self.playback.sleep {
                        Some(s) => format!(
                            "Sleep timer · {} left",
                            super::player_tools::clock(s.remaining)
                        ),
                        None => label.into(),
                    },
                    None,
                    move |this, _, cx| this.open_tool(Tool::Sleep, position, cx),
                ),
                "speed" => Entry::item(glyph_name, label, None, move |this, _, cx| {
                    this.open_tool(Tool::Speed, position, cx)
                }),
                "loop" if playing => Entry::item(
                    glyph_name,
                    if self.playback.loop_range.is_some() {
                        "Clear the A–B loop"
                    } else if self.loop_start.is_some() {
                        "Set point B"
                    } else {
                        "A–B loop: set point A"
                    },
                    None,
                    |this, _, _| this.step_loop(),
                ),
                "sound" => Entry::item(glyph_name, label, None, |this, window, cx| {
                    this.navigate(Page::Sound, window, cx)
                }),
                "mini" => Entry::item(glyph_name, label, Some("Ctrl+M"), |this, window, cx| {
                    this.open_mini(window, cx)
                }),
                _ => continue,
            };
            entries.push(entry);
        }
        entries
    }

    pub(super) fn player_menu_view(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        let (position, serial, narrow) = self.player_menu?;
        let p = pal(cx);
        let entries = self.player_menu_entries(position, narrow);
        let rows = Self::menu_rows(&entries, None, cx);
        let body = div()
            .id("player-menu")
            .occlude()
            .w(px(240.))
            .p_1()
            .map(|el| super::glass::pane(el, 9., 14., cx.theme().popover, cx))
            .border_1()
            .border_color(p.line)
            .shadow_lg()
            .flex()
            .flex_col()
            .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                this.player_menu = None;
                cx.notify();
            }))
            .children(rows);
        let body = motion::animate(body, ("player-menu-in", serial), 140, cx, |el, t| {
            el.opacity(t).mb(px(6. * (1. - t)))
        });
        Some(
            deferred(
                anchored()
                    .anchor(Corner::BottomRight)
                    .position(position - point(px(0.), px(14.)))
                    .snap_to_window_with_margin(px(8.))
                    .child(body),
            )
            .with_priority(2),
        )
    }
}

/// The signal path's first part, for the chip: "FLAC 24/96 → 48 kHz (resampled)" → "FLAC 24/96".
fn short_path(path: &str) -> String {
    path.split(" → ").next().unwrap_or(path).trim().to_string()
}

/// A button being dragged in the Player buttons editor (Settings › Appearance › Layout).
#[derive(Clone)]
pub(super) struct DraggedButton(pub &'static str);

struct ButtonGhost(&'static str);

impl Render for ButtonGhost {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = pal(cx);
        div()
            .size(px(36.))
            .rounded(px(10.))
            .bg(p.raised)
            .border_1()
            .border_color(p.accent.opacity(0.6))
            .shadow_lg()
            .flex()
            .items_center()
            .justify_center()
            .child(glyph(button_glyph(self.0)).size(px(18.)).text_color(p.ink))
    }
}

/// Every button the player can show: its settings name, icon, and name for people.
const BUTTONS: [(&str, &str, &str); 15] = [
    ("shuffle", "shuffle", "Shuffle"),
    ("previous", "previous", "Previous"),
    ("play", "play", "Play and pause"),
    ("next", "next", "Next"),
    ("repeat", "repeat", "Repeat"),
    ("favorite", "heart", "Favorite"),
    ("lyrics", "lyrics", "Lyrics"),
    ("queue", "queue", "Queue"),
    ("speakers", "cast", "Speakers"),
    ("volume", "volume", "Volume"),
    ("sleep", "moon", "Sleep timer"),
    ("speed", "speed", "Speed and pitch"),
    ("loop", "loop", "A–B loop"),
    ("sound", "eq", "Equalizer and sound tools"),
    ("mini", "mini", "Mini player"),
];

fn button_glyph(name: &str) -> &'static str {
    BUTTONS
        .iter()
        .find(|b| b.0 == name)
        .map_or("blank", |b| b.1)
}

fn button_label(name: &str) -> &'static str {
    BUTTONS.iter().find(|b| b.0 == name).map_or("", |b| b.2)
}

/// `list` with `name` moved (or added) just before `before`, or at the end without one.
fn place(list: &[&'static str], name: &'static str, before: Option<&'static str>) -> Vec<String> {
    let mut out: Vec<&'static str> = list.iter().copied().filter(|b| *b != name).collect();
    let at = before
        .and_then(|b| out.iter().position(|x| *x == b))
        .unwrap_or(out.len());
    out.insert(at, name);
    out.into_iter().map(String::from).collect()
}

impl AppView {
    /// Settings › Appearance › Layout › Player buttons: a small copy of the player bar to drag
    /// buttons in, along, and out of, with the unused ones below it.
    pub(super) fn player_buttons_row(&self, cx: &mut Context<Self>) -> Div {
        let chosen = chosen_buttons(&self.settings);
        let count = chosen.len();
        self.expander(
            "player-buttons",
            "Player buttons",
            "Which buttons the player shows, in which order. The rest are in its ••• menu.",
            Some(
                div()
                    .text_size(px(12.5))
                    .text_color(pal(cx).ink_2)
                    .child(format!("{count} buttons"))
                    .into_any_element(),
            ),
            move |cx| {
                let p = pal(cx);
                let unused: Vec<&'static str> = BUTTONS
                    .iter()
                    .map(|b| b.0)
                    .filter(|b| !chosen.contains(b))
                    .collect();
                let chip = |name: &'static str, in_bar: bool, cx: &mut Context<AppView>| {
                    let list = chosen.clone();
                    div()
                        .id(SharedString::from(format!(
                            "button-{}-{name}",
                            if in_bar { "bar" } else { "spare" }
                        )))
                        .size(px(36.))
                        .rounded(px(10.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .cursor_grab()
                        .bg(if in_bar {
                            gpui::transparent_black()
                        } else {
                            p.raised
                        })
                        .hover(|s| s.bg(p.raised_hover))
                        .drag_over::<DraggedButton>(move |s, _, _, _| {
                            s.border_l_2().border_color(p.accent)
                        })
                        .child(glyph(button_glyph(name)).size(px(18.)).text_color(p.ink))
                        .tooltip(move |window, cx| {
                            gpui_component::tooltip::Tooltip::new(button_label(name))
                                .build(window, cx)
                        })
                        .on_drag(DraggedButton(name), |drag: &DraggedButton, _, _, cx| {
                            cx.new(|_| ButtonGhost(drag.0))
                        })
                        .when(in_bar, |el| {
                            el.on_drop(cx.listener(move |this, dragged: &DraggedButton, _, cx| {
                                this.settings.player_buttons = place(&list, dragged.0, Some(name));
                                this.persist_settings();
                                cx.notify();
                            }))
                        })
                };
                let bar_list = chosen.clone();
                let spare_list = chosen.clone();
                div()
                    .px(px(14.))
                    .pb(px(14.))
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(
                        div()
                            .id("player-buttons-bar")
                            .h(px(52.))
                            .px(px(10.))
                            .map(|el| frosted(el, 26., cx))
                            .flex()
                            .items_center()
                            .gap(px(4.))
                            .drag_over::<DraggedButton>(move |s, _, _, _| s.border_color(p.accent))
                            .on_drop(cx.listener(move |this, dragged: &DraggedButton, _, cx| {
                                this.settings.player_buttons = place(&bar_list, dragged.0, None);
                                this.persist_settings();
                                cx.notify();
                            }))
                            .children(
                                chosen
                                    .iter()
                                    .filter(|b| TRANSPORT.contains(b))
                                    .map(|b| chip(b, true, cx)),
                            )
                            // Where the song shows.
                            .child(
                                div()
                                    .flex_1()
                                    .mx_2()
                                    .h(px(32.))
                                    .rounded(px(8.))
                                    .bg(if p.dark {
                                        hsla(0., 0., 1., 0.045)
                                    } else {
                                        hsla(0., 0., 0., 0.035)
                                    })
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .text_size(px(11.5))
                                    .text_color(p.ink_3)
                                    .child("The song"),
                            )
                            .children(
                                chosen
                                    .iter()
                                    .filter(|b| !TRANSPORT.contains(b))
                                    .map(|b| chip(b, true, cx)),
                            )
                            .when(count < BUTTONS.len(), |el| {
                                el.child(
                                    div()
                                        .size(px(36.))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .child(glyph("more").size(px(18.)).text_color(p.ink_3)),
                                )
                            }),
                    )
                    .child(
                        div()
                            .id("player-buttons-spare")
                            .min_h(px(52.))
                            .p(px(8.))
                            .rounded(px(12.))
                            .border_1()
                            .border_dashed()
                            .border_color(p.line)
                            .flex()
                            .flex_wrap()
                            .items_center()
                            .gap(px(6.))
                            .drag_over::<DraggedButton>(move |s, _, _, _| {
                                s.border_color(p.accent).bg(p.accent_soft)
                            })
                            .on_drop(cx.listener(move |this, dragged: &DraggedButton, _, cx| {
                                // Play and pause always stays.
                                if dragged.0 == "play" {
                                    return;
                                }
                                this.settings.player_buttons = spare_list
                                    .iter()
                                    .filter(|b| **b != dragged.0)
                                    .map(|b| b.to_string())
                                    .collect();
                                this.persist_settings();
                                cx.notify();
                            }))
                            .children(unused.iter().map(|b| chip(b, false, cx)))
                            .when(unused.is_empty(), |el| {
                                el.child(
                                    div()
                                        .text_size(px(12.))
                                        .text_color(p.ink_3)
                                        .child("Drag a button here to take it out of the player."),
                                )
                            }),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .text_color(p.ink_3)
                                    .child("Drag to move a button, add one, or take one out. The ones below are in the ••• menu."),
                            )
                            .child(
                                super::widgets::small_button(
                                    "player-buttons-reset",
                                    "Put back Needle's buttons",
                                )
                                .on_click(cx.listener(
                                    |this, _, _, cx| {
                                        this.settings.player_buttons =
                                            DEFAULT.iter().map(|b| b.to_string()).collect();
                                        this.persist_settings();
                                        cx.notify();
                                    },
                                )),
                            ),
                    )
                    .into_any_element()
            },
            cx,
        )
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn buttons_move_and_join() {
        let list = ["shuffle", "play", "next"];
        assert_eq!(
            super::place(&list, "next", Some("shuffle")),
            ["next", "shuffle", "play"]
        );
        assert_eq!(
            super::place(&list, "lyrics", None),
            ["shuffle", "play", "next", "lyrics"]
        );
        assert_eq!(
            super::place(&list, "shuffle", Some("next")),
            ["play", "shuffle", "next"]
        );
    }

    #[test]
    fn an_unchanged_list_is_the_short_one() {
        let mut settings = needle_core::model::Settings::default();
        assert_eq!(super::chosen_buttons(&settings), super::DEFAULT);
        settings.player_buttons = vec!["play".into(), "sleep".into(), "mini".into()];
        assert_eq!(super::chosen_buttons(&settings), ["play", "sleep", "mini"]);
        // Every button can be named in the editor.
        for name in needle_core::model::PLAYER_BUTTONS
            .iter()
            .chain(needle_core::model::MORE_BUTTONS.iter())
        {
            assert!(super::BUTTONS.iter().any(|b| b.0 == *name), "{name}");
        }
    }

    #[test]
    fn the_chip_shows_the_file_part() {
        assert_eq!(
            super::short_path("FLAC 24/96 → 48 kHz (resampled)"),
            "FLAC 24/96"
        );
        assert_eq!(super::short_path("MP3 320"), "MP3 320");
    }
}
