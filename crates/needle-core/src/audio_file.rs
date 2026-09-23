//! Bound AIFF decoding to its sound-data chunk. Some demuxers otherwise read
//! trailing ID3 chunks as PCM; metadata must never become audible samples.
use anyhow::Result;
use rodio::{Decoder, Source};
use std::{
    fs::File,
    io::{self, Read, Seek, SeekFrom},
    path::Path,
};

pub struct AudioReader {
    file: File,
    length: u64,
}
impl AudioReader {
    fn open(path: &Path) -> Result<Self> {
        let mut file = File::open(path)?;
        let mut length = file.metadata()?.len();
        let mut header = [0; 12];
        if file.read_exact(&mut header).is_ok()
            && &header[..4] == b"FORM"
            && [&b"AIFF"[..], &b"AIFC"[..]].contains(&&header[8..12])
        {
            let physical = length;
            let mut offset = 12u64;
            while offset + 8 <= physical {
                file.seek(SeekFrom::Start(offset))?;
                let mut chunk = [0; 8];
                file.read_exact(&mut chunk)?;
                let size = u32::from_be_bytes(chunk[4..].try_into().unwrap()) as u64;
                let end = offset + 8 + size;
                anyhow::ensure!(end <= physical, "AIFF chunk exceeds file size");
                if &chunk[..4] == b"SSND" {
                    length = end;
                    break;
                }
                offset = end + (size % 2);
            }
        }
        file.seek(SeekFrom::Start(0))?;
        Ok(Self { file, length })
    }
}
impl Read for AudioReader {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let remaining = self.length.saturating_sub(self.file.stream_position()?);
        let size = (remaining.min(buffer.len() as u64)) as usize;
        self.file.read(&mut buffer[..size])
    }
}
impl Seek for AudioReader {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        match position {
            SeekFrom::End(offset) => {
                let target = self.length.checked_add_signed(offset).ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidInput, "Seek before beginning")
                })?;
                self.file.seek(SeekFrom::Start(target))
            }
            other => self.file.seek(other),
        }
    }
}
enum Inner {
    Symphonia(Decoder<AudioReader>),
    Opus(Box<crate::opus::OpusSource>),
    /// WavPack, Monkey's Audio, and DSD (see `formats`).
    Other(Box<dyn Source + Send>),
}
pub struct Decoded {
    inner: Inner,
    trim: Option<(u64, u64)>,
    position: u64,
}
impl Decoded {
    fn source(&self) -> &dyn Source<Item = f32> {
        match &self.inner {
            Inner::Symphonia(d) => d,
            Inner::Opus(d) => d.as_ref(),
            Inner::Other(d) => d.as_ref(),
        }
    }
}
impl Iterator for Decoded {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        if self
            .trim
            .is_some_and(|(_, frames)| self.position >= frames * self.channels() as u64)
        {
            return None;
        }
        let sample = match &mut self.inner {
            Inner::Symphonia(d) => d.next(),
            Inner::Opus(d) => d.next(),
            Inner::Other(d) => d.next(),
        }?;
        self.position += 1;
        Some(sample)
    }
}
impl Source for Decoded {
    fn channels(&self) -> u16 {
        self.source().channels()
    }
    fn sample_rate(&self) -> u32 {
        self.source().sample_rate()
    }
    fn current_span_len(&self) -> Option<usize> {
        let inner = self.source().current_span_len();
        if let Some((_, frames)) = self.trim {
            let remaining = (frames * self.channels() as u64)
                .saturating_sub(self.position)
                .min(usize::MAX as u64) as usize;
            Some(inner.unwrap_or(remaining).min(remaining))
        } else {
            inner
        }
    }
    fn total_duration(&self) -> Option<std::time::Duration> {
        self.trim
            .map(|(_, frames)| {
                std::time::Duration::from_secs_f64(frames as f64 / self.sample_rate() as f64)
            })
            .or_else(|| self.source().total_duration())
    }
    fn try_seek(
        &mut self,
        position: std::time::Duration,
    ) -> std::result::Result<(), rodio::source::SeekError> {
        let first = self.trim.map(|t| t.0).unwrap_or(0);
        let offset = std::time::Duration::from_secs_f64(first as f64 / self.sample_rate() as f64);
        match &mut self.inner {
            Inner::Symphonia(d) => d.try_seek(position + offset)?,
            Inner::Opus(d) => d.try_seek(position + offset)?,
            Inner::Other(d) => d.try_seek(position + offset)?,
        }
        self.position = (position.as_secs_f64() * self.sample_rate() as f64).round() as u64
            * self.channels() as u64;
        Ok(())
    }
}
pub fn decode(path: &Path) -> Result<Decoded> {
    let extension = path
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let other: Option<Box<dyn Source + Send>> = match extension.as_str() {
        "wv" => Some(Box::new(crate::formats::WavPack::open(path)?)),
        "ape" => Some(Box::new(crate::formats::Ape::open(path)?)),
        "dsf" => Some(Box::new(crate::formats::Dsf::open(path)?)),
        _ => None,
    };
    if let Some(inner) = other {
        return Ok(Decoded {
            inner: Inner::Other(inner),
            trim: None,
            position: 0,
        });
    }
    if crate::opus::is_ogg_opus(path) {
        return Ok(Decoded {
            inner: Inner::Opus(Box::new(crate::opus::OpusSource::open(path)?)),
            trim: None,
            position: 0,
        });
    }
    let input = AudioReader::open(path)?;
    let length = input.length;
    let built = Decoder::builder()
        .with_data(input)
        .with_byte_len(length)
        .with_seekable(true)
        .with_gapless(true)
        .build();
    let mut inner = match built {
        Ok(inner) => inner,
        Err(error) => {
            // Dolby Digital (Plus) and Atmos music: Needle's own FFmpeg decodes it, and where
            // that is missing, Windows may.
            if crate::ffmpeg::executable().is_some() {
                match crate::ffmpeg::open(path) {
                    Ok(source) => {
                        return Ok(Decoded {
                            inner: Inner::Other(Box::new(source)),
                            trim: None,
                            position: 0,
                        });
                    }
                    Err(ffmpeg) => crate::logfile::warn(format!(
                        "Needle's FFmpeg could not decode {}: {ffmpeg:#}",
                        path.display()
                    )),
                }
            }
            #[cfg(windows)]
            match crate::mediafoundation::open(path) {
                Ok(source) => {
                    return Ok(Decoded {
                        inner: Inner::Other(Box::new(source)),
                        trim: None,
                        position: 0,
                    });
                }
                Err(windows) => {
                    crate::logfile::warn(format!(
                        "Windows could not decode {} either: {windows:#}",
                        path.display()
                    ));
                    // Windows knew what it was, which says more than "unrecognized format".
                    let reason = format!("{windows:#}");
                    if reason.contains("Dolby") {
                        return Err(windows);
                    }
                }
            }
            return Err(anyhow::Error::new(error).context("Unable to decode audio"));
        }
    };
    let trim = crate::mp4_trim::aac_trim(path, inner.sample_rate());
    if let Some((first, _)) = trim {
        for _ in 0..first * inner.channels() as u64 {
            let _ = inner.next();
        }
    }
    Ok(Decoded {
        inner: Inner::Symphonia(inner),
        trim,
        position: 0,
    })
}

