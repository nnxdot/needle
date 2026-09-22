use super::{
    AppView, BIG_MS, Page, album_page, motion, pal,
    widgets::{artwork, faint, glyph, icon_button, meta, quality},
};
use gpui::{prelude::*, *};
use gpui_component::{Sizable, slider::Slider};
use needle_core::{
    audio::{Command, Repeat},
    model::format_duration,
};

#[derive(Clone, Copy, PartialEq)]
pub enum Side {
    Lyrics,
    Queue,
    Stems,
    None,
}

impl AppView {
    /// Grow the big player out of the cover in the bottom-left corner, or shrink it back.
    /// The player keeps its full size inside a growing clip, so nothing re-flows mid-way.
    pub(super) fn big_reveal(&self, player: AnyElement, w: f32, h: f32, cx: &App) -> AnyElement {
        let closing = !self.big;
        let canvas = pal(cx).canvas;
        // Where the cover sits in the player bar, measured from the body's edges.
        let art = [22., h - 77., w - 78., 21.];
        let edge = move |t: f32, i: usize| art[i] * (1. - if closing { 1. - t } else { t });
        let serial = self.big_serial;
        let inner = motion::animate(
            div().absolute().w(px(w)).h(px(h)).child(player),
            ("big-inner", serial),
            BIG_MS,
            cx,
            move |el, t| el.left(px(-edge(t, 0))).top(px(-edge(t, 1))),
        );
        let clip = div().absolute().overflow_hidden().bg(canvas).child(inner);
        motion::animate(clip, ("big-reveal", serial), BIG_MS, cx, move |el, t| {
            let shown = if closing { 1. - t } else { t };
            el.left(px(edge(t, 0)))
                .top(px(edge(t, 1)))
                .right(px(edge(t, 2)))
                .bottom(px(edge(t, 3)))
                .rounded(px(8. * (1. - shown)))
                .opacity((shown * 1.6).min(1.))
        })
    }

