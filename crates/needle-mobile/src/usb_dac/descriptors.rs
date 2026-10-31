//! A USB DAC's descriptors, read: what it is (USB Audio Class 1 or 2), where its music goes
//! (the streaming interface, its alternate settings, their formats and endpoints), its clock,
//! and its volume control. Pure data: the device itself is spoken to in `device.rs`.

/// USB Audio Class version.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Uac {
    One,
    Two,
}

/// How the DAC keeps time against the phone.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sync {
    /// The DAC's own clock; it says how many samples it wants (the feedback endpoint).
    Async,
    /// The DAC follows the stream.
    Adaptive,
    /// Locked to the USB frames.
    Synchronous,
    None,
}

/// One way the DAC takes music: an alternate setting of a streaming interface.
#[derive(Clone, Debug, PartialEq)]
pub struct Format {
    pub interface: u8,
    pub alt: u8,
    pub channels: u8,
    /// Bytes per sample in the stream: 2, 3, or 4.
    pub subslot: u8,
    /// Bits of each sample that carry music (16, 24, or 32).
    pub bits: u8,
    /// Rates it lists itself (UAC1); empty for UAC2, where the clock says.
    pub rates: Vec<u32>,
    /// A UAC1 range of rates instead of a list (low, high).
    pub range: Option<(u32, u32)>,
    /// UAC1: the endpoint has a sampling frequency control (its own descriptor says). Without
    /// one, the setting itself decides the rate. Always true for UAC2, where the clock does.
    pub rate_control: bool,
    pub endpoint: u8,
    pub max_packet: u16,
    /// Polling interval as the descriptor gives it.
    pub interval: u8,
    pub sync: Sync,
    /// The feedback endpoint (IN), for an asynchronous DAC.
    pub feedback: Option<u8>,
    /// The terminal the stream enters by (its clock, its volume).
    pub terminal: u8,
}

/// The volume control on the way to the DAC's output.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Volume {
    pub unit: u8,
    /// The master channel has a volume control (else channel 1 is used).
    pub master: bool,
}

/// What a USB DAC offers.
#[derive(Clone, Debug, PartialEq)]
pub struct Dac {
    pub uac: Uac,
    /// The Audio Control interface's number.
    pub control: u8,
    pub formats: Vec<Format>,
    /// UAC2: the clock source each input terminal uses, by terminal id.
    pub clocks: Vec<(u8, u8)>,
    /// UAC2: clock selectors and their first source.
    pub selectors: Vec<(u8, u8)>,
    pub volume: Option<Volume>,
    /// The DAC takes DSD as raw data itself (UAC2 Type I raw), beside PCM.
    pub native_dsd: bool,
}

impl Dac {
    /// The clock (a clock source) that sets the rate of music entering at `terminal`.
    pub fn clock_for(&self, terminal: u8) -> Option<u8> {
        let mut id = self
            .clocks
            .iter()
            .find(|(t, _)| *t == terminal)
            .map(|(_, c)| *c)?;
        // A selector: follow it to its first source (as Linux does unless told otherwise).
        for _ in 0..4 {
            match self.selectors.iter().find(|(s, _)| *s == id) {
                Some((_, source)) => id = *source,
                None => break,
            }
        }
        Some(id)
    }
}

const INTERFACE: u8 = 0x04;
const ENDPOINT: u8 = 0x05;
const CS_INTERFACE: u8 = 0x24;
const CS_ENDPOINT: u8 = 0x25;
const AUDIO: u8 = 0x01;
const CONTROL: u8 = 0x01;
const STREAMING: u8 = 0x02;

fn u16_at(d: &[u8], i: usize) -> u16 {
    d.get(i..i + 2)
        .map_or(0, |b| u16::from_le_bytes([b[0], b[1]]))
}

fn u24_at(d: &[u8], i: usize) -> u32 {
    d.get(i..i + 3).map_or(0, |b| {
        b[0] as u32 | (b[1] as u32) << 8 | (b[2] as u32) << 16
    })
}

