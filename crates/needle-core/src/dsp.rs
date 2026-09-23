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
    /// "graphic" (the ten bands) or "parametric" (the bands below).
    pub mode: String,
    /// Parametric equalizer bands, used when `mode` is "parametric".
    pub parametric: Vec<ParamBand>,
}

/// One band of the parametric equalizer.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ParamBand {
    /// Keeps a band's sliders when others are added or removed.
    pub uid: String,
    /// "peak", "lowshelf", "highshelf", "lowpass", "highpass", or "notch".
    pub kind: String,
    pub frequency: f32,
    pub gain: f32,
    pub q: f32,
    pub on: bool,
}
impl Default for ParamBand {
    fn default() -> Self {
        Self {
            uid: uuid::Uuid::new_v4().to_string(),
            kind: "peak".into(),
            frequency: 1000.,
            gain: 0.,
            q: 1.,
            on: true,
        }
    }
}
pub const PARAMETRIC_KINDS: [(&str, &str); 6] = [
    ("peak", "Peak"),
    ("lowshelf", "Low shelf"),
    ("highshelf", "High shelf"),
    ("lowpass", "Low-pass"),
    ("highpass", "High-pass"),
    ("notch", "Notch"),
];
pub const MAX_PARAMETRIC: usize = 20;
impl ParamBand {
    fn shape(&self) -> crate::effects::Shape {
        use crate::effects::Shape;
        match self.kind.as_str() {
            "lowshelf" => Shape::Lowshelf,
            "highshelf" => Shape::Highshelf,
            "lowpass" => Shape::Lowpass,
            "highpass" => Shape::Highpass,
            "notch" => Shape::Notch,
            _ => Shape::Peak,
        }
    }
    /// Whether the band changes the sound at all.
    fn active(&self) -> bool {
        self.on
            && (self.gain != 0. || matches!(self.kind.as_str(), "lowpass" | "highpass" | "notch"))
    }
}

/// A listener's own equalizer preset.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct UserPreset {
    pub name: String,
    pub mode: String,
    pub preamp_db: f32,
    pub bands: [f32; 10],
    pub parametric: Vec<ParamBand>,
}
impl Default for UserPreset {
    fn default() -> Self {
        Self {
            name: String::new(),
            mode: "graphic".into(),
            preamp_db: 0.,
            bands: [0.; 10],
            parametric: vec![],
        }
    }
}
impl UserPreset {
    pub fn from_dsp(name: &str, dsp: &Dsp) -> Self {
        Self {
            name: name.into(),
            mode: dsp.mode.clone(),
            preamp_db: dsp.preamp_db,
            bands: dsp.bands,
            parametric: dsp.parametric.clone(),
        }
    }
    /// `dsp` with this preset's equalizer (and the equalizer turned on).
    pub fn apply(&self, dsp: &Dsp) -> Dsp {
        Dsp {
            eq: true,
            mode: self.mode.clone(),
            preamp_db: self.preamp_db,
            bands: self.bands,
            parametric: self
                .parametric
                .iter()
                .map(|b| ParamBand {
                    uid: uuid::Uuid::new_v4().to_string(),
                    ..b.clone()
                })
                .collect(),
            preset: self.name.clone(),
            ..dsp.clone()
        }
    }
}

/// Read an Equalizer APO / AutoEq "ParametricEQ.txt" file: a preamp and filter lines such as
/// `Filter 1: ON PK Fc 105 Hz Gain -2.5 dB Q 0.70`.
pub fn parse_parametric(text: &str) -> anyhow::Result<(f32, Vec<ParamBand>)> {
    let mut preamp: f32 = 0.;
    let mut bands = vec![];
    for line in text.lines().map(str::trim) {
        if let Some(rest) = line.strip_prefix("Preamp:") {
            preamp = rest
                .split_whitespace()
                .next()
                .and_then(|v| v.parse().ok())
                .ok_or_else(|| anyhow::anyhow!("The preamp line is not a number: {line}"))?;
            continue;
        }
        if !line.starts_with("Filter") {
            continue;
        }
        let Some((_, rest)) = line.split_once(':') else {
            continue;
        };
        let words: Vec<&str> = rest.split_whitespace().collect();
        let value = |key: &str| -> Option<f32> {
            let at = words.iter().position(|w| w.eq_ignore_ascii_case(key))?;
            words.get(at + 1)?.parse().ok()
        };
        let on = words.first().is_some_and(|w| w.eq_ignore_ascii_case("ON"));
        let kind = match words.get(1).map(|w| w.to_uppercase()).as_deref() {
            Some("PK" | "PEQ" | "MODAL") => "peak",
            Some("LS" | "LSC" | "LSQ" | "LS 6DB" | "LS 12DB") => "lowshelf",
            Some("HS" | "HSC" | "HSQ" | "HS 6DB" | "HS 12DB") => "highshelf",
            Some("LP" | "LPQ") => "lowpass",
            Some("HP" | "HPQ") => "highpass",
            Some("NO") => "notch",
            _ => continue,
        };
        let frequency =
            value("Fc").ok_or_else(|| anyhow::anyhow!("A filter has no frequency: {line}"))?;
        bands.push(ParamBand {
            kind: kind.into(),
            frequency: frequency.clamp(10., 24000.),
            gain: value("Gain").unwrap_or(0.).clamp(-24., 24.),
            q: value("Q").unwrap_or(0.707).clamp(0.1, 20.),
            on,
            ..Default::default()
        });
    }
    if bands.is_empty() {
        anyhow::bail!(
            "No filters found. Use a ParametricEQ.txt file from AutoEq or an Equalizer APO configuration."
        );
    }
    if bands.len() > MAX_PARAMETRIC {
        anyhow::bail!(
            "Needle takes up to {MAX_PARAMETRIC} filters; this file has {}.",
            bands.len()
        );
    }
    Ok((preamp.clamp(-24., 12.), bands))
}

