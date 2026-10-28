//! Speed and pitch: a song played faster or slower (0.5× to 2×), higher or lower (in
//! semitones), with or without keeping its pitch. Keeping the pitch stretches time (WSOLA: the
//! sound cut into short overlapping pieces, each laid where it fits best); a pitch change is a
//! stretch and a resample. Off on bit-perfect and exclusive output, which stay untouched.
use rodio::Source;
use std::{
    collections::VecDeque,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

pub const MIN_SPEED: f32 = 0.5;
pub const MAX_SPEED: f32 = 2.0;
/// The speed chips: 0.75×, 1×, 1.25×, 1.5×, 2×.
pub const SPEED_PRESETS: [f32; 5] = [0.75, 1.0, 1.25, 1.5, 2.0];
/// How far the pitch goes, up or down, in semitones.
pub const MAX_SEMITONES: f32 = 12.0;

/// How a song plays: its speed, its pitch, and whether a new speed keeps the pitch.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tempo {
    /// 1 is as recorded; 0.5 to 2.
    pub speed: f32,
    /// Semitones up (or down, below 0), −12 to 12.
    pub semitones: f32,
    /// A new speed keeps the pitch (no chipmunk voice); off, faster also sounds higher, as a
    /// record played fast does.
    pub keep_pitch: bool,
}

impl Default for Tempo {
    fn default() -> Self {
        Self {
            speed: 1.,
            semitones: 0.,
            keep_pitch: true,
        }
    }
}

impl Tempo {
    /// Within the limits, with a speed close to a whole step rounded onto it.
    pub fn clamped(self) -> Self {
        let speed = if self.speed.is_finite() {
            self.speed.clamp(MIN_SPEED, MAX_SPEED)
        } else {
            1.
        };
        let semitones = if self.semitones.is_finite() {
            self.semitones.clamp(-MAX_SEMITONES, MAX_SEMITONES)
        } else {
            0.
        };
        Self {
            speed: (speed * 100.).round() / 100.,
            semitones: (semitones * 10.).round() / 10.,
            keep_pitch: self.keep_pitch,
        }
    }
    /// Plays as recorded.
    pub fn is_neutral(&self) -> bool {
        (self.speed - 1.).abs() < 1e-4 && self.semitones.abs() < 1e-4
    }
    /// How much faster the stretch moves through the song than it plays (the time part), and
    /// how much faster the resample reads what the stretch gives (the pitch part). Their
    /// product is the speed.
    fn factors(&self) -> (f64, f64) {
        let pitch = 2f64.powf(self.semitones as f64 / 12.);
        let resample = pitch
            * if self.keep_pitch {
                1.
            } else {
                self.speed as f64
            };
        (self.speed as f64 / resample, resample)
    }
}

/// The tempo every playing song follows (changes apply at once), and where the song playing
/// is in its own time, which runs faster or slower than the clock.
#[derive(Default)]
pub struct TempoControl {
    tempo: Mutex<Tempo>,
    version: AtomicU64,
    /// The song (its id) last heard through a stretch, and its place in seconds of the song.
    clock: Mutex<Option<(Arc<str>, f64)>>,
}

impl TempoControl {
    pub fn new(tempo: Tempo) -> Arc<Self> {
        Arc::new(Self {
            tempo: Mutex::new(tempo.clamped()),
            ..Default::default()
        })
    }
    pub fn set(&self, tempo: Tempo) {
        *self.tempo.lock().unwrap_or_else(|e| e.into_inner()) = tempo.clamped();
        self.version.fetch_add(1, Ordering::Release);
    }
    pub fn get(&self) -> Tempo {
        *self.tempo.lock().unwrap_or_else(|e| e.into_inner())
    }
    /// Where the song `id` is, in its own seconds, if it is the one playing through a stretch.
    pub fn position(&self, id: &str) -> Option<f64> {
        let clock = self.clock.lock().unwrap_or_else(|e| e.into_inner());
        clock
            .as_ref()
            .filter(|(playing, _)| &**playing == id)
            .map(|(_, at)| *at)
    }
    pub fn clear_position(&self) {
        *self.clock.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }
    fn mark(&self, id: &Arc<str>, at: f64) {
        *self.clock.lock().unwrap_or_else(|e| e.into_inner()) = Some((id.clone(), at));
    }
}

