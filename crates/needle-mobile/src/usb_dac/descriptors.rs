//! A USB DAC's descriptors, read: what it is (USB Audio Class 1 or 2), the ways music can go
//! through it (each from a streaming interface to an output terminal), the formats and the
//! volume of the way Needle uses, and its clock. A device may have several ways, such as a
//! dock's speakers and its headset: only one is used, and nothing is mixed from another. Pure
//! data: the device itself is spoken to in `device.rs`.

/// USB Audio Class version.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Uac {
    One,
    Two,
    Three,
}

impl Uac {
    pub fn number(self) -> u8 {
        match self {
            Uac::One => 1,
            Uac::Two => 2,
            Uac::Three => 3,
        }
    }
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
    /// UAC3: the cluster descriptor that says the channels; two until the DAC is asked.
    pub cluster: Option<u16>,
}

/// The volume control on the way to the DAC's output.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Volume {
    pub unit: u8,
    /// The master channel has a volume control (else channels 1 and 2 are used).
    pub master: bool,
    /// The master channel has a mute the host can set: real silence at 0.
    pub mute: bool,
}

/// UAC3: a power domain, which must be awake (D0) for the music to play.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Power {
    pub id: u8,
    /// How long it takes to wake, in 50 µs steps, from D1 and from D2.
    pub from_d1: u16,
    pub from_d2: u16,
}

/// One of a device's configurations, of which only the active one can be used.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Configuration {
    pub value: u8,
    /// Its first interface's class and protocol: audio is class 1, UAC3 protocol 0x30.
    pub class: u8,
    pub protocol: u8,
    /// Its audio function's subclass: 0x20 and up is a UAC3 BADD profile.
    pub function: Option<u8>,
}

/// One way music goes through the DAC: in by a streaming interface, through its units, and out
/// by an output terminal. Each has its own formats and its own volume.
#[derive(Clone, Debug, PartialEq)]
pub struct Path {
    pub interface: u8,
    /// The USB streaming input terminal the music enters by.
    pub terminal: u8,
    /// The output terminal's id and type, when the descriptors lead to one.
    pub output: Option<(u8, u16)>,
    /// The units on the way, nearest the output first.
    pub units: Vec<u8>,
    pub volume: Option<Volume>,
    pub formats: Vec<Format>,
}

