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

/// The immersive cover, as the Mac's and Apple Music's: the cover fills the window's left side
/// from top to foot and fades, to its right and at its foot, into a tone of its own colour.
/// `foot` is how tall the words and controls over its foot are: the tone is solid behind all of
/// them, so no cover makes them hard to read, and the cover fades in above.
/// With a pane beside it (`alone` false), the cover is the window's left side, at most `widest`
/// wide. Alone, it spans the window, and grows and shrinks with it; in a window much wider
/// than tall it stops at 1.4 times the height, in the middle, fading into the tone each side.
/// Gives the layer, how wide it is (the room for the words and controls over its foot), and
/// the tone (which the rest of the player takes).
#[allow(clippy::too_many_arguments)]
pub(super) fn immersive_cover(
    path: &str,
    moving: bool,
    look: Option<&super::ambient::Look>,
    w: f32,
    h: f32,
    widest: f32,
    foot: f32,
    dark: bool,
    alone: bool,
) -> (AnyElement, f32, Hsla) {
    let (side, cover_w) = if alone {
        (w.max(1.), w.min(h * 1.4).max(1.))
    } else {
        let side = h.min(w * 0.62).min(widest).max(1.);
        (side, side)
    };
    let left = (side - cover_w) / 2.;
    let fade = cover_w * 0.25;
    let tone = match look.map(|l| l.vivid) {
        Some(c) if dark => hsla(c.h, (c.s * 0.8).min(0.5), 0.15, 1.),
        Some(c) => hsla(c.h, (c.s * 0.7).min(0.45), 0.88, 1.),
        None if dark => hsla(0., 0., 0.1, 1.),
        None => hsla(0., 0., 0.92, 1.),
    };
    // A square cover fills its box, so it is drawn as large as the box's longer edge. A
    // moving picture is drawn as it is (a smaller copy would be a still one).
    // A moving cover as prepared to fit in memory (see `moving`); until it is, or for a
    // still cover, a copy of the picture at the size it is drawn.
    let file: ImageSource = match moving
        .then(|| super::moving::image(std::path::Path::new(path)))
        .flatten()
    {
        Some(frames) => frames.into(),
        None => super::thumbs::for_size(path, cover_w.max(h))
            .unwrap_or_else(|| path.into())
            .into(),
    };
    let edge_fade = |at: f32, into_tone: bool| {
        div()
            .absolute()
            .top_0()
            .bottom_0()
            .left(px(at))
            .w(px(fade))
            .bg(linear_gradient(
                90.,
                linear_color_stop(tone.opacity(if into_tone { 0. } else { 1. }), 0.),
                linear_color_stop(tone.opacity(if into_tone { 1. } else { 0. }), 1.),
            ))
    };
    let layer = div()
        .absolute()
        .left_0()
        .top_0()
        .w(px(side))
        .h(px(h))
        .overflow_hidden()
        .bg(tone)
        .child(
            img(file)
                // GPUI steps through a moving picture's frames only for an image with an id.
                .id("immersive-cover")
                .absolute()
                .top_0()
                .left(px(left))
                .w(px(cover_w))
                .h(px(h))
                .object_fit(ObjectFit::Cover),
        )
        // Beside a pane: into the tone to its right. Alone and narrower than the window: into
        // the tone on both sides.
        .when(!alone, |el| {
            el.child(
                div()
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .right_0()
                    .w(px(side * 0.45))
                    .bg(linear_gradient(
                        90.,
                        linear_color_stop(tone.opacity(0.), 0.),
                        linear_color_stop(tone, 1.),
                    )),
            )
        })
        .when(alone && cover_w < side, |el| {
            el.child(edge_fade(left, false))
                .child(edge_fade(left + cover_w - fade, true))
        })
        // ...and solid behind the words and controls at its foot.
        .child(
            div()
                .absolute()
                .left_0()
                .right_0()
                .bottom_0()
                .h(px(foot.min(h)))
                .bg(tone),
        )
        .child(
            div()
                .absolute()
                .left_0()
                .right_0()
                .bottom(px(foot.min(h) - 1.))
                .h(px((h * 0.42).max(200.)))
                .bg(linear_gradient(
                    180.,
                    linear_color_stop(tone.opacity(0.), 0.),
                    linear_color_stop(tone, 1.),
                )),
        )
        // At its top, so the buttons over it read.
        .child(
            div()
                .absolute()
                .left_0()
                .right_0()
                .top_0()
                .h(px(140.))
                .bg(linear_gradient(
                    180.,
                    linear_color_stop(tone.opacity(0.8), 0.),
                    linear_color_stop(tone.opacity(0.), 1.),
                )),
        )
        .into_any_element();
    (layer, side, tone)
}

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
    /// The big player growing from the player bar's cover (or shrinking back), in a space `w`
    /// by `h`. The player itself is `lift` taller and starts that far above the space (under the
    /// title bar), so it is drawn the same as once it is open, when it runs under the title bar.
    pub(super) fn big_reveal(
        &self,
        player: AnyElement,
        w: f32,
        h: f32,
        lift: f32,
        cx: &App,
    ) -> AnyElement {
        let closing = !self.big;
        let canvas = pal(cx).canvas;
        // Where the cover sits in the player bar, measured from the body's edges.
        let art = [22., h - 77., w - 78., 21.];
        let edge = move |t: f32, i: usize| art[i] * (1. - if closing { 1. - t } else { t });
        let serial = self.big_serial;
        let inner = motion::animate(
            div().absolute().w(px(w)).h(px(h + lift)).child(player),
            ("big-inner", serial),
            BIG_MS,
            cx,
            move |el, t| el.left(px(-edge(t, 0))).top(px(-edge(t, 1) - lift)),
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
        let liquid = super::glass::liquid_on(cx);
        let p = pal(cx);
        let look = self.now_look();
        let tint = look.as_ref().map(|l| l.vivid);
        let size = super::widgets::content_size(window);
        let (w, h) = (f32::from(size.width), f32::from(size.height));
        let side = if w < 1000. { Side::None } else { self.big_side };
        let current = self.shown_item();
        let playing = self.playback.playing;
        let repeat = self.playback.repeat;
        let left_w = if side == Side::None { w } else { w - 440. };
        let art = (h - 440.).min(left_w - 120.).clamp(140., 520.);
        // Settings > Appearance > Immersive cover, for a song with a cover of its own.
        let moving = current.as_ref().and_then(|c| self.moving_cover(&c.track));
        let immersive = self
            .settings
            .immersive_cover
            .then(|| {
                moving
                    .clone()
                    .or_else(|| current.as_ref().and_then(|c| c.track.artwork.clone()))
            })
            .flatten()
            .map(|path| {
                immersive_cover(
                    &path,
                    moving.is_some(),
                    look.as_ref(),
                    w,
                    h,
                    left_w - 40.,
                    340.,
                    p.dark,
                    side == Side::None,
                )
            });
        let column = immersive
            .as_ref()
            .map_or(art.max(420.), |(_, side, _)| (side - 96.).clamp(300., 480.));
        let top = match tint {
            _ if let Some((_, _, tone)) = &immersive => *tone,
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
        let ambient = self.ambient_look();
        // Ambient: the cover fills the whole screen at full strength.
        let fit = if ambient {
            1.
        } else {
            look.as_ref().map_or(1., |l| l.strength(p.dark))
        };
        let cover_side = immersive.as_ref().map(|(_, side, _)| *side);
        let (cover_layer, plain) = match immersive {
            Some((layer, _, _)) => (Some(layer), false),
            None => (None, true),
        };
        let backdrop = look.and_then(|l| l.blur).filter(|_| plain).map(|blur| {
            img(blur)
                .absolute()
                .top(px(-h * 0.1))
                .left(px(-w * 0.1))
                .w(px(w * 1.2))
                .h(px(h * 1.2))
                .object_fit(ObjectFit::Cover)
                .opacity(
                    if ambient && !p.dark {
                        0.55
                    } else if ambient {
                        1.
                    } else if p.dark {
                        0.85
                    } else {
                        0.55
                    } * fit,
                )
        });
        let player = div()
            .id("big-player")
            .size_full()
            .relative()
            .overflow_hidden()
            .flex()
            .flex_col()
            .children(backdrop)
            .children(cover_layer)
            // Vignette: darken toward the edges and fade into the page at the bottom, so the
            // controls always sit on a calm surface.
            .when(plain, |el| {
                el.child(div().absolute().inset_0().bg(linear_gradient(
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
            })
            // It runs under the title bar (which carries its buttons once it is open): room for
            // that.
            .child(div().h(px(self.title_height(window))).flex_shrink_0())
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .child(
                        div()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .items_center()
                            .gap_5()
                            .pb_8()
                            // Immersive: the cover is the window's side itself, and the words
                            // and controls sit over its foot.
                            .map(|el| match cover_side {
                                Some(side) => el.w(px(side)).flex_shrink_0().justify_end().pb_12(),
                                None => el.flex_1().justify_center().px_12(),
                            })
                            .when(cover_side.is_none(), |el| {
                                el.child(
                                    div()
                                        .rounded(px(10.))
                                        .shadow(vec![BoxShadow {
                                            color: gpui::black().opacity(if p.dark {
                                                0.55
                                            } else {
                                                // Softer on light backgrounds, where the shadow's
                                                // steps would show.
                                                0.14
                                            }),
                                            offset: point(px(0.), px(18.)),
                                            blur_radius: px(48.),
                                            spread_radius: px(-6.),
                                        }])
                                        // Settings › Appearance › Moving covers: the cover
                                        // moves here too with Immersive cover off.
                                        .child(
                                            moving
                                                .as_deref()
                                                .and_then(|path| {
                                                    super::widgets::moving_cover(
                                                        "big-cover",
                                                        path,
                                                        art,
                                                    )
                                                })
                                                .unwrap_or_else(|| {
                                                    artwork(
                                                        current.as_ref().map(|i| &i.track),
                                                        art,
                                                        cx,
                                                    )
                                                }),
                                        ),
                                )
                            })
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
                                        .w(px(column))
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
                                    .w(px(column))
                                    .flex()
                                    .items_center()
                                    .gap_3()
                                    .child(
                                        faint(
                                            format_duration(if current.is_some() {
                                                self.playback.position
                                            } else {
                                                0.
                                            }),
                                            cx,
                                        )
                                        .w(px(40.))
                                        .text_right(),
                                    )
                                    .child(self.wave_seek_bar(
                                        "big-seek",
                                        current.is_none(),
                                        34.,
                                        cx,
                                    ))
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
                                    // macOS Liquid Glass: the controls on a capsule of glass.
                                    .when(liquid, |el| {
                                        el.px(px(20.)).py(px(8.)).map(|el| {
                                            super::glass::pane(
                                                el,
                                                0.,
                                                30.,
                                                gpui::transparent_black(),
                                                cx,
                                            )
                                        })
                                    })
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
                    // The tone beside the cover, up to the side panel.
                    .when(cover_side.is_some(), |el| el.child(div().flex_1()))
                    .when(side != Side::None, |el| {
                        el.child(
                            div()
                                .w(px(420.))
                                .flex_shrink_0()
                                .h_full()
                                .flex()
                                .flex_col()
                                .when(!liquid, |el| el.pr_6().pb_6())
                                // macOS Liquid Glass: lyrics, up next, and stems on a pane.
                                .when(liquid, |el| {
                                    el.mr_6().mb_6().p_4().map(|el| {
                                        super::glass::pane(
                                            el,
                                            0.,
                                            24.,
                                            gpui::transparent_black(),
                                            cx,
                                        )
                                    })
                                })
                                .child(match side {
                                    Side::Lyrics => self
                                        .lyrics_view(super::lyrics::LyricsKind::Big, cx)
                                        .into_any_element(),
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
            let tone = motion::mix(from, to, t);
            // Immersive: all of it the cover's tone.
            el.bg(linear_gradient(
                180.,
                linear_color_stop(tone, 0.),
                linear_color_stop(if plain { canvas } else { tone }, 0.85),
            ))
        })
    }

    /// The big player's own buttons, in the title bar while it is open (one bar, not two):
    /// close, why this song plays, and the side panes, full screen, and the mini player.
    pub(super) fn big_bar(&self, cx: &mut Context<Self>) -> Div {
        let liquid = super::glass::liquid_on(cx);
        let p = pal(cx);
        // The song the big player shows (it was drawn first, and noted it).
        let current = self
            .playback
            .current
            .clone()
            .or_else(|| self.last_item.as_ref().map(|(item, _)| item.clone()));
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
        div()
            .flex_1()
            .h_full()
            .pl_4()
            .pr_2()
            .flex_shrink_0()
            .flex()
            .items_center()
            .gap_2()
            .child(
                div()
                    // macOS Liquid Glass: a round pane of glass.
                    .when(liquid, |el| el.p(px(3.)).rounded_full().glass())
                    .child(
                        icon_button("close-big", "chevron-down", "Close the player · Esc")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.big = false;
                                cx.notify();
                            })),
                    ),
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
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    // macOS Liquid Glass: the buttons on one capsule of glass.
                    .when(liquid, |el| el.px(px(6.)).py(px(3.)).rounded_full().glass())
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
                        icon_button("big-full", "fullscreen", "Full screen · F11").on_click(
                            cx.listener(|this, _, window, cx| this.set_immersive(true, window, cx)),
                        ),
                    )
                    .child(
                        icon_button("big-mini", "mini", "Mini player").on_click(cx.listener(
                            |this, _, window, cx| {
                                this.big = false;
                                this.open_mini(window, cx);
                            },
                        )),
                    ),
            )
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
