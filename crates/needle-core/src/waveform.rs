//! A song's shape for the seek bar: how loud each part of it is, measured once on this device
//! and kept.
use crate::{database::Library, model::Track};
use anyhow::{Result, bail};
use rodio::Source;
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};

/// Bump when the measuring changes, so songs are measured again.
pub const VERSION: u32 = 1;
/// How many parts a song is cut into.
pub const BINS: usize = 240;

/// A song's shape: for each of `BINS` equal parts, its loudest moment and its average
/// loudness, 0 to 255, relative to the song's own loudest part.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Waveform {
    pub version: u32,
    pub peaks: Vec<u8>,
    pub loudness: Vec<u8>,
}

impl Waveform {
    /// The parts as 0-to-1 heights, `count` of them (fewer or more than `BINS`, for a bar of
    /// any width), from the average loudness.
    pub fn heights(&self, count: usize) -> Vec<f32> {
        if self.loudness.is_empty() || count == 0 {
            return vec![];
        }
        (0..count)
            .map(|i| {
                let from = i * self.loudness.len() / count;
                let to = ((i + 1) * self.loudness.len() / count)
                    .max(from + 1)
                    .min(self.loudness.len());
                let part = &self.loudness[from..to];
                part.iter().map(|&v| v as f32).sum::<f32>() / part.len() as f32 / 255.
            })
            .collect()
    }
}

/// Measure a whole song.
pub fn measure(track: &Track) -> Result<Waveform> {
    measure_source(crate::audio_file::decode_track(track)?)
}

fn measure_source(source: impl Source<Item = f32>) -> Result<Waveform> {
    let channels = source.channels().max(1) as usize;
    let rate = source.sample_rate();
    if rate == 0 {
        bail!("No audio");
    }
    // 20 ms pieces first (the length may be unknown or a little off), then BINS parts.
    let piece = (rate as usize / 50).max(1) * channels;
    let mut pieces: Vec<(f32, f32)> = vec![];
    let (mut peak, mut sum, mut count) = (0f32, 0f32, 0usize);
    for sample in source {
        peak = peak.max(sample.abs());
        sum += sample * sample;
        count += 1;
        if count == piece {
            pieces.push((peak, (sum / count as f32).sqrt()));
            (peak, sum, count) = (0., 0., 0);
        }
    }
    if count > 0 {
        pieces.push((peak, (sum / count as f32).sqrt()));
    }
    Ok(shape(&pieces))
}

/// `BINS` parts from (peak, RMS) pieces, scaled to the loudest.
fn shape(pieces: &[(f32, f32)]) -> Waveform {
    if pieces.is_empty() {
        return Waveform {
            version: VERSION,
            ..Default::default()
        };
    }
    let mut peaks = vec![0f32; BINS];
    let mut loudness = vec![0f32; BINS];
    let mut counts = vec![0usize; BINS];
    for (i, (peak, rms)) in pieces.iter().enumerate() {
        let bin = i * BINS / pieces.len();
        peaks[bin] = peaks[bin].max(*peak);
        loudness[bin] += rms;
        counts[bin] += 1;
    }
    for (value, count) in loudness.iter_mut().zip(&counts) {
        if *count > 0 {
            *value /= *count as f32;
        }
    }
    let scale = |values: &[f32]| -> Vec<u8> {
        let top = values.iter().cloned().fold(0f32, f32::max);
        values
            .iter()
            .map(|v| {
                if top > 0. {
                    (v / top * 255.).round().clamp(0., 255.) as u8
                } else {
                    0
                }
            })
            .collect()
    };
    Waveform {
        version: VERSION,
        peaks: scale(&peaks),
        loudness: scale(&loudness),
    }
}

impl Library {
    fn waveforms_table(&self) -> Result<rusqlite::Connection> {
        let db = self.connection()?;
        db.execute(
            "CREATE TABLE IF NOT EXISTS waveforms(track_id TEXT PRIMARY KEY, version INTEGER NOT NULL, data TEXT NOT NULL)",
            [],
        )?;
        Ok(db)
    }

    /// A song's kept shape, if it was measured.
    pub fn waveform(&self, id: &str) -> Result<Option<Waveform>> {
        let data: Option<String> = self
            .waveforms_table()?
            .query_row(
                "SELECT data FROM waveforms WHERE track_id=? AND version=?",
                params![id, VERSION],
                |r| r.get(0),
            )
            .optional()?;
        Ok(data.and_then(|d| serde_json::from_str(&d).ok()))
    }

    pub fn save_waveform(&self, id: &str, waveform: &Waveform) -> Result<()> {
        self.waveforms_table()?.execute(
            "INSERT OR REPLACE INTO waveforms VALUES (?,?,?)",
            params![id, VERSION, serde_json::to_string(waveform)?],
        )?;
        Ok(())
    }