/// The overlap-add stretch.
struct Wsola {
    channels: usize,
    /// A piece's length and the step between pieces as they are laid down, in frames.
    size: usize,
    hop: usize,
    /// How far a piece may move to fit, either way.
    search: usize,
    window: Vec<f32>,
    /// The song's frames being looked at (interleaved), from frame `start` of the song.
    input: Vec<f32>,
    start: usize,
    /// Where the next piece would sit in the song, before it moves to fit.
    nominal: f64,
    /// Where the last piece came from; `None` before the first.
    previous: Option<usize>,
    /// The pieces laid so far, the first `hop` frames finished.
    sum: Vec<f32>,
    ended: bool,
}

impl Wsola {
    fn new(rate: u32, channels: usize, at: usize) -> Self {
        // Pieces of about 40 ms laid every 20 ms, moving up to 12 ms to fit.
        let hop = (rate as usize / 50).max(32);
        let size = hop * 2;
        let search = (rate as usize * 12 / 1000).max(8);
        let window = (0..size)
            .map(|i| {
                let x = i as f32 / size as f32;
                0.5 - 0.5 * (2. * std::f32::consts::PI * x).cos()
            })
            .collect();
        Self {
            channels,
            size,
            hop,
            search,
            window,
            input: Vec::new(),
            start: at,
            nominal: at as f64,
            previous: None,
            sum: vec![0.; size * channels],
            ended: false,
        }
    }

    /// Frames held in `input`.
    fn held(&self) -> usize {
        self.input.len() / self.channels
    }

    /// The mono sum of frame `at` (a song frame), or silence past what is held.
    fn mono(&self, at: usize) -> f32 {
        let Some(i) = at.checked_sub(self.start) else {
            return 0.;
        };
        let base = i * self.channels;
        if base + self.channels > self.input.len() {
            return 0.;
        }
        self.input[base..base + self.channels].iter().sum()
    }

    /// How well the piece from `candidate` follows on from `target`: their likeness over the
    /// overlap, against the candidate's own loudness. Every `step`th frame is compared.
    fn fit(&self, candidate: usize, target: usize, step: usize) -> f32 {
        let (mut dot, mut energy) = (0f32, 1e-9f32);
        let mut i = 0;
        while i < self.hop {
            let c = self.mono(candidate + i);
            dot += c * self.mono(target + i);
            energy += c * c;
            i += step;
        }
        dot / energy.sqrt()
    }

    /// The best start near `nominal` for the next piece.
    fn place(&self, nominal: usize) -> usize {
        let Some(previous) = self.previous else {
            return nominal;
        };
        let target = previous + self.hop;
        let low = nominal.saturating_sub(self.search).max(self.start);
        let high = nominal + self.search;
        // A rough look at every fourth place, then a close one around the best.
        let mut best = (f32::MIN, nominal.max(low));
        let mut at = low;
        while at <= high {
            let fit = self.fit(at, target, 4);
            if fit > best.0 {
                best = (fit, at);
            }
            at += 4;
        }
        let rough = best.1;
        let mut best = (f32::MIN, rough);
        for at in rough.saturating_sub(4).max(low)..=(rough + 4).min(high) {
            let fit = self.fit(at, target, 1);
            if fit > best.0 {
                best = (fit, at);
            }
        }
        best.1
    }