/// Write bands in the Equalizer APO format, which AutoEq and other players read.
pub fn format_parametric(preamp: f32, bands: &[ParamBand]) -> String {
    let mut text = format!("Preamp: {preamp:.1} dB\r\n");
    for (i, band) in bands.iter().enumerate() {
        let code = match band.kind.as_str() {
            "lowshelf" => "LSC",
            "highshelf" => "HSC",
            "lowpass" => "LPQ",
            "highpass" => "HPQ",
            "notch" => "NO",
            _ => "PK",
        };
        text.push_str(&format!(
            "Filter {}: {} {code} Fc {:.0} Hz Gain {:.1} dB Q {:.2}\r\n",
            i + 1,
            if band.on { "ON" } else { "OFF" },
            band.frequency,
            band.gain,
            band.q
        ));
    }
    text
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
            mode: "graphic".into(),
            parametric: vec![],
        }
    }
}

impl Dsp {
    /// True when nothing would change the signal, so playback can skip processing entirely.
    pub fn is_transparent(&self) -> bool {
        let eq_flat = !self.eq
            || (self.preamp_db == 0.
                && if self.parametric_mode() {
                    !self.parametric.iter().any(ParamBand::active)
                } else {
                    self.bands.iter().all(|b| *b == 0.)
                });
        eq_flat
            && self.balance == 0.
            && !self.mono
            && !self.crossfeed
            && !self.effects.iter().any(|e| e.on)
    }
    pub fn parametric_mode(&self) -> bool {
        self.mode == "parametric"
    }
    /// The preamp that keeps the loudest boosted band from clipping.
    pub fn suggested_preamp(&self) -> f32 {
        if self.parametric_mode() {
            -self
                .parametric
                .iter()
                .filter(|b| b.on && matches!(b.kind.as_str(), "peak" | "lowshelf" | "highshelf"))
                .map(|b| b.gain)
                .fold(0., f32::max)
        } else {
            -self.bands.iter().copied().fold(0., f32::max)
        }
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

use crate::effects::{Biquad, Shape};

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
        self.filters = if !active {
            vec![]
        } else if settings.parametric_mode() {
            settings
                .parametric
                .iter()
                .filter(|b| b.active() && (b.frequency as f64) < self.rate * 0.49)
                .take(MAX_PARAMETRIC)
                .map(|b| {
                    Biquad::new(
                        b.shape(),
                        self.rate,
                        b.frequency as f64,
                        b.q.clamp(0.1, 20.) as f64,
                        b.gain.clamp(-24., 24.) as f64,
                    )
                })
                .collect()
        } else {
            BANDS
                .iter()
                .zip(settings.bands)
                .filter(|(f, g)| **f < self.rate * 0.45 && *g != 0.)
                .map(|(f, g)| {
                    Biquad::new(
                        Shape::Peak,
                        self.rate,
                        *f,
                        1.41,
                        g.clamp(-MAX_GAIN_DB, MAX_GAIN_DB) as f64,
                    )
                })
                .collect()
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
    fn parametric_bands_shape_the_sound_and_equalizer_apo_files_round_trip() {
        let text = "Preamp: -6.2 dB\nFilter 1: ON PK Fc 1000 Hz Gain 6 dB Q 1.41\nFilter 2: ON LSC Fc 105 Hz Gain 5.5 dB Q 0.70\nFilter 3: OFF HSC Fc 10000 Hz Gain -2 dB\nFilter 4: ON HPQ Fc 20 Hz Q 0.7\n";
        let (preamp, bands) = parse_parametric(text).unwrap();
        assert_eq!(preamp, -6.2);
        assert_eq!(bands.len(), 4);
        assert_eq!(
            (bands[1].kind.as_str(), bands[1].gain, bands[1].q),
            ("lowshelf", 5.5, 0.7)
        );
        assert!(!bands[2].on && bands[2].q == 0.707);
        let again = parse_parametric(&format_parametric(preamp, &bands)).unwrap();
        assert_eq!(
            again
                .1
                .iter()
                .map(|b| (&b.kind, b.frequency, b.gain, b.on))
                .collect::<Vec<_>>(),
            bands
                .iter()
                .map(|b| (&b.kind, b.frequency, b.gain, b.on))
                .collect::<Vec<_>>()
        );
        assert!(parse_parametric("nothing here").is_err());

        let settings = Dsp {
            eq: true,
            mode: "parametric".into(),
            parametric: vec![ParamBand {
                frequency: 1000.,
                gain: 6.,
                q: 1.41,
                ..Default::default()
            }],
            ..Default::default()
        };
        assert!(!settings.is_transparent());
        assert_eq!(settings.suggested_preamp(), -6.);
        let boost = rms(&run(settings.clone(), &sine(1000., 48000., 48000)))
            / rms(&sine(1000., 48000., 48000));
        assert!((boost - 2.0).abs() < 0.05, "+6 dB at 1 kHz, got {boost}");
        // The graphic bands are ignored in parametric mode, and a switched-off band does nothing.
        let mut off = settings.clone();
        off.parametric[0].on = false;
        off.bands[5] = 12.;
        assert!(off.is_transparent());
        assert_eq!(
            run(off, &sine(1000., 48000., 4800)),
            sine(1000., 48000., 4800)
        );
        // A saved preset brings the same sound back.
        let preset = UserPreset::from_dsp("Mine", &settings);
        let back = preset.apply(&Dsp::default());
        assert_eq!(back.parametric[0].gain, 6.);
        assert_eq!(back.preset, "Mine");
        assert!(back.eq);
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