/// What a USB DAC offers.
#[derive(Clone, Debug, PartialEq)]
pub struct Dac {
    pub uac: Uac,
    /// The Audio Control interface's number.
    pub control: u8,
    /// Every way through the DAC, and which of them Needle uses.
    pub paths: Vec<Path>,
    pub chosen: usize,
    /// The formats of the way Needle uses.
    pub formats: Vec<Format>,
    /// UAC2: the clock source each input terminal uses, by terminal id.
    pub clocks: Vec<(u8, u8)>,
    /// UAC2: clock selectors and their first source.
    pub selectors: Vec<(u8, u8)>,
    /// The volume on the way Needle uses.
    pub volume: Option<Volume>,
    /// The DAC takes DSD as raw data itself (UAC2 Type I raw), beside PCM.
    pub native_dsd: bool,
    /// UAC3: the power domain of the way Needle uses.
    pub power: Option<Power>,
    /// A UAC3 BADD profile: no descriptors of its own, always 48 kHz.
    pub badd: bool,
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

const CONFIGURATION: u8 = 0x02;
const INTERFACE: u8 = 0x04;
const ENDPOINT: u8 = 0x05;
const ASSOCIATION: u8 = 0x0B;
const CS_INTERFACE: u8 = 0x24;
const CS_ENDPOINT: u8 = 0x25;
const AUDIO: u8 = 0x01;
const CONTROL: u8 = 0x01;
const STREAMING: u8 = 0x02;
const UAC3_PROTOCOL: u8 = 0x30;
/// UAC3 function subclasses from here up are BADD profiles.
const BADD: u8 = 0x20;
/// The only rate of a BADD profile.
const BADD_RATE: u32 = 48_000;
/// The terminal type of music to or from the phone.
const USB_STREAMING: u16 = 0x0101;

/// A stop on the way through the DAC, from its Audio Control interface.
struct Node {
    id: u8,
    kind: Kind,
    /// Where its sound comes from.
    sources: Vec<u8>,
}

enum Kind {
    Input,
    Output(u16),
    /// A feature unit, and how its volume works: `Some(true)` on the master channel, `Some(false)`
    /// on each channel, `None` with no volume the host can set; and whether it has a mute.
    Feature(Option<bool>, bool),
    Unit,
}

/// How good a way out a terminal type is for music, as a person would choose: the speakers,
/// headphones, a line, or a digital output first; a headset, a telephone, or a call's speaker
/// last, as such a way may be made for voices.
pub fn rank(output: Option<u16>) -> u8 {
    match output {
        Some(0x0306) => 2,
        Some(kind) => match kind >> 8 {
            0x03 | 0x06 | 0x07 => 3,
            0x04 | 0x05 => 1,
            _ => 2,
        },
        None => 2,
    }
}

/// The way back from `id` to the terminal `to`, `id` first: what the walk went through.
fn way_back(nodes: &[Node], id: u8, to: u8, seen: &mut Vec<u8>) -> Option<Vec<u8>> {
    if id == to {
        return Some(vec![]);
    }
    if seen.contains(&id) {
        return None;
    }
    seen.push(id);
    let node = nodes.iter().find(|n| n.id == id)?;
    node.sources.iter().find_map(|source| {
        let mut way = way_back(nodes, *source, to, seen)?;
        way.insert(0, id);
        Some(way)
    })
}

/// The way for music entering at `f`'s terminal: to the best output terminal it reaches. In
/// descriptors that name no way out, to the farthest feature unit it reaches.
fn path_of(f: &Format, nodes: &[Node]) -> Path {
    let way = |id: u8| way_back(nodes, id, f.terminal, &mut vec![]);
    let out = nodes
        .iter()
        .filter_map(|n| match n.kind {
            Kind::Output(kind) if kind != USB_STREAMING => Some(((n.id, kind), way(n.id)?)),
            _ => None,
        })
        .max_by_key(|((_, kind), _)| rank(Some(*kind)));
    let (output, units) = match out {
        Some((output, units)) => (Some(output), units),
        None => {
            let feature = |n: &&Node| matches!(n.kind, Kind::Feature(..));
            let farthest = nodes
                .iter()
                .filter(feature)
                .filter_map(|n| way(n.id))
                .max_by_key(Vec::len);
            (None, farthest.unwrap_or_default())
        }
    };
    let node = |id: &u8| nodes.iter().find(|n| n.id == *id);
    let volume = units.iter().find_map(|id| match node(id)?.kind {
        Kind::Feature(Some(master), mute) => Some(Volume {
            unit: *id,
            master,
            mute,
        }),
        _ => None,
    });
    Path {
        interface: f.interface,
        terminal: f.terminal,
        output,
        units: units
            .into_iter()
            .filter(|id| !node(id).is_some_and(|n| matches!(n.kind, Kind::Output(_))))
            .collect(),
        volume,
        formats: vec![],
    }
}

/// The way Needle uses: one for stereo music, then the best way out, the deepest samples, and
/// the first interface.
fn best_path(paths: &[Path]) -> Option<usize> {
    let key = |p: &Path| {
        (
            p.formats.iter().any(|f| f.channels == 2),
            rank(p.output.map(|(_, kind)| kind)),
            p.formats.iter().map(|f| f.bits).max().unwrap_or(0),
            255 - p.interface,
        )
    };
    (0..paths.len()).max_by_key(|i| key(&paths[*i]))
}

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

/// A BADD profile's format, known by its endpoint's packet size alone (Linux's table): channels,
/// bytes per sample, and bits. The sizes are the sync and async ones.
fn badd_format(max_packet: u16) -> Option<(u8, u8, u8)> {
    match max_packet {
        0x0060 | 0x0062 => Some((1, 2, 16)),
        0x0090 | 0x0093 => Some((1, 3, 24)),
        0x00C0 | 0x00C4 => Some((2, 2, 16)),
        0x0120 | 0x0126 => Some((2, 3, 24)),
        _ => None,
    }
}

/// Every configuration in a device's raw descriptors.
pub fn configurations(raw: &[u8]) -> Vec<Configuration> {
    let mut out: Vec<Configuration> = vec![];
    let mut first_interface = false;
    let mut i = 0;
    while i + 2 <= raw.len() {
        let len = raw[i] as usize;
        if len < 2 || i + len > raw.len() {
            break;
        }
        let d = &raw[i..i + len];
        match (d[1], out.last_mut()) {
            (CONFIGURATION, _) if len >= 9 => {
                first_interface = true;
                out.push(Configuration {
                    value: d[5],
                    class: 0,
                    protocol: 0,
                    function: None,
                });
            }
            (ASSOCIATION, Some(c)) if len >= 8 && d[4] == AUDIO => c.function = Some(d[5]),
            (INTERFACE, Some(c)) if len >= 9 && first_interface => {
                first_interface = false;
                c.class = d[5];
                c.protocol = d[7];
            }
            _ => {}
        }
        i += len;
    }
    out
}

/// The configuration Linux picks for a device (usb_choose_configuration): the first UAC3 one, a
/// BADD profile included, else the first. Only for when the device cannot be asked.
pub fn likely_configuration(raw: &[u8]) -> Option<u8> {
    let all = configurations(raw);
    let uac3 = |c: &&Configuration| c.class == AUDIO && c.protocol == UAC3_PROTOCOL;
    all.iter().find(uac3).or(all.first()).map(|c| c.value)
}

/// Read a device's raw descriptors as Android's `UsbDeviceConnection.getRawDescriptors` gives
/// them, the device descriptor and then every configuration. None for a device that is not an
/// audio device Needle can play to.
pub fn parse(raw: &[u8]) -> Option<Dac> {
    parse_active(raw, likely_configuration(raw))
}

/// As [`parse`], in the configuration `active`, the only one that counts: a device such as
/// Apple's USB-C adapter describes three, with the same interfaces and numbers in each.
pub fn parse_active(raw: &[u8], active: Option<u8>) -> Option<Dac> {
    let mut uac = None;
    let mut control = None;
    let mut formats = vec![];
    let mut clocks = vec![];
    let mut selectors = vec![];
    let mut nodes = vec![];
    let mut native_dsd = false;
    let mut powers: Vec<(Power, Vec<u8>)> = vec![];
    let mut badd = false;
    let mut other_configuration = false;
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
        if d[1] == CONFIGURATION && len >= 9 {
            other_configuration = active.is_some_and(|value| d[5] != value);
        }
        if other_configuration {
            i += len;
            continue;
        }
        let modern = matches!(uac, Some(Uac::Two | Uac::Three));
        // UAC3's units are numbered one up from UAC2's, with the same layouts.
        let subtype = match (uac, d.get(2).copied().unwrap_or(0)) {
            (Some(Uac::Three), 0x04) => 0xFF,
            (Some(Uac::Three), s @ 0x05..=0x0C) => s - 1,
            (_, s) => s,
        };
        match d[1] {
            ASSOCIATION if len >= 8 => {
                badd |= d[4] == AUDIO && d[6] == UAC3_PROTOCOL && d[5] >= BADD;
            }
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
                    // UAC3's header says no version.
                    if d[7] == UAC3_PROTOCOL {
                        uac = Some(Uac::Three);
                    }
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
                        cluster: None,
                    });
                }
            }
            CS_INTERFACE if len >= 3 => match here {
                Some((_, _, AUDIO, CONTROL)) => match subtype {
                    // UAC3: a power domain and the terminals in it.
                    0x10 if uac == Some(Uac::Three) && len >= 9 => {
                        let count = d[8] as usize;
                        powers.push((
                            Power {
                                id: d[3],
                                from_d1: u16_at(d, 4),
                                from_d2: u16_at(d, 6),
                            },
                            d.get(9..9 + count).unwrap_or_default().to_vec(),
                        ));
                    }
                    // HEADER: bcdADC.
                    0x01 if uac != Some(Uac::Three) => {
                        uac = Some(if u16_at(d, 3) >= 0x0200 {
                            Uac::Two
                        } else {
                            Uac::One
                        })
                    }
                    // INPUT_TERMINAL (UAC2: its clock source).
                    0x02 if len >= 7 => {
                        if modern && len >= 8 {
                            clocks.push((d[3], d[7]));
                        }
                        nodes.push(Node {
                            id: d[3],
                            kind: Kind::Input,
                            sources: vec![],
                        });
                    }
                    // OUTPUT_TERMINAL.
                    0x03 if len >= 8 => nodes.push(Node {
                        id: d[3],
                        kind: Kind::Output(u16_at(d, 4)),
                        sources: vec![d[7]],
                    }),
                    // FEATURE_UNIT.
                    0x06 if len >= 7 => {
                        let (master, first, mute) = if modern {
                            // Mute is bits 0-1 of the master's controls, volume bits 2-3 of each
                            // channel's: 3 is "the host can set it" (1 could only be read, and
                            // would leave it stuck).
                            (
                                (u32_at(d, 5) >> 2) & 0b11 == 0b11,
                                (u32_at(d, 9) >> 2) & 0b11 == 0b11,
                                u32_at(d, 5) & 0b11 == 0b11,
                            )
                        } else {
                            // Mute is bit 0, volume bit 1.
                            let size = d[5].max(1) as usize;
                            let bit = |at: usize| d.get(at).is_some_and(|c| c & 0b10 != 0);
                            (bit(6), bit(6 + size), d.get(6).is_some_and(|c| c & 1 != 0))
                        };
                        nodes.push(Node {
                            id: d[3],
                            kind: Kind::Feature((master || first).then_some(master), mute),
                            sources: vec![d[4]],
                        });
                    }
                    // MIXER_UNIT and SELECTOR_UNIT.
                    0x04 | 0x05 if len >= 6 => nodes.push(Node {
                        id: d[3],
                        kind: Kind::Unit,
                        sources: pins(d, 4),
                    }),
                    // PROCESSING_UNIT and EXTENSION_UNIT (their numbers differ by version).
                    0x07 | 0x08 if !modern && len >= 8 => nodes.push(Node {
                        id: d[3],
                        kind: Kind::Unit,
                        sources: pins(d, 6),
                    }),
                    0x08 | 0x09 if modern && len >= 8 => nodes.push(Node {
                        id: d[3],
                        kind: Kind::Unit,
                        sources: pins(d, 6),
                    }),
                    // UAC2 and 3: EFFECT_UNIT; UAC2: SAMPLE_RATE_CONVERTER.
                    0x07 if modern && len >= 7 => nodes.push(Node {
                        id: d[3],
                        kind: Kind::Unit,
                        sources: vec![d[6]],
                    }),
                    0x0D if uac == Some(Uac::Two) && len >= 5 => nodes.push(Node {
                        id: d[3],
                        kind: Kind::Unit,
                        sources: vec![d[4]],
                    }),
                    // CLOCK_SELECTOR: its first source.
                    0x0B if modern && len >= 6 => selectors.push((d[3], d[5])),
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
                                // UAC3 says all here, but for the channels (see `cluster`).
                                Some(Uac::Three) if len >= 20 => {
                                    f.cluster = Some(u16_at(d, 8));
                                    pcm = u32_at(d, 10) & 1 != 0;
                                    f.channels = 2;
                                    f.subslot = d[18];
                                    f.bits = d[19];
                                }
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
    // A BADD profile has none of the descriptors above: what it is, is fixed (as Linux has it).
    if badd {
        for f in &mut formats {
            if let Some((channels, subslot, bits)) = badd_format(f.max_packet) {
                (f.channels, f.subslot, f.bits) = (channels, subslot, bits);
                f.rates = vec![BADD_RATE];
                f.rate_control = false;
                f.terminal = 1;
            }
        }
        // Volume by channel and a master mute on unit 2, after terminal 1.
        nodes.push(Node {
            id: 2,
            kind: Kind::Feature(Some(false), true),
            sources: vec![1],
        });
        powers.push((
            Power {
                id: 10,
                from_d1: 0x0258,
                from_d2: 0x1770,
            },
            vec![1],
        ));
    }
    formats.retain(|f| f.channels > 0 && matches!(f.subslot, 2..=4) && f.bits >= 16);
    let mut paths: Vec<Path> = vec![];
    for f in formats {
        let same = |p: &&mut Path| p.interface == f.interface && p.terminal == f.terminal;
        let path = match paths.iter_mut().find(same) {
            Some(path) => path,
            None => {
                paths.push(path_of(&f, &nodes));
                paths.last_mut()?
            }
        };
        path.formats.push(f);
    }
    let chosen = best_path(&paths)?;
    let power = powers
        .iter()
        .find(|(_, terminals)| terminals.contains(&paths[chosen].terminal))
        .map(|(power, _)| *power);
    Some(Dac {
        uac: uac?,
        control: control?,
        formats: paths[chosen].formats.clone(),
        volume: paths[chosen].volume,
        paths,
        chosen,
        clocks,
        selectors,
        native_dsd,
        power,
        badd,
    })
}

/// The sources of a unit whose count of them is at `at`, with the sources after it.
fn pins(d: &[u8], at: usize) -> Vec<u8> {
    let count = d[at] as usize;
    d.get(at + 1..at + 1 + count).unwrap_or_default().to_vec()
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

/// A format's rates, from its own list or range (UAC1). A range proves little: a chip may take
/// any rate asked and go on at its own clock, usually 48 kHz, which sounds wrong. With 48 kHz
/// in the range, that is the one rate trusted, and other music goes through Android. So too for
/// a setting that lists several rates and cannot be told one (UAC1 with no sampling frequency
/// control): 48 kHz if it lists it, else its highest.
pub fn listed_rates(f: &Format) -> Vec<u32> {
    match f.range {
        Some((low, high)) if (low..=high).contains(&48_000) => vec![48_000],
        Some((low, high)) => RATES
            .iter()
            .copied()
            .filter(|r| *r >= low && *r <= high)
            .collect(),
        None if !f.rate_control && f.rates.len() > 1 => {
            let top = f.rates.iter().copied().max().unwrap_or(0);
            vec![if f.rates.contains(&48_000) {
                48_000
            } else {
                top
            }]
        }
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
                master: true,
                mute: false
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
                master: true,
                mute: true
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
    fn a_range_with_48_khz_in_it_trusts_only_48_khz() {
        let mut f = parse(&uac2_dac()).unwrap().formats[0].clone();
        f.range = Some((8_000, 48_000));
        assert_eq!(listed_rates(&f), [48_000]);
        f.range = Some((8_000, 44_100));
        assert_eq!(listed_rates(&f), [44_100, 32_000, 22_050]);
    }

    #[test]
    fn a_volume_that_can_only_be_read_is_no_volume() {
        let mut d = uac2_dac();
        // The feature unit's master controls: 0x0C (the host sets it) becomes 0x04 (read only).
        let at = d.windows(6).position(|w| w == [18, 0x24, 6, 10, 2, 0x0C]);
        d[at.unwrap() + 5] = 0x04;
        assert_eq!(parse(&d).unwrap().volume, None);
    }

    use super::super::fake::{APPLE_DAC, DOCK, hex};

    #[test]
    fn every_configuration_of_apples_adapter_is_known_and_linux_would_take_the_badd_one() {
        let raw = hex(APPLE_DAC);
        let all = configurations(&raw);
        let classes: Vec<_> = all
            .iter()
            .map(|c| (c.value, c.protocol, c.function))
            .collect();
        assert_eq!(
            classes,
            [
                (1, 0x20, Some(0x00)),
                (2, 0x30, Some(0x21)),
                (3, 0x30, Some(0x01))
            ]
        );
        assert_eq!(likely_configuration(&raw), Some(2));
    }

    #[test]
    fn only_the_active_configuration_counts() {
        let raw = hex(APPLE_DAC);

        let two = parse_active(&raw, Some(1)).unwrap();
        assert_eq!((two.uac, two.badd, two.power), (Uac::Two, false, None));
        assert_eq!(two.formats.len(), 2, "not pooled from the other two");
        assert_eq!(two.clock_for(1), Some(9));
        assert_eq!(two.formats[0].rates, Vec::<u32>::new());

        let fixed = parse_active(&raw, Some(2)).unwrap();
        assert_eq!((fixed.uac, fixed.badd), (Uac::Three, true));
        let shape: Vec<_> = fixed
            .formats
            .iter()
            .map(|f| (f.alt, f.channels, f.subslot, f.bits, f.endpoint))
            .collect();
        assert_eq!(shape, [(1, 2, 3, 24, 0x02), (2, 2, 2, 16, 0x02)]);
        assert_eq!(listed_rates(&fixed.formats[0]), [48_000]);
        assert!(!fixed.formats[0].rate_control);
        assert_eq!(fixed.formats[0].sync, Sync::Synchronous);
        let volume = fixed.volume.unwrap();
        assert_eq!((volume.unit, volume.master, volume.mute), (2, false, true));
        assert_eq!(fixed.power.map(|p| p.id), Some(10));

        let full = parse_active(&raw, Some(3)).unwrap();
        assert_eq!((full.uac, full.badd), (Uac::Three, false));
        let shape: Vec<_> = full
            .formats
            .iter()
            .map(|f| (f.alt, f.subslot, f.bits, f.cluster))
            .collect();
        assert_eq!(shape, [(1, 3, 24, Some(0x20)), (2, 2, 16, Some(0x20))]);
        assert_eq!(full.clock_for(1), Some(9));
        let volume = full.volume.unwrap();
        assert_eq!((volume.unit, volume.master, volume.mute), (2, false, true));
        let power = full.power.unwrap();
        assert_eq!(
            (power.id, power.from_d1, power.from_d2),
            (0x28, 0x0258, 0x1770)
        );
        assert_eq!(full.paths[0].output, Some((3, 0x0302)));
    }

    #[test]
    fn the_real_dock_is_read() {
        let dock = parse(&hex(DOCK)).unwrap();
        assert_eq!(dock.uac, Uac::One);
        assert_eq!(dock.paths[dock.chosen].output, Some((3, 0x0301)));
        assert_eq!(dock.formats.len(), 1);
        assert_eq!(dock.formats[0].rates, [44_100, 48_000]);
        let volume = dock.volume.unwrap();
        assert_eq!((volume.unit, volume.master, volume.mute), (6, false, true));
    }

    #[test]
    fn something_else_is_not_a_dac() {
        assert_eq!(
            parse(&[9, 4, 0, 0, 1, 3, 1, 1, 0, 7, 5, 0x81, 3, 8, 0, 10]),
            None
        );
    }
}
