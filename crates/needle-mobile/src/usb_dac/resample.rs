//! Music at a rate the DAC does not have, converted to one it has, so the sound never stops: a
//! Kaiser-windowed sinc filter, read from a table with a straight line between its rows, which
//! keeps the time exact for any pair of rates. Not bit-perfect, and the status says so. The rates
//! the DAC has do not come here.
use std::f64::consts::PI;

/// Rows of the table: one for each 512th of a sample.
const PHASES: usize = 512;
/// Samples at each side of the moment asked, when the band is not narrowed.
const SIDE: usize = 48;
/// The window's shape: about 100 dB down outside the band.
const BETA: f64 = 10.;
/// Where the band ends, as a part of the lower rate's top frequency.
const BAND: f64 = 0.955;
/// Frames read at a time: few, as what is read ahead of a pause is lost.
const CHUNK: usize = 256;

pub struct Resampler {
    from: u64,
    to: u64,
    channels: usize,
    side: usize,
    /// `PHASES + 1` rows of `2 * side` taps.
    table: Vec<f32>,
    /// One row, between two of the table's.
    taps: Vec<f32>,
    /// The music, interleaved, after `side` frames of silence.
    input: Vec<f32>,
    /// The frame just before the moment to play next, and how far past it, in `to`ths of a frame.
    at: usize,
    past: u64,
    ended: bool,
}

/// The modified Bessel function of order 0, for the window.
fn bessel(x: f64) -> f64 {
    let (mut sum, mut term) = (1., 1.);
    for k in 1..40 {
        term *= (x / (2. * k as f64)).powi(2);
        sum += term;
        if term < sum * 1e-12 {
            break;
        }
    }
    sum
}

impl Resampler {
    pub fn new(from: u32, to: u32, channels: usize) -> Self {
        // Going down, the band narrows to what the new rate can hold, and the filter grows.
        let narrow = (to as f64 / from as f64).min(1.) * BAND;
        let side = (SIDE as f64 / narrow).ceil() as usize;
        let mut table = Vec::with_capacity((PHASES + 1) * 2 * side);
        for row in 0..=PHASES {
            let start = table.len();
            let shift = row as f64 / PHASES as f64;
            for tap in 0..2 * side {
                let x = tap as f64 - (side as f64 - 1.) - shift;
                let sinc = if x == 0. {
                    1.
                } else {
                    (PI * narrow * x).sin() / (PI * narrow * x)
                };
                let edge = (1. - (x / side as f64).powi(2)).max(0.);
                table.push((narrow * sinc * bessel(BETA * edge.sqrt()) / bessel(BETA)) as f32);
            }
            // Exactly the same level at every moment.
            let sum: f32 = table[start..].iter().sum();
            table[start..].iter_mut().for_each(|tap| *tap /= sum);
        }
        Self {
            from: from as u64,
            to: to as u64,
            channels: channels.max(1),
            side,
            table,
            taps: vec![0.; 2 * side],
            input: vec![0.; side * channels.max(1)],
            at: side,
            past: 0,
            ended: false,
        }
    }

    /// Fills `out` (interleaved) with the music `read` gives, as `Feed::read` does: fewer than
    /// asked for means the music ended. Returns how many were filled, all of `out` until the end.
    pub fn fill(&mut self, out: &mut [f32], mut read: impl FnMut(&mut [f32]) -> usize) -> usize {
        let (channels, side) = (self.channels, self.side);
        let frames = out.len() / channels;
        for frame in 0..frames {
            while (self.at + side + 1) * channels > self.input.len() {
                if self.ended {
                    return frame * channels;
                }
                let have = self.input.len();
                self.input.resize(have + CHUNK * channels, 0.);
                let got = read(&mut self.input[have..]);
                self.input.truncate(have + got);
                if got < CHUNK * channels {
                    // The last of the music leaves the filter.
                    self.ended = true;
                    self.input.resize(have + got + (side + 1) * channels, 0.);
                }
            }
            let place = self.past * PHASES as u64;
            let row = (place / self.to) as usize;
            let between = (place % self.to) as f32 / self.to as f32;
            let width = 2 * side;
            let (first, next) = self.table[row * width..(row + 2) * width].split_at(width);
            for (tap, (a, b)) in self.taps.iter_mut().zip(first.iter().zip(next)) {
                *tap = a + (b - a) * between;
            }
            let window = &self.input[(self.at + 1 - side) * channels..];
            for channel in 0..channels {
                let mut sum = 0f32;
                for (tap, frame) in self.taps.iter().zip(window.chunks_exact(channels)) {
                    sum += tap * frame[channel];
                }
                out[frame * channels + channel] = sum;
            }
            self.past += self.from;
            self.at += (self.past / self.to) as usize;
            self.past %= self.to;
        }
        // What the filter has passed goes.
        let done = self.at + 1 - side;
        if done >= 4 * CHUNK {
            self.input.drain(..done * channels);
            self.at -= done;
        }
        frames * channels
    }
}

