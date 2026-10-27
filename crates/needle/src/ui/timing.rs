//! The lyric timing editor: play the song and tap along. Space stamps the next line (or, in
//! word mode, the next word). Saved as an `.lrc` file beside the song.
use super::{
    AppView, Page, motion, pal,
    widgets::{display, faint, glyph, meta, segmented, small_button},
};
use gpui::{prelude::*, *};
use gpui_component::{
    Disableable,
    button::ButtonVariants,
    input::{Input, InputState},
};
use needle_core::{
    audio::Command,
    media::{self, Lyrics, LyricsSource},
    model::Track,
    timing::LyricTiming,
};

actions!(timing, [Tap, Undo, Earlier, Later]);

/// The keys, active while the editor has focus (and no text box does).
pub fn bind_keys(cx: &mut App) {
    let context = Some("Timing && !Input");
    cx.bind_keys([
        KeyBinding::new("space", Tap, context),
        KeyBinding::new("backspace", Undo, context),
        KeyBinding::new("left", Earlier, context),
        KeyBinding::new("right", Later, context),
    ]);
}

/// The editor for one song: the taps are the core's (`needle_core::timing`), the same on every
/// system.
pub struct Timing {
    pub track: Track,
    pub editor: LyricTiming,
    /// Asking for the words first, when the song has none.
    pub writing: Option<Entity<InputState>>,
}

impl std::ops::Deref for Timing {
    type Target = LyricTiming;
    fn deref(&self) -> &LyricTiming {
        &self.editor
    }
}

impl std::ops::DerefMut for Timing {
    fn deref_mut(&mut self) -> &mut LyricTiming {
        &mut self.editor
    }
}

impl AppView {
    /// Open the timing editor for the song that is playing.
    pub(super) fn open_timing(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(item) = self.playback.current.clone() else {
            self.notify("Play a song first, then time its lyrics.");
            return;
        };
        let track = item.track;
        self.big = false;
        if track.cue.is_some() {
            self.notify("This song is part of a CUE sheet, so it has no file of its own to put lyrics beside.");
            return;
        }
        let lyrics = match &self.lyrics {
            Some((id, lyrics)) if *id == track.id => lyrics.clone(),
            _ => None,
        };
        let editor = match &lyrics {
            Some(l) => LyricTiming::from_lyrics(l),
            None => LyricTiming::default(),
        };
        let writing = editor.lines.is_empty().then(|| {
            cx.new(|cx| {
                InputState::new(window, cx)
                    .multi_line(true)
                    .placeholder("Paste or type the words here, one line per sung line.")
            })
        });
        let focus = self.timing_focus.clone();
        self.timing = Some(Timing {
            track,
            editor,
            writing,
        });
        self.navigate(Page::Timing, window, cx);
        match &self.timing.as_ref().and_then(|t| t.writing.clone()) {
            Some(input) => input.update(cx, |s, cx| s.focus(window, cx)),
            None => window.focus(&focus),
        }
    }

    /// The words are in: split them into lines and start timing.
    fn start_timing(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(timing) = &mut self.timing else {
            return;
        };
        let Some(input) = &timing.writing else { return };
        let text = input.read(cx).value().to_string();
        let editor = LyricTiming::from_text(&text);
        if editor.lines.is_empty() {
            self.notify("Write the words first, one line per sung line.");
            return;
        }
        timing.editor = editor;
        timing.editor.changed = true;
        timing.writing = None;
        window.focus(&self.timing_focus);
        cx.notify();
    }

    fn timing_position(&self) -> Option<f64> {
        let timing = self.timing.as_ref()?;
        let current = self.playback.current.as_ref()?;
        (current.track.id == timing.track.id).then_some(self.playback.position)
    }

    fn timing_tap(&mut self, cx: &mut Context<Self>) {
        let Some(position) = self.timing_position() else {
            self.notify("Play the song to tap along with it.");
            cx.notify();
            return;
        };
        let Some(timing) = &mut self.timing else {
            return;
        };
        if !self.playback.playing {
            self.player.send(Command::Toggle);
        }
        timing.tap(position);
        self.timing_scroll
            .scroll_to_item(timing.next.0.min(timing.lines.len().saturating_sub(1)));
        cx.notify();
    }