    /// Lay the next piece, moving through the song `factor` times as fast as the pieces are
    /// laid, and hand over `hop` finished frames. `pull` gives the song's next frame; false
    /// when nothing is left at all.
    fn step(
        &mut self,
        factor: f64,
        pull: &mut dyn FnMut(&mut Vec<f32>) -> bool,
        out: &mut VecDeque<f32>,
    ) -> bool {
        let channels = self.channels;
        let nominal = self.nominal.round() as usize;
        let need = nominal + self.search + self.size + 4;
        while !self.ended && self.start + self.held() < need {
            if !pull(&mut self.input) {
                self.ended = true;
            }
        }
        if self.ended && nominal >= self.start + self.held() {
            // The song is over: hand over what is still being laid, once.
            if self.sum.iter().any(|s| *s != 0.) {
                out.extend(self.sum[..(self.size - self.hop) * channels].iter());
                self.sum.iter_mut().for_each(|s| *s = 0.);
                return true;
            }
            return false;
        }
        // At the recorded speed, follow on exactly, so nothing changes.
        let at = match self.previous {
            Some(previous) if (factor - 1.).abs() < 1e-6 => previous + self.hop,
            _ => self.place(nominal),
        };
        let first = self.previous.is_none();
        for i in 0..self.size {
            // The first piece has nothing before it to blend into, so it starts at full.
            let w = if first && i < self.hop {
                1.
            } else {
                self.window[i]
            };
            let Some(frame) = (at + i).checked_sub(self.start) else {
                continue;
            };
            let base = frame * channels;
            if base + channels > self.input.len() {
                break;
            }
            for c in 0..channels {
                self.sum[i * channels + c] += self.input[base + c] * w;
            }
        }
        out.extend(self.sum[..self.hop * channels].iter());
        self.sum.drain(..self.hop * channels);
        self.sum.resize(self.size * channels, 0.);
        self.previous = Some(at);
        self.nominal = if (factor - 1.).abs() < 1e-6 {
            (at + self.hop) as f64
        } else {
            self.nominal + self.hop as f64 * factor
        };
        // Let go of what no later piece can reach.
        let keep_from = (self.nominal as usize)
            .saturating_sub(self.search + 8)
            .min(at + self.hop);
        if keep_from > self.start + self.size * 4 {
            let drop = (keep_from - self.start).min(self.held());
            self.input.drain(..drop * channels);
            self.start += drop;
        }
        true
    }
}

/// A second-order low-pass, run before a resample that reads faster than the song (a pitch
/// raised), so the higher sounds do not fold back as noise.
#[derive(Clone, Copy, Default)]
struct LowPass {
    b: [f32; 3],
    a: [f32; 2],
}

impl LowPass {
    fn new(cutoff: f64) -> Self {
        // RBJ's low-pass, Q = 1/√2; `cutoff` is a share of the sample rate.
        let w = 2. * std::f64::consts::PI * cutoff.clamp(0.01, 0.49);
        let (sin, cos) = w.sin_cos();
        let alpha = sin / (2. * std::f64::consts::FRAC_1_SQRT_2);
        let a0 = 1. + alpha;
        let b1 = (1. - cos) / a0;
        Self {
            b: [(b1 / 2.) as f32, b1 as f32, (b1 / 2.) as f32],
            a: [(-2. * cos / a0) as f32, ((1. - alpha) / a0) as f32],
        }
    }
}

/// Reading the stretched sound faster or slower, with a cubic curve between its frames.
struct Resample {
    channels: usize,
    /// Frames waiting (interleaved); the curve needs one before the reading place and two
    /// after.
    frames: VecDeque<f32>,
    /// The reading place, in frames from the front of `frames` (at least 1 once started).
    phase: f64,
    filter: LowPass,
    /// Each channel's filter memory, twice (two filters in a row).
    memory: Vec<[f32; 4]>,
    filtered: bool,
}

