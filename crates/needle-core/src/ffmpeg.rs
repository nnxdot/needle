//! Dolby Digital and Dolby Digital Plus, including the 5.1 or 7.1 mix of Dolby Atmos music
//! (from Apple Music, for example), through a small FFmpeg shipped with Needle.
//!
//! `needle-ffmpeg.exe` is FFmpeg built with only what these files need: MP4 reading, the
//! AC-3 and E-AC-3 decoders, and mixing down to stereo (see `scripts/build-ffmpeg.sh` and
//! `third-party/ffmpeg`). Needle runs it as its own program, which sends 48 kHz stereo
//! samples down a pipe; a thread reads them a few blocks ahead. Seeking starts it again at
//! the new place. Atmos height sound is not decoded.
use anyhow::{Context, Result, bail};
use crossbeam_channel::{Receiver, RecvTimeoutError};
use rodio::Source;
use std::{
    io::Read,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    time::Duration,
};

pub const RATE: u32 = 48_000;
const CHUNK: usize = 4096;

/// Where Needle's FFmpeg is: next to Needle, or (in development builds) in the source tree.
/// `NEEDLE_FFMPEG` points elsewhere.
pub fn executable() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("NEEDLE_FFMPEG").map(PathBuf::from) {
        return path.is_file().then_some(path);
    }
    let name = format!("needle-ffmpeg{}", std::env::consts::EXE_SUFFIX);
    let beside = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join(&name)))
        .filter(|path| path.is_file());
    if beside.is_some() {
        return beside;
    }
    // Linux: the system's FFmpeg, which every distribution packages.
    #[cfg(not(windows))]
    if let Some(system) = std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths)
            .map(|dir| dir.join("ffmpeg"))
            .find(|path| path.is_file())
    }) {
        return Some(system);
    }
    let source_tree =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../third-party/ffmpeg/needle-ffmpeg.exe");
    (cfg!(debug_assertions) && source_tree.is_file()).then_some(source_tree)
}

enum Message {
    Chunk { generation: u64, samples: Vec<f32> },
    End { generation: u64 },
}

/// A file decoded by Needle's FFmpeg, as 48 kHz stereo.
pub struct FfmpegSource {
    exe: PathBuf,
    path: PathBuf,
    duration: Option<Duration>,
    child: Option<Child>,
    messages: Option<Receiver<Message>>,
    /// The last of what the decoder said, for the log when it fails.
    said: std::sync::Arc<std::sync::Mutex<String>>,
    generation: u64,
    chunk: Vec<f32>,
    at: usize,
    ended: bool,
}

pub fn open(path: &Path) -> Result<FfmpegSource> {
    let exe = executable().context("Needle's Dolby decoder (needle-ffmpeg.exe) is missing")?;
    let duration = {
        use lofty::file::AudioFile;
        lofty::probe::Probe::open(path)
            .ok()
            .and_then(|p| p.read().ok())
            .map(|f| f.properties().duration())
            .filter(|d| !d.is_zero())
    };
    let mut source = FfmpegSource {
        exe,
        path: path.to_path_buf(),
        duration,
        child: None,
        messages: None,
        said: Default::default(),
        generation: 0,
        chunk: vec![],
        at: 0,
        ended: false,
    };
    source.start(Duration::ZERO)?;
    // Wait for the first samples, so a file FFmpeg cannot read fails here, not while playing.
    let first = source
        .messages
        .as_ref()
        .unwrap()
        .recv_timeout(Duration::from_secs(10));
    match first {
        Ok(Message::Chunk { samples, .. }) => source.chunk = samples,
        Ok(Message::End { .. }) | Err(_) => {
            let reason = source.stop().unwrap_or_default();
            bail!(
                "Needle's Dolby decoder could not read this file{}",
                if reason.is_empty() {
                    String::new()
                } else {
                    format!(": {reason}")
                }
            );
        }
    }
    Ok(source)
}

