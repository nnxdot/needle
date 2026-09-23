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
        if self.frame >= self.shared.fade_frames {
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
        let fading_in = self.frame < self.fade_frames;
        let own = self.next.next();
        let under = match self.tail.as_mut() {
            Some(tail) => match tail.next() {
                Some(v) => Some(v),
                None => {
                    self.tail = None;
                    None
                }
            },
            None => None,
        };
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

#[cfg(test)]
mod tests {
    use super::*;
    use rodio::buffer::SamplesBuffer;

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
