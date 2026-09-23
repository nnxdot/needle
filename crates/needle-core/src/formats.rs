//! Decoders for formats Symphonia does not read: WavPack (`.wv`), Monkey's Audio (`.ape`), and
//! DSD in DSF files (`.dsf`, converted to PCM). Each streams from disk a block or frame at a
//! time and can seek.
use anyhow::{Context, Result, bail, ensure};
use rodio::{Source, source::SeekError};
use std::{
    fs::File,
    io::{BufReader, Read, Seek, SeekFrom},
    path::Path,
    time::Duration,
};

fn seek_error(what: &'static str) -> SeekError {
    SeekError::NotSupported {
        underlying_source: what,
    }
}

/// Samples decoded but not yet played.
#[derive(Default)]
struct Pending {
    samples: Vec<f32>,
    at: usize,
}
impl Pending {
    fn next(&mut self) -> Option<f32> {
        let s = self.samples.get(self.at).copied();
        self.at += 1;
        s
    }
    fn fill(&mut self, samples: Vec<f32>, skip: usize) {
        self.samples = samples;
        self.at = skip;
    }
}

// ---------- WavPack ----------

pub struct WavPack {
    file: BufReader<File>,
    /// (byte offset, first sample, sample count) of every audio block.
    blocks: Vec<(u64, u64, u32)>,
    next_block: usize,
    channels: u16,
    rate: u32,
    bits: u32,
    float: bool,
    total: u64,
    pending: Pending,
}

impl WavPack {
    pub fn open(path: &Path) -> Result<Self> {
        let mut file = BufReader::new(File::open(path)?);
        let mut blocks = vec![];
        let mut offset = 0u64;
        let mut header = [0u8; wavicle::block::HEADER_LEN];
        let mut first: Option<Vec<u8>> = None;
        while file.read_exact(&mut header).is_ok() {
            // Tags (APEv2 or ID3) follow the audio blocks.
            if &header[..4] != b"wvpk" {
                break;
            }
            let h = wavicle::BlockHeader::parse(&header)
                .map_err(|e| anyhow::anyhow!("WavPack: {e}"))?;
            let len = h.block_len() as u64;
            if h.block_samples > 0 {
                blocks.push((offset, h.block_index, h.block_samples));
                if first.is_none() {
                    let mut bytes = header.to_vec();
                    bytes.resize(len as usize, 0);
                    file.read_exact(&mut bytes[wavicle::block::HEADER_LEN..])?;
                    first = Some(bytes);
                    offset += len;
                    continue;
                }
            }
            offset += len;
            file.seek(SeekFrom::Start(offset))?;
        }
        let first = first.context("WavPack: no audio")?;
        let sample = wavicle::decode_stream(&first).map_err(|e| anyhow::anyhow!("WavPack: {e}"))?;
        let total = blocks.last().map(|(_, s, n)| s + *n as u64).unwrap_or(0);
        let mut this = Self {
            file,
            blocks,
            next_block: 0,
            channels: sample.channels.max(1) as u16,
            rate: sample.sample_rate.max(1),
            bits: sample.bits_per_sample,
            float: sample.is_float,
            total,
            pending: Pending::default(),
        };
        this.load(0, 0)?;
        Ok(this)
    }

    fn load(&mut self, index: usize, skip_frames: u64) -> Result<bool> {
        let Some(&(offset, _, _)) = self.blocks.get(index) else {
            return Ok(false);
        };
        self.file.seek(SeekFrom::Start(offset))?;
        let mut header = [0u8; wavicle::block::HEADER_LEN];
        self.file.read_exact(&mut header)?;
        let h =
            wavicle::BlockHeader::parse(&header).map_err(|e| anyhow::anyhow!("WavPack: {e}"))?;
        let mut bytes = header.to_vec();
        bytes.resize(h.block_len(), 0);
        self.file
            .read_exact(&mut bytes[wavicle::block::HEADER_LEN..])?;
        let decoded =
            wavicle::decode_stream(&bytes).map_err(|e| anyhow::anyhow!("WavPack: {e}"))?;
        let scale = 1. / (1u64 << (self.bits.clamp(8, 32) - 1)) as f32;
        let samples = decoded
            .samples
            .iter()
            .map(|&s| {
                if self.float {
                    f32::from_bits(s as u32)
                } else {
                    s as f32 * scale
                }
            })
            .collect();
        self.pending
            .fill(samples, (skip_frames * self.channels as u64) as usize);
        self.next_block = index + 1;
        Ok(true)
    }
}