impl FfmpegSource {
    fn start(&mut self, at: Duration) -> Result<()> {
        let mut command = Command::new(&self.exe);
        command
            .args(["-nostdin", "-hide_banner", "-loglevel", "error"])
            .arg("-ss")
            .arg(format!("{:.3}", at.as_secs_f64()))
            .arg("-i")
            .arg(&self.path)
            .args(["-map", "0:a:0", "-f", "f32le", "-ac", "2", "-ar"])
            .arg(RATE.to_string())
            .arg("pipe:1")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            // No console window flashing up.
            command.creation_flags(0x0800_0000);
        }
        let mut child = command
            .spawn()
            .context("Could not start Needle's Dolby decoder")?;
        let mut stdout = child.stdout.take().context("No output from the decoder")?;
        // Read its messages as they come: a damaged file can make it say something for every
        // frame, and a full pipe would stop it decoding.
        if let Some(mut stderr) = child.stderr.take() {
            let said = self.said.clone();
            said.lock().unwrap().clear();
            std::thread::Builder::new()
                .name("needle-ffmpeg-messages".into())
                .spawn(move || {
                    let mut buffer = [0u8; 4096];
                    while let Ok(n) = stderr.read(&mut buffer) {
                        if n == 0 {
                            break;
                        }
                        let mut text = said.lock().unwrap();
                        text.push_str(&String::from_utf8_lossy(&buffer[..n]));
                        // Keep the last few lines only.
                        if text.len() > 4096 {
                            let mut cut = text.len() - 2048;
                            while !text.is_char_boundary(cut) {
                                cut += 1;
                            }
                            text.drain(..cut);
                        }
                    }
                })?;
        }
        let (tx, rx) = crossbeam_channel::bounded(16);
        let generation = self.generation;
        std::thread::Builder::new()
            .name("needle-ffmpeg".into())
            .spawn(move || {
                let mut bytes = vec![0u8; CHUNK * 4];
                let mut filled = 0;
                loop {
                    match stdout.read(&mut bytes[filled..]) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => filled += n,
                    }
                    if filled == bytes.len() {
                        let samples = bytes
                            .as_chunks::<4>()
                            .0
                            .iter()
                            .map(|b| f32::from_le_bytes(*b))
                            .collect();
                        if tx
                            .send(Message::Chunk {
                                generation,
                                samples,
                            })
                            .is_err()
                        {
                            return;
                        }
                        filled = 0;
                    }
                }
                let whole = filled / 8 * 8;
                if whole > 0 {
                    let samples = bytes[..whole]
                        .as_chunks::<4>()
                        .0
                        .iter()
                        .map(|b| f32::from_le_bytes(*b))
                        .collect();
                    let _ = tx.send(Message::Chunk {
                        generation,
                        samples,
                    });
                }
                let _ = tx.send(Message::End { generation });
            })?;
        self.child = Some(child);
        self.messages = Some(rx);
        self.chunk.clear();
        self.at = 0;
        self.ended = false;
        Ok(())
    }

    /// End the decoder; returns what it said went wrong, if anything.
    fn stop(&mut self) -> Option<String> {
        self.messages = None;
        let mut child = self.child.take()?;
        let _ = child.kill();
        let _ = child.wait();
        // Give the message reader a moment to take the last words.
        std::thread::sleep(Duration::from_millis(20));
        let reason = self.said.lock().unwrap().trim().to_string();
        Some(reason)
    }
}

impl Iterator for FfmpegSource {
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
            let message = self
                .messages
                .as_ref()?
                .recv_timeout(Duration::from_secs(10));
            match message {
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
                Ok(_) => {}
                Err(RecvTimeoutError::Timeout | RecvTimeoutError::Disconnected) => {
                    self.ended = true;
                    return None;
                }
            }
        }
    }
}

impl Source for FfmpegSource {
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
        self.duration
    }
    fn try_seek(&mut self, position: Duration) -> Result<(), rodio::source::SeekError> {
        self.stop();
        self.generation += 1;
        self.start(position).map_err(|e| {
            crate::logfile::error(format!("Dolby decoder seek failed: {e:#}"));
            rodio::source::SeekError::NotSupported {
                underlying_source: "needle-ffmpeg",
            }
        })
    }
}

impl Drop for FfmpegSource {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("testdata/eac3-5.1.m4a")
    }

    #[test]
    fn dolby_digital_plus_plays_as_stereo_and_seeks() {
        if executable().is_none() {
            panic!("third-party/ffmpeg/needle-ffmpeg.exe is missing; run scripts/build-ffmpeg.sh");
        }
        let source = open(&fixture()).unwrap();
        assert_eq!((source.channels(), source.sample_rate()), (2, 48_000));
        let length = source.total_duration().unwrap().as_secs_f64();
        assert!((length - 1.0).abs() < 0.1, "{length}");
        let samples: Vec<f32> = source.collect();
        let frames = samples.len() / 2;
        assert!((44_000..52_000).contains(&frames), "{frames} frames");
        let rms = (samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32).sqrt();
        assert!(rms > 0.05, "the tone should be heard, rms {rms}");
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
    fn the_player_plays_dolby_files() {
        let decoded = crate::audio_file::decode(&fixture()).unwrap();
        assert_eq!(decoded.channels(), 2);
        assert!(decoded.take(48_000).any(|s| s.abs() > 0.05));
    }

    #[test]
    fn files_it_cannot_read_fail_at_once() {
        let dir = tempfile::tempdir().unwrap();
        let broken = dir.path().join("broken.m4a");
        std::fs::write(&broken, b"not an mp4 file at all").unwrap();
        let error = open(&broken).err().expect("a broken file must not open");
        assert!(format!("{error:#}").contains("could not read"), "{error:#}");
    }
}
