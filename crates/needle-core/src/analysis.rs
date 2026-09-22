use crate::{database::Library, model::Track};
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

/// ReplayGain reference level used for track and album gain.
pub const TARGET_LUFS: f64 = -18.0;

fn meter(path: &Path) -> Result<ebur128::EbuR128> {
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
    Ok(meter)
}
fn peak(meter: &ebur128::EbuR128, channels: u32) -> Result<f64> {
    let mut peak = 0f64;
    for channel in 0..channels {
        peak = peak.max(meter.true_peak(channel)?);
    }
    Ok(peak)
}
fn summarize(integrated: f64, true_peak: f64) -> Result<Loudness> {
    if !integrated.is_finite() {
        bail!("Track is silent or too short for integrated loudness measurement")
    }
    Ok(Loudness {
        integrated_lufs: integrated,
        replay_gain_db: TARGET_LUFS - integrated,
        true_peak,
    })
}
pub fn loudness(path: &Path) -> Result<Loudness> {
    let meter = meter(path)?;
    summarize(meter.loudness_global()?, peak(&meter, meter.channels())?)
}
pub fn scan_loudness(library: &Library, id: &str) -> Result<Loudness> {
    let mut track = library.track(id)?.context("Track not found")?;
    let result = loudness(Path::new(&track.path))?;
    track.replay_gain = Some(result.replay_gain_db);
    track.replay_peak = Some(result.true_peak);
    library.upsert(&track)?;
    Ok(result)
}

