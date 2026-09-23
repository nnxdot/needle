//! Audio only Windows can decode: Dolby Digital and Dolby Digital Plus (AC-3 and E-AC-3,
//! including Dolby Atmos music bought or downloaded from Apple Music), through Media
//! Foundation's own decoders.
//!
//! Decoding runs on its own thread, where COM stays in one apartment, a few blocks ahead of
//! playback. Sound comes out as stereo: Windows mixes it down when it can, and Needle does
//! otherwise. Atmos height sound is not decoded, only the 5.1 or 7.1 mix under it. Windows N
//! editions lack these decoders until the Media Feature Pack is installed.
use anyhow::{Context, Result, bail};
use crossbeam_channel::{Receiver, RecvTimeoutError, SendTimeoutError, Sender};
use rodio::Source;
use std::{path::Path, time::Duration};
use windows::{
    Win32::{
        Media::MediaFoundation::*,
        System::Com::{
            COINIT_MULTITHREADED, CoInitializeEx, CoUninitialize, StructuredStorage::PROPVARIANT,
        },
    },
    core::{GUID, HSTRING},
};

enum Message {
    Opened {
        rate: u32,
        duration: Option<Duration>,
    },
    Chunk {
        generation: u64,
        samples: Vec<f32>,
    },
    End {
        generation: u64,
    },
    Failed(String),
}

enum Control {
    Seek(Duration, u64),
    Stop,
}

/// A file decoded by Windows, as stereo samples.
pub struct MfSource {
    rate: u32,
    duration: Option<Duration>,
    messages: Receiver<Message>,
    control: Sender<Control>,
    chunk: Vec<f32>,
    at: usize,
    /// Bumped by each seek; blocks decoded before it are skipped.
    generation: u64,
    ended: bool,
}

/// Open `path` with Windows' decoders.
pub fn open(path: &Path) -> Result<MfSource> {
    let (messages_tx, messages) = crossbeam_channel::bounded(8);
    let (control, control_rx) = crossbeam_channel::unbounded();
    let path = path.to_path_buf();
    std::thread::Builder::new()
        .name("needle-mediafoundation".into())
        .spawn(move || {
            if let Err(error) = run(&path, &messages_tx, &control_rx) {
                let _ = messages_tx.send(Message::Failed(format!("{error:#}")));
            }
        })?;
    match messages.recv_timeout(Duration::from_secs(10)) {
        Ok(Message::Opened { rate, duration }) => Ok(MfSource {
            rate,
            duration,
            messages,
            control,
            chunk: vec![],
            at: 0,
            generation: 0,
            ended: false,
        }),
        Ok(Message::Failed(error)) => bail!("{error}"),
        _ => bail!("Windows did not open the file in time"),
    }
}

fn run(path: &Path, messages: &Sender<Message>, control: &Receiver<Control>) -> Result<()> {
    // SAFETY: COM and Media Foundation are started and stopped on this thread only, and every
    // interface made here is used and dropped on it.
    unsafe {
        CoInitializeEx(None, COINIT_MULTITHREADED)
            .ok()
            .context("Could not start COM")?;
        let started =
            MFStartup(MF_VERSION, MFSTARTUP_FULL).context("Could not start Media Foundation");
        let result = started.and_then(|()| decode(path, messages, control));
        let _ = MFShutdown();
        CoUninitialize();
        result
    }
}

/// Ask the reader for `subtype` audio, with `channels` channels if given.
unsafe fn ask(
    reader: &IMFSourceReader,
    stream: u32,
    subtype: &GUID,
    channels: Option<u32>,
) -> Result<()> {
    unsafe {
        let wanted = MFCreateMediaType()?;
        wanted.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio)?;
        wanted.SetGUID(&MF_MT_SUBTYPE, subtype)?;
        if let Some(channels) = channels {
            wanted.SetUINT32(&MF_MT_AUDIO_NUM_CHANNELS, channels)?;
        }
        reader.SetCurrentMediaType(stream, None, &wanted)?;
        Ok(())
    }
}