    pub(super) fn big_player(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let p = pal(cx);
        let look = self.now_look();
        let tint = look.as_ref().map(|l| l.vivid);
        let size = window.viewport_size();
        let (w, h) = (f32::from(size.width), f32::from(size.height));
        let side = if w < 1000. { Side::None } else { self.big_side };
        let current = self.playback.current.clone();
        let playing = self.playback.playing;
        let repeat = self.playback.repeat;
        let left_w = if side == Side::None { w } else { w - 440. };
        let art = (h - 440.).min(left_w - 120.).clamp(140., 520.);
        let top = match tint {
            Some(c) if p.dark => hsla(c.h, (c.s * 0.8).min(0.45), 0.2, 1.),
            Some(c) => hsla(c.h, (c.s * 0.7).min(0.4), 0.86, 1.),
            None => p.raised,
        };
        // Fade the background to the new cover's colour instead of snapping.
        if self.big_tint.2 == 0 {
            self.big_tint = (top, top, 1);
        } else if self.big_tint.1 != top {
            self.big_tint = (self.big_tint.1, top, self.big_tint.2 + 1);
        }
        let (from, to, tint_serial) = self.big_tint;
        let canvas = p.canvas;
        let side_button = |id: &'static str,
                           name: &'static str,
                           tip: &'static str,
                           which: Side,
                           cx: &mut Context<Self>| {
            let active = self.big_side == which;
            icon_button(id, name, tip)
                .when(active, |b| b.text_color(p.accent))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.big_side = if this.big_side == which {
                        Side::None
                    } else {
                        which
                    };
                    cx.notify();
                }))
        };
        let backdrop = look.and_then(|l| l.blur).map(|blur| {
            img(blur)
                .absolute()
                .top(px(-h * 0.1))
                .left(px(-w * 0.1))
                .w(px(w * 1.2))
                .h(px(h * 1.2))
                .object_fit(ObjectFit::Cover)
                .opacity(if p.dark { 0.85 } else { 0.55 })
        });
        let player = div()
            .id("big-player")
            .size_full()
            .relative()
            .overflow_hidden()
            .flex()
            .flex_col()
            .children(backdrop)
            // Vignette: darken toward the edges and fade into the page at the bottom, so the
            // controls always sit on a calm surface.
            .child(div().absolute().inset_0().bg(linear_gradient(
                180.,
                linear_color_stop(p.canvas.opacity(0.25), 0.),
                linear_color_stop(p.canvas.opacity(0.8), 1.),
            )))
            .child(div().absolute().inset_0().bg(linear_gradient(
                90.,
                linear_color_stop(p.canvas.opacity(0.45), 0.),
                linear_color_stop(p.canvas.opacity(0.), 0.3),
            )))
            .child(div().absolute().inset_0().bg(linear_gradient(
                270.,
                linear_color_stop(p.canvas.opacity(0.45), 0.),
                linear_color_stop(p.canvas.opacity(0.), 0.3),
            )))
            .child(
                div()
                    .h(px(56.))
                    .px_4()
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        icon_button("close-big", "chevron-down", "Close the player · Esc")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.big = false;
                                cx.notify();
                            })),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_center()
                            .truncate()
                            .text_size(px(12.))
                            .text_color(p.ink_2)
                            .child(
                                current
                                    .as_ref()
                                    .map(|i| i.reason.clone())
                                    .unwrap_or_default(),
                            ),
                    )
                    .child(side_button(
                        "big-lyrics",
                        "lyrics",
                        "Lyrics",
                        Side::Lyrics,
                        cx,
                    ))
                    .child(side_button(
                        "big-queue",
                        "queue",
                        "Up next",
                        Side::Queue,
                        cx,
                    ))
                    .child(side_button("big-stems", "stems", "Stems", Side::Stems, cx))
                    .child(
                        icon_button("big-mini", "mini", "Mini player").on_click(cx.listener(
                            |this, _, window, cx| {
                                this.big = false;
                                this.open_mini(window, cx);
                            },
                        )),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .items_center()
                            .justify_center()
                            .gap_5()
                            .px_12()
                            .pb_8()
                            .child(
                                div()
                                    .rounded(px(10.))
                                    .shadow(vec![BoxShadow {
                                        color: gpui::black().opacity(if p.dark {
                                            0.55
                                        } else {
                                            0.25
                                        }),
                                        offset: point(px(0.), px(18.)),
                                        blur_radius: px(48.),
                                        spread_radius: px(-6.),
                                    }])
                                    .child(artwork(current.as_ref().map(|i| &i.track), art, cx)),
                            )
                            .child(match &current {
                                None => div()
                                    .flex()
                                    .flex_col()
                                    .items_center()
                                    .gap_1()
                                    .child(super::widgets::display("Nothing playing", 28.))
                                    .child(meta("Pick something from your library.", cx)),
                                Some(item) => {
                                    let track = item.track.clone();
                                    let (artist, album) =
                                        (Page::Artist(track.artist.clone()), album_page(&track));
                                    div()
                                        .w(px(art.max(420.)))
                                        .flex()
                                        .flex_col()
                                        .items_center()
                                        .gap_1()
                                        .child(
                                            super::widgets::display(track.title.clone(), 32.)
                                                .max_w_full()
                                                .truncate(),
                                        )
                                        .child(
                                            div()
                                                .flex()
                                                .gap_2()
                                                .text_size(px(15.))
                                                .child(
                                                    div()
                                                        .id("big-artist")
                                                        .cursor_pointer()
                                                        .hover(|s| s.underline())
                                                        .child(track.display_artist().to_string())
                                                        .on_click(cx.listener(
                                                            move |this, _, window, cx| {
                                                                this.big = false;
                                                                this.navigate(
                                                                    artist.clone(),
                                                                    window,
                                                                    cx,
                                                                );
                                                            },
                                                        )),
                                                )
                                                .child(div().text_color(p.ink_3).child("·"))
                                                .child(
                                                    div()
                                                        .id("big-album")
                                                        .text_color(p.ink_2)
                                                        .cursor_pointer()
                                                        .hover(|s| s.underline())
                                                        .child(track.display_album().to_string())
                                                        .on_click(cx.listener(
                                                            move |this, _, window, cx| {
                                                                this.big = false;
                                                                this.navigate(
                                                                    album.clone(),
                                                                    window,
                                                                    cx,
                                                                );
                                                            },
                                                        )),
                                                ),
                                        )
                                        .child(faint(quality(&track), cx))
                                }
                            })
                            .child(
                                div()
                                    .w(px(art.max(420.)))
                                    .flex()
                                    .items_center()
                                    .gap_3()
                                    .child(
                                        faint(format_duration(self.playback.position), cx)
                                            .w(px(40.))
                                            .text_right(),
                                    )
                                    .child(
                                        Slider::new(&self.seek)
                                            .flex_1()
                                            .disabled(current.is_none()),
                                    )
                                    .child(
                                        faint(
                                            current
                                                .as_ref()
                                                .map(|i| format_duration(i.track.duration))
                                                .unwrap_or_else(|| "0:00".into()),
                                            cx,
                                        )
                                        .w(px(40.)),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_4()
                                    .child(
                                        icon_button("big-shuffle", "shuffle", "Shuffle this view")
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.play_view(0, true, cx)
                                            })),
                                    )
                                    .child(
                                        icon_button("big-previous", "previous", "Previous")
                                            .large()
                                            .on_click(cx.listener(|this, _, _, _| {
                                                this.player.send(Command::Previous)
                                            })),
                                    )
                                    .child(
                                        div()
                                            .id("big-play")
                                            .size(px(60.))
                                            .rounded_full()
                                            .bg(p.ink)
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .cursor_pointer()
                                            .hover(|s| s.opacity(0.88))
                                            .active(|s| s.size(px(55.)).m(px(2.5)).opacity(0.8))
                                            .child(
                                                glyph(if playing { "pause" } else { "play" })
                                                    .size(px(26.))
                                                    .text_color(p.canvas),
                                            )
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.toggle_playback(cx)
                                            })),
                                    )
                                    .child(
                                        icon_button("big-next", "next", "Next").large().on_click(
                                            cx.listener(|this, _, _, _| {
                                                this.player.send(Command::Next)
                                            }),
                                        ),
                                    )
                                    .child(
                                        icon_button(
                                            "big-repeat",
                                            if repeat == Repeat::One {
                                                "repeat-one"
                                            } else {
                                                "repeat"
                                            },
                                            "Repeat",
                                        )
                                        .when(repeat != Repeat::Off, |b| b.text_color(p.accent))
                                        .on_click(
                                            cx.listener(|this, _, _, _| {
                                                let next = match this.playback.repeat {
                                                    Repeat::Off => Repeat::All,
                                                    Repeat::All => Repeat::One,
                                                    Repeat::One => Repeat::Off,
                                                };
                                                this.player.send(Command::Repeat(next));
                                            }),
                                        ),
                                    ),
                            )
                            .child(
                                div()
                                    .w(px(220.))
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(glyph("volume-low").size(px(16.)).text_color(p.ink_2))
                                    .child(
                                        Slider::new(&self.volume)
                                            .flex_1()
                                            .disabled(self.playback.exclusive),
                                    )
                                    .child(glyph("volume").size(px(16.)).text_color(p.ink_2)),
                            ),
                    )
                    .when(side != Side::None, |el| {
                        el.child(
                            div()
                                .w(px(420.))
                                .flex_shrink_0()
                                .h_full()
                                .pr_6()
                                .pb_6()
                                .flex()
                                .flex_col()
                                .child(match side {
                                    Side::Lyrics => self.lyrics_view(true, cx).into_any_element(),
                                    Side::Stems => div()
                                        .pt_4()
                                        .child(self.stems_view(
                                            current.as_ref().map(|c| c.track.clone()),
                                            true,
                                            cx,
                                        ))
                                        .into_any_element(),
                                    _ => self.up_next(cx).into_any_element(),
                                }),
                        )
                    }),
            );
        motion::animate(player, ("big-tint", tint_serial), 900, cx, move |el, t| {
            el.bg(linear_gradient(
                180.,
                linear_color_stop(motion::mix(from, to, t), 0.),
                linear_color_stop(canvas, 0.85),
            ))
        })
    }

    /// The upcoming queue as a simple list, shared by the big and mini players.
    pub(super) fn up_next(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let count = self.playback.queue.len();
        div()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .gap_2()
            .child(faint(
                if count == 0 {
                    "Nothing queued".to_string()
                } else {
                    format!("Up next · {count}")
                },
                cx,
            ))
            .child(
                uniform_list(
                    "big-queue-list",
                    count,
                    cx.processor(|this, range: std::ops::Range<usize>, _, cx| {
                        let p = pal(cx);
                        range
                            .map(|index| {
                                let item = &this.playback.queue[index];
                                div()
                                    .id(("big-queue-row", index))
                                    .h(px(48.))
                                    .w_full()
                                    .px_2()
                                    .rounded(px(6.))
                                    .flex()
                                    .items_center()
                                    .gap_3()
                                    .cursor_pointer()
                                    .hover(|s| s.bg(p.ink.opacity(0.06)))
                                    .child(artwork(Some(&item.track), 34., cx))
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w_0()
                                            .child(
                                                div()
                                                    .text_size(px(13.))
                                                    .truncate()
                                                    .child(item.track.title.clone()),
                                            )
                                            .child(
                                                meta(item.track.display_artist().to_string(), cx)
                                                    .truncate(),
                                            ),
                                    )
                                    .on_click(cx.listener(move |this, event: &ClickEvent, _, _| {
                                        if event.click_count() == 2 {
                                            this.player.send(Command::Jump(index));
                                        }
                                    }))
                            })
                            .collect::<Vec<_>>()
                    }),
                )
                .flex_1(),
            )
    }
}
