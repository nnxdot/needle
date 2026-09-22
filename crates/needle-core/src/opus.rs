//! Ogg Opus (RFC 7845) decoding with the pure-Rust `opus-decoder` crate.
//! Symphonia 0.5 has no Opus codec, so Ogg pages are read here directly.
//! Only the first logical Opus stream of a file is played.
use anyhow::{Context, Result, ensure};
use opus_decoder::OpusDecoder;
use rodio::{Source, source::SeekError};
use std::{
    collections::VecDeque,
    fs::File,
    io::{self, BufReader, Read, Seek, SeekFrom},
    path::Path,
    time::Duration,
};

pub const RATE: u32 = 48_000;
/// Decoded audio before a seek target, so the decoder converges (RFC 7845 §4.6).
const PREROLL: i64 = 3840;
/// 120 ms, the longest Opus packet.
const MAX_FRAME: usize = 5760;

struct PageHeader {
    flags: u8,
    granule: i64,
    serial: u32,
    lacing: Vec<u8>,
}
impl PageHeader {
    fn continued(&self) -> bool {
        self.flags & 1 != 0
    }
    fn body_len(&self) -> usize {
        self.lacing.iter().map(|&l| l as usize).sum()
    }
}
fn read_header(reader: &mut impl Read) -> io::Result<Option<PageHeader>> {
    let mut header = [0u8; 27];
    match reader.read_exact(&mut header) {
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
        other => other?,
    }
    if &header[..4] != b"OggS" || header[4] != 0 {
        return Ok(None);
    }
    let mut lacing = vec![0; header[26] as usize];
    reader.read_exact(&mut lacing)?;
    Ok(Some(PageHeader {
        flags: header[5],
        granule: i64::from_le_bytes(header[6..14].try_into().unwrap()),
        serial: u32::from_le_bytes(header[14..18].try_into().unwrap()),
        lacing,
    }))
}
/// Splits a page body into packet pieces; `true` marks a piece that ends a packet.
fn pieces<'a>(header: &'a PageHeader, body: &'a [u8]) -> impl Iterator<Item = (&'a [u8], bool)> {
    let mut start = 0;
    let mut end = 0;
    let mut laces = header.lacing.iter().peekable();
    std::iter::from_fn(move || {
        while let Some(&lace) = laces.next() {
            end += lace as usize;
            if lace < 255 || laces.peek().is_none() {
                let piece = &body[start.min(body.len())..end.min(body.len())];
                start = end;
                return Some((piece, lace < 255));
            }
        }
        None
    })
}

/// Samples per channel at 48 kHz from a packet's TOC (RFC 6716 §3.1).
pub fn packet_samples(packet: &[u8]) -> Option<i64> {
    let toc = *packet.first()?;
    let config = (toc >> 3) as usize;
    let frame = match config {
        0..=11 => [480, 960, 1920, 2880][config % 4],
        12..=15 => [480, 960][config % 2],
        _ => [120, 240, 480, 960][config % 4],
    };
    let frames = match toc & 3 {
        0 => 1,
        1 | 2 => 2,
        _ => (*packet.get(1)? & 0x3f) as i64,
    };
    Some(frame * frames)
}

