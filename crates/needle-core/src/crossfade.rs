//! Crossfade between queued songs without giving up the single playback queue.
//!
//! Song A is split in two. Its [`Head`] plays until `fade` seconds before the end. If by then
//! the next song has claimed A's ending, the head stops and the next song's [`Blend`] plays
//! A's last seconds fading out under its own start fading in. If nothing claimed it (the queue
//! ran out, or crossfade was turned off), A simply plays to the end. Positions stay per song:
//! the head is A from 0, the blend is B from 0.
use rodio::{Source, source::UniformSourceIterator};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicU8, Ordering},
    },
    time::Duration,
};

type Boxed = Box<dyn Source + Send>;

const OPEN: u8 = 0;
const CLAIMED: u8 = 1;
const PASSED: u8 = 2;
/// The song was skipped before its end: the next one starts clean, without its ending.
const SKIPPED: u8 = 3;

/// Song A's decoder, shared by its head and the next song's blend.
pub struct Ending {
    source: Mutex<Boxed>,
    state: AtomicU8,
    channels: u16,
    rate: u32,
    /// The frame where the ending starts.
    fade_at: u64,
    fade_frames: u64,
}

impl Ending {
    /// Let the next song take over A's ending. False if A is already past that point.
    pub fn claim(&self) -> bool {
        self.state
            .compare_exchange(OPEN, CLAIMED, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }
    /// The song was skipped: whatever is left of it must not fade under the next one.
    pub fn skip(&self) {
        self.state.store(SKIPPED, Ordering::Release);
    }
}

/// Song A until its ending, or to its real end when nothing takes the ending over.
pub struct Head {
    shared: Arc<Ending>,
    frame: u64,
    sample: u16,
}

/// Split `source` (of `duration` seconds) so its last `fade` seconds can be handed on.
pub fn split(source: Boxed, duration: f64, fade: f64) -> (Head, Arc<Ending>) {
    let (channels, rate) = (source.channels().max(1), source.sample_rate().max(1));
    let fade_frames = (fade * rate as f64) as u64;
    let fade_at = ((duration - fade).max(0.) * rate as f64) as u64;
    let shared = Arc::new(Ending {
        source: Mutex::new(source),
        state: AtomicU8::new(OPEN),
        channels,
        rate,
        fade_at,
        fade_frames,
    });
    (
        Head {
            shared: shared.clone(),
            frame: 0,
            sample: 0,
        },
        shared,
    )
}

impl Iterator for Head {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        if self.sample == 0 && self.frame >= self.shared.fade_at {
            // At the ending: stop if the next song took it; otherwise play on to the end.
            let state = match self.shared.state.compare_exchange(
                OPEN,
                PASSED,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => PASSED,
                Err(state) => state,
            };
            if state == CLAIMED {
                return None;
            }
        }
        let value = self.shared.source.lock().ok()?.next()?;
        self.sample += 1;
        if self.sample >= self.shared.channels {
            self.sample = 0;
            self.frame += 1;
        }
        Some(value)
    }
}

impl Source for Head {
    fn current_span_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> u16 {
        self.shared.channels
    }
    fn sample_rate(&self) -> u32 {
        self.shared.rate
    }
    fn total_duration(&self) -> Option<Duration> {
        None
    }
    fn try_seek(&mut self, pos: Duration) -> Result<(), rodio::source::SeekError> {
        let mut source =
            self.shared
                .source
                .lock()
                .map_err(|_| rodio::source::SeekError::NotSupported {
                    underlying_source: "crossfade head",
                })?;
        source.try_seek(pos)?;
        self.frame = (pos.as_secs_f64() * self.shared.rate as f64) as u64;
        self.sample = 0;
        // Seeking back before the ending reopens it, so the next song can take it again.
        if self.frame < self.shared.fade_at {
            let _ = self.shared.state.compare_exchange(
                PASSED,
                OPEN,
                Ordering::AcqRel,
                Ordering::Acquire,
            );
        }
        Ok(())
    }
}

