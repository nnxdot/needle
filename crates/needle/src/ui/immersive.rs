//! Immersive mode: the playing song across the whole screen, like Apple Music's full screen
//! player. The blurred cover drifts behind a large cover and, when the song has them, large
//! synced lyrics; the controls fade away while the pointer rests and the music plays.
use super::{
    AppView,
    lyrics::LyricsKind,
    motion, pal,
    widgets::{artwork, display, faint, glyph, icon_button, quality},
};
use gpui::{prelude::*, *};
use gpui_component::slider::Slider;
use needle_core::{
    audio::{Command, Repeat},
    model::format_duration,
};
use std::time::{Duration, Instant};

/// How long the pointer rests before the controls fade.
const IDLE: Duration = Duration::from_secs(3);

impl AppView {
    /// Enter or leave immersive mode, taking the window to full screen and back.
    pub(super) fn set_immersive(&mut self, on: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.immersive == on {
            return;
        }
        self.immersive = on;
        self.immersive_moved = Instant::now();
        if on {
            // Remember the window's size, to give it back afterwards: leaving full screen can
            // otherwise leave the window as large as the screen.
            self.immersive_restore =
                (!window.is_maximized() && !window.is_fullscreen()).then(|| window.viewport_size());
        }
        // Leaving undoes what entering did, even when the system has not reported full screen
        // yet (entering and leaving quickly): asking the window would say it is not.
        if on {
            self.immersive_toggled = !window.is_fullscreen();
            if self.immersive_toggled {
                window.toggle_fullscreen();
            }
        } else if std::mem::take(&mut self.immersive_toggled) {
            window.toggle_fullscreen();
        }
        if !on && let Some(size) = self.immersive_restore.take() {
            // After the system has taken the window out of full screen.
            cx.spawn_in(window, async move |_, cx| {
                cx.background_executor()
                    .timer(Duration::from_millis(200))
                    .await;
                let _ = cx.update(|window, _| {
                    if !window.is_fullscreen() && !window.is_maximized() {
                        window.resize(size);
                    }
                });
            })
            .detach();
        }
        // Bring the sung line into view as the lyrics appear.
        self.lyric_glide = on && self.lyric_line.is_some();
        window.focus(&self.focus);
        cx.notify();
    }

    /// Whether the playing song has lyrics to show.
    fn has_lyrics(&self) -> bool {
        let current = self.playback.current.as_ref().map(|c| &c.track.id);
        matches!(&self.lyrics, Some((id, Some(lyrics)))
            if Some(id) == current && !lyrics.instrumental
                && (!lyrics.lines.is_empty() || !lyrics.plain.trim().is_empty()))
    }

    pub(super) fn immersive_view(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let p = pal(cx);
        let size = window.viewport_size();
        let (w, h) = (f32::from(size.width), f32::from(size.height));
        let current = self.shown_item();
        let playing = self.playback.playing;
        let repeat = self.playback.repeat;
        let look = self.now_look();
        let lyrics = self.immersive_lyrics && self.has_lyrics() && w >= 900.;

        // The controls rest out of sight while the music plays and the pointer is still.
        let shown = !playing || self.immersive_moved.elapsed() < IDLE;
        if shown != self.immersive_shown {
            self.immersive_shown = shown;
            self.immersive_serial += 1;
        }
        // Look again when the pointer has rested long enough: one wait at a time, and the
        // next render starts another if the pointer moved in the meantime.
        if shown && playing && !self.immersive_waiting {
            self.immersive_waiting = true;
            let wait =
                IDLE.saturating_sub(self.immersive_moved.elapsed()) + Duration::from_millis(50);
            cx.spawn(async move |this, cx| {
                cx.background_executor().timer(wait).await;
                let _ = this.update(cx, |this, cx| {
                    this.immersive_waiting = false;
                    cx.notify();
                });
            })
            .detach();
        }
        // Keyed by the serial, so each change between shown and resting fades once.
        let serial = self.immersive_serial;
        let fade = move |el: Div, id: &'static str, cx: &App| {
            motion::animate(el, (id, serial as usize), 450, cx, move |el, t| {
                el.opacity(if shown { t } else { 1. - t })
            })
        };

        let left_w = if lyrics { w * 0.44 } else { w };
        let art = if lyrics {
            (h * 0.46).min(left_w * 0.62)
        } else {
            (h * 0.5).min(w * 0.4)
        }
        .clamp(180., 620.);
        let column_w = art.max(360.);

        // The blurred cover, larger than the screen, drifting slowly.
        let backdrop = look.as_ref().and_then(|l| l.blur.clone()).map(|blur| {
            motion::repeat(
                img(blur)
                    .absolute()
                    .w(px(w * 1.5))
                    .h(px(h * 1.5))
                    .object_fit(ObjectFit::Cover),
                "immersive-drift",
                40_000,
                0.5,
                cx,
                move |el, t| {
                    el.left(px(-w * (0.1 + 0.15 * t)))
                        .top(px(-h * (0.2 - 0.12 * t)))
                },
            )
        });
        let tint = look
            .as_ref()
            .map(|l| hsla(l.vivid.h, (l.vivid.s * 0.8).min(0.5), 0.18, 1.))
            .unwrap_or(p.canvas);

