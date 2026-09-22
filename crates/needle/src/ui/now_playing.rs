use super::{
    AppView, Page, album_page, pal,
    widgets::{artwork, faint, glyph, icon_button, meta, quality},
};
use gpui::{prelude::*, *};
use gpui_component::{Sizable, slider::Slider};
use needle_core::{
    audio::{Command, Repeat},
    model::format_duration,
};
use std::collections::HashMap;

#[derive(Clone, Copy, PartialEq)]
pub enum Side {
    Lyrics,
    Queue,
    Stems,
    None,
}

/// Average colour of each artwork file, used to tint the big player. Computed off the UI thread.
#[derive(Default)]
pub struct ArtColors {
    colors: HashMap<String, Option<Hsla>>,
}

pub fn average_color(path: &str) -> Option<Hsla> {
    let image = image::open(path).ok()?.thumbnail(24, 24).to_rgb8();
    let (mut r, mut g, mut b, mut n) = (0u64, 0u64, 0u64, 0u64);
    for pixel in image.pixels() {
        r += pixel[0] as u64;
        g += pixel[1] as u64;
        b += pixel[2] as u64;
        n += 1;
    }
    if n == 0 {
        return None;
    }
    let rgb = Rgba {
        r: r as f32 / n as f32 / 255.,
        g: g as f32 / n as f32 / 255.,
        b: b as f32 / n as f32 / 255.,
        a: 1.,
    };
    Some(rgb.into())
}

impl AppView {
    /// The tint for the current track's artwork, starting a background measurement the first time.
    pub(super) fn art_tint(&mut self) -> Option<Hsla> {
        let path = self.playback.current.as_ref()?.track.artwork.clone()?;
        if let Some(color) = self.art_colors.colors.get(&path) {
            return *color;
        }
        self.art_colors.colors.insert(path.clone(), None);
        let sender = self.sender.clone();
        std::thread::spawn(move || {
            let color = average_color(&path);
            let _ = sender.send(super::Event::ArtColor(path, color));
        });
        None
    }
    pub(super) fn set_art_color(&mut self, path: String, color: Option<Hsla>) {
        self.art_colors.colors.insert(path, color);
    }

    pub(super) fn big_player(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let p = pal(cx);
        let tint = self.art_tint();
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
        let background = linear_gradient(
            180.,
            linear_color_stop(top, 0.),
            linear_color_stop(p.canvas, 0.85),
        );
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
            .id("big-player")
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .bg(background)
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
                            .child(div().rounded(px(10.)).shadow_lg().child(artwork(
                                current.as_ref().map(|i| &i.track),
                                art,
                                cx,
                            )))
                            .child(match &current {
                                None => div()
                                    .flex()
                                    .flex_col()
                                    .items_center()
                                    .gap_1()
                                    .child(
                                        div()
                                            .text_size(px(22.))
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .child("Nothing playing"),
                                    )
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
                                            div()
                                                .max_w_full()
                                                .truncate()
                                                .text_size(px(24.))
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .child(track.title.clone()),
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