/// A's ending as a source of its own, fading out.
struct Tail {
    shared: Arc<Ending>,
    frame: u64,
    sample: u16,
}

impl Iterator for Tail {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        if self.frame >= self.shared.fade_frames
            || self.shared.state.load(Ordering::Acquire) == SKIPPED
        {
            return None;
        }
        let value = self.shared.source.lock().ok()?.next()?;
        let t = self.frame as f32 / self.shared.fade_frames.max(1) as f32;
        self.sample += 1;
        if self.sample >= self.shared.channels {
            self.sample = 0;
            self.frame += 1;
        }
        // Equal-power curve: the sum stays about as loud through the blend.
        Some(value * (t * std::f32::consts::FRAC_PI_2).cos())
    }
}

impl Source for Tail {
    fn current_span_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> u16 {
        self.shared.channels
    }
    fn sample_rate(&self) -> u32 {
        self.shared.rate
    }
    fn total_duration(&self) -> Option<Duration> {
        None
    }
}

/// Song B, with A's ending mixed under its first seconds.
pub struct Blend {
    next: Boxed,
    tail: Option<UniformSourceIterator<Tail>>,
    channels: u16,
    rate: u32,
    frame: u64,
    sample: u16,
    fade_frames: u64,
}

/// Start `next` over the ending of the previous song.
pub fn blend(ending: Arc<Ending>, next: Boxed) -> Blend {
    let (channels, rate) = (next.channels().max(1), next.sample_rate().max(1));
    let fade_frames = (ending.fade_frames as f64 * rate as f64 / ending.rate as f64) as u64;
    let tail = Tail {
        shared: ending,
        frame: 0,
        sample: 0,
    };
    Blend {
        next,
        tail: Some(UniformSourceIterator::new(tail, channels, rate)),
        channels,
        rate,
        frame: 0,
        sample: 0,
        fade_frames,
    }
}

impl Iterator for Blend {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        let own = self.next.next();
        let under = match self.tail.as_mut() {
            Some(tail) => match tail.next() {
                Some(v) => Some(v),
                None => {
                    self.tail = None;
                    // Nothing to blend from the start (the last song was skipped): play at
                    // full strength straight away.
                    if self.frame == 0 && self.sample == 0 {
                        self.fade_frames = 0;
                    }
                    None
                }
            },
            None => None,
        };
        let fading_in = self.frame < self.fade_frames;
        let own_gain = if fading_in {
            let t = self.frame as f32 / self.fade_frames.max(1) as f32;
            (t * std::f32::consts::FRAC_PI_2).sin()
        } else {
            1.
        };
        self.sample += 1;
        if self.sample >= self.channels {
            self.sample = 0;
            self.frame += 1;
        }
        match (own, under) {
            (None, None) => None,
            (own, under) => Some(own.unwrap_or(0.) * own_gain + under.unwrap_or(0.)),
        }
    }
}

impl Source for Blend {
    fn current_span_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> u16 {
        self.channels
    }
    fn sample_rate(&self) -> u32 {
        self.rate
    }
    fn total_duration(&self) -> Option<Duration> {
        self.next.total_duration()
    }
    fn try_seek(&mut self, pos: Duration) -> Result<(), rodio::source::SeekError> {
        // Seeking within B drops what is left of A.
        self.tail = None;
        self.frame = self.fade_frames;
        self.next.try_seek(pos)
    }
}