        let now = match &current {
            None => div()
                .flex()
                .flex_col()
                .items_center()
                .child(display("Nothing playing", 34.)),
            Some(item) => {
                let track = &item.track;
                div()
                    .w(px(column_w))
                    .flex()
                    .flex_col()
                    .when(!lyrics, |el| el.items_center())
                    .gap_1()
                    .child(
                        display(track.title.clone(), if lyrics { 30. } else { 38. })
                            .max_w_full()
                            .truncate(),
                    )
                    .child(
                        div()
                            .max_w_full()
                            .truncate()
                            .text_size(px(if lyrics { 16. } else { 19. }))
                            .text_color(p.ink_2)
                            .child(format!(
                                "{} · {}",
                                track.display_artist(),
                                track.display_album()
                            )),
                    )
                    .child(faint(quality(track), cx).mt_1())
            }
        };

        let seek = div()
            .w(px(column_w))
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
            .child(self.seek_bar("imm-seek", current.is_none(), cx))
            .child(
                faint(
                    current
                        .as_ref()
                        .map(|i| format_duration(i.track.duration))
                        .unwrap_or_else(|| "0:00".into()),
                    cx,
                )
                .w(px(40.)),
            );
        let buttons = div()
            .w(px(column_w))
            .flex()
            .items_center()
            .justify_center()
            .gap_5()
            .child(
                icon_button("imm-shuffle", "shuffle", "Shuffle this view")
                    .on_click(cx.listener(|this, _, _, cx| this.play_view(0, true, cx))),
            )
            .child(
                icon_button("imm-previous", "previous", "Previous")
                    .on_click(cx.listener(|this, _, _, _| this.player.send(Command::Previous))),
            )
            .child(
                div()
                    .id("imm-play")
                    .size(px(64.))
                    .rounded_full()
                    .bg(p.ink)
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .hover(|s| s.opacity(0.88))
                    .child(
                        glyph(if playing { "pause" } else { "play" })
                            .size(px(28.))
                            .text_color(p.canvas),
                    )
                    .on_click(cx.listener(|this, _, _, cx| this.toggle_playback(cx))),
            )
            .child(
                icon_button("imm-next", "next", "Next")
                    .on_click(cx.listener(|this, _, _, _| this.player.send(Command::Next))),
            )
            .child(
                icon_button(
                    "imm-repeat",
                    if repeat == Repeat::One {
                        "repeat-one"
                    } else {
                        "repeat"
                    },
                    "Repeat",
                )
                .when(repeat != Repeat::Off, |b| b.text_color(p.accent))
                .on_click(cx.listener(|this, _, _, _| {
                    let next = match this.playback.repeat {
                        Repeat::Off => Repeat::All,
                        Repeat::All => Repeat::One,
                        Repeat::One => Repeat::Off,
                    };
                    this.player.send(Command::Repeat(next));
                })),
            );
        let volume = div()
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
            .child(glyph("volume").size(px(16.)).text_color(p.ink_2));
        let controls = div()
            .flex()
            .flex_col()
            .when(!lyrics, |el| el.items_center())
            .gap_4()
            .child(seek)
            .child(buttons)
            .child(volume);

        let corner = div()
            .absolute()
            .top(px(20.))
            .right(px(24.))
            .flex()
            .gap_2()
            .when(self.has_lyrics(), |el| {
                el.child(
                    icon_button(
                        "imm-lyrics",
                        "lyrics",
                        if self.immersive_lyrics {
                            "Hide the lyrics"
                        } else {
                            "Show the lyrics"
                        },
                    )
                    .when(self.immersive_lyrics, |b| b.text_color(p.accent))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.immersive_lyrics = !this.immersive_lyrics;
                        this.lyric_glide = this.immersive_lyrics && this.lyric_line.is_some();
                        cx.notify();
                    })),
                )
            })
            .child(
                icon_button("imm-exit", "fullscreen-exit", "Leave full screen · Esc").on_click(
                    cx.listener(|this, _, window, cx| this.set_immersive(false, window, cx)),
                ),
            );

        let cover = div()
            .rounded(px(12.))
            .shadow(vec![BoxShadow {
                color: gpui::black().opacity(0.5),
                offset: point(px(0.), px(24.)),
                blur_radius: px(64.),
                spread_radius: px(-8.),
            }])
            .child(artwork(current.as_ref().map(|i| &i.track), art, cx));

        let left = div()
            .w(px(left_w))
            .h_full()
            .flex_shrink_0()
            .flex()
            .flex_col()
            .justify_center()
            .when(lyrics, |el| el.items_end().pr(px(w * 0.05)))
            .when(!lyrics, |el| el.items_center())
            .child(
                div()
                    .flex()
                    .flex_col()
                    .when(!lyrics, |el| el.items_center())
                    .gap_6()
                    .child(cover)
                    .child(now)
                    .child(fade(controls, "imm-controls", cx)),
            );

        div()
            .id("immersive")
            .size_full()
            .relative()
            .overflow_hidden()
            .bg(tint)
            .text_color(p.ink)
            .on_mouse_move(cx.listener(|this, _, _, cx| {
                this.immersive_moved = Instant::now();
                cx.notify();
            }))
            .children(backdrop)
            // Calm the backdrop so text stays readable over any cover.
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .bg(p.canvas.opacity(if p.dark { 0.42 } else { 0.5 })),
            )
            .child(div().absolute().inset_0().bg(linear_gradient(
                180.,
                linear_color_stop(p.canvas.opacity(0.), 0.55),
                linear_color_stop(p.canvas.opacity(0.55), 1.),
            )))
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .flex()
                    .child(left)
                    .when(lyrics, |el| {
                        el.child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .h_full()
                                .pl(px(w * 0.04))
                                .pr(px(w * 0.06))
                                .flex()
                                .flex_col()
                                .child(self.lyrics_view(LyricsKind::Immersive, cx)),
                        )
                    }),
            )
            .child(fade(corner, "imm-corner", cx))
            .into_any_element()
    }
}
