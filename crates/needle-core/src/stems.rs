//! Stems: split a song into drums, bass, other, and vocals on this computer, then play them
//! back with a live mix.
//!
//! Separation uses HT-Demucs (Rouard, Massa and Défossez, 2023; MIT licence) exported to
//! ONNX by StemSplit, run with ONNX Runtime on the CPU. The model (166 MB) is downloaded on
//! first use into `<library>/models`. Songs are cut into 7.8-second windows that overlap by a
//! quarter and are blended back with linear crossfades, as in the exporter's reference code.
//! Stems are cached as 16-bit WAV files in `<library>/stems/<track id>/`.
use crate::{database::Library, integrations::client, model::Track};
use anyhow::{Context, Result, bail};
use rodio::Source;
use std::{
    io::{BufReader, Read, Write},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU32, Ordering},
    },
    time::Duration,
};

pub const STEMS: [&str; 4] = ["drums", "bass", "other", "vocals"];
pub const MODEL_URL: &str =
    "https://huggingface.co/StemSplitio/htdemucs-onnx/resolve/main/htdemucs_fp16weights.onnx";
const MODEL_FILE: &str = "htdemucs_fp16weights.onnx";
const RATE: u32 = 44_100;
/// The model's fixed window: 7.8 seconds at 44.1 kHz.
const WINDOW: usize = 343_980;
const OVERLAP: usize = WINDOW / 4;

pub fn model_path(library: &Library) -> PathBuf {
    library.directory.join("models").join(MODEL_FILE)
}
pub fn model_ready(library: &Library) -> bool {
    model_path(library)
        .metadata()
        .is_ok_and(|m| m.len() > 100 * 1024 * 1024)
}
pub fn stems_dir(library: &Library, track_id: &str) -> PathBuf {
    library
        .directory
        .join("stems")
        .join(track_id.replace(['\\', '/', ':'], "_"))
}
/// The folder of finished stems for a track, if it has been separated.
pub fn stems_for(library: &Library, track_id: &str) -> Option<PathBuf> {
    let dir = stems_dir(library, track_id);
    STEMS
        .iter()
        .all(|s| dir.join(format!("{s}.wav")).is_file())
        .then_some(dir)
}
pub fn delete_stems(library: &Library, track_id: &str) -> Result<()> {
    let dir = stems_dir(library, track_id);
    if dir.exists() {
        std::fs::remove_dir_all(dir)?;
    }
    Ok(())
}
/// Disk space used by all separated stems, in bytes.
pub fn cache_size(library: &Library) -> u64 {
    walkdir::WalkDir::new(library.directory.join("stems"))
        .into_iter()
        .filter_map(|e| e.ok())
        .filter_map(|e| e.metadata().ok())
        .filter(|m| m.is_file())
        .map(|m| m.len())
        .sum()
}

/// Download the separation model, reporting (bytes, total). Safe to call again after a stop.
pub fn download_model(
    library: &Library,
    cancel: &AtomicBool,
    mut progress: impl FnMut(u64, u64),
) -> Result<()> {
    if model_ready(library) {
        return Ok(());
    }
    let path = model_path(library);
    std::fs::create_dir_all(path.parent().unwrap())?;
    let mut response = client()?
        .get(MODEL_URL)
        .timeout(Duration::from_secs(60 * 30))
        .send()?
        .error_for_status()
        .context("The stem model could not be downloaded")?;
    let total = response.content_length().unwrap_or(0);
    let partial = path.with_extension("part");
    let mut file = std::io::BufWriter::new(std::fs::File::create(&partial)?);
    let mut buffer = vec![0u8; 1 << 16];
    let mut done = 0u64;
    loop {
        if cancel.load(Ordering::Relaxed) {
            drop(file);
            let _ = std::fs::remove_file(&partial);
            bail!("Download stopped");
        }
        let n = response.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        file.write_all(&buffer[..n])?;
        done += n as u64;
        progress(done, total);
    }
    file.flush()?;
    drop(file);
    if total > 0 && done != total {
        bail!("The download was incomplete ({done} of {total} bytes)");
    }
    std::fs::rename(partial, &path)?;
    Ok(())
}

/// Decode a file to planar stereo at 44.1 kHz.
fn load_stereo(track: &Track) -> Result<[Vec<f32>; 2]> {
    let source = crate::audio_file::decode_track(track)?;
    let channels = source.channels().max(1) as usize;
    let rate = source.sample_rate();
    let mut planar = [Vec::new(), Vec::new()];
    for (i, sample) in source.enumerate() {
        match (i % channels, channels) {
            (0, 1) => {
                planar[0].push(sample);
                planar[1].push(sample);
            }
            (c, _) if c < 2 => planar[c].push(sample),
            _ => {}
        }
    }
    let frames = planar[0].len().min(planar[1].len());
    planar[0].truncate(frames);
    planar[1].truncate(frames);
    if rate == RATE || frames == 0 {
        return Ok(planar);
    }
    resample(planar, rate)
}

