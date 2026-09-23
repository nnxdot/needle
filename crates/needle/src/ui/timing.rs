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
    media::{self, LyricLine, LyricWord, Lyrics, LyricsSource},
    model::Track,
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

#[derive(Clone)]
pub struct TimedLine {
    pub text: String,
    pub time: Option<f64>,
    /// Each word with the spaces after it.
    pub words: Vec<(String, Option<f64>)>,
}

impl TimedLine {
    fn new(text: &str) -> Self {
        Self {
            text: text.to_string(),
            time: None,
            words: split_words(text).into_iter().map(|w| (w, None)).collect(),
        }
    }
    /// Move the line and its stamped words by `delta` seconds.
    fn shift(&mut self, delta: f64) {
        if let Some(time) = &mut self.time {
            *time = (*time + delta).max(0.);
        }
        for (_, time) in &mut self.words {
            if let Some(time) = time {
                *time = (*time + delta).max(0.);
            }
        }
    }
}

/// "Hold on  tight" → ["Hold ", "on  ", "tight"].
fn split_words(text: &str) -> Vec<String> {
    let mut words: Vec<String> = vec![];
    for piece in text.trim().split_inclusive(' ') {
        match words.last_mut() {
            Some(last) if piece.trim().is_empty() => last.push_str(piece),
            _ => words.push(piece.to_string()),
        }
    }
    words
}

pub struct Timing {
    pub track: Track,
    pub lines: Vec<TimedLine>,
    /// Tapping words, not lines.
    pub words: bool,
    /// The next line and word a tap stamps.
    pub next: (usize, usize),
    /// The last line stamped or clicked; the arrow keys move it.
    pub selected: Option<usize>,
    /// For undo: the next position and the line as it was before each tap.
    history: Vec<((usize, usize), usize, TimedLine)>,
    pub changed: bool,
    /// Asking for the words first, when the song has none.
    pub writing: Option<Entity<InputState>>,
}

/// Fill the gaps in `times`: between two stamps evenly, before the first at `start`, and
/// after the last every `step` seconds.
fn fill(times: &mut [Option<f64>], start: f64, step: f64) {
    let known: Vec<usize> = (0..times.len()).filter(|&i| times[i].is_some()).collect();
    for i in 0..times.len() {
        if times[i].is_some() {
            continue;
        }
        let before = known.iter().rev().find(|&&k| k < i).copied();
        let after = known.iter().find(|&&k| k > i).copied();
        times[i] = Some(match (before, after) {
            (Some(b), Some(a)) => {
                let (tb, ta) = (times[b].unwrap_or(start), times[a].unwrap_or(start));
                tb + (ta - tb) * (i - b) as f64 / (a - b) as f64
            }
            (Some(b), None) => times[b].unwrap_or(start) + step * (i - b) as f64,
            (None, Some(a)) => {
                let ta = times[a].unwrap_or(start);
                start + (ta - start) * i as f64 / a as f64
            }
            (None, None) => start + step * i as f64,
        });
    }
}

impl Timing {
    /// Tap: stamp the next line or word at `position`.
    fn tap(&mut self, position: f64) {
        let Some(line) = self.lines.get(self.next.0) else {
            return;
        };
        self.history.push((self.next, self.next.0, line.clone()));
        let (index, word) = self.next;
        let line = &mut self.lines[index];
        if self.words && !line.words.is_empty() {
            if word == 0 {
                line.time = Some(position);
            }
            if let Some(slot) = line.words.get_mut(word) {
                slot.1 = Some(position);
            }
            self.next = if word + 1 < line.words.len() {
                (index, word + 1)
            } else {
                (index + 1, 0)
            };
        } else {
            // Re-timing a line keeps its words where they sit within it.
            match line.time {
                Some(old) => line.shift(position - old),
                None => line.time = Some(position),
            }
            self.next = (index + 1, 0);
        }
        self.selected = Some(index);
        self.changed = true;
    }

    fn undo(&mut self) -> bool {
        let Some((next, index, line)) = self.history.pop() else {
            return false;
        };
        self.lines[index] = line;
        self.next = next;
        self.selected = Some(index);
        true
    }

    fn nudge(&mut self, delta: f64) {
        if let Some(line) = self.selected.and_then(|i| self.lines.get_mut(i)) {
            line.shift(delta);
            self.changed = true;
        }
    }