impl Iterator for WavPack {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        loop {
            if let Some(s) = self.pending.next() {
                return Some(s);
            }
            if !self.load(self.next_block, 0).ok()? {
                return None;
            }
        }
    }
}

impl Source for WavPack {
    fn current_span_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> u16 {
        self.channels
    }
    fn sample_rate(&self) -> u32 {
        self.rate
    }
    fn total_duration(&self) -> Option<Duration> {
        Some(Duration::from_secs_f64(
            self.total as f64 / self.rate as f64,
        ))
    }
    fn try_seek(&mut self, pos: Duration) -> Result<(), SeekError> {
        let target = (pos.as_secs_f64() * self.rate as f64) as u64;
        let index = self
            .blocks
            .iter()
            .rposition(|(_, start, _)| *start <= target)
            .unwrap_or(0);
        let skip = target.saturating_sub(self.blocks[index].1);
        self.load(index, skip).map_err(|_| seek_error("WavPack"))?;
        Ok(())
    }
}

// ---------- Monkey's Audio ----------

pub struct Ape {
    decoder: ape_decoder::ApeDecoder<BufReader<File>>,
    next_frame: u32,
    channels: u16,
    rate: u32,
    bits: u16,
    total: u64,
    pending: Pending,
}

impl Ape {
    pub fn open(path: &Path) -> Result<Self> {
        let decoder = ape_decoder::ApeDecoder::new(BufReader::new(File::open(path)?))
            .map_err(|e| anyhow::anyhow!("Monkey's Audio: {e}"))?;
        let info = decoder.info();
        let (channels, rate, bits, total) = (
            info.channels.max(1),
            info.sample_rate.max(1),
            info.bits_per_sample,
            info.total_samples,
        );
        ensure!(
            matches!(bits, 8 | 16 | 24 | 32),
            "Monkey's Audio: {bits}-bit audio is not supported"
        );
        Ok(Self {
            decoder,
            next_frame: 0,
            channels,
            rate,
            bits,
            total,
            pending: Pending::default(),
        })
    }

    fn convert(&self, bytes: &[u8]) -> Vec<f32> {
        match self.bits {
            8 => bytes.iter().map(|&b| (b as f32 - 128.) / 128.).collect(),
            16 => bytes
                .as_chunks::<2>()
                .0
                .iter()
                .map(|c| i16::from_le_bytes([c[0], c[1]]) as f32 / 32768.)
                .collect(),
            24 => bytes
                .as_chunks::<3>()
                .0
                .iter()
                .map(|c| (i32::from_le_bytes([0, c[0], c[1], c[2]]) >> 8) as f32 / 8_388_608.)
                .collect(),
            _ => bytes
                .as_chunks::<4>()
                .0
                .iter()
                .map(|c| i32::from_le_bytes([c[0], c[1], c[2], c[3]]) as f32 / 2_147_483_648.)
                .collect(),
        }
    }

    fn load(&mut self, frame: u32, skip_frames: u64) -> Result<bool> {
        if frame >= self.decoder.total_frames() {
            return Ok(false);
        }
        let bytes = self
            .decoder
            .decode_frame(frame)
            .map_err(|e| anyhow::anyhow!("Monkey's Audio: {e}"))?;
        let samples = self.convert(&bytes);
        self.pending
            .fill(samples, (skip_frames * self.channels as u64) as usize);
        self.next_frame = frame + 1;
        Ok(true)
    }
}

impl Iterator for Ape {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        loop {
            if let Some(s) = self.pending.next() {
                return Some(s);
            }
            if !self.load(self.next_frame, 0).ok()? {
                return None;
            }
        }
    }
}

impl Source for Ape {
    fn current_span_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> u16 {
        self.channels
    }
    fn sample_rate(&self) -> u32 {
        self.rate
    }
    fn total_duration(&self) -> Option<Duration> {
        Some(Duration::from_secs_f64(
            self.total as f64 / self.rate as f64,
        ))
    }
    fn try_seek(&mut self, pos: Duration) -> Result<(), SeekError> {
        let target = ((pos.as_secs_f64() * self.rate as f64) as u64).min(self.total);
        let per_frame = self.decoder.info().blocks_per_frame.max(1) as u64;
        let frame = (target / per_frame) as u32;
        self.load(frame, target - frame as u64 * per_frame)
            .map_err(|_| seek_error("Monkey's Audio"))?;
        Ok(())
    }
}

