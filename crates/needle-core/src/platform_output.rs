//! An output an app brings itself, such as Needle's own USB DAC driver on Android. While one
//! is set, Needle hands it the music as it is decoded: each song at its own rate and channels,
//! with no resampling, volume, equalizer, or crossfade, so the samples stay bit-perfect (as
//! WASAPI exclusive output on Windows gets them). DSD songs come as DoP when the output asks
//! for it. When the output fails, Needle plays through the system's output again.
use crate::audio::PlaybackState;
use anyhow::Result;
use rodio::{Sink, Source, queue::SourcesQueueOutput};
use std::{
    sync::{
        Arc, Mutex, Weak,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

/// An output an app brings. Its `render` runs on a thread of its own for as long as the output
/// is open, pulling the music from the feed.
pub trait PlatformOutput: Send + Sync {
    /// The name the player shows, such as "FiiO BTR17 (USB)".
    fn name(&self) -> String;
    /// Send DSD as DoP (DSD over PCM: 24-bit frames at a sixteenth of the DSD rate, with
    /// markers) rather than turned into PCM. Only for a DAC that takes DoP.
    fn dop(&self) -> bool {
        false
    }
    /// Play the feed until it says to stop. Return an error when the device cannot play (it
    /// was unplugged, it refused the format); Needle then plays through the system's output.
    fn render(&self, feed: &mut Feed) -> Result<()>;
    /// The listener moved Needle's volume (0 to 1): the output may set the DAC's own volume.
    /// The samples it gets are never made quieter.
    fn set_volume(&self, _volume: f32) {}
}

static OUTPUT: Mutex<Option<Arc<dyn PlatformOutput>>> = Mutex::new(None);
/// Set when the output in `OUTPUT` failed, so Needle does not open it again until it is set
/// anew.
static FAILED: AtomicBool = AtomicBool::new(false);
/// Bumped when the output changes, so the playback reopens.
pub(crate) static CHANGED: AtomicBool = AtomicBool::new(false);

/// Use `output` for playback from the next song on (the playing song moves over at once), or
/// none to play through the system's output again.
pub fn set_platform_output(output: Option<Arc<dyn PlatformOutput>>) {
    *OUTPUT.lock().unwrap_or_else(|e| e.into_inner()) = output;
    FAILED.store(false, Ordering::SeqCst);
    CHANGED.store(true, Ordering::SeqCst);
}

/// The output to use now: set and not failed.
pub(crate) fn active() -> Option<Arc<dyn PlatformOutput>> {
    if FAILED.load(Ordering::SeqCst) {
        return None;
    }
    OUTPUT.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

/// Whether DSD should be decoded as DoP now.
pub(crate) fn dop() -> bool {
    active().is_some_and(|o| o.dop())
}

/// The music for a platform output, and what the playback is doing.
pub struct Feed {
    source: SourcesQueueOutput,
    sink: Weak<Sink>,
    stop: Arc<AtomicBool>,
    state: Arc<Mutex<PlaybackState>>,
    name: String,
    /// A sample read to learn the next song's format, not yet given out.
    held: Option<f32>,
}

/// The format of what the feed gives now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FeedFormat {
    pub rate: u32,
    pub channels: u16,
}

impl Feed {
    /// Whether to stop rendering (the output was closed, or Needle is quitting).
    pub fn stopped(&self) -> bool {
        self.stop.load(Ordering::Relaxed) || self.sink.upgrade().is_none()
    }

    /// Whether there is music to play now: not paused, and something queued. While this is
    /// false the output should be idle (it may close the device) and call [`Feed::idle`].
    pub fn playing(&self) -> bool {
        self.sink
            .upgrade()
            .is_some_and(|s| !s.is_paused() && !s.empty())
    }

    /// Call about every 10 ms while not playing: it lets seeks and skips made while paused
    /// finish.
    pub fn idle(&mut self) {
        if let Some(sink) = self.sink.upgrade()
            && sink.is_paused()
            && !sink.empty()
        {
            let samples =
                self.source.sample_rate() as usize * self.source.channels() as usize / 100;
            for _ in 0..samples.max(1) {
                let _ = self.source.next();
            }
        }
        std::thread::sleep(Duration::from_millis(10));
    }

    /// The format of the next samples. When it changes (a song at another rate), the output
    /// sets the DAC to the new one before reading on.
    pub fn format(&mut self) -> FeedFormat {
        if self.held.is_none() {
            self.held = self.source.next();
        }
        FeedFormat {
            rate: self.source.sample_rate(),
            channels: self.source.channels(),
        }
    }

    /// Fill `samples` (interleaved, -1 to 1, exact for 16- and 24-bit music) in the current
    /// format. Returns how many were filled: fewer when the format changes (call `format`
    /// again), the rest of `samples` left as silence.
    pub fn read(&mut self, samples: &mut [f32]) -> usize {
        let format = self.format();
        for (filled, slot) in samples.iter_mut().enumerate() {
            let sample = match self.held.take() {
                Some(sample) => Some(sample),
                None => self.source.next(),
            };
            if self.source.sample_rate() != format.rate || self.source.channels() != format.channels
            {
                self.held = sample;
                samples[filled..].fill(0.);
                return filled;
            }
            *slot = sample.unwrap_or(0.);
        }
        samples.len()
    }

    /// Tell the player what the output plays now, for its signal path ("→ USB 96 kHz").
    pub fn playing_at(&self, format: FeedFormat) {
        if let Ok(mut state) = self.state.lock() {
            state.output = self.name.clone();
            state.output_rate = format.rate;
            state.output_channels = format.channels;
            state.exclusive = true;
        }
    }
}

/// A running platform output: its thread, and whether it failed and why.
pub(crate) struct Running {
    stop: Arc<AtomicBool>,
    pub failed: Arc<AtomicBool>,
    /// What to tell the listener when it failed. The player shows it once it plays on through
    /// the system's output: opening that output clears the notice, so it cannot be set here.
    pub failure: Arc<Mutex<Option<String>>>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Running {
    pub(crate) fn start(
        output: Arc<dyn PlatformOutput>,
        source: SourcesQueueOutput,
        sink: Weak<Sink>,
        state: Arc<Mutex<PlaybackState>>,
    ) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let failed = Arc::new(AtomicBool::new(false));
        let failure = Arc::new(Mutex::new(None));
        let (stop_worker, failed_worker, failure_worker) =
            (stop.clone(), failed.clone(), failure.clone());
        let thread = std::thread::Builder::new()
            .name("needle-platform-output".into())
            .spawn(move || {
                let name = output.name();
                let mut feed = Feed {
                    source,
                    sink,
                    stop: stop_worker,
                    state: state.clone(),
                    name: name.clone(),
                    held: None,
                };
                if let Err(error) = output.render(&mut feed) {
                    crate::logfile::warn(format!("{name}: {error:#}"));
                    FAILED.store(true, Ordering::SeqCst);
                    CHANGED.store(true, Ordering::SeqCst);
                    if let Ok(mut failure) = failure_worker.lock() {
                        *failure = Some(format!(
                            "{name} stopped ({error}). Playing through the phone instead."
                        ));
                    }
                    failed_worker.store(true, Ordering::SeqCst);
                }
            })
            .ok();
        Self {
            stop,
            failed,
            failure,
            thread,
        }
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// A sample as a whole number of `bits` (16, 24, or 32), exact for music of that depth or
/// less: 16-bit music in a 24-bit slot is the same music shifted up.
pub fn to_int(sample: f32, bits: u32) -> i32 {
    let sample = if sample.is_finite() {
        sample as f64
    } else {
        0.
    };
    let bits = bits.clamp(8, 32);
    // f32 holds 24 bits exactly: 32-bit output is 24-bit music in the top bytes.
    let used = bits.min(24);
    let scale = (1u64 << (used - 1)) as f64;
    let value = (sample * scale).round().clamp(-scale, scale - 1.) as i64;
    (value << (bits - used)) as i32
}

/// A sample as little-endian 24-bit PCM in a slot of 2, 3, or 4 bytes (left-aligned in 4).
pub fn pcm24(sample: f32, bytes: &mut [u8]) {
    let raw = (to_int(sample, 24) << 8).to_le_bytes();
    match bytes.len() {
        4 => bytes.copy_from_slice(&raw),
        3 => bytes.copy_from_slice(&raw[1..]),
        2 => bytes.copy_from_slice(&raw[2..]),
        _ => bytes.fill(0),
    }
}

/// DoP's markers: alternating each frame, in the top byte of each 24-bit sample.
const DOP_MARKERS: [u8; 2] = [0x05, 0xFA];

/// One DoP sample: a marker and two bytes of DSD (oldest bit highest), as a 24-bit sample
/// that 24-bit PCM output carries exactly.
pub(crate) fn dop_sample(frame: u64, high: u8, low: u8) -> f32 {
    let raw = (DOP_MARKERS[(frame & 1) as usize] as i32) << 16 | (high as i32) << 8 | low as i32;
    let signed = if raw >= 1 << 23 { raw - (1 << 24) } else { raw };
    signed as f32 / 8_388_608.
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whole_numbers_are_exact() {
        assert_eq!(to_int(-1., 16), i16::MIN as i32);
        assert_eq!(to_int(12345. / 32768., 16), 12345);
        assert_eq!(to_int(12345. / 32768., 24), 12345 << 8);
        assert_eq!(to_int(-8_388_608. / 8_388_608., 32), i32::MIN);
        assert_eq!(to_int(1., 16), i16::MAX as i32);
    }

    #[test]
    fn dop_samples_survive_24_bit_output() {
        for (frame, high, low) in [
            (0u64, 0x69u8, 0x96u8),
            (1, 0xFF, 0x00),
            (2, 0, 0),
            (3, 0xAB, 0xCD),
        ] {
            let sample = dop_sample(frame, high, low);
            let mut bytes = [0u8; 3];
            pcm24(sample, &mut bytes);
            assert_eq!(
                bytes,
                [low, high, DOP_MARKERS[(frame & 1) as usize]],
                "frame {frame}"
            );
        }
    }
}
