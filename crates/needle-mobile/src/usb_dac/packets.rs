//! How much music goes in each USB packet, and how it is laid out: pure arithmetic, tested on
//! any computer. The device side is `device.rs`.

/// Packets a second for an isochronous endpoint at `interval` (the descriptor's bInterval):
/// full speed counts in 1 ms frames, high speed in 125 µs microframes, each 2^(interval-1).
pub fn packets_per_second(high_speed: bool, interval: u8) -> u32 {
    let every = 1u32 << (interval.clamp(1, 16) - 1);
    if high_speed {
        (8000 / every).max(1)
    } else {
        // Full speed audio endpoints use 1 ms frames whatever bInterval says.
        1000
    }
}

/// Samples per packet in 16.16 fixed point, at `rate`.
pub fn nominal(rate: u32, per_second: u32) -> u32 {
    (((rate as u64) << 16) / per_second as u64) as u32
}

/// Whether music at `rate` fits packets of up to `limit` bytes, `frame` bytes for each frame. An
/// endpoint may be made for exactly its rate: 288 bytes is 48 kHz of 24-bit stereo in 1 ms.
pub fn rate_fits(rate: u32, per_second: u32, frame: usize, limit: usize) -> bool {
    (rate as usize).div_ceil(per_second.max(1) as usize) * frame <= limit
}

/// A feedback endpoint's answer in 16.16 samples per packet, or None when it makes no sense.
/// Full speed sends 10.14 in 3 bytes and high speed 16.16 in 4, but DACs differ, so the reading
/// closest to the nominal rate is taken, as Linux does.
pub fn feedback(bytes: &[u8], nominal: u32) -> Option<u32> {
    let raw = match bytes.len() {
        3 => bytes[0] as u32 | (bytes[1] as u32) << 8 | (bytes[2] as u32) << 16,
        n if n >= 4 => u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
        _ => return None,
    };
    if raw == 0 {
        return None;
    }
    let candidates = [raw, raw << 2, raw >> 2, raw << 1];
    let best = candidates
        .into_iter()
        .min_by_key(|c| (*c as i64 - nominal as i64).unsigned_abs())?;
    // Within a quarter of the nominal: anything else is a misreading.
    ((best as i64 - nominal as i64).unsigned_abs() <= nominal as u64 / 4).then_some(best)
}

/// The number of whole samples (frames) for the next packet, carrying the rest over: exact
/// for the nominal rate (44.1 kHz at 8000 packets a second is 44 100 frames a second, never one
/// short), then the DAC's own rate once its feedback comes (16.16 samples per packet).
pub struct Pacer {
    rate: u32,
    per_second: u32,
    rest: u64,
    feedback: Option<u32>,
}

impl Pacer {
    pub fn new(rate: u32, per_second: u32) -> Self {
        Self {
            rate,
            per_second: per_second.max(1),
            rest: 0,
            feedback: None,
        }
    }

    /// The DAC's own rate, from its feedback endpoint (16.16 samples per packet).
    pub fn set(&mut self, per_packet: u32) {
        if self.feedback.is_none() {
            // Into 16.16 from here on, keeping the fraction already carried.
            self.rest = (self.rest << 16) / self.per_second as u64;
        }
        self.feedback = Some(per_packet);
    }

    pub fn next(&mut self) -> usize {
        match self.feedback {
            Some(per_packet) => {
                let total = self.rest + per_packet as u64;
                self.rest = total & 0xFFFF;
                (total >> 16) as usize
            }
            None => {
                let total = self.rest + self.rate as u64;
                self.rest = total % self.per_second as u64;
                (total / self.per_second as u64) as usize
            }
        }
    }
}

/// Lay `samples` (interleaved, -1 to 1, `from` channels) into `out` as `to` channels of
/// `subslot` bytes each, little-endian, the music `bits` deep and left-aligned in its slot.
/// Mono goes to both sides of a stereo DAC. Returns the bytes written.
pub fn pack(
    samples: &[f32],
    from: usize,
    to: usize,
    subslot: usize,
    bits: u8,
    out: &mut [u8],
) -> usize {
    let frames = samples.len() / from.max(1);
    let mut at = 0;
    for f in 0..frames {
        for c in 0..to {
            let sample = samples[f * from + c.min(from - 1)];
            let slot = &mut out[at..at + subslot];
            write_sample(sample, bits, slot);
            at += subslot;
        }
    }
    at
}