    fn save_timing(&mut self, cx: &mut Context<Self>) {
        let Some(timing) = &mut self.timing else {
            return;
        };
        let Some(lines) = timing.finished() else {
            self.notify("Tap at least one line before saving.");
            cx.notify();
            return;
        };
        match media::save_lrc(&timing.track, &lines) {
            Ok(path) => {
                timing.changed = false;
                let plain = lines
                    .iter()
                    .map(|l| l.text.as_str())
                    .collect::<Vec<_>>()
                    .join("\n");
                let id = timing.track.id.clone();
                self.lyrics = Some((
                    id,
                    Some(Lyrics {
                        lines,
                        plain,
                        instrumental: false,
                        source: LyricsSource::Sidecar,
                        provider: String::new(),
                    }),
                ));
                self.lyric_line = None;
                self.follow_lyrics();
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                self.notify(format!("Saved as {name}, beside the song."));
            }
            Err(e) => self.fail(format!("Could not save the lyrics: {e:#}")),
        }
        cx.notify();
    }

    pub(super) fn timing_view(&self, cx: &mut Context<Self>) -> AnyElement {
        let p = pal(cx);
        let Some(timing) = &self.timing else {
            return div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .child(faint(
                    "Open the lyric timing editor from the lyrics of a playing song.",
                    cx,
                ))
                .into_any_element();
        };
        let playing_this = self.timing_position().is_some();
        let header = div()
            .flex()
            .items_end()
            .gap_4()
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(display("Lyric timing", 34.))
                    .child(
                        meta(
                            format!("{} · {}", timing.track.title, timing.track.display_artist()),
                            cx,
                        )
                        .truncate(),
                    ),
            )
            .when(timing.writing.is_none(), |el| {
                el.child(segmented(
                    "timing-mode",
                    &["Lines", "Words"],
                    timing.words as usize,
                    cx,
                    {
                        let view = cx.entity().downgrade();
                        move |i, window, cx| {
                            view.update(cx, |this, cx| {
                                if let Some(t) = &mut this.timing {
                                    t.set_words(i == 1);
                                }
                                window.focus(&this.timing_focus);
                                cx.notify();
                            })
                            .ok();
                        }
                    },
                ))
                .child(
                    small_button("timing-save", "Save")
                        .primary()
                        .disabled(!timing.changed)
                        .on_click(cx.listener(|this, _, _, cx| this.save_timing(cx))),
                )
            })
            .child(
                small_button("timing-close", "Close")
                    .ghost()
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.go_back(window, cx);
                        cx.notify();
                    })),
            );

        if let Some(input) = &timing.writing {
            return div()
                .id("timing")
                .size_full()
                .overflow_y_scroll()
                .child(
                    div()
                        .px_6()
                        .pt_6()
                        .pb_10()
                        .flex()
                        .flex_col()
                        .gap_4()
                        .child(header)
                        .child(meta("This song has no lyrics yet. Write them here, then tap along to time them.", cx))
                        .child(div().h(px(320.)).child(Input::new(input).h_full()))
                        .child(
                            div().child(
                                small_button("timing-start", "Start timing")
                                    .primary()
                                    .on_click(cx.listener(|this, _, window, cx| this.start_timing(window, cx))),
                            ),
                        ),
                )
                .into_any_element();
        }

        let hint = if !playing_this {
            "Play this song, then tap along."
        } else if timing.words {
            "Press Space as each word starts. Backspace undoes. ← and → move the last line by a twentieth of a second."
        } else {
            "Press Space as each line starts. Backspace undoes. ← and → move the last line by a twentieth of a second."
        };
        let position = self.timing_position();
        let rows: Vec<AnyElement> = timing
            .lines
            .iter()
            .enumerate()
            .map(|(i, line)| {
                let is_next = i == timing.next.0;
                let selected = timing.selected == Some(i);
                let time_label = line
                    .time
                    .map(media::format_stamp)
                    .unwrap_or_else(|| "--:--.--".into());
                // Now singing: this line started and the next has not.
                let now = position.is_some_and(|pos| {
                    line.time.is_some_and(|t| t <= pos)
                        && timing
                            .lines
                            .get(i + 1)
                            .and_then(|l| l.time)
                            .is_none_or(|t| pos < t)
                });
                // Word mode: tapped words full strength, the rest faint, the next one underlined.
                let mut at = 0;
                let highlights: Vec<(std::ops::Range<usize>, HighlightStyle)> = line
                    .words
                    .iter()
                    .enumerate()
                    .map(|(w, (text, time))| {
                        let range = at..at + text.len();
                        at = range.end;
                        let next_word = is_next && w == timing.next.1;
                        // Highlight colours paint over the text's own, so fading uses fade_out.
                        let style = HighlightStyle {
                            color: next_word.then_some(p.accent),
                            fade_out: (!next_word && time.is_none()).then_some(0.62),
                            underline: next_word.then_some(UnderlineStyle {
                                thickness: px(1.5),
                                color: Some(p.accent),
                                wavy: false,
                            }),
                            ..Default::default()
                        };
                        (range, style)
                    })
                    .collect();
                let word_text: String = line.words.iter().map(|w| w.0.as_str()).collect();
                let time = line.time;
                div()
                    .id(("timing-line", i))
                    .px_3()
                    .py(px(7.))
                    .rounded(px(8.))
                    .flex()
                    .items_center()
                    .gap_4()
                    .cursor_pointer()
                    .when(selected, |el| el.bg(p.raised))
                    .when(is_next, |el| {
                        el.border_1().border_color(p.accent.opacity(0.6))
                    })
                    .when(!is_next, |el| {
                        el.border_1().border_color(transparent_black())
                    })
                    .hover(|s| s.bg(p.raised.opacity(0.7)))
                    .child(
                        div()
                            .w(px(64.))
                            .flex_shrink_0()
                            .text_size(px(12.))
                            .font_family("Consolas")
                            .text_color(if line.time.is_some() {
                                p.accent
                            } else {
                                p.ink_3
                            })
                            .child(time_label),
                    )
                    .child(if timing.words && !line.words.is_empty() {
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_size(px(15.))
                            .child(StyledText::new(word_text).with_highlights(highlights))
                            .into_any_element()
                    } else {
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_size(px(15.))
                            .text_color(if line.time.is_some() {
                                p.ink
                            } else {
                                p.ink.opacity(0.45)
                            })
                            .child(if line.text.is_empty() {
                                "♪".to_string()
                            } else {
                                line.text.clone()
                            })
                            .into_any_element()
                    })
                    .when(now, |el| el.font_weight(FontWeight::SEMIBOLD))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        if let Some(t) = &mut this.timing {
                            // Tapping starts again from here; a stamped line plays from a little before.
                            t.select(i);
                        }
                        window.focus(&this.timing_focus);
                        if let Some(time) = time
                            && this.timing_position().is_some()
                        {
                            this.player.send(Command::Seek((time - 2.).max(0.)));
                        }
                        cx.notify();
                    }))
                    .into_any_element()
            })
            .collect();
        let done = timing.next.0 >= timing.lines.len();
        let tap = div()
            .id("timing-tap")
            .h(px(64.))
            .rounded(px(12.))
            .bg(if playing_this { p.accent } else { p.raised })
            .text_color(if playing_this { p.accent_ink } else { p.ink_3 })
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .hover(|s| s.opacity(0.9))
            .child(
                div()
                    .text_size(px(15.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(if done {
                        "All stamped. Save, or click a line to redo it."
                    } else if timing.words {
                        "Tap: next word"
                    } else {
                        "Tap: next line"
                    }),
            )
            .child(div().text_size(px(11.5)).opacity(0.8).child("Space"))
            .on_click(cx.listener(|this, _, window, cx| {
                window.focus(&this.timing_focus);
                this.timing_tap(cx)
            }));
        let tap = if playing_this {
            motion::animate(tap, ("timing-tap", timing.taps()), 180, cx, move |el, t| {
                el.opacity(0.75 + 0.25 * t)
            })
        } else {
            tap.into_any_element()
        };
        div()
            .id("timing")
            .key_context("Timing")
            .track_focus(&self.timing_focus)
            .size_full()
            .flex()
            .flex_col()
            .on_action(cx.listener(|this, _: &Tap, _, cx| this.timing_tap(cx)))
            .on_action(cx.listener(|this, _: &Undo, _, cx| {
                if let Some(t) = &mut this.timing
                    && t.undo()
                {
                    cx.notify();
                }
            }))
            .on_action(cx.listener(|this, _: &Earlier, _, cx| {
                if let Some(t) = &mut this.timing {
                    t.nudge(-0.05);
                }
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &Later, _, cx| {
                if let Some(t) = &mut this.timing {
                    t.nudge(0.05);
                }
                cx.notify();
            }))
            .child(
                div()
                    .flex_shrink_0()
                    .px_6()
                    .pt_6()
                    .pb_4()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .child(header)
                    .child(tap)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(glyph("lyrics").size(px(14.)).text_color(p.ink_3))
                            .child(meta(hint, cx).flex_1().min_w_0()),
                    ),
            )
            .child(
                div()
                    .id("timing-lines")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .track_scroll(&self.timing_scroll)
                    .px_6()
                    .pb_10()
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .children(rows),
            )
            .into_any_element()
    }
}