fn resample(input: [Vec<f32>; 2], from: u32) -> Result<[Vec<f32>; 2]> {
    use rubato::{FftFixedIn, Resampler};
    let chunk = 4096;
    let mut resampler = FftFixedIn::<f32>::new(from as usize, RATE as usize, chunk, 2, 2)?;
    let frames = input[0].len();
    let expected = (frames as f64 * RATE as f64 / from as f64).round() as usize;
    let mut out = [
        Vec::with_capacity(expected + chunk),
        Vec::with_capacity(expected + chunk),
    ];
    let delay = resampler.output_delay();
    let mut position = 0;
    while position < frames {
        let need = resampler.input_frames_next();
        let end = (position + need).min(frames);
        let block: Vec<Vec<f32>> = input
            .iter()
            .map(|c| {
                let mut v = c[position..end].to_vec();
                v.resize(need, 0.);
                v
            })
            .collect();
        let result = resampler.process(&block, None)?;
        out[0].extend_from_slice(&result[0]);
        out[1].extend_from_slice(&result[1]);
        position = end;
    }
    // Flush the resampler's delay with silence.
    while out[0].len() < expected + delay {
        let need = resampler.input_frames_next();
        let result = resampler.process(&[vec![0.; need], vec![0.; need]], None)?;
        out[0].extend_from_slice(&result[0]);
        out[1].extend_from_slice(&result[1]);
    }
    for channel in out.iter_mut() {
        channel.drain(..delay.min(channel.len()));
        channel.truncate(expected);
    }
    Ok(out)
}

/// Linear fade-in and fade-out over the overlap, as the reference implementation uses.
fn window_weights() -> Vec<f32> {
    let mut w = vec![1f32; WINDOW];
    for i in 0..OVERLAP {
        let fade = i as f32 / (OVERLAP - 1) as f32;
        w[i] = fade;
        w[WINDOW - 1 - i] = fade;
    }
    w
}

/// Where each window starts, so windows overlap by a quarter and cover the whole song.
pub fn window_starts(frames: usize) -> Vec<usize> {
    let stride = WINDOW - OVERLAP;
    (0..)
        .map(|i| i * stride)
        .take_while(|s| *s < frames.max(1))
        .collect()
}

/// Split `track` into stems, reporting progress from 0 to 1. Returns the stem folder.
pub fn separate(
    library: &Library,
    track: &Track,
    cancel: &AtomicBool,
    mut progress: impl FnMut(f32),
) -> Result<PathBuf> {
    if let Some(dir) = stems_for(library, &track.id) {
        return Ok(dir);
    }
    if !model_ready(library) {
        bail!("The stem model is not downloaded yet");
    }
    progress(0.);
    let mix = load_stereo(track).with_context(|| format!("Cannot read {}", track.title))?;
    let frames = mix[0].len();
    if frames == 0 {
        bail!("{} has no audio", track.title);
    }
    let threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);
    let model = model_path(library);
    let build = || -> ort::Result<ort::session::Session> {
        ort::session::Session::builder()?
            .with_intra_threads(threads)?
            .commit_from_file(&model)
    };
    let mut session = build().map_err(|e| {
        anyhow::anyhow!(
            "The stem model could not be loaded ({e}). Delete it in Settings and download it again."
        )
    })?;
    let weights = window_weights();
    let mut out = vec![vec![0f32; frames]; STEMS.len() * 2];
    let mut total_weight = vec![0f32; frames];
    let starts = window_starts(frames);
    for (n, start) in starts.iter().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            bail!("Stopped");
        }
        let length = (frames - start).min(WINDOW);
        let mut input = vec![0f32; 2 * WINDOW];
        input[..length].copy_from_slice(&mix[0][*start..start + length]);
        input[WINDOW..WINDOW + length].copy_from_slice(&mix[1][*start..start + length]);
        let ort = |e: ort::Error| anyhow::anyhow!("Stem separation failed: {e}");
        let tensor = ort::value::Tensor::from_array(([1usize, 2, WINDOW], input)).map_err(ort)?;
        let outputs = session.run(ort::inputs!["mix" => tensor]).map_err(ort)?;
        let (_, stems) = outputs["stems"].try_extract_tensor::<f32>().map_err(ort)?;
        for (i, weight) in weights.iter().take(length).enumerate() {
            // The first window starts at full weight, the last ends at full weight.
            let w = if (*start == 0 && i < OVERLAP)
                || (n + 1 == starts.len() && i >= WINDOW - OVERLAP)
            {
                1.
            } else {
                *weight
            };
            total_weight[start + i] += w;
            for s in 0..STEMS.len() * 2 {
                out[s][start + i] += stems[s * WINDOW + i] * w;
            }
        }
        progress((n + 1) as f32 / starts.len() as f32);
    }
    let dir = stems_dir(library, &track.id);
    let partial = dir.with_extension("part");
    let _ = std::fs::remove_dir_all(&partial);
    std::fs::create_dir_all(&partial)?;
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: RATE,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    for (s, name) in STEMS.iter().enumerate() {
        let mut writer = hound::WavWriter::create(partial.join(format!("{name}.wav")), spec)?;
        for i in 0..frames {
            let weight = total_weight[i].max(1e-6);
            for c in 0..2 {
                let v = (out[s * 2 + c][i] / weight).clamp(-1., 1.);
                writer.write_sample((v * i16::MAX as f32).round() as i16)?;
            }
        }
        writer.finalize()?;
    }
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::rename(&partial, &dir)?;
    Ok(dir)
}