/// The rate of those a DAC has for music of `rate`: the rate itself, else an exact multiple (the
/// lowest above it, then the highest below it), else the highest.
pub fn nearest_rate(rate: u32, rates: &[u32]) -> Option<u32> {
    if rates.contains(&rate) {
        return Some(rate);
    }
    let multiple = |r: &&u32| rate > 0 && **r > 0 && (**r % rate == 0 || rate % **r == 0);
    let multiples: Vec<&u32> = rates.iter().filter(multiple).collect();
    let above = multiples.iter().filter(|r| ***r > rate).min();
    above
        .or(multiples.iter().max())
        .map(|r| **r)
        .or(rates.iter().max().copied())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(frequency: f64, rate: u32, seconds: f64, channels: usize) -> Vec<f32> {
        let frames = (rate as f64 * seconds) as usize;
        let mut out = Vec::with_capacity(frames * channels);
        for i in 0..frames {
            let value = (2. * PI * frequency * i as f64 / rate as f64).sin() as f32 * 0.5;
            out.extend(std::iter::repeat_n(value, channels));
        }
        out
    }

    /// All of `music` through a resampler, asked in small pieces as packets are.
    fn convert(music: &[f32], from: u32, to: u32, channels: usize) -> Vec<f32> {
        let mut resampler = Resampler::new(from, to, channels);
        let mut at = 0;
        let mut out = vec![];
        loop {
            let mut piece = vec![0.; 6 * channels];
            let got = resampler.fill(&mut piece, |slots| {
                let n = slots.len().min(music.len() - at);
                slots[..n].copy_from_slice(&music[at..at + n]);
                at += n;
                n
            });
            out.extend_from_slice(&piece[..got]);
            if got < piece.len() {
                return out;
            }
        }
    }

    /// How strong `frequency` is in `samples` (one channel), by the Goertzel sum: 0.5 for a sine
    /// of that level.
    fn level(samples: &[f32], frequency: f64, rate: u32) -> f64 {
        let (mut re, mut im) = (0f64, 0f64);
        for (i, sample) in samples.iter().enumerate() {
            let turn = 2. * PI * frequency * i as f64 / rate as f64;
            re += *sample as f64 * turn.cos();
            im += *sample as f64 * turn.sin();
        }
        2. * (re * re + im * im).sqrt() / samples.len() as f64
    }

    #[test]
    fn the_song_keeps_its_length() {
        let music = sine(1000., 44_100, 1., 2);
        let out = convert(&music, 44_100, 48_000, 2);
        let frames = out.len() / 2;
        // A second of music, and the filter's short tail.
        assert!((48_000..48_060).contains(&frames), "{frames}");
    }

    #[test]
    fn a_tone_keeps_its_pitch_and_its_level() {
        for (from, to) in [
            (44_100, 48_000),
            (96_000, 48_000),
            (22_050, 48_000),
            (44_100, 96_000),
        ] {
            let music = sine(1000., from, 0.5, 1);
            let out = convert(&music, from, to, 1);
            // The middle, away from both ends.
            let middle = &out[out.len() / 4..out.len() * 3 / 4];
            let tone = level(middle, 1000., to);
            assert!((tone - 0.5).abs() < 0.002, "{from} to {to}: {tone}");
            let beside = level(middle, 1100., to);
            assert!(beside < 0.01, "{from} to {to}: {beside}");
        }
    }

    #[test]
    fn nothing_folds_back_into_the_music() {
        // 20 kHz at 44.1 kHz leaves no mirror image at 24.1 kHz, as near as 48 kHz can show it.
        let music = sine(20_000., 44_100, 0.5, 1);
        let out = convert(&music, 44_100, 48_000, 1);
        let middle = &out[out.len() / 4..out.len() * 3 / 4];
        assert!(level(middle, 20_000., 48_000) > 0.4);
        assert!(level(middle, 23_900., 48_000) < 0.0001);
        // Going down: 30 kHz has no place at 48 kHz, and does not come back as 18 kHz.
        let music = sine(30_000., 96_000, 0.5, 1);
        let out = convert(&music, 96_000, 48_000, 1);
        let middle = &out[out.len() / 4..out.len() * 3 / 4];
        assert!(level(middle, 18_000., 48_000) < 0.0001);
    }

    #[test]
    fn channels_stay_apart() {
        let mut music = sine(1000., 44_100, 0.2, 2);
        music.iter_mut().skip(1).step_by(2).for_each(|s| *s = 0.);
        let out = convert(&music, 44_100, 48_000, 2);
        assert!(out.iter().step_by(2).any(|s| s.abs() > 0.4));
        assert!(out.iter().skip(1).step_by(2).all(|s| s.abs() < 0.0001));
    }

    #[test]
    fn the_right_rate_is_chosen() {
        // Its only rate.
        assert_eq!(nearest_rate(44_100, &[48_000]), Some(48_000));
        // The song's own.
        assert_eq!(nearest_rate(44_100, &[44_100, 48_000]), Some(44_100));
        // An exact multiple first, above before below.
        assert_eq!(
            nearest_rate(44_100, &[48_000, 88_200, 96_000]),
            Some(88_200)
        );
        assert_eq!(nearest_rate(96_000, &[44_100, 48_000]), Some(48_000));
        assert_eq!(
            nearest_rate(32_000, &[44_100, 48_000, 96_000]),
            Some(96_000)
        );
        // None: the highest.
        assert_eq!(
            nearest_rate(44_100, &[32_000, 48_000, 96_000]),
            Some(96_000)
        );
        assert_eq!(nearest_rate(44_100, &[]), None);
    }
}