fn u32_at(d: &[u8], i: usize) -> u32 {
    d.get(i..i + 4)
        .map_or(0, |b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

/// Read a device's raw descriptors (the device descriptor, then its configuration with all of
/// its interfaces, as Android's `UsbDeviceConnection.getRawDescriptors` gives them). None for
/// a device that is not an audio device Needle can play to.
pub fn parse(raw: &[u8]) -> Option<Dac> {
    let mut uac = None;
    let mut control = None;
    let mut formats = vec![];
    let mut clocks = vec![];
    let mut selectors = vec![];
    let mut volume = None;
    let mut native_dsd = false;
    // Where the walk is: the interface (number, alt, class, subclass), and the streaming
    // alt setting being read.
    let mut here: Option<(u8, u8, u8, u8)> = None;
    let mut pending: Option<Format> = None;
    let mut pcm = true;
    let mut i = 0;
    while i + 2 <= raw.len() {
        let len = raw[i] as usize;
        if len < 2 || i + len > raw.len() {
            break;
        }
        let d = &raw[i..i + len];
        match d[1] {
            INTERFACE if len >= 9 => {
                if let Some(f) = pending.take() {
                    if pcm && f.endpoint != 0 {
                        formats.push(f);
                    }
                }
                here = Some((d[2], d[3], d[5], d[6]));
                pcm = true;
                if d[5] == AUDIO && d[6] == CONTROL {
                    control = Some(d[2]);
                }
                if d[5] == AUDIO && d[6] == STREAMING && d[4] > 0 {
                    pending = Some(Format {
                        interface: d[2],
                        alt: d[3],
                        channels: 0,
                        subslot: 0,
                        bits: 0,
                        rates: vec![],
                        range: None,
                        rate_control: true,
                        endpoint: 0,
                        max_packet: 0,
                        interval: 1,
                        sync: Sync::None,
                        feedback: None,
                        terminal: 0,
                    });
                }
            }
            CS_INTERFACE if len >= 3 => match here {
                Some((_, _, AUDIO, CONTROL)) => match d[2] {
                    // HEADER: bcdADC.
                    0x01 => {
                        uac = Some(if u16_at(d, 3) >= 0x0200 {
                            Uac::Two
                        } else {
                            Uac::One
                        })
                    }
                    // INPUT_TERMINAL (UAC2: its clock source).
                    0x02 if uac == Some(Uac::Two) && len >= 8 => clocks.push((d[3], d[7])),
                    // FEATURE_UNIT: the first with a volume control.
                    0x06 if volume.is_none() && len >= 7 => {
                        let unit = d[3];
                        match uac {
                            Some(Uac::Two) => {
                                let master = u32_at(d, 5);
                                let first = u32_at(d, 9);
                                // Volume is bits 2-3 of each channel's controls.
                                if master & 0b1100 != 0 {
                                    volume = Some(Volume { unit, master: true });
                                } else if first & 0b1100 != 0 {
                                    volume = Some(Volume {
                                        unit,
                                        master: false,
                                    });
                                }
                            }
                            _ => {
                                let size = d[5].max(1) as usize;
                                let master = d.get(6).copied().unwrap_or(0);
                                let first = d.get(6 + size).copied().unwrap_or(0);
                                // Volume is bit 1.
                                if master & 0b10 != 0 {
                                    volume = Some(Volume { unit, master: true });
                                } else if first & 0b10 != 0 {
                                    volume = Some(Volume {
                                        unit,
                                        master: false,
                                    });
                                }
                            }
                        }
                    }
                    // CLOCK_SELECTOR (UAC2): its first source.
                    0x0B if len >= 6 => selectors.push((d[3], d[5])),
                    _ => {}
                },
                Some((_, _, AUDIO, STREAMING)) => {
                    let Some(f) = pending.as_mut() else {
                        i += len;
                        continue;
                    };
                    match d[2] {
                        // AS_GENERAL.
                        0x01 => {
                            f.terminal = d.get(3).copied().unwrap_or(0);
                            match uac {
                                Some(Uac::Two) if len >= 11 => {
                                    let formats = u32_at(d, 6);
                                    pcm = formats & 1 != 0;
                                    if formats & (1 << 31) != 0 {
                                        native_dsd = true;
                                    }
                                    f.channels = d[10];
                                }
                                _ => pcm = u16_at(d, 5) == 0x0001,
                            }
                        }
                        // FORMAT_TYPE I.
                        0x02 if d.get(3) == Some(&1) => match uac {
                            Some(Uac::Two) if len >= 6 => {
                                f.subslot = d[4];
                                f.bits = d[5];
                            }
                            _ if len >= 8 => {
                                f.channels = d[4];
                                f.subslot = d[5];
                                f.bits = d[6];
                                let count = d[7] as usize;
                                if count == 0 {
                                    f.range = Some((u24_at(d, 8), u24_at(d, 11)));
                                } else {
                                    f.rates = (0..count)
                                        .map(|k| u24_at(d, 8 + k * 3))
                                        .filter(|r| *r > 0)
                                        .collect();
                                }
                            }
                            _ => {}
                        },
                        _ => {}
                    }
                }
                _ => {}
            },
            // EP_GENERAL (UAC1): bit 0 of its attributes is the sampling frequency control.
            CS_ENDPOINT if len >= 4 && d[2] == 0x01 && uac == Some(Uac::One) => {
                if let Some(f) = pending.as_mut() {
                    f.rate_control = d[3] & 1 != 0;
                }
            }
            ENDPOINT if len >= 7 => {
                if let Some(f) = pending.as_mut() {
                    let address = d[2];
                    let attributes = d[3];
                    // Isochronous only.
                    if attributes & 0b11 == 0b01 {
                        let usage = (attributes >> 4) & 0b11;
                        if address & 0x80 == 0 && usage == 0 {
                            f.endpoint = address;
                            f.max_packet = u16_at(d, 4);
                            f.interval = d[6].max(1);
                            f.sync = match (attributes >> 2) & 0b11 {
                                1 => Sync::Async,
                                2 => Sync::Adaptive,
                                3 => Sync::Synchronous,
                                _ => Sync::None,
                            };
                        } else if address & 0x80 != 0 && usage == 1 {
                            f.feedback = Some(address);
                        }
                    }
                }
            }
            _ => {}
        }
        i += len;
    }
    if let Some(f) = pending.take() {
        if pcm && f.endpoint != 0 {
            formats.push(f);
        }
    }
    formats.retain(|f| f.channels > 0 && matches!(f.subslot, 2..=4) && f.bits >= 16);
    if formats.is_empty() {
        return None;
    }
    Some(Dac {
        uac: uac?,
        control: control?,
        formats,
        clocks,
        selectors,
        volume,
        native_dsd,
    })
}

/// The rates a DAC is asked about (a UAC2 clock answers with ranges).
pub const RATES: [u32; 12] = [
    44_100, 48_000, 88_200, 96_000, 176_400, 192_000, 352_800, 384_000, 705_600, 768_000, 32_000,
    22_050,
];

/// The rates in a UAC2 clock's answer to RANGE: subranges of (min, max, resolution).
pub fn rates_in_ranges(answer: &[u8]) -> Vec<u32> {
    let count = u16_at(answer, 0) as usize;
    let mut out = vec![];
    for k in 0..count {
        let at = 2 + k * 12;
        if at + 12 > answer.len() {
            break;
        }
        let (min, max, res) = (
            u32_at(answer, at),
            u32_at(answer, at + 4),
            u32_at(answer, at + 8),
        );
        if min == max {
            out.push(min);
            continue;
        }
        for r in RATES {
            if r >= min && r <= max && (res == 0 || (r - min) % res == 0) {
                out.push(r);
            }
        }
    }
    out.sort_unstable();
    out.dedup();
    out
}

/// A format's rates, from its own list or range (UAC1).
pub fn listed_rates(f: &Format) -> Vec<u32> {
    match f.range {
        Some((low, high)) => RATES
            .iter()
            .copied()
            .filter(|r| *r >= low && *r <= high)
            .collect(),
        None => f.rates.clone(),
    }
}

/// The best way to send music of `channels` and `bits`: as many channels, else stereo for
/// mono; the deepest samples (music of 16 bits goes out as it is in a deeper slot too).
pub fn choose(formats: &[Format], channels: u16, wanted_bits: u8) -> Option<&Format> {
    let fits = |f: &&Format| f.channels as u16 == channels || (channels == 1 && f.channels == 2);
    formats.iter().filter(fits).max_by_key(|f| {
        // Deep enough first, then deeper, then the one Linux would pick (the lowest alt).
        (
            (f.bits >= wanted_bits) as u8,
            f.bits,
            f.subslot,
            255 - f.alt,
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A UAC2 DAC as many are: an audio control interface with a clock source, an input
    /// terminal, a feature unit with volume, and one streaming interface whose alternate
    /// settings take 16-bit and 32-bit stereo, with an async feedback endpoint.
    fn uac2_dac() -> Vec<u8> {
        let mut d = vec![];
        // Device descriptor (18 bytes, values unused).
        d.extend([18, 1, 0, 2, 0xEF, 2, 1, 64, 0, 0, 0, 0, 0, 1, 1, 2, 3, 1]);
        // Configuration.
        d.extend([9, 2, 0, 0, 2, 1, 0, 0x80, 50]);
        // Interface 0: audio control (protocol 0x20).
        d.extend([9, 4, 0, 0, 0, 1, 1, 0x20, 0]);
        // Header, bcdADC 2.00.
        d.extend([9, 0x24, 1, 0x00, 0x02, 0, 0, 0, 0]);
        // Clock source 0x29.
        d.extend([8, 0x24, 0x0A, 0x29, 3, 7, 0, 0]);
        // Input terminal 2, USB streaming, clock 0x29.
        d.extend([
            17, 0x24, 2, 2, 0x01, 0x01, 0, 0x29, 2, 3, 0, 0, 0, 0, 0, 0, 0,
        ]);
        // Feature unit 10 from 2: master volume read/write (0x0C).
        d.extend([18, 0x24, 6, 10, 2, 0x0C, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        // Interface 1 alt 0: no endpoints.
        d.extend([9, 4, 1, 0, 0, 1, 2, 0x20, 0]);
        // Interface 1 alt 1: 16-bit stereo.
        d.extend([9, 4, 1, 1, 2, 1, 2, 0x20, 0]);
        d.extend([16, 0x24, 1, 2, 0, 1, 1, 0, 0, 0, 2, 3, 0, 0, 0, 0]);
        d.extend([6, 0x24, 2, 1, 2, 16]);
        // Endpoint 0x01 OUT, iso async (0x05), 392 bytes, interval 1.
        d.extend([7, 5, 0x01, 0x05, 0x88, 0x01, 1]);
        d.extend([8, 0x25, 1, 0, 0, 0, 0, 0]);
        // Feedback endpoint 0x81 IN, iso, usage feedback (0x11), 4 bytes, interval 4.
        d.extend([7, 5, 0x81, 0x11, 4, 0, 4]);
        // Interface 1 alt 2: 32-bit (24 used) stereo, DSD raw too.
        d.extend([9, 4, 1, 2, 2, 1, 2, 0x20, 0]);
        d.extend([16, 0x24, 1, 2, 0, 1, 1, 0, 0, 0x80, 2, 3, 0, 0, 0, 0]);
        d.extend([6, 0x24, 2, 1, 4, 24]);
        d.extend([7, 5, 0x01, 0x05, 0x10, 0x03, 1]);
        d.extend([8, 0x25, 1, 0, 0, 0, 0, 0]);
        d.extend([7, 5, 0x81, 0x11, 4, 0, 4]);
        d
    }

    #[test]
    fn a_uac2_dac_is_read() {
        let dac = parse(&uac2_dac()).expect("a DAC");
        assert_eq!(dac.uac, Uac::Two);
        assert_eq!(dac.control, 0);
        assert_eq!(dac.formats.len(), 2);
        let deep = &dac.formats[1];
        assert_eq!(
            (
                deep.interface,
                deep.alt,
                deep.channels,
                deep.subslot,
                deep.bits
            ),
            (1, 2, 2, 4, 24)
        );
        assert_eq!(
            (deep.endpoint, deep.max_packet, deep.sync, deep.feedback),
            (0x01, 784, Sync::Async, Some(0x81))
        );
        assert_eq!(dac.clock_for(deep.terminal), Some(0x29));
        assert_eq!(
            dac.volume,
            Some(Volume {
                unit: 10,
                master: true
            })
        );
        assert!(dac.native_dsd);
        assert_eq!(choose(&dac.formats, 2, 24).map(|f| f.alt), Some(2));
        assert_eq!(choose(&dac.formats, 1, 16).map(|f| f.alt), Some(2));
        assert_eq!(choose(&dac.formats, 6, 16), None);
    }

    #[test]
    fn a_uac1_dac_lists_its_rates() {
        let mut d = vec![];
        d.extend([9, 4, 0, 0, 0, 1, 1, 0, 0]);
        d.extend([9, 0x24, 1, 0x00, 0x01, 0, 0, 1, 1]);
        // Feature unit 2: master mute and volume (0x03), control size 1.
        d.extend([9, 0x24, 6, 2, 1, 1, 0x03, 0, 0]);
        d.extend([9, 4, 1, 1, 1, 1, 2, 0, 0]);
        d.extend([7, 0x24, 1, 1, 1, 0x01, 0x00]);
        // Type I, 2 channels, 3 bytes, 24 bits, two rates.
        d.extend([
            14, 0x24, 2, 1, 2, 3, 24, 2, 0x44, 0xAC, 0x00, 0x80, 0xBB, 0x00,
        ]);
        // Endpoint 0x02 OUT, iso adaptive (0x09), 9-byte audio endpoint.
        d.extend([9, 5, 0x02, 0x09, 0x20, 0x01, 1, 0, 0]);
        let dac = parse(&d).expect("a DAC");
        assert_eq!(dac.uac, Uac::One);
        let f = &dac.formats[0];
        assert_eq!(
            (f.channels, f.subslot, f.bits, f.sync),
            (2, 3, 24, Sync::Adaptive)
        );
        assert_eq!(listed_rates(f), vec![44_100, 48_000]);
        assert_eq!(
            dac.volume,
            Some(Volume {
                unit: 2,
                master: true
            })
        );
    }

    #[test]
    fn clock_ranges_give_the_usual_rates() {
        let mut answer = vec![2, 0];
        // 44.1 to 384 kHz in no steps (any), and 768 kHz alone.
        answer.extend(44_100u32.to_le_bytes());
        answer.extend(384_000u32.to_le_bytes());
        answer.extend(0u32.to_le_bytes());
        answer.extend(768_000u32.to_le_bytes());
        answer.extend(768_000u32.to_le_bytes());
        answer.extend(0u32.to_le_bytes());
        assert_eq!(
            rates_in_ranges(&answer),
            vec![
                44_100, 48_000, 88_200, 96_000, 176_400, 192_000, 352_800, 384_000, 768_000
            ]
        );
    }

    #[test]
    fn something_else_is_not_a_dac() {
        assert_eq!(
            parse(&[9, 4, 0, 0, 1, 3, 1, 1, 0, 7, 5, 0x81, 3, 8, 0, 10]),
            None
        );
    }
}