#[derive(Clone, Debug, PartialEq)]
pub struct Head {
    pub channels: u16,
    pub pre_skip: i64,
    pub input_rate: u32,
    /// Output gain in dB, to be applied by the player.
    pub gain_db: f64,
    family: u8,
    streams: usize,
    coupled: usize,
    mapping: Vec<u8>,
}
impl Head {
    pub fn parse(packet: &[u8]) -> Result<Self> {
        ensure!(
            packet.len() >= 19 && &packet[..8] == b"OpusHead",
            "Missing Opus identification header"
        );
        ensure!(packet[8] >> 4 == 0, "Unsupported Opus mapping version");
        let channels = packet[9];
        ensure!(channels > 0, "Opus stream has no channels");
        let family = packet[18];
        let (streams, coupled, mapping) = if family == 0 {
            ensure!(channels <= 2, "Opus family 0 allows only mono or stereo");
            (1, channels as usize - 1, (0..channels).collect())
        } else {
            ensure!(
                family == 1 && channels <= 8,
                "Opus channel mapping family {family} is not supported"
            );
            let table = packet
                .get(19..21 + channels as usize)
                .context("Truncated Opus channel mapping")?;
            // Vorbis channel order to the WAVE order output devices expect.
            const WAVE: [&[usize]; 8] = [
                &[0],
                &[0, 1],
                &[0, 2, 1],
                &[0, 1, 2, 3],
                &[0, 2, 1, 3, 4],
                &[0, 2, 1, 5, 3, 4],
                &[0, 2, 1, 6, 5, 3, 4],
                &[0, 2, 1, 7, 5, 6, 3, 4],
            ];
            let order = WAVE[channels as usize - 1].iter();
            let mapping = order.map(|&c| table[2 + c]).collect();
            (table[0] as usize, table[1] as usize, mapping)
        };
        Ok(Self {
            channels: channels as u16,
            pre_skip: u16::from_le_bytes([packet[10], packet[11]]) as i64,
            input_rate: u32::from_le_bytes(packet[12..16].try_into().unwrap()),
            gain_db: i16::from_le_bytes([packet[16], packet[17]]) as f64 / 256.0,
            family,
            streams,
            coupled,
            mapping,
        })
    }
}

/// Reads a self-delimiting length (RFC 6716 §3.2.1): value and bytes used.
fn length(data: &[u8]) -> Option<(usize, usize)> {
    let first = *data.first()? as usize;
    if first < 252 {
        Some((first, 1))
    } else {
        Some((first + 4 * *data.get(1)? as usize, 2))
    }
}
/// Converts one self-delimited packet (RFC 6716 Appendix B) at the start of
/// `data` into an ordinary packet. Returns it and the bytes consumed.
fn undelimit(data: &[u8]) -> Option<(Vec<u8>, usize)> {
    let toc = *data.first()?;
    let mut packet = vec![toc];
    let mut at = 1;
    let body = match toc & 3 {
        0 | 1 => {
            let (size, used) = length(data.get(at..)?)?;
            at += used;
            if toc & 3 == 0 { size } else { 2 * size }
        }
        2 => {
            let (first, used) = length(data.get(at..)?)?;
            packet.extend_from_slice(&data[at..at + used]);
            at += used;
            let (second, used) = length(data.get(at..)?)?;
            at += used;
            first + second
        }
        _ => {
            let count = *data.get(at)?;
            packet.push(count);
            at += 1;
            let frames = (count & 0x3f) as usize;
            let mut padding = 0;
            if count & 0x40 != 0 {
                loop {
                    let byte = *data.get(at)?;
                    packet.push(byte);
                    at += 1;
                    padding += if byte == 255 { 254 } else { byte as usize };
                    if byte != 255 {
                        break;
                    }
                }
            }
            let mut total = 0;
            if count & 0x80 != 0 {
                for _ in 1..frames {
                    let (size, used) = length(data.get(at..)?)?;
                    packet.extend_from_slice(&data[at..at + used]);
                    at += used;
                    total += size;
                }
                let (last, used) = length(data.get(at..)?)?;
                at += used;
                total + last + padding
            } else {
                let (size, used) = length(data.get(at..)?)?;
                at += used;
                size * frames + padding
            }
        }
    };
    packet.extend_from_slice(data.get(at..at + body)?);
    Some((packet, at + body))
}