impl Resample {
    fn new(channels: usize) -> Self {
        Self {
            channels,
            frames: VecDeque::new(),
            phase: 1.,
            filter: LowPass::default(),
            memory: vec![[0.; 4]; channels * 2],
            filtered: false,
        }
    }
    fn set_factor(&mut self, factor: f64) {
        let filtered = factor > 1.02;
        if filtered {
            self.filter = LowPass::new(0.47 / factor);
        }
        if filtered != self.filtered {
            self.memory.iter_mut().for_each(|m| *m = [0.; 4]);
        }
        self.filtered = filtered;
    }
    /// Take in frames, through the low-pass when reading fast.
    fn push(&mut self, samples: impl Iterator<Item = f32>) {
        if self.frames.is_empty() {
            // One silent frame before the first, for the curve.
            self.frames.extend(std::iter::repeat_n(0., self.channels));
        }
        if !self.filtered {
            self.frames.extend(samples);
            return;
        }
        let f = self.filter;
        for (i, x) in samples.enumerate() {
            let c = i % self.channels;
            let mut x = x;
            for stage in 0..2 {
                let m = &mut self.memory[c * 2 + stage];
                let y = f.b[0] * x + f.b[1] * m[0] + f.b[2] * m[1] - f.a[0] * m[2] - f.a[1] * m[3];
                *m = [x, m[0], y, m[2]];
                x = y;
            }
            self.frames.push_back(x);
        }
    }
    fn held(&self) -> usize {
        self.frames.len() / self.channels
    }
    /// One frame out, if enough has come in; then the reading place moves on by `factor`.
    fn pull(&mut self, factor: f64, out: &mut VecDeque<f32>) -> bool {
        let at = self.phase.floor() as usize;
        if at + 2 >= self.held() {
            return false;
        }
        let t = (self.phase - at as f64) as f32;
        let ch = self.channels;
        for c in 0..ch {
            let p = |i: usize| self.frames[i * ch + c];
            let (y0, y1, y2, y3) = (p(at - 1), p(at), p(at + 1), p(at + 2));
            // Catmull-Rom.
            let a = -0.5 * y0 + 1.5 * y1 - 1.5 * y2 + 0.5 * y3;
            let b = y0 - 2.5 * y1 + 2. * y2 - 0.5 * y3;
            let cc = -0.5 * y0 + 0.5 * y2;
            out.push_back(((a * t + b) * t + cc) * t + y1);
        }
        self.phase += factor;
        let done = (self.phase.floor() as usize).saturating_sub(1);
        if done > 0 {
            self.frames.drain(..done * ch);
            self.phase -= done as f64;
        }
        true
    }
}

/// A song played at the chosen speed and pitch. While the tempo is as recorded and has not
/// changed in this song, the sound passes through untouched.
pub struct Stretched<S: Source> {
    inner: S,
    control: Arc<TempoControl>,
    id: Arc<str>,
    version: u64,
    tempo: Tempo,
    rate: u32,
    channels: usize,
    /// Frames read from the song so far (from its start, or from where it was sought to).
    read: usize,
    wsola: Option<Wsola>,
    resample: Option<Resample>,
    /// The stretch's output, waiting for the resample (or to be played).
    middle: VecDeque<f32>,
    out: VecDeque<f32>,
    /// Where the sound handed out is, in seconds of the song, and frames since it was told.
    song_time: f64,
    unmarked: usize,
    /// The channel the next sample out belongs to.
    channel: usize,
    ended: bool,
}

impl<S: Source> Stretched<S> {
    pub fn new(inner: S, control: Arc<TempoControl>, id: &str) -> Self {
        let rate = inner.sample_rate().max(1);
        let channels = inner.channels().max(1) as usize;
        Self {
            inner,
            id: Arc::from(id),
            version: u64::MAX,
            tempo: Tempo::default(),
            control,
            rate,
            channels,
            read: 0,
            wsola: None,
            resample: None,
            middle: VecDeque::new(),
            out: VecDeque::new(),
            song_time: 0.,
            unmarked: 0,
            channel: 0,
            ended: false,
        }
    }

    fn follow(&mut self) {
        let version = self.control.version.load(Ordering::Acquire);
        if version == self.version {
            return;
        }
        self.version = version;
        self.tempo = self.control.get();
        if !self.tempo.is_neutral() && self.wsola.is_none() {
            self.wsola = Some(Wsola::new(self.rate, self.channels, self.read));
            self.resample = Some(Resample::new(self.channels));
        }
        let (_, resample) = self.tempo.factors();
        if let Some(r) = &mut self.resample {
            r.set_factor(resample);
            // Back at the song's own rate: read on whole frames again.
            if (resample - 1.).abs() < 1e-6 {
                r.phase = r.phase.round().max(1.);
            }
        }
    }