/// Decode a library track: its file, or its stretch of the album file for a CUE track.
pub fn decode_track(track: &crate::model::Track) -> Result<Box<dyn Source + Send>> {
    if track.is_streamed() {
        return crate::sources::open(track);
    }
    let decoded = decode(Path::new(track.audio_path()))?;
    let Some(cue) = &track.cue else {
        return Ok(Box::new(decoded));
    };
    Ok(Box::new(Span::new(decoded, cue.start, cue.end)))
}

/// Part of a source, from `start` to `end` seconds, with positions counted from `start`.
struct Span<S> {
    inner: S,
    start: f64,
    remaining: Option<u64>,
    length: Option<u64>,
}
impl<S: Source> Span<S> {
    fn new(mut inner: S, start: f64, end: Option<f64>) -> Self {
        if start > 0.
            && inner
                .try_seek(std::time::Duration::from_secs_f64(start))
                .is_err()
        {
            // Not seekable: skip ahead sample by sample.
            let skip = (start * inner.sample_rate() as f64) as u64 * inner.channels() as u64;
            for _ in 0..skip {
                if inner.next().is_none() {
                    break;
                }
            }
        }
        let length = end.map(|end| {
            ((end - start).max(0.) * inner.sample_rate() as f64) as u64 * inner.channels() as u64
        });
        Self {
            inner,
            start,
            remaining: length,
            length,
        }
    }
}
impl<S: Source> Iterator for Span<S> {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        if let Some(remaining) = self.remaining.as_mut() {
            if *remaining == 0 {
                return None;
            }
            *remaining -= 1;
        }
        self.inner.next()
    }
}
impl<S: Source> Source for Span<S> {
    fn current_span_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> u16 {
        self.inner.channels()
    }
    fn sample_rate(&self) -> u32 {
        self.inner.sample_rate()
    }
    fn total_duration(&self) -> Option<std::time::Duration> {
        self.length.map(|l| {
            std::time::Duration::from_secs_f64(
                l as f64 / self.channels() as f64 / self.sample_rate() as f64,
            )
        })
    }
    fn try_seek(
        &mut self,
        position: std::time::Duration,
    ) -> std::result::Result<(), rodio::source::SeekError> {
        self.inner
            .try_seek(position + std::time::Duration::from_secs_f64(self.start))?;
        if let Some(length) = self.length {
            let done = (position.as_secs_f64() * self.sample_rate() as f64) as u64
                * self.channels() as u64;
            self.remaining = Some(length.saturating_sub(done));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn aiff_metadata_after_sound_is_never_decoded_as_audio() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("trailing.aiff");
        let mut bytes=b"FORM\0\0\0\0AIFFCOMM\0\0\0\x12\0\x01\0\0\0\x04\0\x10\x40\x0e\xac\x44\0\0\0\0\0\0SSND\0\0\0\x10\0\0\0\0\0\0\0\0".to_vec();
        for value in [0i16, 8192, -8192, 16384] {
            bytes.extend(value.to_be_bytes());
        }
        bytes.extend(b"ANNO\0\0\0\x08metadata");
        let size = (bytes.len() - 8) as u32;
        bytes[4..8].copy_from_slice(&size.to_be_bytes());
        std::fs::write(&path, bytes).unwrap();
        let samples: Vec<_> = decode(&path).unwrap().collect();
        assert_eq!(samples, vec![0., 0.25, -0.25, 0.5]);
    }
}