/// Decodes one Opus stream, or several per RFC 7845 §5.1.1 mapped onto output
/// channels. Multistream packets are split here because `opus-decoder` 0.1.1
/// reads the self-delimited length before the TOC byte instead of after it.
struct Decoder {
    streams: Vec<OpusDecoder>,
    coupled: usize,
    channels: usize,
    mapping: Vec<u8>,
    buffers: Vec<Vec<f32>>,
}
impl Decoder {
    fn new(head: &Head) -> Result<Self> {
        let streams = (0..head.streams)
            .map(|i| OpusDecoder::new(RATE, if i < head.coupled { 2 } else { 1 }))
            .collect::<Result<Vec<_>, _>>()?;
        ensure!(
            !streams.is_empty()
                && head.coupled <= head.streams
                && head
                    .mapping
                    .iter()
                    .all(|&m| m == 255 || (m as usize) < head.streams + head.coupled),
            "Invalid Opus channel mapping"
        );
        Ok(Self {
            buffers: (0..head.streams)
                .map(|i| vec![0.0; MAX_FRAME * if i < head.coupled { 2 } else { 1 }])
                .collect(),
            streams,
            coupled: head.coupled,
            channels: head.channels as usize,
            mapping: head.mapping.clone(),
        })
    }
    /// Decodes into interleaved `pcm`; an empty packet conceals a lost one.
    fn decode(&mut self, packet: &[u8], pcm: &mut [f32]) -> Option<usize> {
        let direct = self
            .mapping
            .iter()
            .enumerate()
            .all(|(i, &m)| m as usize == i);
        if direct && self.streams.len() == 1 && self.coupled + 1 == self.channels {
            return self.streams[0].decode_float(packet, pcm, false).ok();
        }
        let mut rest = packet;
        let mut frames = None;
        for (index, stream) in self.streams.iter_mut().enumerate() {
            let last = index + 1 == self.buffers.len();
            let (piece, used) = if packet.is_empty() {
                (vec![], 0)
            } else if last {
                (rest.to_vec(), rest.len())
            } else {
                undelimit(rest)?
            };
            rest = &rest[used..];
            let n = stream
                .decode_float(&piece, &mut self.buffers[index], false)
                .ok()?;
            if frames.is_some_and(|f| f != n) {
                return None;
            }
            frames = Some(n);
        }
        let frames = frames?;
        for (channel, &slot) in self.mapping.iter().enumerate() {
            let slot = slot as usize;
            let (stream, width, offset) = if slot == 255 {
                (usize::MAX, 0, 0)
            } else if slot < 2 * self.coupled {
                (slot / 2, 2, slot % 2)
            } else {
                (slot - self.coupled, 1, 0)
            };
            for frame in 0..frames {
                pcm[frame * self.channels + channel] = self
                    .buffers
                    .get(stream)
                    .map_or(0.0, |b| b[frame * width + offset]);
            }
        }
        Some(frames)
    }
    fn reset(&mut self) {
        self.streams.iter_mut().for_each(OpusDecoder::reset);
    }
}

/// A seekable, gapless Ogg Opus source at 48 kHz. Pre-skip and end trimming
/// follow the granule positions; the header's output gain is applied.
pub struct OpusSource {
    reader: BufReader<File>,
    head: Head,
    decoder: Decoder,
    gain: f32,
    serial: u32,
    /// Offset and granule of every audio page of the stream.
    pages: Vec<(u64, i64)>,
    next_page: usize,
    packets: VecDeque<Vec<u8>>,
    partial: Vec<u8>,
    /// Drop the first packet piece of the next page: its start was not read.
    skip_continuation: bool,
    /// Drop packets completed on the next page: they precede the seek point.
    skip_completed: bool,
    /// Granule position of decoded frame 0 of the first audio page.
    base: i64,
    /// Index of the next decoded frame, counted from the first audio page.
    frame: i64,
    start: i64,
    end: i64,
    discard_until: i64,
    scratch: Vec<f32>,
    out: Vec<f32>,
    cursor: usize,
}

/// Whether the file is an Ogg stream whose first packet is an Opus header.
pub fn is_ogg_opus(path: &Path) -> bool {
    let Ok(file) = File::open(path) else {
        return false;
    };
    let mut reader = BufReader::new(file);
    let Ok(Some(header)) = read_header(&mut reader) else {
        return false;
    };
    let mut magic = [0u8; 8];
    header.body_len() >= 8 && reader.read_exact(&mut magic).is_ok() && &magic == b"OpusHead"
}