fn write_sample(sample: f32, bits: u8, slot: &mut [u8]) {
    match slot.len() {
        2 => slot.copy_from_slice(
            &(needle_core::platform_output::to_int(sample, 16) as i16).to_le_bytes(),
        ),
        3 => needle_core::platform_output::pcm24(sample, slot),
        4 => {
            // The music's depth, left-aligned in the 32-bit slot.
            let v = needle_core::platform_output::to_int(sample, bits.clamp(16, 32) as u32)
                << (32 - bits.clamp(16, 32) as u32);
            slot.copy_from_slice(&v.to_le_bytes());
        }
        _ => slot.fill(0),
    }
}

/// Decibels below the top for Needle's volume (0 to 1) on a DAC whose volume spans `range` dB.
/// Loudness follows the ear, 40 dB for each tenth of the slider, so with a range of 60 dB or more
/// a slider's middle is about -12 dB. A narrower range goes with a weaker output, whose bottom
/// cannot be heard (a dock of -45 to 0 dB is silent by -40 dB), so the curve is squeezed: the
/// phone's lowest step, 1 of 16, sits at about -30 dB, or at the bottom of a smaller range.
fn volume_db(volume: f32, range: f32) -> f32 {
    let wide = ((range - 45.) / 15.).clamp(0., 1.);
    let squeeze = (0.625 + wide * 0.375).min(range / 48.);
    40. * volume.clamp(0.001, 1.).log10() * squeeze
}

/// The DAC's volume for Needle's (0 to 1), in its own steps of 1/256 dB between `min` and
/// `max` (see [`volume_db`]); 0 is the quietest.
pub fn volume_steps(volume: f32, min: i16, max: i16) -> i16 {
    if volume <= 0.001 {
        return min;
    }
    let range = (max as f32 - min as f32) / 256.;
    let steps = (volume_db(volume, range) * 256.).round() as i32 + max as i32;
    steps.clamp(min as i32, max as i32) as i16
}

/// The range [`software_gain`] takes for a DAC with no volume: the squeezed curve, as on a dock.
const SOFTWARE_RANGE: f32 = 45.;

/// How much to scale the samples for Needle's volume (0 to 1), for a DAC with no volume of its
/// own: the same curve as [`volume_steps`], so a slider feels alike on every DAC; 0 is silence.
pub fn software_gain(volume: f32) -> f32 {
    if volume <= 0.001 {
        return 0.;
    }
    10f32.powf(volume_db(volume, SOFTWARE_RANGE) / 20.)
}