/// Live per-stem gains shared between the interface and the playing source (0 = silent).
#[derive(Debug)]
pub struct StemMix {
    gains: [AtomicU32; 4],
}
impl Default for StemMix {
    fn default() -> Self {
        Self {
            gains: std::array::from_fn(|_| AtomicU32::new(1f32.to_bits())),
        }
    }
}
impl StemMix {
    pub fn set(&self, stem: usize, gain: f32) {
        if let Some(slot) = self.gains.get(stem) {
            slot.store(gain.clamp(0., 2.).to_bits(), Ordering::Relaxed);
        }
    }
    pub fn get(&self, stem: usize) -> f32 {
        self.gains
            .get(stem)
            .map_or(0., |g| f32::from_bits(g.load(Ordering::Relaxed)))
    }
}

type Reader = hound::WavReader<BufReader<std::fs::File>>;

/// Plays the four stem files mixed together with live gains.
pub struct StemSource {
    readers: Vec<Reader>,
    mix: Arc<StemMix>,
    frames: u32,
    channel: usize,
    frame: [f32; 2],
}
impl StemSource {
    pub fn open(dir: &Path, mix: Arc<StemMix>) -> Result<Self> {
        let readers: Vec<Reader> = STEMS
            .iter()
            .map(|s| {
                hound::WavReader::open(dir.join(format!("{s}.wav"))).map_err(anyhow::Error::from)
            })
            .collect::<Result<_>>()?;
        let frames = readers.iter().map(|r| r.duration()).min().unwrap_or(0);
        Ok(Self {
            readers,
            mix,
            frames,
            channel: 2,
            frame: [0.; 2],
        })
    }
}
impl Iterator for StemSource {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        if self.channel >= 2 {
            let mut frame = [0f32; 2];
            for (s, reader) in self.readers.iter_mut().enumerate() {
                let gain = self.mix.get(s);
                let mut samples = reader.samples::<i16>();
                for slot in frame.iter_mut() {
                    let v = samples.next()?.ok()?;
                    *slot += v as f32 / i16::MAX as f32 * gain;
                }
            }
            self.frame = frame;
            self.channel = 0;
        }
        let sample = self.frame[self.channel];
        self.channel += 1;
        Some(sample)
    }
}
impl Source for StemSource {
    fn current_span_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> u16 {
        2
    }
    fn sample_rate(&self) -> u32 {
        RATE
    }
    fn total_duration(&self) -> Option<Duration> {
        Some(Duration::from_secs_f64(self.frames as f64 / RATE as f64))
    }
    fn try_seek(&mut self, pos: Duration) -> Result<(), rodio::source::SeekError> {
        let frame = ((pos.as_secs_f64() * RATE as f64) as u32).min(self.frames);
        for reader in self.readers.iter_mut() {
            reader
                .seek(frame)
                .map_err(|e| rodio::source::SeekError::Other(Box::new(e)))?;
        }
        self.channel = 2;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_cover_the_song_and_blend_to_unity() {
        assert_eq!(window_starts(10), vec![0]);
        let frames = WINDOW * 3;
        let starts = window_starts(frames);
        assert!(starts.windows(2).all(|p| p[1] - p[0] == WINDOW - OVERLAP));
        assert!(*starts.last().unwrap() + WINDOW >= frames);
        // Overlapping fades add up to a constant weight.
        let w = window_weights();
        for i in 0..OVERLAP {
            assert!((w[WINDOW - OVERLAP + i] + w[i] - 1.).abs() < 1e-5);
        }
    }

    #[test]
    fn resampling_keeps_length_and_pitch() {
        let from = 48_000;
        let seconds = 2.;
        let n = (from as f64 * seconds) as usize;
        let tone: Vec<f32> = (0..n)
            .map(|i| (2. * std::f32::consts::PI * 1000. * i as f32 / from as f32).sin() * 0.5)
            .collect();
        let out = resample([tone.clone(), tone], from).unwrap();
        assert_eq!(out[0].len(), (RATE as f64 * seconds) as usize);
        // Count zero crossings in the middle second: a 1 kHz tone has about 2000.
        let middle = &out[0][RATE as usize / 2..RATE as usize * 3 / 2];
        let crossings = middle
            .windows(2)
            .filter(|p| (p[0] < 0.) != (p[1] < 0.))
            .count();
        assert!((1990..=2010).contains(&crossings), "{crossings}");
    }

    #[test]
    fn stem_source_mixes_with_live_gains_and_seeks() {
        let dir = tempfile::tempdir().unwrap();
        let spec = hound::WavSpec {
            channels: 2,
            sample_rate: RATE,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        for (s, name) in STEMS.iter().enumerate() {
            let mut w =
                hound::WavWriter::create(dir.path().join(format!("{name}.wav")), spec).unwrap();
            for i in 0..RATE {
                let v = ((s + 1) as i16) * 1000 + (i % 2) as i16;
                w.write_sample(v).unwrap();
                w.write_sample(v).unwrap();
            }
            w.finalize().unwrap();
        }
        let mix = Arc::new(StemMix::default());
        let mut source = StemSource::open(dir.path(), mix.clone()).unwrap();
        let all = source.next().unwrap();
        assert!(
            (all - 10_000. / i16::MAX as f32).abs() < 1e-4,
            "1000+2000+3000+4000"
        );
        mix.set(3, 0.);
        mix.set(0, 0.);
        source.next();
        let without = source.next().unwrap();
        assert!(
            (without - (2000. + 3000. + 2.) / i16::MAX as f32).abs() < 1e-3,
            "{without}"
        );
        source.try_seek(Duration::from_millis(500)).unwrap();
        assert!(source.next().is_some());
        assert_eq!(source.total_duration(), Some(Duration::from_secs(1)));
    }

    /// Runs the real model when it is present at `artifacts/models` (download it with
    /// `curl -L -o artifacts/models/htdemucs_fp16weights.onnx <MODEL_URL>`).
    #[test]
    #[ignore = "needs the 166 MB model file"]
    fn real_model_splits_a_mix() {
        let model = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../artifacts/models")
            .join(MODEL_FILE);
        let dir = tempfile::tempdir().unwrap();
        let library = Library::open(dir.path()).unwrap();
        std::fs::create_dir_all(dir.path().join("models")).unwrap();
        std::fs::copy(&model, model_path(&library)).unwrap();
        // A low tone (bass-like) plus a noisy click track (drum-like), 10 seconds.
        let audio = dir.path().join("mix.wav");
        let spec = hound::WavSpec {
            channels: 2,
            sample_rate: RATE,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut w = hound::WavWriter::create(&audio, spec).unwrap();
        for i in 0..RATE * 10 {
            let t = i as f32 / RATE as f32;
            let bass = (2. * std::f32::consts::PI * 55. * t).sin() * 0.3;
            let click = if (i % (RATE / 2)) < 400 {
                ((i * 7919) % 200) as f32 / 200. - 0.5
            } else {
                0.
            };
            let v = ((bass + click) * 0.8 * i16::MAX as f32) as i16;
            w.write_sample(v).unwrap();
            w.write_sample(v).unwrap();
        }
        w.finalize().unwrap();
        let track = Track {
            id: "mix".into(),
            path: audio.to_string_lossy().into(),
            title: "mix".into(),
            ..Default::default()
        };
        let started = std::time::Instant::now();
        let out = separate(&library, &track, &AtomicBool::new(false), |_| {}).unwrap();
        eprintln!("separated 10 s in {:?}", started.elapsed());
        let energy = |name: &str| -> f64 {
            let mut r = hound::WavReader::open(out.join(format!("{name}.wav"))).unwrap();
            let s: Vec<i16> = r.samples::<i16>().map(|s| s.unwrap()).collect();
            assert_eq!(s.len(), RATE as usize * 10 * 2);
            s.iter().map(|v| (*v as f64).powi(2)).sum::<f64>() / s.len() as f64
        };
        let (drums, bass, vocals) = (energy("drums"), energy("bass"), energy("vocals"));
        eprintln!("energy drums {drums:.0} bass {bass:.0} vocals {vocals:.0}");
        assert!(
            bass > vocals,
            "the low tone should land in bass, not vocals"
        );
    }
}