impl OpusSource {
    pub fn open(path: &Path) -> Result<Self> {
        let mut reader = BufReader::new(File::open(path)?);
        let mut serial = None;
        let mut head = None;
        let mut headers_done = 0;
        let mut packet = vec![];
        let mut pages = vec![];
        let mut first_audio = None;
        let mut offset = 0u64;
        while let Some(header) = read_header(&mut reader)? {
            let length = header.body_len();
            let next = offset + 27 + header.lacing.len() as u64 + length as u64;
            let ours = *serial.get_or_insert(header.serial) == header.serial;
            if ours && headers_done < 2 {
                let mut body = vec![0; length];
                reader.read_exact(&mut body)?;
                for (piece, complete) in pieces(&header, &body) {
                    packet.extend_from_slice(piece);
                    if complete {
                        if headers_done == 0 {
                            head = Some(Head::parse(&packet)?);
                        }
                        headers_done += 1;
                        packet.clear();
                    }
                }
            } else {
                let last = ours && header.flags & 4 != 0;
                if ours {
                    pages.push((offset, header.granule));
                }
                if ours && first_audio.is_none() {
                    let mut body = vec![0; length];
                    reader.read_exact(&mut body)?;
                    first_audio = Some((header, body));
                } else {
                    reader.seek_relative(length as i64)?;
                }
                if last {
                    break;
                }
            }
            offset = next;
        }
        let head = head.context("Not an Ogg Opus file")?;
        let end_granule = pages
            .iter()
            .rev()
            .map(|&(_, g)| g)
            .find(|&g| g >= 0)
            .context("Opus stream has no audio")?;
        // Frames decoded from packets completed on the first audio page,
        // compared with its granule, give the granule of decoded frame 0. On
        // the final page a smaller granule means end trimming instead (§4.5).
        let (first, body) = first_audio.unwrap();
        let decoded: i64 = pieces(&first, &body)
            .filter(|(_, complete)| *complete)
            .filter_map(|(p, _)| packet_samples(p))
            .sum();
        let base = if first.granule >= 0 && first.flags & 4 == 0 {
            first.granule - decoded
        } else {
            0
        };
        let start = (head.pre_skip - base).max(0);
        let end = end_granule - base;
        ensure!(end > start, "Opus stream is empty");
        let gain = 10f64.powf(head.gain_db / 20.0) as f32;
        Ok(Self {
            reader,
            decoder: Decoder::new(&head)?,
            scratch: vec![0.0; MAX_FRAME * head.channels as usize],
            head,
            gain,
            serial: serial.unwrap(),
            pages,
            next_page: 0,
            packets: VecDeque::new(),
            partial: vec![],
            skip_continuation: false,
            skip_completed: false,
            base,
            frame: 0,
            start,
            end,
            discard_until: start,
            out: vec![],
            cursor: 0,
        })
    }
    /// Playable frames after pre-skip and end trimming.
    pub fn frames(&self) -> u64 {
        (self.end - self.start) as u64
    }
    fn read_page(&mut self) -> Result<bool> {
        let Some(&(offset, _)) = self.pages.get(self.next_page) else {
            return Ok(false);
        };
        self.next_page += 1;
        self.reader.seek(SeekFrom::Start(offset))?;
        let header = read_header(&mut self.reader)?.context("Damaged Ogg page")?;
        ensure!(header.serial == self.serial, "Damaged Ogg page");
        let mut body = vec![0; header.body_len()];
        self.reader.read_exact(&mut body)?;
        let mut skip = self.skip_continuation && header.continued();
        if !header.continued() {
            self.partial.clear();
        }
        for (piece, complete) in pieces(&header, &body) {
            if skip {
                skip = !complete;
                continue;
            }
            self.partial.extend_from_slice(piece);
            if complete {
                let packet = std::mem::take(&mut self.partial);
                if !self.skip_completed {
                    self.packets.push_back(packet);
                }
            }
        }
        self.skip_continuation = false;
        self.skip_completed = false;
        Ok(true)
    }
    /// Decodes the next packet into `out`. Returns false at the end of the stream.
    fn refill(&mut self) -> bool {
        self.out.clear();
        self.cursor = 0;
        while self.out.is_empty() {
            if self.frame >= self.end {
                return false;
            }
            let packet = loop {
                if let Some(packet) = self.packets.pop_front() {
                    break packet;
                }
                match self.read_page() {
                    Ok(true) => {}
                    _ => return false,
                }
            };
            let decoded = match self.decoder.decode(&packet, &mut self.scratch) {
                Some(n) => n,
                // Conceal a damaged packet so timing stays aligned with the granules.
                None if packet_samples(&packet).is_some() => {
                    self.decoder.decode(&[], &mut self.scratch).unwrap_or(0)
                }
                None => continue,
            } as i64;
            let channels = self.head.channels as usize;
            let first = self.frame.max(self.discard_until).max(self.start);
            let last = (self.frame + decoded).min(self.end);
            if last > first {
                let range = (first - self.frame) as usize * channels
                    ..(last - self.frame) as usize * channels;
                self.out
                    .extend(self.scratch[range].iter().map(|s| s * self.gain));
            }
            self.frame += decoded;
        }
        true
    }
    fn seek_frames(&mut self, target: i64) -> Result<()> {
        let target = (self.start + target).clamp(self.start, self.end);
        let resume = target - PREROLL + self.base;
        let page = self
            .pages
            .iter()
            .rposition(|&(_, granule)| granule >= 0 && granule <= resume);
        self.packets.clear();
        self.partial.clear();
        self.decoder.reset();
        match page {
            Some(page) => {
                self.next_page = page;
                self.skip_continuation = true;
                self.skip_completed = true;
                self.frame = self.pages[page].1 - self.base;
            }
            None => {
                self.next_page = 0;
                self.frame = 0;
            }
        }
        self.discard_until = target;
        self.out.clear();
        self.cursor = 0;
        Ok(())
    }
}
impl Iterator for OpusSource {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        if self.cursor >= self.out.len() && !self.refill() {
            return None;
        }
        self.cursor += 1;
        Some(self.out[self.cursor - 1])
    }
}
impl Source for OpusSource {
    fn current_span_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> u16 {
        self.head.channels
    }
    fn sample_rate(&self) -> u32 {
        RATE
    }
    fn total_duration(&self) -> Option<Duration> {
        Some(Duration::from_secs_f64(self.frames() as f64 / RATE as f64))
    }
    fn try_seek(&mut self, position: Duration) -> Result<(), SeekError> {
        let frames = (position.as_secs_f64() * RATE as f64).round() as i64;
        self.seek_frames(frames)
            .map_err(|e| SeekError::Other(Box::new(io::Error::other(format!("{e:#}")))))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    // Fixtures were encoded by FFmpeg/libopus from sine tones (amplitude 1/8):
    // tone.opus 440 Hz stereo 1 s, mono44.opus 1 kHz mono 1.5 s from 44.1 kHz,
    // surround.opus 300 Hz 5.1 0.5 s.
    fn fixture(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("testdata")
            .join(name)
    }
    /// Signal-to-noise ratio in dB of channel 0 against an ideal sine.
    fn snr(samples: &[f32], channels: usize, frequency: f64, first_frame: usize) -> f64 {
        // FFmpeg's mono-to-stereo upmix scales by 1/sqrt(2).
        let amplitude = if channels == 2 {
            0.125 / 2f64.sqrt()
        } else {
            0.125
        };
        let (mut signal, mut noise) = (0f64, 0f64);
        for (i, frame) in samples.chunks(channels).enumerate() {
            let t = (first_frame + i) as f64 / RATE as f64;
            let ideal = (std::f64::consts::TAU * frequency * t).sin() * amplitude;
            signal += ideal * ideal;
            noise += (frame[0] as f64 - ideal).powi(2);
        }
        10.0 * (signal / noise).log10()
    }

    #[test]
    fn packet_durations_follow_the_toc() {
        assert_eq!(packet_samples(&[]), None);
        assert_eq!(packet_samples(&[0 << 3]), Some(480));
        assert_eq!(packet_samples(&[3 << 3]), Some(2880));
        assert_eq!(packet_samples(&[13 << 3 | 1]), Some(1920));
        assert_eq!(packet_samples(&[16 << 3]), Some(120));
        assert_eq!(packet_samples(&[31 << 3 | 2]), Some(1920));
        assert_eq!(packet_samples(&[31 << 3 | 3, 6]), Some(5760));
        assert_eq!(packet_samples(&[31 << 3 | 3]), None);
    }

    #[test]
    fn identification_header_is_validated() {
        let mut head = b"OpusHead\x01\x02\x38\x01\x44\xac\0\0\x00\x01\0".to_vec();
        let parsed = Head::parse(&head).unwrap();
        assert_eq!((parsed.channels, parsed.pre_skip), (2, 312));
        assert_eq!(parsed.input_rate, 44100);
        assert_eq!(parsed.gain_db, 1.0);
        head[18] = 2;
        assert!(Head::parse(&head).is_err());
        head[18] = 1;
        assert!(
            Head::parse(&head).is_err(),
            "family 1 needs a mapping table"
        );
        head.extend([1, 1, 0, 1]);
        assert_eq!(Head::parse(&head).unwrap().mapping, [0, 1]);
        let mut surround = head[..19].to_vec();
        surround[9] = 6;
        surround.extend([4, 2, 0, 4, 1, 2, 3, 5]);
        assert_eq!(Head::parse(&surround).unwrap().mapping, [0, 1, 4, 5, 2, 3]);
        head[8] = 0x10;
        assert!(Head::parse(&head).is_err());
        assert!(Head::parse(b"OpusTags\0\0\0\0\0\0\0\0\0\0\0").is_err());
    }

    #[test]
    fn decodes_with_exact_length_and_pre_skip_alignment() {
        let mut source = OpusSource::open(&fixture("tone.opus")).unwrap();
        assert_eq!(source.channels(), 2);
        assert_eq!(source.frames(), 48_000);
        assert!(source.head.pre_skip > 0);
        let samples: Vec<f32> = source.by_ref().collect();
        assert_eq!(samples.len(), 96_000);
        // Past the encoder's start-up transient, the output lines up with the
        // original tone; a pre-skip error would shift the phase.
        let steady = &samples[9600..86400];
        assert!(snr(steady, 2, 440.0, 4800) > 25.0);
        let mono = OpusSource::open(&fixture("mono44.opus")).unwrap();
        assert_eq!((mono.channels(), mono.frames()), (1, 72_000));
        assert_eq!(mono.count(), 72_000);
    }

    #[test]
    fn seeks_to_the_sample_after_preroll() {
        let path = fixture("tone.opus");
        let mut source = OpusSource::open(&path).unwrap();
        for seconds in [0.5, 0.02, 0.9, 0.0] {
            source.try_seek(Duration::from_secs_f64(seconds)).unwrap();
            let first = (seconds * RATE as f64) as usize;
            let samples: Vec<f32> = source.by_ref().take(9600).collect();
            assert_eq!(samples.len(), 9600.min((48_000 - first) * 2));
            if seconds > 0.05 {
                assert!(snr(&samples, 2, 440.0, first) > 25.0, "{seconds}");
            }
        }
        source.try_seek(Duration::from_secs(5)).unwrap();
        assert_eq!(source.next(), None);
        // Seeking lands on the same samples as decoding straight through.
        let all: Vec<f32> = OpusSource::open(&path).unwrap().collect();
        let mut sought = OpusSource::open(&path).unwrap();
        sought.try_seek(Duration::from_millis(600)).unwrap();
        let part: Vec<f32> = sought.take(4800).collect();
        let reference = &all[57_600..62_400];
        let error = part
            .iter()
            .zip(reference)
            .map(|(a, b)| (a - b).abs())
            .fold(0f32, f32::max);
        assert!(error < 0.01, "{error}");
    }

    #[test]
    fn surround_and_the_shared_decoder_path() {
        let mut surround = crate::audio_file::decode(&fixture("surround.opus")).unwrap();
        assert_eq!(surround.channels(), 6);
        assert_eq!(surround.sample_rate(), RATE);
        assert_eq!(surround.total_duration(), Some(Duration::from_millis(500)));
        assert_eq!(surround.by_ref().count(), 24_000 * 6);
        assert!(is_ogg_opus(&fixture("tone.opus")));
        assert!(!is_ogg_opus(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("Cargo.toml")
                .as_path()
        ));
    }

    #[test]
    fn opus_files_import_with_tags_and_duration() {
        let dir = tempfile::tempdir().unwrap();
        let library = crate::database::Library::open(dir.path().join("db")).unwrap();
        let path = dir.path().join("tone.opus");
        std::fs::copy(fixture("tone.opus"), &path).unwrap();
        assert!(crate::scan::import_one(&library, &path).unwrap());
        let track = library.search("").unwrap().remove(0);
        assert_eq!(track.title, "Opus tone");
        assert_eq!(track.artist, "Needle Tests");
        assert_eq!(track.format, "OPUS");
        assert_eq!(track.album_replay_gain, Some(-3.5));
        assert!((track.duration - 1.0).abs() < 0.01, "{}", track.duration);
        let loudness = crate::analysis::loudness(&path).unwrap();
        assert!(
            (loudness.integrated_lufs + 21.0).abs() < 1.5,
            "{loudness:?}"
        );
        // Tag writes go through lofty and are verified against decoded audio.
        let before: Vec<f32> = OpusSource::open(&path).unwrap().collect();
        let edit = crate::scan::TagEdit {
            title: Some("Renamed".into()),
            ..Default::default()
        };
        crate::scan::write_tags(&library, &track.id, &edit).unwrap();
        assert_eq!(library.track(&track.id).unwrap().unwrap().title, "Renamed");
        let after: Vec<f32> = OpusSource::open(&path).unwrap().collect();
        assert_eq!(before, after);
    }

    #[test]
    #[ignore = "needs FFmpeg with libopus on PATH"]
    fn decodes_faster_than_real_time() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("long.opus");
        let status = std::process::Command::new("ffmpeg")
            .args([
                "-v",
                "error",
                "-f",
                "lavfi",
                "-i",
                "anoisesrc=d=60:c=pink:a=0.2",
            ])
            .args(["-ac", "2", "-c:a", "libopus", "-b:a", "160k"])
            .arg(&path)
            .status()
            .unwrap();
        assert!(status.success());
        let start = std::time::Instant::now();
        let source = OpusSource::open(&path).unwrap();
        assert_eq!(source.frames(), 60 * RATE as u64);
        assert_eq!(source.count(), 120 * RATE as usize);
        let speed = 60.0 / start.elapsed().as_secs_f64();
        println!("decoded 60 s of stereo Opus at {speed:.0}x real time");
        assert!(speed > 20.0);
    }