unsafe fn decode(
    path: &Path,
    messages: &Sender<Message>,
    control: &Receiver<Control>,
) -> Result<()> {
    unsafe {
        let url = HSTRING::from(path.as_os_str());
        let reader =
            MFCreateSourceReaderFromURL(&url, None).context("Windows could not open the file")?;
        let stream = MF_SOURCE_READER_FIRST_AUDIO_STREAM.0 as u32;
        reader.SetStreamSelection(MF_SOURCE_READER_ALL_STREAMS.0 as u32, false)?;
        reader.SetStreamSelection(stream, true)?;
        // Stereo floats if the decoder can mix down itself; otherwise what it gives.
        let asked = ask(&reader, stream, &MFAudioFormat_Float, Some(2))
            .or_else(|_| ask(&reader, stream, &MFAudioFormat_Float, None))
            .or_else(|_| ask(&reader, stream, &MFAudioFormat_PCM, Some(2)))
            .or_else(|_| ask(&reader, stream, &MFAudioFormat_PCM, None));
        if asked.is_err() {
            let native = reader
                .GetNativeMediaType(stream, 0)
                .and_then(|t| t.GetGUID(&MF_MT_SUBTYPE))
                .unwrap_or_default();
            if native == MFAudioFormat_Dolby_DDPlus || native == MFAudioFormat_Dolby_AC3 {
                bail!(
                    "This song is in Dolby Atmos or Dolby Digital. Windows on this PC can only pass that on to an AV receiver, not play it through speakers or headphones. The stereo version of the song plays normally."
                );
            }
            bail!("Windows has no decoder for this audio.");
        }
        let current = reader.GetCurrentMediaType(stream)?;
        let channels = current.GetUINT32(&MF_MT_AUDIO_NUM_CHANNELS)?.max(1) as usize;
        let rate = current.GetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND)?;
        let bits = current
            .GetUINT32(&MF_MT_AUDIO_BITS_PER_SAMPLE)
            .unwrap_or(32);
        let float = current.GetGUID(&MF_MT_SUBTYPE)? == MFAudioFormat_Float;
        let duration = reader
            .GetPresentationAttribute(MF_SOURCE_READER_MEDIASOURCE.0 as u32, &MF_PD_DURATION)
            .ok()
            .and_then(|value| u64::try_from(&value).ok())
            .map(|hundreds| Duration::from_nanos(hundreds.saturating_mul(100)));
        if messages.send(Message::Opened { rate, duration }).is_err() {
            return Ok(());
        }
        let mut generation = 0;
        // Handle a control message; `false` means stop.
        let handle = |message: Control, generation: &mut u64| -> Result<bool> {
            match message {
                Control::Stop => Ok(false),
                Control::Seek(position, new) => {
                    *generation = new;
                    let at =
                        PROPVARIANT::from((position.as_nanos() / 100).min(i64::MAX as u128) as i64);
                    reader.SetCurrentPosition(&GUID::zeroed(), &at)?;
                    Ok(true)
                }
            }
        };
        'reading: loop {
            while let Ok(message) = control.try_recv() {
                if !handle(message, &mut generation)? {
                    return Ok(());
                }
            }
            let mut flags = 0u32;
            let mut sample: Option<IMFSample> = None;
            reader.ReadSample(stream, 0, None, Some(&mut flags), None, Some(&mut sample))?;
            if flags & MF_SOURCE_READERF_ENDOFSTREAM.0 as u32 != 0 {
                if messages.send(Message::End { generation }).is_err() {
                    return Ok(());
                }
                // Wait for a seek back into the song, or the end.
                match control.recv() {
                    Ok(message) => {
                        if handle(message, &mut generation)? {
                            continue 'reading;
                        }
                        return Ok(());
                    }
                    Err(_) => return Ok(()),
                }
            }
            let Some(sample) = sample else { continue };
            let buffer = sample.ConvertToContiguousBuffer()?;
            let mut data = std::ptr::null_mut();
            let mut length = 0u32;
            buffer.Lock(&mut data, None, Some(&mut length))?;
            let bytes = std::slice::from_raw_parts(data, length as usize);
            let decoded = samples(bytes, float, bits);
            buffer.Unlock()?;
            let mut message = Message::Chunk {
                generation,
                samples: stereo(&decoded, channels),
            };
            // Wait for room, still listening for a seek or the end.
            loop {
                match messages.send_timeout(message, Duration::from_millis(100)) {
                    Ok(()) => break,
                    Err(SendTimeoutError::Disconnected(_)) => return Ok(()),
                    Err(SendTimeoutError::Timeout(back)) => {
                        message = back;
                        match control.try_recv() {
                            Ok(control_message) => {
                                if !handle(control_message, &mut generation)? {
                                    return Ok(());
                                }
                                // This block is from before the seek.
                                continue 'reading;
                            }
                            Err(_) => continue,
                        }
                    }
                }
            }
        }
    }
}