    /// A song's shape, measuring it now when it was not (a whole song decodes in about a
    /// second). A song that cannot be decoded keeps an empty shape, so it is not tried again.
    pub fn waveform_now(&self, track: &Track) -> Result<Waveform> {
        let empty = Waveform {
            version: VERSION,
            ..Default::default()
        };
        let kept = self.waveform(&track.id)?;
        // A server's song: measured from the whole song once it is on this device (kept, or
        // fully downloaded while it played); until then it has no shape yet, nothing is kept,
        // and it is asked again later. Nothing is fetched to measure it. An empty shape kept
        // for it before (measured from a stream still coming) is tried again.
        if track.source().is_some() {
            if let Some(found) = kept.filter(|w| !w.loudness.is_empty()) {
                return Ok(found);
            }
            return self.streamed_waveform(track, crate::sources::local_copy(track));
        }
        if let Some(found) = kept {
            return Ok(found);
        }
        let measured = measure(track).unwrap_or(empty);
        self.save_waveform(&track.id, &measured)?;
        Ok(measured)
    }
}

impl Library {
    /// A server's song's shape from `file`, its whole song on this device; none yet (and
    /// nothing kept) while there is no such file.
    fn streamed_waveform(
        &self,
        track: &Track,
        file: Option<std::path::PathBuf>,
    ) -> Result<Waveform> {
        let empty = Waveform {
            version: VERSION,
            ..Default::default()
        };
        let Some(file) = file else {
            return Ok(empty);
        };
        let measured = crate::audio_file::decode(&file)
            .and_then(measure_source)
            .unwrap_or(empty);
        self.save_waveform(&track.id, &measured)?;
        Ok(measured)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_server_song_is_measured_once_it_is_here() {
        let dir = tempfile::tempdir().unwrap();
        let library = Library::open(dir.path().join("data")).unwrap();
        let music = dir.path().join("music");
        crate::demo::create(&music).unwrap();
        let file = std::fs::read_dir(&music)
            .unwrap()
            .flatten()
            .map(|e| e.path())
            .find(|p| p.is_file())
            .unwrap();
        let track = Track {
            id: "server-song".into(),
            path: format!("{}subsonic/abc", crate::sources::SCHEME),
            ..Default::default()
        };
        // Still coming: no shape, and none kept, so it is asked again.
        assert!(
            library
                .streamed_waveform(&track, None)
                .unwrap()
                .loudness
                .is_empty()
        );
        assert!(library.waveform(&track.id).unwrap().is_none());
        // An empty shape kept by an older Needle is no reason to stop asking.
        library
            .save_waveform(
                &track.id,
                &Waveform {
                    version: VERSION,
                    ..Default::default()
                },
            )
            .unwrap();
        assert!(library.waveform_now(&track).unwrap().loudness.is_empty());
        // Here: measured, and kept.
        let measured = library.streamed_waveform(&track, Some(file)).unwrap();
        assert_eq!(measured.loudness.len(), BINS);
        assert_eq!(library.waveform_now(&track).unwrap(), measured);
    }

    #[test]
    fn shapes_scale_to_the_loudest_part() {
        let quiet_then_loud: Vec<(f32, f32)> = (0..BINS * 2)
            .map(|i| if i < BINS { (0.1, 0.05) } else { (0.8, 0.4) })
            .collect();
        let w = shape(&quiet_then_loud);
        assert_eq!(w.peaks.len(), BINS);
        assert_eq!(w.loudness[BINS - 1], 255);
        assert_eq!(w.loudness[0], 32);
        let h = w.heights(4);
        assert_eq!(h.len(), 4);
        assert!(h[0] < 0.2 && h[3] > 0.99);
        assert!(shape(&[]).loudness.is_empty());
    }

    #[test]
    fn a_tone_file_is_measured_and_kept() {
        let dir = tempfile::tempdir().unwrap();
        let library = Library::open(dir.path().join("data")).unwrap();
        let path = dir.path().join("tone.wav");
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 8000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::create(&path, spec).unwrap();
        // One quiet second, then one loud one.
        for i in 0..16_000 {
            let level = if i < 8000 { 2000. } else { 20_000. };
            writer
                .write_sample((level * (i as f32 * 0.3).sin()) as i16)
                .unwrap();
        }
        writer.finalize().unwrap();
        let track = Track {
            id: "t".into(),
            path: path.to_string_lossy().into(),
            duration: 2.,
            ..Default::default()
        };
        let w = library.waveform_now(&track).unwrap();
        assert!(
            w.loudness[10] < 40 && w.loudness[BINS - 10] > 200,
            "{:?}",
            &w.loudness[..12]
        );
        assert_eq!(library.waveform("t").unwrap(), Some(w));
    }
}
