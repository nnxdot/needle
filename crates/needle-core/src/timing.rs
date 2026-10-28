//! Timing lyrics by hand ("Time the lyrics"): the song plays and the listener taps as each line
//! (or, in word mode, each word) starts; −0.2 s and +0.2 s move the line last tapped, Redo line
//! taps it again, and Undo takes back a tap. The result is saved as an `.lrc` file
//! ([`crate::media::save_lrc`] beside the song; the phones keep it in their own storage).
use crate::media::{LyricLine, LyricWord, Lyrics};

/// How far −0.2 s and +0.2 s move a line.
pub const NUDGE: f64 = 0.2;

#[derive(Clone, Debug, PartialEq)]
pub struct TimedLine {
    pub text: String,
    pub time: Option<f64>,
    /// Each word with the spaces after it.
    pub words: Vec<(String, Option<f64>)>,
}

impl TimedLine {
    pub fn new(text: &str) -> Self {
        Self {
            text: text.to_string(),
            time: None,
            words: split_words(text).into_iter().map(|w| (w, None)).collect(),
        }
    }
    /// Move the line and its stamped words by `delta` seconds.
    pub fn shift(&mut self, delta: f64) {
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
pub fn split_words(text: &str) -> Vec<String> {
    let mut words: Vec<String> = vec![];
    for piece in text.trim().split_inclusive(' ') {
        match words.last_mut() {
            Some(last) if piece.trim().is_empty() => last.push_str(piece),
            _ => words.push(piece.to_string()),
        }
    }
    words
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

/// One song's lyrics being timed.
#[derive(Clone, Debug, Default)]
pub struct LyricTiming {
    pub lines: Vec<TimedLine>,
    /// Tapping words, not lines.
    pub words: bool,
    /// The next line and word a tap stamps.
    pub next: (usize, usize),
    /// The last line stamped or chosen; −0.2 s and +0.2 s move it.
    pub selected: Option<usize>,
    /// For undo: the next position, and the line as it was before each tap.
    history: Vec<((usize, usize), usize, TimedLine)>,
    pub changed: bool,
}

impl LyricTiming {
    /// Start from the song's lyrics: timed ones keep their times (to correct), plain ones are
    /// split into lines to tap. Word mode is on when the lyrics already time words.
    pub fn from_lyrics(lyrics: &Lyrics) -> Self {
        if lyrics.lines.is_empty() {
            return Self::from_text(&lyrics.plain);
        }
        let lines: Vec<TimedLine> = lyrics
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
            .collect();
        let words = lines.iter().any(|l| l.words.iter().any(|w| w.1.is_some()));
        Self {
            lines,
            words,
            ..Default::default()
        }
    }

    /// Start from words typed or pasted: one line per sung line, empty lines left out.
    pub fn from_text(text: &str) -> Self {
        Self {
            lines: text
                .lines()
                .map(str::trim)
                .filter(|l| !l.is_empty())
                .map(TimedLine::new)
                .collect(),
            ..Default::default()
        }
    }

    /// Every line has been tapped.
    pub fn done(&self) -> bool {
        self.next.0 >= self.lines.len()
    }

    /// Tap: stamp the next line or word at `position` seconds.
    pub fn tap(&mut self, position: f64) {
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

    /// Take back the last tap. False when there is none.
    pub fn undo(&mut self) -> bool {
        let Some((next, index, line)) = self.history.pop() else {
            return false;
        };
        self.lines[index] = line;
        self.next = next;
        self.selected = Some(index);
        true
    }

    /// Move the chosen line (the last tapped) by `delta` seconds.
    pub fn nudge(&mut self, delta: f64) {
        if let Some(line) = self.selected.and_then(|i| self.lines.get_mut(i)) {
            line.shift(delta);
            self.changed = true;
        }
    }

    /// Redo line: the next tap stamps the chosen line (the last tapped) again.
    pub fn redo_line(&mut self) {
        let index = self
            .selected
            .unwrap_or_else(|| self.next.0.saturating_sub(1));
        if index < self.lines.len() {
            self.next = (index, 0);
            self.selected = Some(index);
        }
    }

    /// Choose a line: the next tap stamps it, and −0.2 s / +0.2 s move it.
    pub fn select(&mut self, index: usize) {
        if index < self.lines.len() {
            self.next = (index, 0);
            self.selected = Some(index);
        }
    }

    /// Tap words instead of lines (or lines again), from the chosen line.
    pub fn set_words(&mut self, words: bool) {
        self.words = words;
        self.next = (self.selected.unwrap_or(0), 0);
    }

    /// The taps so far (Undo can take back this many).
    pub fn taps(&self) -> usize {
        self.history.len()
    }

    /// The lines to save, every gap filled in. None when nothing is stamped yet.
    pub fn finished(&self) -> Option<Vec<LyricLine>> {
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

    /// The finished lines as `.lrc` text; empty when nothing is stamped yet.
    pub fn lrc(&self) -> String {
        self.finished()
            .map(|lines| crate::media::to_lrc(&lines))
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn timing(lines: &[&str]) -> LyricTiming {
        LyricTiming {
            lines: lines.iter().map(|l| TimedLine::new(l)).collect(),
            ..Default::default()
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
    fn tapping_stamps_lines_then_words_and_fills_the_gaps() {
        let mut t = timing(&["Hold on tight", "Never let go", "Again"]);
        t.tap(1.0);
        t.tap(3.0);
        assert_eq!(t.next, (2, 0));
        let lines = t.finished().unwrap();
        assert_eq!(lines[2].time, 6.0);
        assert!(lines[0].words.is_empty());

        t.set_words(true);
        t.next = (0, 0);
        t.tap(1.2);
        t.tap(1.5);
        t.tap(1.9);
        assert_eq!(t.next, (1, 0));
        let lines = t.finished().unwrap();
        assert_eq!(
            lines[0].words.iter().map(|w| w.time).collect::<Vec<_>>(),
            vec![1.2, 1.5, 1.9]
        );
        assert!(t.undo());
        assert_eq!(t.next, (0, 2));
        assert_eq!(t.lines[0].words[2].1, None);
    }

    #[test]
    fn nudges_redo_and_the_lrc() {
        let mut t = LyricTiming::from_text("First line\n\n  Second line \n");
        assert_eq!(t.lines.len(), 2);
        assert!(t.finished().is_none());
        assert_eq!(t.lrc(), "");
        t.tap(10.0);
        t.nudge(-NUDGE);
        assert!((t.lines[0].time.unwrap() - 9.8).abs() < 1e-9);
        t.tap(12.0);
        assert!(t.done());
        // Redo line: the last line is tapped again.
        t.redo_line();
        assert_eq!(t.next, (1, 0));
        t.tap(12.5);
        assert_eq!(t.lines[1].time, Some(12.5));
        assert_eq!(t.lrc(), "[00:09.80]First line\n[00:12.50]Second line\n");
        assert_eq!(t.taps(), 3);
    }

    #[test]
    fn timed_lyrics_keep_their_times_to_correct() {
        let lyrics = Lyrics {
            lines: crate::media::parse_lrc(
                "[00:01.00]<00:01.00>One <00:01.50>two\n[00:03.00]Three",
            ),
            plain: String::new(),
            instrumental: false,
            source: crate::media::LyricsSource::Sidecar,
            provider: String::new(),
        };
        let t = LyricTiming::from_lyrics(&lyrics);
        assert!(t.words);
        assert_eq!(t.lines[0].words[1].1, Some(1.5));
        assert_eq!(t.lines[1].time, Some(3.0));
    }
}