/// Decoded bytes as samples from −1 to 1.
fn samples(bytes: &[u8], float: bool, bits: u32) -> Vec<f32> {
    match (float, bits) {
        (true, _) => bytes
            .as_chunks::<4>()
            .0
            .iter()
            .map(|b| f32::from_le_bytes(*b))
            .collect(),
        (false, 16) => bytes
            .as_chunks::<2>()
            .0
            .iter()
            .map(|b| i16::from_le_bytes(*b) as f32 / 32768.)
            .collect(),
        (false, 24) => bytes
            .as_chunks::<3>()
            .0
            .iter()
            .map(|b| i32::from_le_bytes([0, b[0], b[1], b[2]]) as f32 / 2_147_483_648.)
            .collect(),
        (false, _) => bytes
            .as_chunks::<4>()
            .0
            .iter()
            .map(|b| i32::from_le_bytes(*b) as f32 / 2_147_483_648.)
            .collect(),
    }
}

/// Mix interleaved audio down to stereo. Channels come in Windows' order: front left and
/// right, centre, low frequency, then left–right pairs (back, then side). The low-frequency
/// channel is left out, as players do when mixing down.
pub(crate) fn stereo(samples: &[f32], channels: usize) -> Vec<f32> {
    match channels {
        2 => samples.to_vec(),
        1 => samples.iter().flat_map(|s| [*s, *s]).collect(),
        _ => {
            let pairs = channels.saturating_sub(4) / 2;
            let centre = if channels >= 3 { 0.707 } else { 0. };
            let scale = 1. / (1. + centre + 0.707 * pairs as f32);
            samples
                .chunks_exact(channels)
                .flat_map(|frame| {
                    let mut left = frame[0];
                    let mut right = frame[1];
                    if channels >= 3 {
                        left += centre * frame[2];
                        right += centre * frame[2];
                    }
                    for pair in 0..pairs {
                        left += 0.707 * frame[4 + pair * 2];
                        right += 0.707 * frame[5 + pair * 2];
                    }
                    [left * scale, right * scale]
                })
                .collect()
        }
    }
}

impl Iterator for MfSource {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        loop {
            if self.at < self.chunk.len() {
                self.at += 1;
                return Some(self.chunk[self.at - 1]);
            }
            if self.ended {
                return None;
            }
            match self.messages.recv_timeout(Duration::from_secs(10)) {
                Ok(Message::Chunk {
                    generation,
                    samples,
                }) if generation == self.generation => {
                    self.chunk = samples;
                    self.at = 0;
                }
                Ok(Message::End { generation }) if generation == self.generation => {
                    self.ended = true;
                    return None;
                }
                Ok(Message::Failed(error)) => {
                    crate::logfile::error(format!("Windows decoding stopped: {error}"));
                    self.ended = true;
                    return None;
                }
                // Blocks from before a seek.
                Ok(_) => {}
                Err(RecvTimeoutError::Timeout | RecvTimeoutError::Disconnected) => {
                    self.ended = true;
                    return None;
                }
            }
        }
    }
}

