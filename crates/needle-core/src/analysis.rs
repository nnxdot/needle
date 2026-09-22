use crate::database::Library;
use anyhow::{Context, Result, bail};
use rodio::Source;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Loudness {
    pub integrated_lufs: f64,
    pub replay_gain_db: f64,
    pub true_peak: f64,
}

pub fn loudness(path: &Path) -> Result<Loudness> {
    let decoder = crate::audio_file::decode(path)?;
    let channels = decoder.channels() as u32;
    let rate = decoder.sample_rate();
    let mut meter =
        ebur128::EbuR128::new(channels, rate, ebur128::Mode::I | ebur128::Mode::TRUE_PEAK)?;
    let chunk_size = 4096 * channels as usize;
    let mut chunk = Vec::with_capacity(chunk_size);
    for sample in decoder {
        chunk.push(sample);
        if chunk.len() == chunk_size {
            meter.add_frames_f32(&chunk)?;
            chunk.clear();
        }
    }
    if !chunk.is_empty() {
        chunk.truncate(chunk.len() / channels as usize * channels as usize);
        meter.add_frames_f32(&chunk)?;
    }
    let integrated = meter.loudness_global()?;
    if !integrated.is_finite() {
        bail!("Track is silent or too short for integrated loudness measurement")
    }
    let mut peak = 0f64;
    for channel in 0..channels {
        peak = peak.max(meter.true_peak(channel)?);
    }
    Ok(Loudness {
        integrated_lufs: integrated,
        replay_gain_db: -18.0 - integrated,
        true_peak: peak,
    })
}
pub fn scan_loudness(library: &Library, id: &str) -> Result<Loudness> {
    let mut track = library.track(id)?.context("Track not found")?;
    let result = loudness(Path::new(&track.path))?;
    track.replay_gain = Some(result.replay_gain_db);
    track.replay_peak = Some(result.true_peak);
    library.upsert(&track)?;
    Ok(result)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Fingerprint {
    pub duration: f64,
    pub encoded: String,
    pub raw: Vec<u32>,
}
pub fn fingerprint(path: &Path) -> Result<Fingerprint> {
    use base64::Engine;
    let decoder = crate::audio_file::decode(path)?;
    let rate = decoder.sample_rate();
    let channels = decoder.channels();
    let duration = decoder
        .total_duration()
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0);
    let configuration = rusty_chromaprint::Configuration::preset_test2();
    let mut printer = rusty_chromaprint::Fingerprinter::new(&configuration);
    printer.start(rate, channels as u32)?;
    let mut chunk = Vec::with_capacity(8192);
    for sample in decoder.take(rate as usize * channels as usize * 120) {
        chunk.push((sample.clamp(-1., 1.) * 32767.) as i16);
        if chunk.len() == 8192 {
            printer.consume(&chunk);
            chunk.clear();
        }
    }
    if !chunk.is_empty() {
        printer.consume(&chunk);
    }
    printer.finish();
    let raw = printer.fingerprint().to_vec();
    if raw.is_empty() {
        bail!("Track is too short to fingerprint")
    }
    let compressed = rusty_chromaprint::FingerprintCompressor::from(&configuration).compress(&raw);
    Ok(Fingerprint {
        duration,
        encoded: base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(compressed),
        raw,
    })
}

#[derive(Clone, Debug, Serialize)]
pub struct Duplicate {
    pub first: String,
    pub second: String,
    pub confidence: f64,
    pub exact_file: bool,
}
pub fn duplicates(library: &Library, expression: &str) -> Result<Vec<Duplicate>> {
    let tracks = library.search(expression)?;
    if tracks.len() > 5000 {
        bail!("Narrow the duplicate scan to at most 5,000 tracks using a query")
    }
    let mut fingerprints = std::collections::HashMap::new();
    let mut result = vec![];
    for (a, first) in tracks.iter().enumerate() {
        for second in tracks.iter().skip(a + 1) {
            if first.missing || second.missing || (first.duration - second.duration).abs() > 2.0 {
                continue;
            }
            if first.content_hash == second.content_hash && !first.content_hash.is_empty() {
                result.push(Duplicate {
                    first: first.id.clone(),
                    second: second.id.clone(),
                    confidence: 1.0,
                    exact_file: true,
                });
                continue;
            }
            for track in [first, second] {
                if !fingerprints.contains_key(&track.id)
                    && let Ok(f) = fingerprint(Path::new(&track.path))
                {
                    fingerprints.insert(track.id.clone(), f.raw);
                }
            }
            let (Some(a), Some(b)) = (fingerprints.get(&first.id), fingerprints.get(&second.id))
            else {
                continue;
            };
            let mut best = 0f64;
            for offset in -3i32..=3 {
                let pairs = a.iter().enumerate().filter_map(|(i, x)| {
                    let j = i as i32 + offset;
                    if j < 0 {
                        None
                    } else {
                        b.get(j as usize).map(|y| (x, y))
                    }
                });
                let (mut bits, mut count) = (0u64, 0u64);
                for (x, y) in pairs {
                    bits += (x ^ y).count_ones() as u64;
                    count += 1;
                }
                if count >= 32 {
                    best = best.max(1. - bits as f64 / (count * 32) as f64);
                }
            }
            if best >= 0.93 {
                result.push(Duplicate {
                    first: first.id.clone(),
                    second: second.id.clone(),
                    confidence: best,
                    exact_file: false,
                });
            }
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn measures_known_sine_and_fingerprints_audio() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sine.wav");
        let rate = 44100;
        let mut writer = hound::WavWriter::create(
            &path,
            hound::WavSpec {
                channels: 1,
                sample_rate: rate,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            },
        )
        .unwrap();
        for i in 0..rate * 12 {
            let sample = (i as f32 * 1000. * std::f32::consts::TAU / rate as f32).sin() * 0.1;
            writer.write_sample((sample * 32767.) as i16).unwrap();
        }
        writer.finalize().unwrap();
        let result = loudness(&path).unwrap();
        assert!((result.integrated_lufs + 23.0).abs() < 0.3);
        assert!((result.true_peak - 0.1).abs() < 0.005);
        let fp = fingerprint(&path).unwrap();
        assert!(!fp.encoded.is_empty());
        assert!(fp.raw.len() > 32);
    }
}