// ---------- DSD (DSF) ----------

/// Bytes of DSD per output sample: 1-bit DSD at `rate` becomes PCM at `rate / 32`
/// (88.2 kHz from DSD64).
const DECIMATION_BYTES: usize = 4;
/// Filter length in bytes (8 DSD samples each).
const TAPS_BYTES: usize = 24;

/// DSD in a DSF file, low-pass filtered and decimated to PCM.
pub struct Dsf {
    file: BufReader<File>,
    data_start: u64,
    channels: u16,
    dsd_rate: u32,
    block: usize,
    /// DSD bytes per channel in the file (without padding).
    bytes_per_channel: u64,
    /// Next block group to read.
    next_group: u64,
    /// Per channel: recent DSD bytes, MSB-first, oldest first.
    history: Vec<Vec<u8>>,
    table: Vec<[f32; 256]>,
    pending: Pending,
}

/// Per byte position in the filter, the filter's contribution for each possible byte.
fn dsd_table() -> Vec<[f32; 256]> {
    let taps = TAPS_BYTES * 8;
    let decimation = (DECIMATION_BYTES * 8) as f64;
    // Windowed sinc low-pass at a little under the output Nyquist frequency.
    let cutoff = 0.45 / decimation;
    let mut h: Vec<f64> = (0..taps)
        .map(|i| {
            let x = i as f64 - (taps - 1) as f64 / 2.;
            let sinc = if x == 0. {
                2. * cutoff
            } else {
                (2. * std::f64::consts::PI * cutoff * x).sin() / (std::f64::consts::PI * x)
            };
            let window = 0.42
                - 0.5 * (2. * std::f64::consts::PI * i as f64 / (taps - 1) as f64).cos()
                + 0.08 * (4. * std::f64::consts::PI * i as f64 / (taps - 1) as f64).cos();
            sinc * window
        })
        .collect();
    let sum: f64 = h.iter().sum();
    h.iter_mut().for_each(|v| *v /= sum);
    (0..TAPS_BYTES)
        .map(|b| {
            let mut row = [0f32; 256];
            for (byte, slot) in row.iter_mut().enumerate() {
                *slot = (0..8)
                    .map(|bit| {
                        let one = (byte >> (7 - bit)) & 1 == 1;
                        h[b * 8 + bit] * if one { 1. } else { -1. }
                    })
                    .sum::<f64>() as f32;
            }
            row
        })
        .collect()
}

impl Dsf {
    pub fn open(path: &Path) -> Result<Self> {
        let mut file = BufReader::new(File::open(path)?);
        let mut head = [0u8; 28];
        file.read_exact(&mut head)?;
        ensure!(&head[..4] == b"DSD ", "Not a DSF file");
        let mut fmt = [0u8; 52];
        file.read_exact(&mut fmt)?;
        ensure!(&fmt[..4] == b"fmt ", "DSF: missing format chunk");
        let le32 = |o: usize| u32::from_le_bytes(fmt[o..o + 4].try_into().unwrap());
        let le64 = |o: usize| u64::from_le_bytes(fmt[o..o + 8].try_into().unwrap());
        let channels = le32(24) as u16;
        let dsd_rate = le32(28);
        let bits = le32(32);
        let sample_count = le64(36);
        let block = le32(44) as usize;
        let fmt_size = le64(4);
        ensure!(
            (1..=8).contains(&channels) && dsd_rate > 0 && block > 0,
            "DSF: unsupported layout"
        );
        file.seek(SeekFrom::Start(28 + fmt_size))?;
        let mut data = [0u8; 12];
        file.read_exact(&mut data)?;
        ensure!(&data[..4] == b"data", "DSF: missing data chunk");
        let data_start = 28 + fmt_size + 12;
        if bits != 1 {
            bail!("DSF: only 1-bit (LSB-first) data is supported");
        }
        let mut this = Self {
            file,
            data_start,
            channels,
            dsd_rate,
            block,
            bytes_per_channel: sample_count.div_ceil(8),
            next_group: 0,
            history: vec![vec![0x69; TAPS_BYTES]; channels as usize],
            table: dsd_table(),
            pending: Pending::default(),
        };
        this.load(0, 0)?;
        Ok(this)
    }