impl Source for MfSource {
    fn current_span_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> u16 {
        2
    }
    fn sample_rate(&self) -> u32 {
        self.rate
    }
    fn total_duration(&self) -> Option<Duration> {
        self.duration
    }
    fn try_seek(&mut self, position: Duration) -> Result<(), rodio::source::SeekError> {
        self.generation += 1;
        self.chunk.clear();
        self.at = 0;
        self.ended = false;
        let _ = self.control.send(Control::Seek(position, self.generation));
        Ok(())
    }
}

impl Drop for MfSource {
    fn drop(&mut self) {
        let _ = self.control.send(Control::Stop);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("testdata/eac3-5.1.m4a")
    }

    /// Needs a Windows whose Dolby decoder makes sound (Dolby Access, or some PC makers'
    /// drivers); the stock decoder only passes Dolby on to an AV receiver.
    #[test]
    #[ignore]
    fn dolby_digital_plus_plays_as_stereo_and_seeks() {
        let source = open(&fixture()).unwrap();
        assert_eq!((source.channels(), source.sample_rate()), (2, 48_000));
        let length = source.total_duration().unwrap().as_secs_f64();
        assert!((length - 1.0).abs() < 0.1, "{length}");
        let samples: Vec<f32> = source.collect();
        let frames = samples.len() / 2;
        assert!((44_000..52_000).contains(&frames), "{frames} frames");
        let loud = samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32;
        assert!(
            loud.sqrt() > 0.05,
            "the tone should be heard, rms {}",
            loud.sqrt()
        );
        assert!(samples.iter().all(|s| s.is_finite() && s.abs() <= 1.01));
        // Half way in, about half the song is left.
        let mut source = open(&fixture()).unwrap();
        let _: Vec<f32> = source.by_ref().take(2000).collect();
        source.try_seek(Duration::from_millis(500)).unwrap();
        let rest = source.count() / 2;
        assert!(
            (20_000..30_000).contains(&rest),
            "{rest} frames after the seek"
        );
    }

    #[test]
    fn dolby_files_play_or_say_why_not() {
        match crate::audio_file::decode(&fixture()) {
            Ok(decoded) => {
                assert_eq!(decoded.channels(), 2);
                assert!(decoded.take(48_000).any(|s| s.abs() > 0.05));
            }
            Err(error) => {
                let text = format!("{error:#}");
                assert!(text.contains("Dolby"), "{text}");
            }
        }
    }

    #[test]
    fn surround_mixes_down_without_clipping() {
        // 5.1: front left and right, centre, low frequency, back left and right.
        let frame = [1.0, 0.0, 1.0, 1.0, 1.0, 0.0];
        let mixed = stereo(&frame, 6);
        assert!(mixed[0] <= 1.0 && mixed[0] > 0.9, "{mixed:?}");
        assert!(mixed[1] > 0.2 && mixed[1] < 0.4, "{mixed:?}");
        assert_eq!(stereo(&[0.5], 1), vec![0.5, 0.5]);
    }

    /// `NEEDLE_DOLBY_FILE=<path> cargo test -p needle-core dolby_file -- --ignored`
    #[test]
    #[ignore]
    fn a_real_dolby_file() {
        let path = std::env::var("NEEDLE_DOLBY_FILE").unwrap();
        let decoded = crate::audio_file::decode(Path::new(&path)).unwrap();
        let rate = decoded.sample_rate();
        let samples: Vec<f32> = decoded.take(rate as usize * 2 * 5).collect();
        let rms = (samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32).sqrt();
        println!("{} samples at {rate} Hz, rms {rms}", samples.len());
        assert!(rms > 0.01);
    }
}