    /// Make more sound through the stretch, about 1024 frames.
    fn refill(&mut self) -> bool {
        self.follow();
        let channels = self.channels;
        let (time, resample) = self.tempo.factors();
        let Some(wsola) = &mut self.wsola else {
            return false;
        };
        let inner = &mut self.inner;
        let read = &mut self.read;
        let mut pull = |into: &mut Vec<f32>| -> bool {
            for _ in 0..256 {
                let start = into.len();
                for _ in 0..channels {
                    match inner.next() {
                        Some(s) => into.push(s),
                        None => {
                            into.truncate(start);
                            return false;
                        }
                    }
                }
                *read += 1;
            }
            true
        };
        let resample_state = self.resample.as_mut().expect("made with the stretch");
        let mut made = 0;
        while made < 1024 {
            if (resample - 1.).abs() < 1e-6 && resample_state.frames.is_empty() {
                // No resample: the stretch's frames go straight out.
                if self.middle.is_empty() && !wsola.step(time, &mut pull, &mut self.middle) {
                    break;
                }
                made += self.middle.len() / channels;
                self.out.extend(self.middle.drain(..));
                continue;
            }
            if resample_state.pull(resample, &mut self.out) {
                made += 1;
                continue;
            }
            if self.middle.is_empty() && !wsola.step(time, &mut pull, &mut self.middle) {
                // The end (the curve's last two frames are left: a few hundredths of a ms).
                break;
            }
            resample_state.push(self.middle.drain(..));
        }
        !self.out.is_empty()
    }
}

impl<S: Source> Iterator for Stretched<S> {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        if self.ended {
            return None;
        }
        if self.out.is_empty() && self.wsola.is_none() && self.channel == 0 {
            self.follow();
        }
        let sample = if self.wsola.is_none() {
            // Untouched, a sample at a time.
            let sample = self.inner.next();
            if sample.is_some() && self.channel + 1 >= self.channels {
                self.read += 1;
            }
            sample
        } else if self.out.is_empty() && !self.refill() {
            None
        } else {
            self.out.pop_front()
        };
        if sample.is_none() {
            self.ended = true;
            self.control.mark(&self.id, self.song_time);
            return None;
        }
        self.channel += 1;
        if self.channel >= self.channels {
            self.channel = 0;
            let speed = if self.wsola.is_some() {
                self.tempo.speed as f64
            } else {
                1.
            };
            self.song_time += speed / self.rate as f64;
            self.unmarked += 1;
            if self.unmarked >= 256 {
                self.unmarked = 0;
                self.control.mark(&self.id, self.song_time);
            }
        }
        sample
    }
}

