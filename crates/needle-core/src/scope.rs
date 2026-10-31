//! What the visualizer sees: the last moment of sound on its way to the speakers, as a
//! spectrum. The playback keeps a short copy of the sound only while something asked for it in
//! the last two seconds, so nothing is spent on it otherwise.
use realfft::RealFftPlanner;
use std::collections::HashMap;
use std::sync::{
    LazyLock, Mutex,
    atomic::{AtomicI64, AtomicU32, Ordering},
};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

/// How long a band takes to rise most of the way to a louder sound, and to fall back.
const RISE: f32 = 0.07;
const FALL: f32 = 0.35;
/// The quietest and loudest a band shows (dB below a full-scale tone): 0 and 1.
const FLOOR_DB: f32 = -60.;
const TOP_DB: f32 = -6.;

/// Samples kept: about a twentieth of a second at 44.1 kHz, enough for a 2048-point spectrum.
const KEPT: usize = 2048;

struct Scope {
    /// Mono samples, oldest first.
    samples: Mutex<Vec<f32>>,
    rate: AtomicU32,
    /// When a visualizer last asked, and when sound last came (milliseconds since 1970).
    asked: AtomicI64,
    pushed: AtomicI64,
}

/// The values a visualizer was shown last, and when.
type Shown = (Vec<f32>, Instant);

/// What each visualizer was shown last, by its number of bands (0 for the level), and when:
/// each new reading moves toward the sound, quickly up and slowly down, by the time passed.
static SHOWN: LazyLock<Mutex<HashMap<usize, Shown>>> = LazyLock::new(Default::default);

static SCOPE: LazyLock<Scope> = LazyLock::new(|| Scope {
    samples: Mutex::new(Vec::with_capacity(KEPT * 2)),
    rate: AtomicU32::new(44_100),
    asked: AtomicI64::new(0),
    pushed: AtomicI64::new(0),
});

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Whether a visualizer is looking (asked in the last two seconds).
pub(crate) fn wanted() -> bool {
    now_ms() - SCOPE.asked.load(Ordering::Relaxed) < 2000
}

/// The playback hands over a piece of sound it just played, as mono samples.
pub(crate) fn push(mono: &[f32], rate: u32) {
    SCOPE.rate.store(rate, Ordering::Relaxed);
    SCOPE.pushed.store(now_ms(), Ordering::Relaxed);
    let mut samples = SCOPE.samples.lock().unwrap_or_else(|e| e.into_inner());
    samples.extend_from_slice(mono);
    if samples.len() > KEPT {
        let extra = samples.len() - KEPT;
        samples.drain(..extra);
    }
}

/// The sound kept, or nothing when none came in the last quarter second (paused, stopped).
fn recent() -> Vec<f32> {
    if now_ms() - SCOPE.pushed.load(Ordering::Relaxed) > 250 {
        return vec![];
    }
    SCOPE
        .samples
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
}

/// The spectrum of what is playing now, in `bands` bands from 30 Hz to 16 kHz spaced as the
/// ear hears them, each 0 (quiet, -60 dB) to 1 (loud), eased: a band rises over about 70 ms
/// and falls over about 350 ms, so it moves with the music without flickering. It falls to
/// zeros when nothing plays.
pub fn spectrum(bands: usize) -> Vec<f32> {
    SCOPE.asked.store(now_ms(), Ordering::Relaxed);
    let samples = recent();
    let rate = SCOPE.rate.load(Ordering::Relaxed).max(1) as f32;
    eased(bands, bands_of(&samples, rate, bands))
}

/// How loud what is playing now is, 0 to 1 (RMS), eased as the spectrum is.
pub fn level() -> f32 {
    SCOPE.asked.store(now_ms(), Ordering::Relaxed);
    let samples = recent();
    let now = if samples.is_empty() {
        0.
    } else {
        (samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32)
            .sqrt()
            .min(1.)
    };
    eased(0, vec![now])[0]
}

