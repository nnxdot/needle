//! The core's platform output (what Needle's USB DAC driver uses) end to end: the real player
//! and library, with a fake output that takes the music in real time.
use crate::Needle;
use anyhow::Result;
use needle_core::platform_output::{Feed, FeedFormat, PlatformOutput, set_platform_output};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

/// Whether the system's output can open in this test program: not in a bare program on Android
/// (no Java context; the app has one), so there only what comes before a fallback is checked.
const FALLBACK: bool = cfg!(not(target_os = "android"));

/// An output that takes the music as a DAC would, ten milliseconds at a time, and keeps a note
/// of what it got.
#[derive(Default)]
struct Fake {
    frames: AtomicU64,
    formats: Mutex<Vec<FeedFormat>>,
    /// The loudest sample seen: music, not silence, reached it.
    loudest: Mutex<f32>,
    rendering: AtomicBool,
}

impl PlatformOutput for Fake {
    fn name(&self) -> String {
        "Fake DAC".into()
    }

    fn render(&self, feed: &mut Feed) -> Result<()> {
        self.rendering.store(true, Ordering::SeqCst);
        let mut buf = vec![0f32; 48_000];
        // Keeps time as a DAC's clock does: each read waits until the music before it has played.
        let mut clock: Option<(Instant, f64)> = None;
        while !feed.stopped() {
            if !feed.playing() {
                clock = None;
                feed.idle();
                continue;
            }
            let f = feed.format();
            {
                let mut formats = self.formats.lock().unwrap();
                if formats.last() != Some(&f) {
                    formats.push(f);
                    feed.playing_at(f);
                }
            }
            let want = (f.rate as usize / 100).max(1) * f.channels as usize;
            let n = feed.read(&mut buf[..want]);
            let loud = buf[..n].iter().fold(0f32, |m, s| m.max(s.abs()));
            {
                let mut l = self.loudest.lock().unwrap();
                *l = l.max(loud);
            }
            let frames = (n / f.channels.max(1) as usize) as u64;
            self.frames.fetch_add(frames, Ordering::SeqCst);
            let (start, played) = clock.get_or_insert((Instant::now(), 0.));
            *played += frames as f64 / f.rate.max(1) as f64;
            let due = *start + Duration::from_secs_f64(*played);
            if let Some(wait) = due.checked_duration_since(Instant::now()) {
                std::thread::sleep(wait);
            }
        }
        self.rendering.store(false, Ordering::SeqCst);
        Ok(())
    }
}

/// A short stereo 16-bit WAV of a tone at `rate`.
fn wav(path: &std::path::Path, rate: u32, seconds: u32) {
    let frames = rate * seconds;
    let mut data = Vec::with_capacity(frames as usize * 4);
    for i in 0..frames {
        let v = ((i as f32 / rate as f32 * 440. * std::f32::consts::TAU).sin() * 12_000.) as i16;
        data.extend(v.to_le_bytes());
        data.extend(v.to_le_bytes());
    }
    let mut out = vec![];
    out.extend(b"RIFF");
    out.extend((36 + data.len() as u32).to_le_bytes());
    out.extend(b"WAVEfmt ");
    out.extend(16u32.to_le_bytes());
    out.extend(1u16.to_le_bytes());
    out.extend(2u16.to_le_bytes());
    out.extend(rate.to_le_bytes());
    out.extend((rate * 4).to_le_bytes());
    out.extend(4u16.to_le_bytes());
    out.extend(16u16.to_le_bytes());
    out.extend(b"data");
    out.extend((data.len() as u32).to_le_bytes());
    out.extend(data);
    std::fs::write(path, out).unwrap();
}

