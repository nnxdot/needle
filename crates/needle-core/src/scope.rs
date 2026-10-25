//! What the visualizer sees: the last moment of sound on its way to the speakers, as a
//! spectrum. The playback keeps a short copy of the sound only while something asked for it in
//! the last two seconds, so nothing is spent on it otherwise.
use realfft::RealFftPlanner;
use std::sync::{
    LazyLock, Mutex,
    atomic::{AtomicI64, AtomicU32, Ordering},
};
use std::time::{SystemTime, UNIX_EPOCH};

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
/// ear hears them, each 0 (silent, -70 dB) to 1 (full). All zeros while nothing plays.
pub fn spectrum(bands: usize) -> Vec<f32> {
    SCOPE.asked.store(now_ms(), Ordering::Relaxed);
    let samples = recent();
    let rate = SCOPE.rate.load(Ordering::Relaxed).max(1) as f32;
    bands_of(&samples, rate, bands)
}

/// How loud what is playing now is, 0 to 1 (RMS).
pub fn level() -> f32 {
    SCOPE.asked.store(now_ms(), Ordering::Relaxed);
    let samples = recent();
    if samples.is_empty() {
        return 0.;
    }
    (samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32)
        .sqrt()
        .min(1.)
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
            ((db + 70.) / 70.).clamp(0., 1.)
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
}
