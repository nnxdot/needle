//! Bound AIFF decoding to its sound-data chunk. Some demuxers otherwise read
//! trailing ID3 chunks as PCM; metadata must never become audible samples.
use anyhow::{Context, Result};
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
pub struct Decoded {
    inner: Decoder<AudioReader>,
    trim: Option<(u64, u64)>,
    position: u64,
}
impl Iterator for Decoded {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        if self
            .trim
            .is_some_and(|(_, frames)| self.position >= frames * self.inner.channels() as u64)
        {
            return None;
        }
        let sample = self.inner.next()?;
        self.position += 1;
        Some(sample)
    }
}
impl Source for Decoded {
    fn channels(&self) -> u16 {
        self.inner.channels()
    }
    fn sample_rate(&self) -> u32 {
        self.inner.sample_rate()
    }
    fn current_span_len(&self) -> Option<usize> {
        let inner = self.inner.current_span_len();
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
            .or_else(|| self.inner.total_duration())
    }
    fn try_seek(
        &mut self,
        position: std::time::Duration,
    ) -> std::result::Result<(), rodio::source::SeekError> {
        let first = self.trim.map(|t| t.0).unwrap_or(0);
        let offset = std::time::Duration::from_secs_f64(first as f64 / self.sample_rate() as f64);
        self.inner.try_seek(position + offset)?;
        self.position = (position.as_secs_f64() * self.sample_rate() as f64).round() as u64
            * self.channels() as u64;
        Ok(())
    }
}
pub fn decode(path: &Path) -> Result<Decoded> {
    let input = AudioReader::open(path)?;
    let length = input.length;
    let mut inner = Decoder::builder()
        .with_data(input)
        .with_byte_len(length)
        .with_seekable(true)
        .with_gapless(true)
        .build()
        .context("Unable to decode audio")?;
    let trim = crate::mp4_trim::aac_trim(path, inner.sample_rate());
    if let Some((first, _)) = trim {
        for _ in 0..first * inner.channels() as u64 {
            let _ = inner.next();
        }
    }
    Ok(Decoded {
        inner,
        trim,
        position: 0,
    })
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