impl<S: Source> Source for Stretched<S> {
    fn current_span_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> u16 {
        self.channels as u16
    }
    fn sample_rate(&self) -> u32 {
        self.rate
    }
    fn total_duration(&self) -> Option<Duration> {
        None
    }
    fn try_seek(&mut self, pos: Duration) -> Result<(), rodio::source::SeekError> {
        self.inner.try_seek(pos)?;
        let frame = (pos.as_secs_f64() * self.rate as f64).round() as usize;
        self.read = frame;
        self.out.clear();
        self.middle.clear();
        self.channel = 0;
        self.ended = false;
        self.song_time = pos.as_secs_f64();
        self.control.mark(&self.id, self.song_time);
        // Start afresh from the new place.
        self.wsola = None;
        self.resample = None;
        self.version = u64::MAX;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rodio::buffer::SamplesBuffer;

    fn tone(frequency: f32, rate: u32, seconds: f32) -> Vec<f32> {
        let frames = (rate as f32 * seconds) as usize;
        (0..frames)
            .flat_map(|i| {
                let s =
                    (2. * std::f32::consts::PI * frequency * i as f32 / rate as f32).sin() * 0.5;
                [s, s]
            })
            .collect()
    }

    /// The tone's frequency, from how often it crosses zero going up (left channel).
    fn frequency(samples: &[f32], rate: u32) -> f32 {
        let left: Vec<f32> = samples.iter().step_by(2).copied().collect();
        let mut crossings = vec![];
        for i in 1..left.len() {
            if left[i - 1] < 0. && left[i] >= 0. {
                crossings.push(i);
            }
        }
        let (first, last) = (crossings[2], crossings[crossings.len() - 3]);
        (crossings.len() - 5) as f32 * rate as f32 / (last - first) as f32
    }

    fn play(tempo: Tempo, input: Vec<f32>) -> (Vec<f32>, Arc<TempoControl>) {
        let control = TempoControl::new(tempo);
        let source = SamplesBuffer::new(2, 44100, input);
        let out: Vec<f32> = Stretched::new(source, control.clone(), "song").collect();
        (out, control)
    }

    #[test]
    fn as_recorded_it_passes_through_untouched() {
        let input = tone(440., 44100, 1.);
        let (out, control) = play(Tempo::default(), input.clone());
        assert_eq!(out, input);
        assert!((control.position("song").unwrap() - 1.).abs() < 0.01);
        assert_eq!(control.position("other"), None);
    }

    #[test]
    fn faster_keeping_the_pitch_is_shorter_at_the_same_note() {
        let tempo = Tempo {
            speed: 1.5,
            ..Default::default()
        };
        let (out, control) = play(tempo, tone(440., 44100, 3.));
        let seconds = out.len() as f32 / 2. / 44100.;
        assert!((seconds - 2.).abs() < 0.08, "{seconds} s");
        let f = frequency(&out[8820..out.len() - 8820], 44100);
        assert!((f - 440.).abs() < 6., "{f} Hz");
        // The song's own time reached its end.
        assert!((control.position("song").unwrap() - 3.).abs() < 0.08);
    }

    #[test]
    fn slower_keeping_the_pitch_is_longer_at_the_same_note() {
        let tempo = Tempo {
            speed: 0.5,
            ..Default::default()
        };
        let (out, _) = play(tempo, tone(330., 44100, 1.));
        let seconds = out.len() as f32 / 2. / 44100.;
        assert!((seconds - 2.).abs() < 0.08, "{seconds} s");
        let f = frequency(&out[8820..out.len() - 8820], 44100);
        assert!((f - 330.).abs() < 5., "{f} Hz");
    }

    #[test]
    fn faster_without_keeping_the_pitch_sounds_higher() {
        let tempo = Tempo {
            speed: 1.25,
            keep_pitch: false,
            ..Default::default()
        };
        let (out, _) = play(tempo, tone(400., 44100, 2.));
        let seconds = out.len() as f32 / 2. / 44100.;
        assert!((seconds - 1.6).abs() < 0.05, "{seconds} s");
        let f = frequency(&out[4410..out.len() - 4410], 44100);
        assert!((f - 500.).abs() < 6., "{f} Hz");
    }

    #[test]
    fn a_semitone_up_keeps_the_length() {
        let tempo = Tempo {
            semitones: 12.,
            ..Default::default()
        };
        let (out, _) = play(tempo, tone(220., 44100, 2.));
        let seconds = out.len() as f32 / 2. / 44100.;
        assert!((seconds - 2.).abs() < 0.08, "{seconds} s");
        let f = frequency(&out[8820..out.len() - 8820], 44100);
        assert!((f - 440.).abs() < 8., "{f} Hz");
    }

    #[test]
    fn limits_hold() {
        let t = Tempo {
            speed: 9.,
            semitones: -40.,
            keep_pitch: true,
        }
        .clamped();
        assert_eq!((t.speed, t.semitones), (2., -12.));
        let t = Tempo {
            speed: f32::NAN,
            semitones: 1.04,
            keep_pitch: true,
        }
        .clamped();
        assert_eq!((t.speed, t.semitones), (1., 1.));
        assert!(Tempo::default().is_neutral());
    }

    #[test]
    fn a_change_while_playing_takes_effect_and_a_seek_starts_afresh() {
        let control = TempoControl::new(Tempo::default());
        let source = SamplesBuffer::new(2, 44100, tone(440., 44100, 4.));
        let mut stretched = Stretched::new(source, control.clone(), "song");
        let first: Vec<f32> = stretched.by_ref().take(44100 * 2).collect();
        assert_eq!(first.len(), 88200);
        control.set(Tempo {
            speed: 2.,
            ..Default::default()
        });
        let rest: Vec<f32> = stretched.by_ref().collect();
        // One second as recorded, then three at twice the speed.
        let seconds = rest.len() as f32 / 2. / 44100.;
        assert!((seconds - 1.5).abs() < 0.08, "{seconds} s");
        let mut stretched = Stretched::new(
            SamplesBuffer::new(2, 44100, tone(440., 44100, 4.)),
            control.clone(),
            "song",
        );
        stretched.try_seek(Duration::from_secs(3)).unwrap();
        assert_eq!(control.position("song"), Some(3.));
        let tail: Vec<f32> = stretched.collect();
        let seconds = tail.len() as f32 / 2. / 44100.;
        assert!((seconds - 0.5).abs() < 0.06, "{seconds} s");
    }
}