/// Scales `samples` (interleaved, `channels` of them) by a gain that moves from `from` to `to`
/// across them, so a change of volume makes no click.
pub fn ramp(samples: &mut [f32], channels: usize, from: f32, to: f32) {
    let frames = samples.len() / channels.max(1);
    for (i, frame) in samples.chunks_mut(channels.max(1)).enumerate() {
        let gain = from + (to - from) * (i + 1) as f32 / frames.max(1) as f32;
        frame.iter_mut().for_each(|sample| *sample *= gain);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packets_made_exactly_for_their_rate_fit() {
        // Apple's adapter, in 1 ms packets: 24-bit and 16-bit stereo at 48 kHz.
        assert!(rate_fits(48_000, 1000, 6, 288));
        assert!(rate_fits(48_000, 1000, 4, 192));
        // The dock: 16-bit stereo, 192 bytes, at both its rates.
        assert!(rate_fits(44_100, 1000, 4, 192));
        // Too much.
        assert!(!rate_fits(96_000, 1000, 6, 288));
        assert!(!rate_fits(48_100, 1000, 6, 288));
    }

    #[test]
    fn packets_carry_the_rate_exactly() {
        let per = packets_per_second(true, 1);
        assert_eq!(per, 8000);
        let mut pacer = Pacer::new(44_100, per);
        let total: usize = (0..8000).map(|_| pacer.next()).sum();
        assert_eq!(total, 44_100);
        let mut pacer = Pacer::new(96_000, packets_per_second(true, 4));
        assert_eq!(pacer.next(), 96);
        // With the DAC's feedback, its own rate.
        let mut pacer = Pacer::new(48_000, 8000);
        pacer.set(nominal(48_000, 8000) + 655);
        let total: usize = (0..8000).map(|_| pacer.next()).sum();
        assert_eq!(total, 48_079);
        assert_eq!(packets_per_second(false, 4), 1000);
    }

    #[test]
    fn feedback_is_read_in_either_format() {
        let nominal = nominal(44_100, 8000);
        // 16.16 in 4 bytes.
        assert_eq!(feedback(&nominal.to_le_bytes(), nominal), Some(nominal));
        // 10.14 in 3 bytes, full speed: 44.1 samples a frame.
        let fs = super::nominal(44_100, 1000);
        let raw = fs >> 2;
        assert_eq!(feedback(&raw.to_le_bytes()[..3], fs), Some(raw << 2));
        assert_eq!(feedback(&[0, 0, 0, 0], nominal), None);
        // Far from the nominal rate: a misreading, not used.
        assert_eq!(feedback(&(nominal * 3).to_le_bytes(), nominal), None);
    }

    #[test]
    fn samples_are_packed_in_their_slots() {
        let mut out = [0u8; 16];
        // One stereo frame of 24-bit music in 32-bit slots.
        let s = 0x123456 as f32 / 8_388_608.;
        assert_eq!(pack(&[s, -s], 2, 2, 4, 24, &mut out), 8);
        assert_eq!(&out[..4], &[0x00, 0x56, 0x34, 0x12]);
        // Mono to both sides, 16-bit.
        let mut out = [0u8; 4];
        assert_eq!(pack(&[0.5], 1, 2, 2, 16, &mut out), 4);
        assert_eq!(out, [0x00, 0x40, 0x00, 0x40]);
        // 3-byte slots.
        let mut out = [0u8; 6];
        assert_eq!(pack(&[s, s], 2, 2, 3, 24, &mut out), 6);
        assert_eq!(&out[..3], &[0x56, 0x34, 0x12]);
    }

    #[test]
    fn volume_follows_the_ear() {
        assert_eq!(volume_steps(1., -127 * 256, 0), 0);
        assert_eq!(volume_steps(0., -127 * 256, 0), -127 * 256);
        let half = volume_steps(0.5, -127 * 256, 0);
        assert!((-13 * 256..-11 * 256).contains(&half), "{half}");
    }

    #[test]
    fn software_volume_follows_the_same_ear_as_a_dacs_own() {
        assert_eq!(software_gain(1.), 1.);
        assert_eq!(software_gain(0.), 0.);
        // As on a dock of -45 to 0 dB.
        for volume in [0.0625, 0.1, 0.2, 0.5, 0.9] {
            let db = 20. * software_gain(volume).log10();
            let steps = volume_steps(volume, -45 * 256, 0) as f32 / 256.;
            assert!((db - steps).abs() < 0.01, "{volume}: {db} and {steps}");
        }
    }

    #[test]
    fn every_step_of_a_narrow_range_can_be_heard_and_is_louder_than_the_last() {
        let steps = |volume: f32| volume_steps(volume, -45 * 256, 0) as f32 / 256.;
        let heard = [0.06, 0.1, 0.2, 0.5, 1.];
        for pair in heard.windows(2) {
            assert!(steps(pair[0]) < steps(pair[1]), "{pair:?}");
        }
        for volume in heard {
            assert!(steps(volume) >= -31., "{volume}: {} dB", steps(volume));
        }
        // All 16 steps of the phone, with room between them, and the top at the top.
        let all: Vec<f32> = (1..=16).map(|n| steps(n as f32 / 16.)).collect();
        assert!(all[0] >= -31. && all[0] <= -29., "{all:?}");
        assert_eq!(all[15], 0.);
        for pair in all.windows(2) {
            assert!(pair[1] - pair[0] > 0.3, "{all:?}");
        }
        // 0 is the bottom, which the caller mutes.
        assert_eq!(volume_steps(0., -45 * 256, 0), -45 * 256);
    }

    #[test]
    fn a_small_range_keeps_every_step_inside_it() {
        let steps = |volume: f32| volume_steps(volume, -12 * 256, 0) as f32 / 256.;
        assert!(
            (-12.1..-11.9).contains(&steps(1. / 16.)),
            "{}",
            steps(1. / 16.)
        );
        assert!(steps(2. / 16.) > steps(1. / 16.));
    }

    #[test]
    fn a_wide_range_keeps_its_curve() {
        for (min, max) in [(-60 * 256, 0), (-100 * 256, 0), (-127 * 256, 0)] {
            for volume in [0.0625f32, 0.25, 0.5, 0.75] {
                let want = 40. * volume.log10();
                let got = volume_steps(volume, min, max) as f32 / 256.;
                assert!((got - want).abs() < 0.01, "{volume}: {got} and {want}");
            }
        }
    }

    #[test]
    fn a_change_of_volume_moves_across_the_packet_and_ends_on_time() {
        let mut samples = [1f32; 8];
        ramp(&mut samples, 2, 1., 0.5);
        assert_eq!(samples, [0.875, 0.875, 0.75, 0.75, 0.625, 0.625, 0.5, 0.5]);
        let mut samples = [0.5f32; 4];
        ramp(&mut samples, 2, 0.5, 0.5);
        assert_eq!(samples, [0.25; 4]);
    }
}
