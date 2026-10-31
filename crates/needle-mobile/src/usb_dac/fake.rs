//! A pretend USB DAC for tests: it takes transfers at the pace a real one does, answers the
//! clock and volume requests, and can stall, vanish, or make the driver panic. The tests play
//! through the real Needle player and the real driver, and check that every trouble ends in an
//! error (the core then plays through the system's output), never a hang.
use super::bus::{Bus, Reaped};
use anyhow::{Result, bail};
use std::{
    collections::VecDeque,
    sync::Mutex,
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Trouble {
    None,
    /// Takes this many transfers, then never gives any back.
    StallAfter(usize),
    /// Takes this many transfers, then is unplugged.
    GoneAfter(usize),
    /// Setting the alternate setting panics (a bug in the driver's way).
    Panic,
    /// A clock that stays at this rate: it takes the request and changes nothing.
    StaysAt(u32),
    /// A clock fixed at this rate that refuses the request, and says the rate it is at.
    FixedAt(u32),
    /// The rate cannot be read back.
    NoRateAnswer,
    /// The clock does not say its rates, as a DAC just plugged in may not.
    NoClockAnswer,
}

#[derive(Default)]
struct State {
    /// Transfers with the device: (when back, id, endpoint).
    queue: VecDeque<(Instant, usize, u8)>,
    /// When the device is free for the next data transfer.
    free_at: Option<Instant>,
    done: usize,
    pub bytes: usize,
    pub rates: Vec<u32>,
    pub alts: Vec<(u8, u8)>,
    pub loudest: i32,
    /// The rate the clock runs at.
    pub clock: u32,
    /// The last volume set: its unit, and its value.
    pub volume: Option<(u8, i16)>,
    /// Control requests and settings, counted: none may come after the driver has closed.
    pub requests: usize,
    /// Pulled out: every request fails from now on.
    pub gone: bool,
    /// The configuration it runs (1 unless told).
    pub configuration: u8,
    /// The power domain's state when asked (idle, D1, unless told), and the last set.
    pub power_state: Option<u8>,
    pub power: Option<(u8, u8)>,
    pub muted: Option<bool>,
    /// The bottom of its volume in dB, if not -100.
    pub volume_floor: Option<i16>,
}

pub struct FakeDac {
    trouble: Trouble,
    state: Mutex<State>,
}

impl FakeDac {
    pub fn new(trouble: Trouble) -> Self {
        Self {
            trouble,
            state: Mutex::default(),
        }
    }

    pub fn in_configuration(trouble: Trouble, configuration: u8) -> Self {
        let fake = Self::new(trouble);
        fake.state.lock().unwrap().configuration = configuration;
        fake
    }

    pub fn set_clock(&self, rate: u32) {
        self.state.lock().unwrap().clock = rate;
    }

    pub fn set_volume_floor(&self, db: i16) {
        self.state.lock().unwrap().volume_floor = Some(db);
    }

    pub fn set_power_state(&self, state: u8) {
        self.state.lock().unwrap().power_state = Some(state);
    }

    /// The last power domain set, and the state asked.
    pub fn power(&self) -> Option<(u8, u8)> {
        self.state.lock().unwrap().power
    }

    pub fn muted(&self) -> Option<bool> {
        self.state.lock().unwrap().muted
    }

    pub fn bytes(&self) -> usize {
        self.state.lock().unwrap().bytes
    }

    pub fn rates(&self) -> Vec<u32> {
        self.state.lock().unwrap().rates.clone()
    }

    /// What the clock says when asked; `None` when it cannot answer.
    fn clock_says(&self) -> Option<u32> {
        match self.trouble {
            Trouble::NoRateAnswer => None,
            Trouble::StaysAt(rate) | Trouble::FixedAt(rate) => Some(rate),
            _ => Some(self.state.lock().unwrap().clock),
        }
    }

    /// A request to set the rate.
    fn take_rate(&self, rate: u32) -> Result<()> {
        if matches!(self.trouble, Trouble::FixedAt(_)) {
            bail!("The DAC refused the request");
        }
        let mut s = self.state.lock().unwrap();
        s.rates.push(rate);
        if !matches!(self.trouble, Trouble::StaysAt(_)) {
            s.clock = rate;
        }
        Ok(())
    }

    pub fn alts(&self) -> Vec<(u8, u8)> {
        self.state.lock().unwrap().alts.clone()
    }

    pub fn volume(&self) -> Option<(u8, i16)> {
        self.state.lock().unwrap().volume
    }

    pub fn requests(&self) -> usize {
        self.state.lock().unwrap().requests
    }

    /// Pulled out of the phone.
    pub fn unplug(&self) {
        self.state.lock().unwrap().gone = true;
    }

    pub fn forget_loudest(&self) {
        self.state.lock().unwrap().loudest = 0;
    }

    pub fn loudest(&self) -> i32 {
        self.state.lock().unwrap().loudest
    }
}

/// The raw descriptors of two real devices, from their "Copy DAC details".
/// Apple's USB-C to 3.5 mm adapter (05ac:110a): three configurations, UAC2, a UAC3 BADD
/// headphone profile, and full UAC3, with the same interface numbers in each.
pub const APPLE_DAC: &str = "\
12 01 01 02 ef 02 01 40 ac 05 0a 11 50 26 01 02 03 03 09 02 f8 00 04 01 00 a0 96 08 0b 00 02 01
00 20 00 09 04 00 00 01 01 01 20 00 09 24 01 00 02 04 40 00 00 11 24 02 01 01 01 00 09 02 03 00
00 00 00 00 00 00 12 24 06 02 01 03 00 00 00 0c 00 00 00 0c 00 00 00 00 0c 24 03 03 02 03 00 02
09 00 00 00 08 24 0a 09 07 07 00 00 07 05 81 03 06 00 01 09 04 01 00 00 01 02 20 00 09 04 01 01
01 01 02 20 00 10 24 01 01 05 01 01 00 00 00 02 03 00 00 00 00 06 24 02 01 03 18 07 05 02 0d 20
01 01 08 25 01 00 00 01 64 00 09 04 01 02 01 01 02 20 00 10 24 01 01 05 01 01 00 00 00 02 03 00
00 00 00 06 24 02 01 02 10 07 05 02 0d c0 00 01 08 25 01 00 00 01 64 00 09 04 02 00 01 03 00 00
00 09 21 11 01 00 01 22 1f 00 07 05 85 03 40 00 01 09 04 03 00 01 03 00 00 00 09 21 11 01 00 01
22 2a 00 07 05 86 03 40 00 01 09 02 7c 00 04 02 00 a0 96 08 0b 00 02 01 21 30 00 09 04 00 00 01
01 01 30 00 07 05 81 03 06 00 01 09 04 01 00 00 01 02 30 00 09 04 01 01 01 01 02 30 00 07 05 02
0d 20 01 01 09 04 01 02 01 01 02 30 00 07 05 02 0d c0 00 01 09 04 02 00 01 03 00 00 00 09 21 11
01 00 01 22 1f 00 07 05 85 03 40 00 01 09 04 03 00 01 03 00 00 00 09 21 11 01 00 01 22 2a 00 07
05 86 03 40 00 01 09 02 1b 01 04 03 00 a0 96 08 0b 00 02 01 01 30 00 09 04 00 00 01 01 01 30 00
0a 24 01 0f 5d 00 01 00 00 00 14 24 02 01 01 01 00 09 00 00 00 00 20 00 00 00 00 00 00 00 13 24
07 02 01 03 00 00 00 0c 00 00 00 0c 00 00 00 00 00 13 24 03 03 02 03 00 02 09 00 00 00 00 00 00
00 00 00 00 0c 24 0b 09 03 07 00 00 00 00 00 00 0d 24 10 28 58 02 70 17 02 01 03 00 00 07 05 81
03 06 00 01 09 04 01 00 00 01 02 30 00 09 04 01 01 01 01 02 30 00 17 24 01 01 05 00 00 00 20 00
01 00 00 00 00 00 00 00 03 18 00 00 00 07 05 02 0d 20 01 01 0a 25 01 00 00 00 00 01 64 00 09 04
01 02 01 01 02 30 00 17 24 01 01 05 00 00 00 20 00 01 00 00 00 00 00 00 00 02 10 00 00 00 07 05
02 0d c0 00 01 0a 25 01 00 00 00 00 01 64 00 09 04 02 00 01 03 00 00 00 09 21 11 01 00 01 22 1f
00 07 05 85 03 40 00 01 09 04 03 00 01 03 00 00 00 09 21 11 01 00 01 22 2a 00 07 05 86 03 40 00
01";

/// A dock's sound chip (1b3f:2008): UAC1, speakers and a microphone, volume -45 to 0 dB.
pub const DOCK: &str = "\
12 01 10 01 00 00 00 08 3f 1b 08 20 00 01 01 02 00 01 09 02 fd 00 04 01 00 80 32 09 04 00 00 00
01 01 00 00 0a 24 01 00 01 64 00 02 01 02 0c 24 02 01 01 01 00 02 03 00 00 00 0c 24 02 04 01 02
00 01 01 00 00 00 09 24 03 03 01 03 00 06 00 09 24 03 02 01 01 00 09 00 07 24 05 09 01 05 00 0a
24 06 06 08 01 01 02 02 00 09 24 06 05 04 01 43 00 00 09 24 06 07 04 01 03 00 00 0d 24 04 08 02
01 07 02 03 00 00 00 00 09 04 01 00 00 01 02 00 00 09 04 01 01 01 01 02 00 00 07 24 01 01 01 01
00 0e 24 02 01 02 02 10 02 44 ac 00 80 bb 00 09 05 05 01 c0 00 01 00 00 07 25 01 01 00 00 00 09
04 02 00 00 01 02 00 00 09 04 02 01 01 01 02 00 00 07 24 01 02 01 01 00 0e 24 02 01 01 02 10 02
44 ac 00 80 bb 00 09 05 86 01 64 00 01 00 00 07 25 01 01 00 00 00 09 04 03 00 01 03 00 00 00 09
21 01 02 00 01 22 29 00 07 05 83 03 08 00 20";

/// Bytes from their text as "Copy DAC details" gives it.
pub fn hex(text: &str) -> Vec<u8> {
    text.split_whitespace()
        .map(|byte| u8::from_str_radix(byte, 16).unwrap())
        .collect()
}

/// A UAC2 DAC at high speed: one clock (0x29) with 44.1, 48, and 96 kHz, a master volume, and
/// 16-bit and 24-in-32-bit stereo settings with async feedback.
pub fn descriptors() -> Vec<u8> {
    let mut d = vec![];
    d.extend([18, 1, 0, 2, 0xEF, 2, 1, 64, 0, 0, 0, 0, 0, 1, 1, 2, 3, 1]);
    d.extend([9, 2, 0, 0, 2, 1, 0, 0x80, 50]);
    d.extend([9, 4, 0, 0, 0, 1, 1, 0x20, 0]);
    d.extend([9, 0x24, 1, 0x00, 0x02, 0, 0, 0, 0]);
    d.extend([8, 0x24, 0x0A, 0x29, 3, 7, 0, 0]);
    d.extend([
        17, 0x24, 2, 2, 0x01, 0x01, 0, 0x29, 2, 3, 0, 0, 0, 0, 0, 0, 0,
    ]);
    d.extend([18, 0x24, 6, 10, 2, 0x0C, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    d.extend([9, 4, 1, 0, 0, 1, 2, 0x20, 0]);
    d.extend([9, 4, 1, 1, 2, 1, 2, 0x20, 0]);
    d.extend([16, 0x24, 1, 2, 0, 1, 1, 0, 0, 0, 2, 3, 0, 0, 0, 0]);
    d.extend([6, 0x24, 2, 1, 2, 16]);
    d.extend([7, 5, 0x01, 0x05, 0x88, 0x01, 1]);
    d.extend([7, 5, 0x81, 0x11, 4, 0, 4]);
    d.extend([9, 4, 1, 2, 2, 1, 2, 0x20, 0]);
    d.extend([16, 0x24, 1, 2, 0, 1, 1, 0, 0, 0, 2, 3, 0, 0, 0, 0]);
    d.extend([6, 0x24, 2, 1, 4, 24]);
    d.extend([7, 5, 0x01, 0x05, 0x10, 0x03, 1]);
    d.extend([7, 5, 0x81, 0x11, 4, 0, 4]);
    d
}

/// The same with no volume its host can set (the feature unit has only a mute).
pub fn descriptors_without_volume() -> Vec<u8> {
    let mut d = descriptors();
    let at = d.windows(6).position(|w| w == [18, 0x24, 6, 10, 2, 0x0C]);
    d[at.unwrap() + 5] = 0x03;
    d
}

/// What a dock's sound chip may offer (UAC1, as cheap chips are), in the order of its
/// descriptors:
/// - a microphone (a terminal, its feature unit with a volume, an input stream), first, so a
///   driver that takes the first volume it finds takes the microphone's;
/// - interface 1, a headset: stereo 16-bit with a range of rates, 8 to 48 kHz, as a voice chip
///   may give, to a headset terminal (0x0402);
/// - interface 3, the speakers: 16-bit at 44.1 and 48 kHz and 24-bit at 48 kHz, to a speaker
///   terminal (0x0301), with a volume unless told otherwise.
#[derive(Clone, Copy)]
pub struct Dock {
    pub speakers: bool,
    pub volume: bool,
}

impl Default for Dock {
    fn default() -> Self {
        Self {
            speakers: true,
            volume: true,
        }
    }
}

pub fn dock(dock: Dock) -> Vec<u8> {
    let mut d = vec![];
    d.extend([
        18, 1, 0, 2, 0, 0, 0, 64, 0xDA, 0x0B, 0x52, 0x48, 0, 1, 1, 2, 0, 1,
    ]);
    d.extend([9, 2, 0, 0, 4, 1, 0, 0x80, 50]);
    // The audio control interface, UAC 1.00.
    d.extend([9, 4, 0, 0, 0, 1, 1, 0, 0]);
    d.extend([9, 0x24, 1, 0x00, 0x01, 9, 0, 1, 1]);
    // The microphone: terminal 31, feature unit 32 (mute and volume), to USB (terminal 33).
    d.extend([12, 0x24, 2, 31, 0x01, 0x02, 0, 1, 0, 0, 0, 0]);
    d.extend([9, 0x24, 6, 32, 31, 1, 0x03, 0, 0]);
    d.extend([9, 0x24, 3, 33, 0x01, 0x01, 0, 32, 0]);
    // The headset: USB (terminal 11), feature unit 12 (its own volume), headset terminal 13.
    d.extend([12, 0x24, 2, 11, 0x01, 0x01, 0, 2, 3, 0, 0, 0]);
    d.extend([9, 0x24, 6, 12, 11, 1, 0x03, 0, 0]);
    d.extend([9, 0x24, 3, 13, 0x02, 0x04, 0, 12, 0]);
    if dock.speakers {
        // The speakers: USB (terminal 21), feature unit 22, speaker terminal 23.
        d.extend([12, 0x24, 2, 21, 0x01, 0x01, 0, 2, 3, 0, 0, 0]);
        let controls = if dock.volume { 0x03 } else { 0x01 };
        d.extend([9, 0x24, 6, 22, 21, 1, controls, 0, 0]);
        d.extend([9, 0x24, 3, 23, 0x01, 0x03, 0, 22, 0]);
    }
    let stream = |d: &mut Vec<u8>, interface: u8, alt: u8, terminal: u8, format: &[u8], ep: u8| {
        d.extend([9, 4, interface, alt, 1, 1, 2, 0, 0]);
        d.extend([7, 0x24, 1, terminal, 1, 1, 0]);
        d.extend(format);
        d.extend([9, 5, ep, 0x09, 0x40, 0x01, 1, 0, 0]);
        d.extend([7, 0x25, 1, 1, 0, 0, 0]);
    };
    // Interface 1: the headset.
    d.extend([9, 4, 1, 0, 0, 1, 2, 0, 0]);
    let range = [
        14, 0x24, 2, 1, 2, 2, 16, 0, 0x40, 0x1F, 0x00, 0x80, 0xBB, 0x00,
    ];
    stream(&mut d, 1, 1, 11, &range, 0x01);
    // Interface 2: the microphone's stream, into the phone.
    d.extend([9, 4, 2, 0, 0, 1, 2, 0, 0]);
    d.extend([9, 4, 2, 1, 1, 1, 2, 0, 0]);
    d.extend([7, 0x24, 1, 33, 1, 1, 0]);
    d.extend([11, 0x24, 2, 1, 1, 2, 16, 1, 0x80, 0xBB, 0x00]);
    d.extend([9, 5, 0x82, 0x05, 0x60, 0x00, 1, 0, 0]);
    d.extend([7, 0x25, 1, 1, 0, 0, 0]);
    if dock.speakers {
        // Interface 3: the speakers.
        d.extend([9, 4, 3, 0, 0, 1, 2, 0, 0]);
        let cd = [
            14, 0x24, 2, 1, 2, 2, 16, 2, 0x44, 0xAC, 0x00, 0x80, 0xBB, 0x00,
        ];
        stream(&mut d, 3, 1, 21, &cd, 0x03);
        let deep = [11, 0x24, 2, 1, 2, 3, 24, 1, 0x80, 0xBB, 0x00];
        stream(&mut d, 3, 2, 21, &deep, 0x03);
    }
    d
}

/// A UAC1 DAC with the given settings, each `(alt, rates, has a sampling frequency control)`:
/// 24-bit stereo in 3-byte slots.
pub fn uac1_descriptors(settings: &[(u8, &[u32], bool)]) -> Vec<u8> {
    let mut d = vec![];
    d.extend([18, 1, 0, 2, 0, 0, 0, 64, 0, 0, 0, 0, 0, 1, 1, 2, 3, 1]);
    d.extend([9, 2, 0, 0, 2, 1, 0, 0x80, 50]);
    d.extend([9, 4, 0, 0, 0, 1, 1, 0, 0]);
    d.extend([9, 0x24, 1, 0x00, 0x01, 9, 0, 1, 1]);
    d.extend([9, 4, 1, 0, 0, 1, 2, 0, 0]);
    for (alt, rates, control) in settings {
        d.extend([9, 4, 1, *alt, 1, 1, 2, 0, 0]);
        d.extend([7, 0x24, 1, 1, 1, 1, 0]);
        d.extend([
            8 + rates.len() as u8 * 3,
            0x24,
            2,
            1,
            2,
            3,
            24,
            rates.len() as u8,
        ]);
        for r in *rates {
            d.extend(&r.to_le_bytes()[..3]);
        }
        d.extend([9, 5, 0x01, 0x09, 0x20, 0x01, 1, 0, 0]);
        d.extend([7, 0x25, 1, *control as u8, 0, 0, 0]);
    }
    d
}

impl Bus for FakeDac {
    fn high_speed(&self) -> bool {
        true
    }

    fn claim(&self, _interface: u8) {}

    fn set_interface(&self, interface: u8, alt: u8) -> Result<()> {
        if self.trouble == Trouble::Panic && alt != 0 {
            panic!("a pretend bug");
        }
        let mut s = self.state.lock().unwrap();
        if s.gone {
            bail!("The DAC is gone");
        }
        s.alts.push((interface, alt));
        s.requests += 1;
        Ok(())
    }

    fn control(
        &self,
        request_type: u8,
        request: u8,
        value: u16,
        index: u16,
        data: &mut [u8],
    ) -> Result<usize> {
        let selector = (value >> 8) as u8;
        self.state.lock().unwrap().requests += 1;
        match (request_type, request, selector) {
            // GET_CONFIGURATION.
            (0x80, 0x08, _) => {
                data[0] = self.state.lock().unwrap().configuration.max(1);
                Ok(1)
            }
            // A power domain: its state, and setting it (one byte, not a volume's two).
            (0xA1, 0x01, 0x02) if data.len() == 1 => {
                data[0] = self.state.lock().unwrap().power_state.unwrap_or(1);
                Ok(1)
            }
            (0x21, 0x01, 0x02) if data.len() == 1 => {
                self.state.lock().unwrap().power = Some(((index >> 8) as u8, data[0]));
                Ok(1)
            }
            // Mute (one byte, not a rate's four).
            (0x21, 0x01, 0x01) if data.len() == 1 => {
                self.state.lock().unwrap().muted = Some(data[0] != 0);
                Ok(1)
            }
            // UAC3: a cluster descriptor's head, with two channels at its seventh byte.
            (0xA1, 0x06, _) => {
                data.copy_from_slice(&[7, 0, 0x24, 1, 0x20, 0, 2]);
                Ok(7)
            }
            (0xA1, 0x02, 0x01) if self.trouble == Trouble::NoClockAnswer => {
                bail!("The DAC refused the request");
            }
            // The clock's rates: three subranges of one rate each.
            (0xA1, 0x02, 0x01) => {
                let mut answer = vec![3, 0];
                for r in [44_100u32, 48_000, 96_000] {
                    answer.extend(r.to_le_bytes());
                    answer.extend(r.to_le_bytes());
                    answer.extend(0u32.to_le_bytes());
                }
                let n = data.len().min(answer.len());
                data[..n].copy_from_slice(&answer[..n]);
                Ok(n)
            }
            // The volume's range: -100 dB to 0.
            (0xA1, 0x02, 0x02) => {
                let mut answer = vec![1, 0];
                answer.extend((-100i16 * 256).to_le_bytes());
                answer.extend(0i16.to_le_bytes());
                answer.extend(256i16.to_le_bytes());
                let n = data.len().min(answer.len());
                data[..n].copy_from_slice(&answer[..n]);
                Ok(n)
            }
            // Clock rate now, for a pretend clock not yet running: no answer.
            (0xA1, 0x01, 0x01) if self.clock_says() == Some(0) => {
                bail!("The DAC refused the request")
            }
            // UAC1's volume: -100 dB to 0.
            (0xA1, 0x82 | 0x83, 0x02) => {
                let floor = self.state.lock().unwrap().volume_floor.unwrap_or(-100);
                let value: i16 = if request == 0x82 { floor * 256 } else { 0 };
                data[..2].copy_from_slice(&value.to_le_bytes());
                Ok(2)
            }
            // Setting the volume.
            (0x21, 0x01, 0x02) => {
                self.state.lock().unwrap().volume =
                    Some(((index >> 8) as u8, i16::from_le_bytes([data[0], data[1]])));
                Ok(data.len())
            }
            // Setting the rate: UAC2's clock, or UAC1's endpoint.
            (0x21, 0x01, 0x01) => {
                self.take_rate(u32::from_le_bytes([data[0], data[1], data[2], data[3]]))?;
                Ok(data.len())
            }
            (0x22, 0x01, 0x01) => {
                self.take_rate(u32::from_le_bytes([data[0], data[1], data[2], 0]))?;
                Ok(data.len())
            }
            // Reading the rate back.
            (0xA1, 0x01, 0x01) | (0xA2, 0x81, 0x01) => {
                let Some(rate) = self.clock_says() else {
                    bail!("The DAC refused the request");
                };
                let n = data.len().min(4);
                data[..n].copy_from_slice(&rate.to_le_bytes()[..n]);
                Ok(n)
            }
            _ => Ok(data.len()),
        }
    }

    fn submit(&self, id: usize, endpoint: u8, bytes: &[u8], lengths: &[usize]) -> Result<()> {
        let mut s = self.state.lock().unwrap();
        if s.gone {
            bail!("The DAC took no more music");
        }
        if let Trouble::GoneAfter(n) = self.trouble {
            if s.done >= n {
                bail!("The DAC took no more music");
            }
        }
        let now = Instant::now();
        let due = if endpoint & 0x80 != 0 {
            // Feedback, every few milliseconds.
            now + Duration::from_millis(8)
        } else {
            // A transfer of 32 high-speed packets is 4 ms of music, played one after another.
            let start = s.free_at.filter(|t| *t > now).unwrap_or(now);
            let end = start + Duration::from_micros(125 * lengths.len() as u64);
            s.free_at = Some(end);
            s.bytes += bytes.len();
            // 24-bit music in 32-bit slots: how loud it got.
            for c in bytes.chunks_exact(4) {
                let v = i32::from_le_bytes([c[0], c[1], c[2], c[3]]) >> 8;
                s.loudest = s.loudest.max(v.abs());
            }
            end
        };
        s.queue.push_back((due, id, endpoint));
        Ok(())
    }

    fn reap(&self, wait_ms: u32) -> Result<Option<Reaped>> {
        let until = Instant::now() + Duration::from_millis(wait_ms as u64);
        loop {
            {
                let mut s = self.state.lock().unwrap();
                if s.gone {
                    bail!("The DAC is gone");
                }
                if let Trouble::GoneAfter(n) = self.trouble {
                    if s.done >= n {
                        bail!("The DAC is gone");
                    }
                }
                let stalled = matches!(self.trouble, Trouble::StallAfter(n) if s.done >= n);
                let now = Instant::now();
                if !stalled {
                    if let Some(i) = s.queue.iter().position(|(due, ..)| *due <= now) {
                        let (_, id, endpoint) = s.queue.remove(i).unwrap();
                        s.done += 1;
                        // Feedback: the nominal 44.1 kHz in 16.16 per microframe.
                        let data = if endpoint & 0x80 != 0 {
                            super::packets::nominal(44_100, 8000).to_le_bytes().to_vec()
                        } else {
                            vec![]
                        };
                        return Ok(Some(Reaped {
                            id,
                            status: 0,
                            data,
                        }));
                    }
                }
            }
            if Instant::now() >= until {
                return Ok(None);
            }
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    fn discard(&self, id: usize) {
        let mut s = self.state.lock().unwrap();
        // A stalled device does not give back even what is discarded.
        if matches!(self.trouble, Trouble::StallAfter(n) if s.done >= n) {
            return;
        }
        let now = Instant::now();
        for q in s.queue.iter_mut().filter(|q| q.1 == id) {
            q.0 = now;
        }
    }

    fn busy(&self) -> usize {
        self.state.lock().unwrap().queue.len()
    }
}

/// Checks for the driver with the real player (run from `platform_output_tests`: the output is
/// one for the whole program, so one player runs every check).
pub mod checks {
    use super::*;
    use crate::{Needle, usb_dac::driver::UsbDac};
    use needle_core::platform_output::set_platform_output;
    use std::sync::Arc;

    fn wait(what: &str, limit: Duration, mut done: impl FnMut() -> bool) {
        let start = Instant::now();
        while !done() {
            assert!(start.elapsed() < limit, "timed out waiting for {what}");
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    /// The driver gave up (its own report, fresh) and the core said so in its notice. The core
    /// says it once the system's output opens: without one (see FALLBACK), the report is all.
    fn said(needle: &Needle, words: &str, limit: Duration, fallback: bool) -> bool {
        let start = Instant::now();
        while start.elapsed() < limit {
            let reported = super::super::STATUS.lock().unwrap().1.is_some();
            if reported
                && (!fallback
                    || needle
                        .player
                        .state()
                        .output_notice
                        .is_some_and(|n| n.contains(words)))
            {
                return true;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        false
    }

    fn driver(trouble: Trouble) -> (Arc<FakeDac>, Arc<UsbDac>) {
        let fake = Arc::new(FakeDac::new(trouble));
        let raw = descriptors();
        let dac = Arc::new(UsbDac::new(fake.clone(), &raw, "Pretend DAC".into(), 1., &[]).unwrap());
        (fake, dac)
    }

    /// A DAC with no volume of its own: the phone's volume scales the samples, and the status
    /// says it is not bit-perfect meanwhile. Then, with a system's output to go to (see
    /// FALLBACK), closing the driver ends all talk with the DAC before Android gets it back:
    /// Android's own driver takes the interfaces at that moment.
    fn no_volume_and_the_way_back_to_android(needle: &Needle, fallback: bool) {
        let fake = Arc::new(FakeDac::new(Trouble::None));
        let raw = descriptors_without_volume();
        let dac = Arc::new(UsbDac::new(fake.clone(), &raw, "Pretend DAC".into(), 1., &[]).unwrap());
        assert!(!dac.info().volume);
        crate::usb_dac::use_driver(dac);
        wait("the DAC playing", Duration::from_secs(10), || {
            fake.bytes() > 100_000
        });
        // The player's volume is 0 for silence on the system's output; only the DAC plays now.
        needle.player.send(needle_core::audio::Command::Volume(1.));
        wait("music at the DAC", Duration::from_secs(5), || {
            fake.loudest() > 1000
        });
        assert!(needle.usb_dac_status().bit_perfect);
        fake.forget_loudest();
        std::thread::sleep(Duration::from_millis(600));
        let full = fake.loudest();
        needle.usb_dac_volume(0.25);
        std::thread::sleep(Duration::from_millis(100));
        fake.forget_loudest();
        std::thread::sleep(Duration::from_millis(600));
        let low = fake.loudest();
        assert!(
            low > 0 && (low as f32) < full as f32 * 0.4,
            "{low} of {full}, at about a fifth"
        );
        assert!(!needle.usb_dac_status().bit_perfect);
        needle.usb_dac_volume(1.);
        assert!(needle.usb_dac_status().bit_perfect);

        // Off: when the call returns, the driver has stopped and let the DAC rest.
        needle.player.send(needle_core::audio::Command::Volume(0.));
        wait("the volume at 0", Duration::from_secs(3), || {
            !needle.usb_dac_status().bit_perfect
        });
        if !fallback {
            return;
        }
        needle.usb_dac_close();
        assert_eq!(fake.alts().last(), Some(&(1, 0)));
        assert_eq!(
            Arc::strong_count(&fake),
            1,
            "the driver still holds the DAC"
        );
        let requests = fake.requests();
        std::thread::sleep(Duration::from_millis(300));
        assert_eq!(
            fake.requests(),
            requests,
            "the driver spoke to the DAC after closing"
        );
    }

    /// What the app does when a DAC is pulled out and plugged in again, with the system's output
    /// between (see FALLBACK), so it needs one: `usb_dac_close`, then a new driver for the new
    /// device. The old driver's last word must not reach the new one, the song goes on from
    /// where it was, and a pause stays a pause with a Play that works.
    fn unplugged_and_plugged_in_again(needle: &Needle, ids: Vec<String>) {
        needle.play(ids, 0).unwrap();
        let (old, dac) = driver(Trouble::None);
        crate::usb_dac::use_driver(dac);
        wait("music at the DAC", Duration::from_secs(10), || {
            old.bytes() > 100_000
        });

        // Pulled out while it plays: an error at once, not read yet, as the app may first hear
        // of it from Android.
        old.unplug();
        wait("the driver's word", Duration::from_secs(3), || {
            super::super::STATUS.lock().unwrap().1.is_some()
        });
        needle.usb_dac_close();
        let status = needle.usb_dac_status();
        assert_eq!((status.playing, status.problem), (None, None));
        assert_eq!(
            Arc::strong_count(&old),
            1,
            "the old driver still holds the DAC"
        );

        // The system's output has the song, which goes on.
        wait("the song going on", Duration::from_secs(5), || {
            needle.playback().playing
        });
        let at = needle.playback().position;
        wait("the song moving", Duration::from_secs(5), || {
            needle.playback().position > at + 0.3
        });

        // Plugged in again, with the switch on: the new driver plays on from there and stays.
        let at = needle.playback().position;
        let (new, dac) = driver(Trouble::None);
        crate::usb_dac::use_driver(dac);
        wait("music at the new DAC", Duration::from_secs(10), || {
            new.bytes() > 100_000
        });
        assert!(
            needle.playback().position >= at - 0.5,
            "the song began again"
        );
        std::thread::sleep(Duration::from_millis(800));
        let status = needle.usb_dac_status();
        assert_eq!(
            status.problem, None,
            "the old driver's word reached the new one"
        );
        assert!(status.playing.is_some());

        // Paused, then pulled out, and Android's output takes over still paused.
        needle.toggle();
        wait("the DAC resting", Duration::from_secs(3), || {
            new.alts().last() == Some(&(1, 0))
        });
        new.unplug();
        needle.usb_dac_close();
        std::thread::sleep(Duration::from_millis(300));
        assert!(!needle.playback().playing, "a pause stays a pause");

        // Plugged in again: still paused, nothing sent, and Play plays.
        let (again, dac) = driver(Trouble::None);
        crate::usb_dac::use_driver(dac);
        std::thread::sleep(Duration::from_millis(500));
        assert!(!needle.playback().playing, "a pause stays a pause");
        assert_eq!(again.bytes(), 0);
        needle.toggle();
        wait("music after Play", Duration::from_secs(5), || {
            again.bytes() > 100_000
        });

        // Paused, pulled out, and Android has not told the app yet: Play is an error at once,
        // and the song goes on through the system's output, not silence.
        needle.toggle();
        wait("the DAC resting", Duration::from_secs(3), || {
            again.alts().last() == Some(&(1, 0))
        });
        again.unplug();
        needle.toggle();
        wait("the driver's word", Duration::from_secs(3), || {
            super::super::STATUS.lock().unwrap().1.is_some()
        });
        wait("the song going on", Duration::from_secs(5), || {
            needle.playback().playing
        });
        let at = needle.playback().position;
        wait("the song moving", Duration::from_secs(5), || {
            needle.playback().position > at + 0.3
        });
        needle.usb_dac_close();
    }

    /// The driver on a pretend DAC: it plays, rests while paused, and every trouble (a stall,
    /// the DAC gone, a panic, a rate it does not play) ends in an error within a second or two,
    /// with the song going on through the system's output. `ids` are songs at 44.1 kHz.
    pub fn the_driver_plays_and_every_trouble_ends_in_an_error(
        needle: &Needle,
        music: &std::path::Path,
        ids: Vec<String>,
        fallback: bool,
    ) {
        // Silent: after a trouble the system's output takes over.
        needle.player.send(needle_core::audio::Command::Volume(0.));
        let (fake, dac) = driver(Trouble::None);
        let info = dac.info();
        assert_eq!(
            (info.uac, info.bits, info.rates.clone(), info.volume),
            (2, 24, vec![44_100, 48_000, 96_000], true)
        );
        set_platform_output(Some(dac));

        // It plays: 24-bit in 4-byte slots at the song's rate, in real time, and not silence.
        needle.play(ids.clone(), 0).unwrap();
        // (The song playing before moves over first, at its own rate.)
        wait(
            "the 44.1 kHz song at the DAC",
            Duration::from_secs(10),
            || fake.rates().last() == Some(&44_100),
        );
        wait("music at the DAC", Duration::from_secs(10), || {
            fake.bytes() > 100_000
        });
        assert_eq!(fake.alts().last(), Some(&(1, 2)));
        assert!(fake.loudest() > 1000, "music, not silence");
        let before = fake.bytes();
        std::thread::sleep(Duration::from_secs(1));
        let second = fake.bytes() - before;
        assert!(
            (250_000..450_000).contains(&second),
            "{second} bytes a second (44.1 kHz × 8 = 352 800)"
        );
        assert!(super::super::STATUS.lock().unwrap().0.as_deref() == Some("44.1 kHz, 24-bit"));

        // Paused: the DAC rests (setting 0), and plays again after.
        needle.toggle();
        wait("the DAC resting", Duration::from_secs(3), || {
            fake.alts().last() == Some(&(1, 0))
        });
        needle.toggle();
        wait("playing again", Duration::from_secs(3), || {
            fake.alts().last() == Some(&(1, 2))
        });

        no_volume_and_the_way_back_to_android(needle, fallback);
        if fallback {
            unplugged_and_plugged_in_again(needle, ids.clone());
        }

        // Each trouble: an error within a second, and the song goes on through the system.
        for (trouble, words) in [
            (Trouble::StallAfter(40), &["stopped taking music"][..]),
            (Trouble::GoneAfter(40), &["gone", "no more music"][..]),
            (Trouble::Panic, &["driver stopped"][..]),
            // A clock that stays at 48 kHz, for a song at 44.1: not played fast and high.
            (
                Trouble::StaysAt(48_000),
                &["stayed at 48 kHz, so it cannot play 44.1 kHz bit-perfect"][..],
            ),
        ] {
            let (_, dac) = driver(trouble);
            let at = needle.playback().position;
            set_platform_output(Some(dac));
            let start = Instant::now();
            assert!(
                said(needle, "Pretend DAC", Duration::from_secs(5), fallback),
                "{trouble:?}: the failure is said"
            );
            assert!(
                start.elapsed() < Duration::from_secs(3),
                "{trouble:?}: it took {:?}",
                start.elapsed()
            );
            let why = super::super::STATUS
                .lock()
                .unwrap()
                .1
                .take()
                .unwrap_or_default();
            assert!(words.iter().any(|w| why.contains(w)), "{trouble:?}: {why}");
            // Without the system's output (see FALLBACK), the first trouble is the last check.
            if !fallback {
                return;
            }
            std::thread::sleep(Duration::from_millis(800));
            assert!(
                needle.playback().position > at,
                "{trouble:?}: the song went on"
            );
        }

        // A song at a rate the DAC does not play: an error, not silence.
        let (_, dac) = driver(Trouble::None);
        set_platform_output(Some(dac));
        let odd = music.join("05 - Odd.wav");
        let frames = 32_000 * 3;
        let mut wav = vec![];
        wav.extend(b"RIFF");
        wav.extend((36 + frames * 4u32).to_le_bytes());
        wav.extend(b"WAVEfmt ");
        wav.extend(16u32.to_le_bytes());
        wav.extend(1u16.to_le_bytes());
        wav.extend(2u16.to_le_bytes());
        wav.extend(32_000u32.to_le_bytes());
        wav.extend((32_000u32 * 4).to_le_bytes());
        wav.extend(4u16.to_le_bytes());
        wav.extend(16u16.to_le_bytes());
        wav.extend(b"data");
        wav.extend((frames * 4).to_le_bytes());
        wav.extend(vec![0u8; (frames * 4) as usize]);
        std::fs::write(&odd, wav).unwrap();
        needle.rescan();
        wait("the odd song", Duration::from_secs(30), || {
            needle
                .songs()
                .unwrap_or_default()
                .iter()
                .any(|s| s.title.contains("Odd"))
        });
        let odd_id = needle
            .songs()
            .unwrap()
            .into_iter()
            .find(|s| s.title.contains("Odd"))
            .unwrap()
            .id;
        needle.play(vec![odd_id], 0).unwrap();
        assert!(
            said(needle, "Pretend DAC", Duration::from_secs(5), fallback),
            "32 kHz: the failure is said"
        );
        let why = super::super::STATUS
            .lock()
            .unwrap()
            .1
            .take()
            .unwrap_or_default();
        assert!(why.contains("32000 Hz"), "{why}");

        set_platform_output(None);
        needle.toggle();
    }
}