    fn groups(&self) -> u64 {
        self.bytes_per_channel.div_ceil(self.block as u64)
    }

    /// Read one block per channel and turn it into PCM.
    fn load(&mut self, group: u64, skip_frames: u64) -> Result<bool> {
        if group >= self.groups() {
            return Ok(false);
        }
        let channels = self.channels as usize;
        let offset = self.data_start + group * (self.block * channels) as u64;
        self.file.seek(SeekFrom::Start(offset))?;
        let mut raw = vec![0u8; self.block * channels];
        let read = self.file.read(&mut raw)?;
        raw.truncate(read);
        let valid =
            (self.bytes_per_channel - group * self.block as u64).min(self.block as u64) as usize;
        let frames = valid / DECIMATION_BYTES;
        let mut out = vec![0f32; frames * channels];
        for c in 0..channels {
            let bytes = raw
                .get(c * self.block..c * self.block + valid)
                .unwrap_or(&[]);
            let history = &mut self.history[c];
            for (f, chunk) in bytes.chunks(DECIMATION_BYTES).enumerate().take(frames) {
                for &b in chunk {
                    // DSF stores bits LSB first; the table expects the oldest bit highest.
                    history.remove(0);
                    history.push(b.reverse_bits());
                }
                let mut sum = 0.;
                for (i, &byte) in history.iter().enumerate() {
                    sum += self.table[i][byte as usize];
                }
                out[f * channels + c] = sum.clamp(-1., 1.);
            }
        }
        self.pending
            .fill(out, (skip_frames * channels as u64) as usize);
        self.next_group = group + 1;
        Ok(true)
    }
}

impl Iterator for Dsf {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        loop {
            if let Some(s) = self.pending.next() {
                return Some(s);
            }
            if !self.load(self.next_group, 0).ok()? {
                return None;
            }
        }
    }
}

impl Source for Dsf {
    fn current_span_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> u16 {
        self.channels
    }
    fn sample_rate(&self) -> u32 {
        self.dsd_rate / (DECIMATION_BYTES as u32 * 8)
    }
    fn total_duration(&self) -> Option<Duration> {
        Some(Duration::from_secs_f64(
            self.bytes_per_channel as f64 * 8. / self.dsd_rate as f64,
        ))
    }
    fn try_seek(&mut self, pos: Duration) -> Result<(), SeekError> {
        let frame = (pos.as_secs_f64() * self.sample_rate() as f64) as u64;
        let frames_per_group = (self.block / DECIMATION_BYTES) as u64;
        let group = frame / frames_per_group;
        for h in &mut self.history {
            h.iter_mut().for_each(|b| *b = 0x69);
        }
        self.load(group, frame - group * frames_per_group)
            .map_err(|_| seek_error("DSF"))?;
        Ok(())
    }
}