/// The platform output is one global: the tests that set it take turns.
static ONE_AT_A_TIME: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn wait(what: &str, limit: Duration, mut done: impl FnMut() -> bool) {
    let start = Instant::now();
    while !done() {
        assert!(start.elapsed() < limit, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
fn a_platform_output_plays_pauses_seeks_follows_the_rate_and_hands_back() {
    let _turn = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let dir = tempfile::tempdir().unwrap();
    let music = dir.path().join("music");
    needle_core::demo::create(&music).unwrap();
    wav(&music.join("04 - Faster.wav"), 48_000, 6);
    let needle = Needle::new(dir.path().join("data").to_string_lossy().into()).unwrap();
    let fake = Arc::new(Fake::default());
    // Set before anything plays, so the system's output is never opened (no sound here).
    set_platform_output(Some(fake.clone()));
    needle.add_folder(music.to_string_lossy().into()).unwrap();
    wait("the scan", Duration::from_secs(60), || {
        !needle.scan_status().running && needle.song_count().unwrap_or(0) >= 4
    });
    let mut songs = needle.songs().unwrap();
    songs.sort_by(|a, b| a.title.cmp(&b.title));
    let first = songs
        .iter()
        .position(|s| s.title.contains("room"))
        .expect("a demo song");
    let faster = songs
        .iter()
        .position(|s| s.title.contains("Faster"))
        .expect("the 48 kHz song");
    let ids: Vec<String> = songs.iter().map(|s| s.id.clone()).collect();

    needle.play(ids.clone(), first as u32).unwrap();
    wait("music at the fake output", Duration::from_secs(10), || {
        fake.frames.load(Ordering::SeqCst) > 20_000
    });
    assert!(needle.playback().playing, "the player says it plays");
    assert_eq!(
        fake.formats
            .lock()
            .unwrap()
            .first()
            .map(|f| (f.rate, f.channels)),
        Some((44_100, 2))
    );
    assert!(*fake.loudest.lock().unwrap() > 0.01, "music, not silence");
    // Real time: about a second of music in about a second.
    let before = fake.frames.load(Ordering::SeqCst);
    std::thread::sleep(Duration::from_secs(1));
    let second = fake.frames.load(Ordering::SeqCst) - before;
    assert!(
        (30_000..60_000).contains(&second),
        "{second} frames in a second"
    );

    // Paused: nothing more is taken.
    needle.toggle();
    std::thread::sleep(Duration::from_millis(400));
    let paused = fake.frames.load(Ordering::SeqCst);
    std::thread::sleep(Duration::from_millis(800));
    assert!(
        fake.frames.load(Ordering::SeqCst) - paused < 2_000,
        "the output was idle while paused"
    );
    assert!(!needle.playback().playing);

    // A seek while paused, then on again from there.
    needle.seek(15.);
    std::thread::sleep(Duration::from_millis(300));
    needle.toggle();
    wait("playing again", Duration::from_secs(5), || {
        needle.playback().playing
    });
    std::thread::sleep(Duration::from_millis(600));
    let at = needle.playback().position;
    assert!(
        (14.5..18.).contains(&at),
        "after the seek the song is at {at}"
    );

    // Another song at another rate: the output is told the new format.
    needle.play(ids, faster as u32).unwrap();
    wait("the 48 kHz song", Duration::from_secs(10), || {
        fake.formats
            .lock()
            .unwrap()
            .iter()
            .any(|f| f.rate == 48_000)
    });

    // Needle's USB driver on a pretend DAC, with the same player.
    let ids: Vec<String> = needle
        .songs()
        .unwrap()
        .iter()
        .filter(|s| !s.title.contains("Faster"))
        .map(|s| s.id.clone())
        .collect();
    crate::usb_dac::fake::checks::the_driver_plays_and_every_trouble_ends_in_an_error(
        &needle, &music, ids, FALLBACK,
    );
    if !FALLBACK {
        return;
    }
    // Playing again (the driver checks end paused), on the system's output.
    let ids: Vec<String> = needle
        .songs()
        .unwrap()
        .iter()
        .filter(|s| !s.title.contains("Odd"))
        .map(|s| s.id.clone())
        .collect();
    needle.play(ids, 0).unwrap();
    wait("playing", Duration::from_secs(5), || {
        needle.playback().playing
    });

    // An output that fails (a DAC unplugged): a notice, it is not opened again, and the song
    // goes on through the system's output (silent here).
    needle.player.send(needle_core::audio::Command::Volume(0.));
    std::thread::sleep(Duration::from_millis(1500));
    let before = needle.playback().position;
    let failing = Arc::new(Failing::default());
    set_platform_output(Some(failing.clone()));
    // The song moves to the new output where it was, not from its start.
    wait("the new output", Duration::from_secs(5), || {
        failing.frames.load(Ordering::SeqCst) > 0
    });
    let moved = needle.playback().position;
    assert!(
        moved >= before - 0.3,
        "the song moved over from {before} to {moved}"
    );
    wait(
        "music at the failing output",
        Duration::from_secs(10),
        || failing.frames.load(Ordering::SeqCst) > 0,
    );
    // The notice is said.
    let start = Instant::now();
    let mut said = false;
    while start.elapsed() < Duration::from_secs(10) && !said {
        said = needle
            .player
            .state()
            .output_notice
            .is_some_and(|n| n.contains("Failing DAC"));
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(said, "the failure is said");
    let taken = failing.frames.load(Ordering::SeqCst);
    let at = needle.playback().position;
    std::thread::sleep(Duration::from_millis(1500));
    assert_eq!(
        failing.frames.load(Ordering::SeqCst),
        taken,
        "the failed output is not opened again"
    );
    let later = needle.playback().position;
    assert!(later > at + 0.5, "the song went on ({at} -> {later})");
    // The notice stays once the system's output has opened, so the listener sees why.
    assert!(
        needle
            .player
            .state()
            .output_notice
            .is_some_and(|n| n.contains("Failing DAC")),
        "the notice is still there"
    );
    set_platform_output(None);
    needle.toggle();
}

/// An output that takes music for a moment, then fails as an unplugged DAC does.
#[derive(Default)]
struct Failing {
    frames: AtomicU64,
}

impl PlatformOutput for Failing {
    fn name(&self) -> String {
        "Failing DAC".into()
    }

    fn render(&self, feed: &mut Feed) -> Result<()> {
        let mut buf = vec![0f32; 4_800];
        let start = Instant::now();
        while !feed.stopped() {
            if !feed.playing() {
                feed.idle();
                continue;
            }
            let f = feed.format();
            feed.playing_at(f);
            let n = feed.read(&mut buf[..(f.rate as usize / 100) * f.channels as usize]);
            self.frames.fetch_add(n as u64, Ordering::SeqCst);
            std::thread::sleep(Duration::from_millis(10));
            if start.elapsed() > Duration::from_millis(500) {
                anyhow::bail!("the DAC was unplugged");
            }
        }
        Ok(())
    }
}

/// When a platform output fails, its notice ("… stopped (why). Playing through the phone
/// instead.") stays after the system's output opens (opening it clears older notices).
#[test]
fn the_failure_notice_stays() {
    if !FALLBACK {
        return;
    }
    let _turn = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let dir = tempfile::tempdir().unwrap();
    let music = dir.path().join("music");
    needle_core::demo::create(&music).unwrap();
    let needle = Needle::new(dir.path().join("data").to_string_lossy().into()).unwrap();
    needle.player.send(needle_core::audio::Command::Volume(0.));
    let failing = Arc::new(Failing::default());
    set_platform_output(Some(failing.clone()));
    needle.add_folder(music.to_string_lossy().into()).unwrap();
    wait("the scan", Duration::from_secs(60), || {
        !needle.scan_status().running && needle.song_count().unwrap_or(0) >= 3
    });
    let ids: Vec<String> = needle
        .songs()
        .unwrap()
        .iter()
        .map(|s| s.id.clone())
        .collect();
    needle.play(ids, 0).unwrap();
    wait("the failure", Duration::from_secs(10), || {
        failing.frames.load(Ordering::SeqCst) > 40_000
    });
    // Said once the system's output plays, and still there for a while after (a second
    // restart would clear it).
    wait("the notice", Duration::from_secs(5), || {
        needle
            .player
            .state()
            .output_notice
            .is_some_and(|n| n.contains("Failing DAC"))
    });
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(3) {
        assert!(
            needle
                .player
                .state()
                .output_notice
                .is_some_and(|n| n.contains("Failing DAC")),
            "the notice is still there after {:?}",
            start.elapsed()
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    set_platform_output(None);
    needle.toggle();
}