/// Smart mixes: where a song's music really ends (before a silent or fading tail), and a
/// fade that lasts a whole number of bars at its tempo. `fade` is the crossfade setting,
/// `duration` the song's length, `shape` its waveform, `bpm` its tempo when known. Returns
/// (where the ending ends, how long it fades), in seconds.
pub fn smart_ending(
    fade: f64,
    duration: f64,
    shape: Option<&crate::waveform::Waveform>,
    bpm: Option<f64>,
) -> (f64, f64) {
    let mut end = duration;
    if let Some(shape) = shape.filter(|s| !s.loudness.is_empty()) {
        let mut sorted: Vec<u8> = shape.loudness.iter().copied().filter(|&v| v > 0).collect();
        sorted.sort_unstable();
        if let Some(&median) = sorted.get(sorted.len() / 2) {
            let quiet = (median as f64 * 0.25).max(1.);
            let bins = shape.loudness.len();
            if let Some(last) = shape.loudness.iter().rposition(|&v| v as f64 >= quiet) {
                let music_end = (last + 1) as f64 / bins as f64 * duration;
                // A second or more of near silence at the end is left out; never more than
                // half the song.
                if duration - music_end >= 1. {
                    end = music_end.max(duration * 0.5);
                }
            }
        }
    }
    let mut length = fade;
    if let Some(bpm) = bpm.filter(|b| (60.0..=200.0).contains(b)) {
        let bar = 4. * 60. / bpm;
        let bars = (fade / bar).round().max(1.);
        let barred = bars * bar;
        if (fade * 0.5..=fade * 1.6).contains(&barred) {
            length = barred;
        }
    }
    (end, length.min(end * 0.4))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rodio::buffer::SamplesBuffer;

    #[test]
    fn smart_endings_leave_out_the_silent_tail_and_fade_in_bars() {
        let mut loudness = vec![200u8; 240];
        // The last 24 parts (a tenth of a 100 s song) are near silent.
        loudness[216..].iter_mut().for_each(|v| *v = 3);
        let shape = crate::waveform::Waveform {
            version: 1,
            peaks: loudness.clone(),
            loudness,
        };
        let (end, fade) = smart_ending(6., 100., Some(&shape), Some(120.));
        assert!((end - 90.).abs() < 0.5, "{end}");
        // At 120 BPM a bar is 2 s: 6 s is three bars.
        assert!((fade - 6.).abs() < 1e-9);
        let (_, fade) = smart_ending(5., 100., None, Some(128.));
        assert!((fade - 3.75).abs() < 1e-9, "two bars at 128 BPM: {fade}");
        assert_eq!(smart_ending(5., 100., None, None), (100., 5.));
    }

    fn tone(value: f32, seconds: f32) -> Boxed {
        Box::new(SamplesBuffer::new(
            1,
            100,
            vec![value; (seconds * 100.) as usize],
        ))
    }

    #[test]
    fn hands_the_ending_to_the_next_song() {
        let (head, ending) = split(tone(1., 10.), 10., 2.);
        assert!(ending.claim());
        let head: Vec<f32> = head.collect();
        assert_eq!(head.len(), 800, "A stops two seconds early");
        let blended: Vec<f32> = blend(ending, tone(1., 5.)).collect();
        assert_eq!(blended.len(), 500, "B keeps its own length");
        // Through the blend the level stays near full (equal power); at the start it is all A.
        assert!((blended[0] - 1.).abs() < 0.01);
        assert!(blended[100] > 0.9 && blended[100] < 1.5);
        assert_eq!(blended[300], 1.);
    }

    #[test]
    fn a_skipped_song_does_not_fade_under_the_next() {
        let (head, ending) = split(tone(1., 10.), 10., 2.);
        assert!(ending.claim());
        // A plays a second, then Next skips it.
        let _: Vec<f32> = head.take(100).collect();
        ending.skip();
        let next: Vec<f32> = blend(ending, tone(0.5, 5.)).collect();
        assert_eq!(next.len(), 500);
        // Nothing of A, and B starts at its full level, not faded in.
        assert!(
            next.iter().all(|v| (*v - 0.5).abs() < 1e-6),
            "{:?}",
            &next[..5]
        );
    }

    #[test]
    fn plays_to_the_end_when_nothing_takes_over() {
        let (head, ending) = split(tone(1., 10.), 10., 2.);
        let played: Vec<f32> = head.collect();
        assert_eq!(played.len(), 1000);
        assert!(
            !ending.claim(),
            "too late to take over once A is past its ending"
        );
    }
}