/// Duration, sample rate, channels, and bits of a DSF file, for the library.
pub fn dsf_properties(path: &Path) -> Result<(f64, u32, u16)> {
    let dsf = Dsf::open(path)?;
    Ok((
        dsf.total_duration().unwrap_or_default().as_secs_f64(),
        dsf.dsd_rate,
        dsf.channels,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn testdata(name: &str) -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("testdata")
            .join(name)
    }

    /// Estimate the frequency of a tone from zero crossings.
    fn frequency(samples: &[f32], channels: usize, rate: u32) -> f64 {
        let mono: Vec<f32> = samples.chunks(channels).map(|f| f[0]).collect();
        let crossings = mono.windows(2).filter(|w| w[0] <= 0. && w[1] > 0.).count();
        crossings as f64 * rate as f64 / mono.len() as f64
    }

    #[test]
    fn wavpack_decodes_and_seeks() {
        let mut wv = WavPack::open(&testdata("tone.wv")).unwrap();
        assert_eq!((wv.channels(), wv.sample_rate()), (2, 44100));
        let duration = wv.total_duration().unwrap().as_secs_f64();
        assert!((duration - 3.).abs() < 0.05, "{duration}");
        let samples: Vec<f32> = wv.by_ref().take(44100 * 2).collect();
        let hz = frequency(&samples, 2, 44100);
        assert!((hz - 440.).abs() < 3., "{hz} Hz");
        wv.try_seek(Duration::from_secs(2)).unwrap();
        assert_eq!(
            wv.count(),
            44100 * 2,
            "one second left after seeking to 2 s"
        );
    }

    #[test]
    fn monkeys_audio_decodes_and_seeks() {
        for name in ["multiframe_16s_c2000.ape", "sine_24s_c2000.ape"] {
            let mut ape = Ape::open(&testdata(name)).unwrap();
            let total = ape.total;
            let channels = ape.channels() as usize;
            let all: Vec<f32> = ape.by_ref().collect();
            assert_eq!(all.len() as u64, total * channels as u64, "{name}");
            assert!(all.iter().any(|s| s.abs() > 0.01), "{name} is not silent");
            assert!(all.iter().all(|s| s.abs() <= 1.), "{name} stays in range");
            let half = Duration::from_secs_f64(total as f64 / 2. / ape.sample_rate() as f64);
            ape.try_seek(half).unwrap();
            let rest = ape.count() as i64;
            assert!(
                (rest - (total as i64 * channels as i64) / 2).abs() <= channels as i64 * 2,
                "{name}"
            );
        }
    }

    /// A 1 kHz tone as DSD64 in a DSF file, from a simple second-order sigma-delta modulator.
    fn write_dsf(path: &Path, seconds: f64) {
        let rate = 2_822_400u32;
        let block = 4096usize;
        let total_bits = (seconds * rate as f64) as u64;
        let bytes = total_bits / 8;
        let (mut i1, mut i2) = (0f64, 0f64);
        let mut data = vec![];
        for n in 0..bytes * 8 {
            let x = 0.5 * (2. * std::f64::consts::PI * 1000. * n as f64 / rate as f64).sin();
            let y = if i2 >= 0. { 1. } else { -1. };
            i1 += x - y;
            i2 += i1 - y;
            data.push(y > 0.);
        }
        let mut channel = vec![0u8; bytes.div_ceil(block as u64) as usize * block];
        for (i, bit) in data.iter().enumerate() {
            if *bit {
                channel[i / 8] |= 1 << (i % 8); // LSB first
            }
        }
        let mut body = vec![];
        for group in channel.chunks(block) {
            body.extend_from_slice(group); // left
            body.extend_from_slice(group); // right
        }
        let mut f = File::create(path).unwrap();
        let data_len = 12 + body.len() as u64;
        f.write_all(b"DSD ").unwrap();
        f.write_all(&28u64.to_le_bytes()).unwrap();
        f.write_all(&(28 + 52 + data_len).to_le_bytes()).unwrap();
        f.write_all(&0u64.to_le_bytes()).unwrap();
        f.write_all(b"fmt ").unwrap();
        f.write_all(&52u64.to_le_bytes()).unwrap();
        for v in [1u32, 0, 2, 2] {
            f.write_all(&v.to_le_bytes()).unwrap(); // version, format id, channel type, channels
        }
        f.write_all(&rate.to_le_bytes()).unwrap();
        f.write_all(&1u32.to_le_bytes()).unwrap();
        f.write_all(&total_bits.to_le_bytes()).unwrap();
        f.write_all(&(block as u32).to_le_bytes()).unwrap();
        f.write_all(&0u32.to_le_bytes()).unwrap();
        f.write_all(b"data").unwrap();
        f.write_all(&data_len.to_le_bytes()).unwrap();
        f.write_all(&body).unwrap();
    }

    #[test]
    fn dsd_becomes_a_clean_pcm_tone() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("tone.dsf");
        write_dsf(&path, 1.);
        let dsf = Dsf::open(&path).unwrap();
        assert_eq!((dsf.channels(), dsf.sample_rate()), (2, 88200));
        assert!((dsf.total_duration().unwrap().as_secs_f64() - 1.).abs() < 0.01);
        let samples: Vec<f32> = dsf.collect();
        let settled = &samples[2 * 2000..];
        let hz = frequency(settled, 2, 88200);
        assert!((hz - 1000.).abs() < 5., "{hz} Hz");
        let peak = settled.iter().fold(0f32, |m, s| m.max(s.abs()));
        assert!(peak > 0.35 && peak < 0.65, "peak {peak}");
    }
}