    /// The lines to save, every gap filled in. None when nothing is stamped yet.
    fn finished(&self) -> Option<Vec<LyricLine>> {
        if self.lines.iter().all(|l| l.time.is_none()) {
            return None;
        }
        let mut starts: Vec<Option<f64>> = self.lines.iter().map(|l| l.time).collect();
        fill(&mut starts, 0., 3.);
        let mut lines = vec![];
        for (i, line) in self.lines.iter().enumerate() {
            let start = starts[i].unwrap_or(0.);
            let end = starts
                .get(i + 1)
                .copied()
                .flatten()
                .unwrap_or(start + 4.)
                .max(start);
            let words = if line.words.iter().any(|w| w.1.is_some()) {
                let mut times: Vec<Option<f64>> = line.words.iter().map(|w| w.1).collect();
                if times[0].is_none() {
                    times[0] = Some(start);
                }
                fill(
                    &mut times,
                    start,
                    ((end - start) / line.words.len() as f64).min(0.6),
                );
                line.words
                    .iter()
                    .zip(times)
                    .map(|((text, _), time)| LyricWord {
                        time: time.unwrap_or(start),
                        text: text.clone(),
                    })
                    .collect()
            } else {
                vec![]
            };
            lines.push(LyricLine {
                time: start,
                text: line.text.clone(),
                words,
            });
        }
        lines.sort_by(|a, b| a.time.total_cmp(&b.time));
        Some(lines)
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
        let lines: Vec<TimedLine> = match &lyrics {
            Some(l) if !l.lines.is_empty() => l
                .lines
                .iter()
                .map(|line| TimedLine {
                    text: line.text.clone(),
                    time: Some(line.time),
                    words: if line.words.is_empty() {
                        split_words(&line.text)
                            .into_iter()
                            .map(|w| (w, None))
                            .collect()
                    } else {
                        line.words
                            .iter()
                            .map(|w| (w.text.clone(), Some(w.time)))
                            .collect()
                    },
                })
                .collect(),
            Some(l) => l.plain.lines().map(TimedLine::new).collect(),
            None => vec![],
        };
        let writing = lines.is_empty().then(|| {
            cx.new(|cx| {
                InputState::new(window, cx)
                    .multi_line(true)
                    .placeholder("Paste or type the words here, one line per sung line.")
            })
        });
        let words = lines.iter().any(|l| l.words.iter().any(|w| w.1.is_some()));
        let focus = self.timing_focus.clone();
        self.timing = Some(Timing {
            track,
            lines,
            words,
            next: (0, 0),
            selected: None,
            history: vec![],
            changed: false,
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
        let lines: Vec<TimedLine> = text
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .map(TimedLine::new)
            .collect();
        if lines.is_empty() {
            self.notify("Write the words first, one line per sung line.");
            return;
        }
        timing.lines = lines;
        timing.writing = None;
        timing.changed = true;
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
                                    t.words = i == 1;
                                    t.next = (t.selected.unwrap_or(0), 0);
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
                            t.next = (i, 0);
                            t.selected = Some(i);
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
            motion::animate(
                tap,
                ("timing-tap", timing.history.len()),
                180,
                cx,
                move |el, t| el.opacity(0.75 + 0.25 * t),
            )
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

#[cfg(test)]
mod tests {
    use super::{TimedLine, Timing, fill, split_words};

    fn timing(lines: &[&str], words: bool) -> Timing {
        Timing {
            track: Default::default(),
            lines: lines.iter().map(|l| TimedLine::new(l)).collect(),
            words,
            next: (0, 0),
            selected: None,
            history: vec![],
            changed: false,
            writing: None,
        }
    }

    #[test]
    fn splits_words_keeping_spaces() {
        assert_eq!(
            split_words(" Hold on  tight "),
            vec!["Hold ", "on  ", "tight"]
        );
    }

    #[test]
    fn fills_gaps() {
        let mut times = vec![None, Some(2.), None, None, Some(8.), None];
        fill(&mut times, 0., 3.);
        assert_eq!(
            times,
            vec![Some(0.), Some(2.), Some(4.), Some(6.), Some(8.), Some(11.)]
        );
    }

    #[test]
    fn taps_lines_then_words_and_undoes() {
        let mut t = timing(&["one two", "three"], false);
        t.tap(1.0);
        t.tap(4.0);
        assert_eq!(t.next, (2, 0));
        let lines = t.finished().unwrap();
        assert_eq!((lines[0].time, lines[1].time), (1.0, 4.0));
        assert!(lines[0].words.is_empty());
        // Word mode, from the first line again.
        t.words = true;
        t.next = (0, 0);
        t.tap(1.2);
        t.tap(1.8);
        assert_eq!(t.next, (1, 0));
        let lines = t.finished().unwrap();
        assert_eq!(lines[0].time, 1.2);
        assert_eq!(
            lines[0].words.iter().map(|w| w.time).collect::<Vec<_>>(),
            vec![1.2, 1.8]
        );
        assert_eq!(lines[0].text, "one two");
        assert!(t.undo());
        assert_eq!(t.next, (0, 1));
        assert_eq!(t.lines[0].words[1].1, None);
        // Nudging moves the line and its words together.
        t.selected = Some(0);
        t.nudge(0.5);
        assert_eq!(t.lines[0].time, Some(1.7));
        assert_eq!(t.lines[0].words[0].1, Some(1.7));
    }

    #[test]
    fn nothing_to_save_before_a_tap() {
        assert!(timing(&["a"], false).finished().is_none());
    }
}