/// Album identity for album gain: album artist (else artist) and album title,
/// compared case-insensitively. Tracks without an album title have none.
pub fn album_key(track: &Track) -> Option<(String, String)> {
    let artist = match track.album_artist.trim() {
        "" => track.artist.trim(),
        artist => artist,
    };
    let album = track.album.trim();
    (!album.is_empty()).then(|| (artist.to_lowercase(), album.to_lowercase()))
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AlbumTrack {
    pub id: String,
    pub title: String,
    /// `None` when the track alone is silent or too short to measure.
    pub loudness: Option<Loudness>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AlbumLoudness {
    pub album: String,
    pub album_artist: String,
    /// Gated integrated loudness of all tracks measured as one programme.
    pub integrated_lufs: f64,
    pub replay_gain_db: f64,
    /// Highest true peak of any track.
    pub true_peak: f64,
    pub tracks: Vec<AlbumTrack>,
}
/// Album loudness over several files, plus each file's own loudness.
fn measure_album(paths: &[&Path]) -> Result<(Loudness, Vec<Option<Loudness>>)> {
    let mut meters = vec![];
    for path in paths {
        meters.push(meter(path).with_context(|| format!("Cannot measure {}", path.display()))?);
    }
    let mut tracks = vec![];
    let mut album_peak = 0f64;
    for meter in &meters {
        let peak = peak(meter, meter.channels())?;
        album_peak = album_peak.max(peak);
        tracks.push(summarize(meter.loudness_global()?, peak).ok());
    }
    let integrated = ebur128::EbuR128::loudness_global_multiple(meters.iter())?;
    let album = summarize(integrated, album_peak)
        .context("Album is silent or too short for integrated loudness measurement")?;
    Ok((album, tracks))
}
/// Measures the album containing `track_id` and stores album gain and peak on
/// every available track of that album, along with each track's own gain.
pub fn scan_album_loudness(library: &Library, track_id: &str) -> Result<AlbumLoudness> {
    let track = library.track(track_id)?.context("Track not found")?;
    let key = album_key(&track).context("Track has no album title")?;
    let ids: Vec<String> = {
        let db = library.connection()?;
        let mut statement = db.prepare(
            "SELECT id FROM tracks WHERE trim(album) = ?1 COLLATE NOCASE AND missing = 0
             ORDER BY disc, track_number, path",
        )?;
        statement
            .query_map([track.album.trim()], |r| r.get(0))?
            .collect::<std::result::Result<_, _>>()?
    };
    let mut tracks: Vec<Track> = library
        .tracks_by_ids(&ids)?
        .into_iter()
        .filter(|t| album_key(t).as_ref() == Some(&key))
        .collect();
    if tracks.is_empty() {
        bail!("No playable tracks found for this album")
    }
    let paths: Vec<&Path> = tracks.iter().map(|t| Path::new(&t.path)).collect();
    let (album, measured) = measure_album(&paths)?;
    let mut result = AlbumLoudness {
        album: track.album.clone(),
        album_artist: match track.album_artist.as_str() {
            "" => track.artist.clone(),
            artist => artist.into(),
        },
        integrated_lufs: album.integrated_lufs,
        replay_gain_db: album.replay_gain_db,
        true_peak: album.true_peak,
        tracks: vec![],
    };
    for (track, loudness) in tracks.iter_mut().zip(measured) {
        track.album_replay_gain = Some(album.replay_gain_db);
        track.album_peak = Some(album.true_peak);
        if let Some(loudness) = &loudness {
            track.replay_gain = Some(loudness.replay_gain_db);
            track.replay_peak = Some(loudness.true_peak);
        }
        library.upsert(track)?;
        result.tracks.push(AlbumTrack {
            id: track.id.clone(),
            title: track.title.clone(),
            loudness,
        });
    }
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
    fn sine(path: &Path, rate: u32, seconds: u32, amplitude: f32) {
        let mut writer = hound::WavWriter::create(
            path,
            hound::WavSpec {
                channels: 1,
                sample_rate: rate,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            },
        )
        .unwrap();
        for i in 0..rate * seconds {
            let sample = (i as f32 * 1000. * std::f32::consts::TAU / rate as f32).sin() * amplitude;
            writer.write_sample((sample * 32767.) as i16).unwrap();
        }
        writer.finalize().unwrap();
    }
    #[test]
    fn measures_known_sine_and_fingerprints_audio() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sine.wav");
        sine(&path, 44100, 12, 0.1);
        let result = loudness(&path).unwrap();
        assert!((result.integrated_lufs + 23.0).abs() < 0.3);
        assert!((result.true_peak - 0.1).abs() < 0.005);
        let fp = fingerprint(&path).unwrap();
        assert!(!fp.encoded.is_empty());
        assert!(fp.raw.len() > 32);
    }
    #[test]
    fn album_loudness_combines_tracks_as_one_programme() {
        let dir = tempfile::tempdir().unwrap();
        let loud = dir.path().join("loud.wav");
        let quiet = dir.path().join("quiet.wav");
        let silent = dir.path().join("silent.wav");
        sine(&loud, 44100, 12, 0.1);
        sine(&quiet, 48000, 12, 0.05);
        sine(&silent, 48000, 2, 0.0);
        let (album, tracks) = measure_album(&[&loud, &quiet, &silent]).unwrap();
        let (a, b) = (tracks[0].as_ref().unwrap(), tracks[1].as_ref().unwrap());
        assert!((a.integrated_lufs + 23.0).abs() < 0.3);
        assert!((b.integrated_lufs + 29.0).abs() < 0.3);
        assert!(tracks[2].is_none());
        // Equal durations: the album is the mean energy, about -25 LUFS.
        let energy = |lufs: f64| 10f64.powf(lufs / 10.0);
        let expected =
            10.0 * ((energy(a.integrated_lufs) + energy(b.integrated_lufs)) / 2.0).log10();
        assert!((album.integrated_lufs - expected).abs() < 0.05, "{album:?}");
        assert!((album.replay_gain_db - (TARGET_LUFS - album.integrated_lufs)).abs() < 1e-9);
        assert_eq!(album.true_peak, a.true_peak.max(b.true_peak));
        assert!(measure_album(&[&silent]).is_err());
    }
    #[test]
    fn album_scan_groups_by_album_artist_and_stores_gain() {
        let dir = tempfile::tempdir().unwrap();
        let library = Library::open(dir.path().join("db")).unwrap();
        let add = |id: &str, artist: &str, album_artist: &str, album: &str, amplitude| {
            let path = dir.path().join(format!("{id}.wav"));
            sine(&path, 44100, 6, amplitude);
            library
                .upsert(&Track {
                    id: id.into(),
                    path: path.to_string_lossy().into(),
                    title: id.into(),
                    artist: artist.into(),
                    album_artist: album_artist.into(),
                    album: album.into(),
                    ..Default::default()
                })
                .unwrap();
        };
        add("one", "Ann", "Ann", "Record", 0.1);
        add("two", "Guest", "ann", "record ", 0.05);
        add("three", "Ann", "", "RECORD", 0.1);
        add("other", "Bob", "Bob", "Record", 0.2);
        add("single", "Ann", "Ann", "", 0.2);
        let result = scan_album_loudness(&library, "two").unwrap();
        let mut ids: Vec<_> = result.tracks.iter().map(|t| t.id.as_str()).collect();
        ids.sort();
        assert_eq!(ids, ["one", "three", "two"]);
        for id in ["one", "two", "three"] {
            let track = library.track(id).unwrap().unwrap();
            assert_eq!(track.album_replay_gain, Some(result.replay_gain_db));
            assert_eq!(track.album_peak, Some(result.true_peak));
            assert!(track.replay_gain.is_some());
        }
        let other = library.track("other").unwrap().unwrap();
        assert!(other.album_replay_gain.is_none());
        assert!((result.true_peak - 0.1).abs() < 0.005);
        assert!(result.integrated_lufs < -23.0 && result.integrated_lufs > -29.0);
        assert!(scan_album_loudness(&library, "single").is_err());
    }
}