    #[test]
    #[ignore = "needs FFmpeg with libopus on PATH"]
    fn matches_ffmpeg_libopus_decoding() {
        for (name, channels) in [("tone.opus", 2), ("mono44.opus", 1), ("surround.opus", 6)] {
            let output = std::process::Command::new("ffmpeg")
                .args(["-v", "error", "-c:a", "libopus", "-i"])
                .arg(fixture(name))
                .args(["-f", "f32le", "-ar", "48000", "-"])
                .output()
                .unwrap();
            assert!(output.status.success());
            let reference: Vec<f32> = output
                .stdout
                .as_chunks::<4>()
                .0
                .iter()
                .map(|b| f32::from_le_bytes(*b))
                .collect();
            let ours: Vec<f32> = OpusSource::open(&fixture(name)).unwrap().collect();
            assert_eq!(ours.len(), reference.len(), "{name}");
            // Per channel: over the whole file and over its second half.
            let snr = |channel: usize, from: usize| {
                let (mut signal, mut noise) = (0f64, 0f64);
                let pairs = ours.iter().zip(&reference).skip(from * channels + channel);
                for (a, b) in pairs.step_by(channels) {
                    signal += (*b as f64).powi(2);
                    noise += (*a as f64 - *b as f64).powi(2);
                }
                10.0 * (signal / noise.max(1e-20)).log10()
            };
            let frames = ours.len() / channels;
            for channel in 0..channels {
                let (all, settled) = (snr(channel, 0), snr(channel, frames / 2));
                println!("{name} channel {channel}: {all:.1} dB, second half {settled:.1} dB");
                // CELT-only packets agree with libopus to 16-bit precision. Hybrid
                // (SILK+CELT) packets agree less closely: mono44 opens with 400 ms
                // of them and the surround file's centre and LFE streams use them.
                assert!(all > 35.0, "{name} {channel}");
                if !(name == "surround.opus" && channel > 1) {
                    assert!(settled > 80.0, "{name} {channel}");
                }
            }
        }
    }
}
