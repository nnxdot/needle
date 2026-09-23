//! Sound tools for shared output: a 10-band graphic equalizer with preamp, balance, mono,
//! headphone crossfeed, and effects from plugins. Exclusive output never passes through here.
//!
//! Settings change live: the playing source re-reads them every few milliseconds.
use rodio::Source;
use serde::{Deserialize, Serialize};
use std::{
    f64::consts::PI,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

/// Centre frequencies of the graphic equalizer, one octave apart.
pub const BANDS: [f64; 10] = [
    31., 62., 125., 250., 500., 1000., 2000., 4000., 8000., 16000.,
];
pub const MAX_GAIN_DB: f32 = 12.;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Dsp {
    pub eq: bool,
    pub preamp_db: f32,
    /// Gain per band in dB, matching [`BANDS`].
    pub bands: [f32; 10],
    /// −1 is fully left, +1 fully right.
    pub balance: f32,
    pub mono: bool,
    /// Blend a little of each channel into the other, as speakers do, for easier headphone listening.
    pub crossfeed: bool,
    pub preset: String,
    /// Effects from plugins, in the order they play.
    pub effects: Vec<crate::effects::EffectSlot>,
}

impl Default for Dsp {
    fn default() -> Self {
        Self {
            eq: false,
            preamp_db: 0.,
            bands: [0.; 10],
            balance: 0.,
            mono: false,
            crossfeed: false,
            preset: "Flat".into(),
            effects: vec![],
        }
    }
}

impl Dsp {
    /// True when nothing would change the signal, so playback can skip processing entirely.
    pub fn is_transparent(&self) -> bool {
        let eq_flat = !self.eq || (self.preamp_db == 0. && self.bands.iter().all(|b| *b == 0.));
        eq_flat
            && self.balance == 0.
            && !self.mono
            && !self.crossfeed
            && !self.effects.iter().any(|e| e.on)
    }
    /// The preamp that keeps the loudest boosted band from clipping.
    pub fn suggested_preamp(&self) -> f32 {
        -self.bands.iter().copied().fold(0., f32::max)
    }
}

/// Built-in equalizer curves: name, preamp, and band gains.
pub const PRESETS: &[(&str, f32, [f32; 10])] = &[
    ("Flat", 0., [0., 0., 0., 0., 0., 0., 0., 0., 0., 0.]),
    ("Bass boost", -6., [6., 5., 4., 2., 0., 0., 0., 0., 0., 0.]),
    (
        "Treble boost",
        -5.,
        [0., 0., 0., 0., 0., 0., 1., 3., 4., 5.],
    ),
    ("Loudness", -5., [5., 4., 2., 0., -1., -1., 0., 2., 3., 4.]),
    ("Vocal", -4., [-2., -2., -1., 0., 2., 4., 4., 2., 0., -1.]),
    ("Rock", -4., [4., 3., 1., -1., -2., -1., 1., 3., 4., 4.]),
    ("Electronic", -5., [5., 4., 1., 0., -2., 1., 0., 1., 4., 5.]),
    ("Classical", -3., [3., 2., 1., 0., 0., 0., -1., -1., 1., 3.]),
    (
        "Spoken word",
        -3.,
        [-4., -3., -1., 1., 3., 3., 2., 1., -1., -3.],
    ),
    (
        "Headphones",
        -4.,
        [3., 3., 1., 0., -1., 0., 1., 2., 0., -2.],
    ),
];

/// Settings shared with the playing sources.
#[derive(Default)]
pub struct DspControl {
    settings: Mutex<Dsp>,
    version: AtomicU64,
    effects: Arc<crate::effects::Registry>,
}
impl DspControl {
    pub fn new(settings: Dsp, effects: Arc<crate::effects::Registry>) -> Arc<Self> {
        Arc::new(Self {
            settings: Mutex::new(settings),
            version: AtomicU64::new(1),
            effects,
        })
    }
    pub fn set(&self, settings: Dsp) {
        *self.settings.lock().unwrap_or_else(|p| p.into_inner()) = settings;
        self.version.fetch_add(1, Ordering::Release);
    }
    pub fn get(&self) -> Dsp {
        self.settings
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }
}

/// RBJ "Audio EQ Cookbook" peaking filter, transposed direct form II.
#[derive(Clone, Copy, Default)]
struct Biquad {
    b0: f64,
    b1: f64,
    b2: f64,
    a1: f64,
    a2: f64,
}
impl Biquad {
    fn peaking(rate: f64, frequency: f64, gain_db: f64, q: f64) -> Self {
        let a = 10f64.powf(gain_db / 40.);
        let w = 2. * PI * frequency / rate;
        let alpha = w.sin() / (2. * q);
        let a0 = 1. + alpha / a;
        Self {
            b0: (1. + alpha * a) / a0,
            b1: -2. * w.cos() / a0,
            b2: (1. - alpha * a) / a0,
            a1: -2. * w.cos() / a0,
            a2: (1. - alpha / a) / a0,
        }
    }
    fn run(&self, state: &mut [f64; 2], x: f64) -> f64 {
        let y = self.b0 * x + state[0];
        state[0] = self.b1 * x - self.a1 * y + state[1];
        state[1] = self.b2 * x - self.a2 * y;
        y
    }
}

/// The processing state for one source: filter coefficients and per-channel history.
pub struct Chain {
    rate: f64,
    channels: usize,
    settings: Dsp,
    filters: Vec<Biquad>,
    state: Vec<[f64; 2]>,
    preamp: f64,
    crossfeed_lp: [f64; 2],
    crossfeed_a: f64,
}
impl Chain {
    pub fn new(settings: Dsp, rate: u32, channels: u16) -> Self {
        let mut chain = Self {
            rate: rate.max(1) as f64,
            channels: channels.max(1) as usize,
            settings: Dsp::default(),
            filters: vec![],
            state: vec![],
            preamp: 1.,
            crossfeed_lp: [0.; 2],
            // One-pole low-pass near 700 Hz for the blended opposite channel.
            crossfeed_a: (-2. * PI * 700. / rate.max(1) as f64).exp(),
        };
        chain.configure(settings);
        chain
    }
    pub fn configure(&mut self, settings: Dsp) {
        let active = settings.eq;
        let old_filters = self.filters.len();
        self.filters = if active {
            BANDS
                .iter()
                .zip(settings.bands)
                .filter(|(f, g)| **f < self.rate * 0.45 && *g != 0.)
                .map(|(f, g)| {
                    Biquad::peaking(
                        self.rate,
                        *f,
                        g.clamp(-MAX_GAIN_DB, MAX_GAIN_DB) as f64,
                        1.41,
                    )
                })
                .collect()
        } else {
            vec![]
        };
        // Keep filter history when only gains change, so adjusting a slider does not click.
        if self.filters.len() != old_filters {
            self.state = vec![[0.; 2]; self.filters.len() * self.channels];
        }
        self.preamp = if active {
            10f64.powf(settings.preamp_db.clamp(-24., 12.) as f64 / 20.)
        } else {
            1.
        };
        self.settings = settings;
    }
    /// Process one interleaved frame in place, and keep it within full scale.
    pub fn frame(&mut self, frame: &mut [f32]) {
        self.shape(frame);
        for sample in frame.iter_mut() {
            *sample = sample.clamp(-1., 1.);
        }
    }
    /// The equalizer and listening tools, without the final limit.
    fn shape(&mut self, frame: &mut [f32]) {
        for (channel, sample) in frame.iter_mut().enumerate() {
            let mut x = *sample as f64 * self.preamp;
            for (band, filter) in self.filters.iter().enumerate() {
                x = filter.run(&mut self.state[band * self.channels + channel], x);
            }
            *sample = x as f32;
        }
        if frame.len() >= 2 {
            let (mut l, mut r) = (frame[0] as f64, frame[1] as f64);
            if self.settings.crossfeed {
                let a = self.crossfeed_a;
                self.crossfeed_lp[0] = self.crossfeed_lp[0] * a + r * (1. - a);
                self.crossfeed_lp[1] = self.crossfeed_lp[1] * a + l * (1. - a);
                let level = 0.3;
                l = (l + level * self.crossfeed_lp[0]) / (1. + level);
                r = (r + level * self.crossfeed_lp[1]) / (1. + level);
            }
            if self.settings.mono {
                let m = (l + r) * 0.5;
                (l, r) = (m, m);
            }
            let balance = self.settings.balance.clamp(-1., 1.) as f64;
            l *= (1. - balance).min(1.);
            r *= (1. + balance).min(1.);
            frame[0] = l as f32;
            frame[1] = r as f32;
        }
    }
}

/// A source wrapper that runs [`Chain`] and the plugin effects, and follows live changes.
/// It works a block at a time, since effects written in WebAssembly process blocks.
pub struct Processed<S: Source> {
    inner: S,
    control: Arc<DspControl>,
    version: (u64, u64),
    chain: Chain,
    rack: crate::effects::Rack,
    channels: usize,
    block: Vec<f32>,
    position: usize,
}
impl<S: Source> Processed<S> {
    pub fn new(inner: S, control: Arc<DspControl>) -> Self {
        let settings = control.get();
        let chain = Chain::new(settings.clone(), inner.sample_rate(), inner.channels());
        let mut rack = crate::effects::Rack::new(
            control.effects.clone(),
            inner.sample_rate(),
            inner.channels(),
        );
        rack.sync(&settings.effects);
        let channels = inner.channels().max(1) as usize;
        Self {
            version: (
                control.version.load(Ordering::Acquire),
                control.effects.version(),
            ),
            inner,
            control,
            chain,
            rack,
            channels,
            block: Vec::with_capacity(crate::effects::BLOCK_FRAMES * channels),
            position: 0,
        }
    }
    fn refill(&mut self) -> bool {
        let version = (
            self.control.version.load(Ordering::Acquire),
            self.control.effects.version(),
        );
        if version != self.version {
            self.version = version;
            let settings = self.control.get();
            self.chain.configure(settings.clone());
            self.rack.sync(&settings.effects);
        }
        self.block.clear();
        self.position = 0;
        'frames: for _ in 0..crate::effects::BLOCK_FRAMES {
            let start = self.block.len();
            for _ in 0..self.channels {
                match self.inner.next() {
                    Some(sample) => self.block.push(sample),
                    None => {
                        self.block.truncate(start);
                        break 'frames;
                    }
                }
            }
            self.chain.shape(&mut self.block[start..]);
        }
        if !self.rack.is_empty() {
            self.rack.process(&mut self.block);
        }
        for sample in self.block.iter_mut() {
            *sample = sample.clamp(-1., 1.);
        }
        !self.block.is_empty()
    }
}
impl<S: Source> Iterator for Processed<S> {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        if self.position >= self.block.len() && !self.refill() {
            return None;
        }
        let sample = self.block[self.position];
        self.position += 1;
        Some(sample)
    }
}
impl<S: Source> Source for Processed<S> {
    fn current_span_len(&self) -> Option<usize> {
        // Blocks run ahead of the source's own spans; the format does not change mid-song.
        None
    }
    fn channels(&self) -> u16 {
        self.inner.channels()
    }
    fn sample_rate(&self) -> u32 {
        self.inner.sample_rate()
    }
    fn total_duration(&self) -> Option<Duration> {
        self.inner.total_duration()
    }
    fn try_seek(&mut self, pos: Duration) -> Result<(), rodio::source::SeekError> {
        self.block.clear();
        self.position = 0;
        self.rack.reset();
        self.inner.try_seek(pos)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(frequency: f64, rate: f64, frames: usize) -> Vec<f32> {
        (0..frames)
            .flat_map(|i| {
                let s = (2. * PI * frequency * i as f64 / rate).sin() as f32 * 0.25;
                [s, s]
            })
            .collect()
    }
    fn run(settings: Dsp, input: &[f32]) -> Vec<f32> {
        let mut chain = Chain::new(settings, 48000, 2);
        let mut out = input.to_vec();
        for frame in out.chunks_mut(2) {
            chain.frame(frame);
        }
        out
    }
    fn rms(samples: &[f32]) -> f64 {
        let tail = &samples[samples.len() / 2..];
        (tail.iter().map(|s| (*s as f64).powi(2)).sum::<f64>() / tail.len() as f64).sqrt()
    }

    #[test]
    fn defaults_are_transparent() {
        let input = sine(1000., 48000., 4800);
        assert!(Dsp::default().is_transparent());
        assert_eq!(run(Dsp::default(), &input), input);
        let flat_eq = Dsp {
            eq: true,
            ..Default::default()
        };
        assert!(flat_eq.is_transparent());
        assert_eq!(run(flat_eq, &input), input);
    }

    #[test]
    fn a_band_boosts_its_own_frequency_and_not_distant_ones() {
        let mut settings = Dsp {
            eq: true,
            ..Default::default()
        };
        settings.bands[5] = 6.; // 1 kHz
        let at_band = rms(&run(settings.clone(), &sine(1000., 48000., 48000)))
            / rms(&sine(1000., 48000., 48000));
        let far = rms(&run(settings, &sine(62., 48000., 48000))) / rms(&sine(62., 48000., 48000));
        assert!(
            (at_band - 2.0).abs() < 0.05,
            "+6 dB should double amplitude, got {at_band}"
        );
        assert!(
            (far - 1.0).abs() < 0.03,
            "a distant band should be nearly untouched, got {far}"
        );
    }

    #[test]
    fn preamp_scales_and_output_never_clips() {
        let loud: Vec<f32> = vec![0.9; 2000];
        let boosted = run(
            Dsp {
                eq: true,
                preamp_db: 12.,
                ..Default::default()
            },
            &loud,
        );
        assert!(boosted.iter().all(|s| (-1.0..=1.0).contains(s)));
        let quieter = run(
            Dsp {
                eq: true,
                preamp_db: -6.,
                ..Default::default()
            },
            &[0.5, 0.5],
        );
        assert!((quieter[0] - 0.2506).abs() < 0.001);
    }

    #[test]
    fn mono_balance_and_crossfeed_mix_channels() {
        let mono = run(
            Dsp {
                mono: true,
                ..Default::default()
            },
            &[0.4, 0.0],
        );
        assert_eq!(mono, vec![0.2, 0.2]);
        let right = run(
            Dsp {
                balance: 0.5,
                ..Default::default()
            },
            &[0.4, 0.4],
        );
        assert!((right[0] - 0.2).abs() < 1e-6 && (right[1] - 0.4).abs() < 1e-6);
        let only_left: Vec<f32> = (0..4800).flat_map(|_| [0.5, 0.0]).collect();
        let fed = run(
            Dsp {
                crossfeed: true,
                ..Default::default()
            },
            &only_left,
        );
        assert!(
            fed[fed.len() - 1] > 0.05,
            "some of the left channel should reach the right"
        );
    }

    #[test]
    fn bands_above_nyquist_are_skipped_and_presets_are_well_formed() {
        let mut settings = Dsp {
            eq: true,
            ..Default::default()
        };
        settings.bands[9] = 6.;
        let chain = Chain::new(settings, 22050, 2);
        assert!(
            chain.filters.is_empty(),
            "16 kHz cannot be shaped at 22.05 kHz"
        );
        for (name, preamp, bands) in PRESETS {
            let max = bands.iter().copied().fold(0., f32::max);
            assert!(*preamp <= -max + 0.01 || max == 0., "{name} could clip");
        }
    }

    #[test]
    fn plugin_effects_play_in_the_source_and_follow_changes() {
        let registry: Arc<crate::effects::Registry> = Arc::default();
        let half = crate::effects::parse(
            "p",
            "P",
            serde_json::json!([{ "id": "quiet", "name": "Quiet",
                "params": [{ "id": "db", "name": "Level", "min": -24, "max": 0, "value": -6.0206 }],
                "blocks": [{ "kind": "gain", "db": "$db" }] }]),
            |_| anyhow::bail!("no files"),
        )
        .unwrap();
        registry.set(half.into_iter().map(Arc::new).collect());
        let slot = crate::effects::EffectSlot::new("p", "quiet");
        let settings = Dsp {
            effects: vec![slot.clone()],
            ..Default::default()
        };
        assert!(!settings.is_transparent());
        let control = DspControl::new(settings.clone(), registry.clone());
        // An odd length: the last block is short, and a half frame at the end is dropped.
        let source = rodio::buffer::SamplesBuffer::new(2, 48000, vec![0.8f32; 2 * 1300 + 1]);
        let mut processed = Processed::new(source, control.clone());
        let first: Vec<f32> = processed.by_ref().take(4).collect();
        assert!(first.iter().all(|s| (*s - 0.4).abs() < 1e-3), "{first:?}");
        let mut louder = slot.clone();
        louder.params.insert("db".into(), 0.);
        control.set(Dsp {
            effects: vec![louder],
            ..settings.clone()
        });
        let rest: Vec<f32> = processed.collect();
        assert_eq!(rest.len() + 4, 2 * 1300);
        assert!((rest[rest.len() - 1] - 0.8).abs() < 1e-3);
        // Turning the plugin off removes its effect from songs already playing.
        let source = rodio::buffer::SamplesBuffer::new(2, 48000, vec![0.8f32; 4096]);
        let mut processed = Processed::new(source, control.clone());
        registry.set(vec![]);
        let later: Vec<f32> = processed.by_ref().skip(2048).take(2).collect();
        assert_eq!(later, vec![0.8, 0.8]);
    }

    #[test]
    fn webassembly_effects_keep_up_with_the_music() {
        let dir = tempfile::tempdir().unwrap();
        let library = crate::database::Library::open(dir.path()).unwrap();
        crate::plugins::install_examples(&library).unwrap();
        let folder = library.directory.join("plugins/bitcrusher");
        let definitions = crate::effects::load(
            "bitcrusher",
            "Bitcrusher",
            &folder,
            serde_json::json!([{ "id": "crush", "name": "Bitcrusher", "wasm": "crush.wat",
                "params": [{ "id": "bits", "name": "Bits", "min": 2, "max": 16, "value": 6 }] }]),
        )
        .unwrap();
        let registry: Arc<crate::effects::Registry> = Arc::default();
        registry.set(definitions.into_iter().map(Arc::new).collect());
        let mut rack = crate::effects::Rack::new(registry.clone(), 48000, 2);
        rack.sync(&[crate::effects::EffectSlot::new("bitcrusher", "crush")]);
        let mut block = vec![0.3f32; crate::effects::BLOCK_FRAMES * 2];
        let seconds = 10.;
        let blocks = (48000. * seconds / crate::effects::BLOCK_FRAMES as f64) as usize;
        let start = std::time::Instant::now();
        for _ in 0..blocks {
            rack.process(&mut block);
        }
        let took = start.elapsed().as_secs_f64();
        assert_eq!(registry.failure("bitcrusher", "crush"), None);
        assert!(
            took < seconds / 2.,
            "10 s of sound took {took:.2} s to process"
        );
    }

    #[test]
    fn live_changes_reach_a_playing_source() {
        let control = DspControl::new(Dsp::default(), Default::default());
        let source = rodio::buffer::SamplesBuffer::new(2, 48000, vec![0.5f32; 4096]);
        let mut processed = Processed::new(source, control.clone());
        assert_eq!(processed.next(), Some(0.5));
        control.set(Dsp {
            eq: true,
            preamp_db: -6.,
            ..Default::default()
        });
        let later: Vec<f32> = processed.by_ref().skip(1024).take(2).collect();
        assert!(
            later.iter().all(|s| (*s - 0.2506).abs() < 0.001),
            "{later:?}"
        );
    }
}