/// `now` eased from what the visualizer with this `key` was shown last.
fn eased(key: usize, now: Vec<f32>) -> Vec<f32> {
    let at = Instant::now();
    let mut shown = SHOWN.lock().unwrap_or_else(|e| e.into_inner());
    let (held, last) = shown
        .entry(key)
        .or_insert_with(|| (vec![0.; now.len()], at));
    let seconds = at.duration_since(*last).as_secs_f32().min(0.5);
    *last = at;
    if held.len() != now.len() {
        *held = vec![0.; now.len()];
    }
    ease(held, &now, seconds);
    held.clone()
}

/// Move `held` toward `now` over `seconds`: up quickly, down slowly.
fn ease(held: &mut [f32], now: &[f32], seconds: f32) {
    let rise = 1. - (-seconds / RISE).exp();
    let fall = 1. - (-seconds / FALL).exp();
    for (h, v) in held.iter_mut().zip(now) {
        let k = if *v > *h { rise } else { fall };
        *h += (v - *h) * k;
    }
}

fn bands_of(samples: &[f32], rate: f32, bands: usize) -> Vec<f32> {
    if bands == 0 {
        return vec![];
    }
    if samples.len() < KEPT {
        return vec![0.; bands];
    }
    let samples = &samples[samples.len() - KEPT..];
    let mut planner = RealFftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(KEPT);
    let mut input: Vec<f32> = samples
        .iter()
        .enumerate()
        .map(|(i, s)| s * (0.5 - 0.5 * (std::f32::consts::TAU * i as f32 / KEPT as f32).cos()))
        .collect();
    let mut output = fft.make_output_vec();
    if fft.process(&mut input, &mut output).is_err() {
        return vec![0.; bands];
    }
    let bin_hz = rate / KEPT as f32;
    let (low, high) = (30f32, 16_000f32.min(rate / 2.));
    (0..bands)
        .map(|b| {
            let from = low * (high / low).powf(b as f32 / bands as f32);
            let to = low * (high / low).powf((b + 1) as f32 / bands as f32);
            let first = ((from / bin_hz) as usize).max(1);
            let last = ((to / bin_hz) as usize).max(first + 1).min(output.len());
            let power = output[first..last]
                .iter()
                .map(|c| c.norm_sqr())
                .fold(0f32, f32::max);
            // A full-scale sine's peak bin is about (KEPT/4)²; that is 0 dB.
            let db = 10. * (power / (KEPT as f32 / 4.).powi(2)).max(1e-12).log10();
            ((db - FLOOR_DB) / (TOP_DB - FLOOR_DB)).clamp(0., 1.)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tone_lights_its_own_band() {
        let rate = 44_100.;
        let tone: Vec<f32> = (0..KEPT)
            .map(|i| 0.5 * (std::f32::consts::TAU * 1000. * i as f32 / rate).sin())
            .collect();
        let bands = bands_of(&tone, rate, 24);
        let loudest = bands
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(b.1))
            .unwrap()
            .0;
        // 1 kHz sits about halfway up a 30 Hz – 16 kHz scale spaced as the ear hears.
        assert!((11..=14).contains(&loudest), "{loudest}: {bands:?}");
        assert!(bands[loudest] > 0.8);
        assert!(bands[0] < 0.3);
        assert_eq!(bands_of(&[], rate, 8), vec![0.; 8]);
    }

    #[test]
    fn bands_rise_quickly_and_fall_slowly() {
        let mut held = [0.];
        // A thirtieth of a second: most of the way up, a little way down.
        ease(&mut held, &[1.], 1. / 30.);
        assert!(held[0] > 0.3 && held[0] < 0.5, "{held:?}");
        ease(&mut held, &[1.], 0.2);
        assert!(held[0] > 0.95);
        ease(&mut held, &[0.], 1. / 30.);
        assert!(held[0] > 0.85, "{held:?}");
        ease(&mut held, &[0.], 1.);
        assert!(held[0] < 0.1);
    }
}
