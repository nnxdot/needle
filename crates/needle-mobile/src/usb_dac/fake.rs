//! A pretend USB DAC for tests: it takes transfers at the pace a real one does, answers the
//! clock and volume requests, and can stall, vanish, or make the driver panic. The tests play
//! through the real Needle player and the real driver, and check that every trouble ends in an
//! error (the core then plays through the system's output), never a hang.
use super::bus::{Bus, Reaped};
use anyhow::{Result, bail};
use std::{
    collections::VecDeque,
    sync::Mutex,
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Trouble {
    None,
    /// Takes this many transfers, then never gives any back.
    StallAfter(usize),
    /// Takes this many transfers, then is unplugged.
    GoneAfter(usize),
    /// Setting the alternate setting panics (a bug in the driver's way).
    Panic,
}

#[derive(Default)]
struct State {
    /// Transfers with the device: (when back, id, endpoint).
    queue: VecDeque<(Instant, usize, u8)>,
    /// When the device is free for the next data transfer.
    free_at: Option<Instant>,
    done: usize,
    pub bytes: usize,
    pub rates: Vec<u32>,
    pub alts: Vec<(u8, u8)>,
    pub loudest: i32,
}

pub struct FakeDac {
    trouble: Trouble,
    state: Mutex<State>,
}

impl FakeDac {
    pub fn new(trouble: Trouble) -> Self {
        Self {
            trouble,
            state: Mutex::default(),
        }
    }

    pub fn bytes(&self) -> usize {
        self.state.lock().unwrap().bytes
    }

    pub fn rates(&self) -> Vec<u32> {
        self.state.lock().unwrap().rates.clone()
    }

    pub fn alts(&self) -> Vec<(u8, u8)> {
        self.state.lock().unwrap().alts.clone()
    }

    pub fn loudest(&self) -> i32 {
        self.state.lock().unwrap().loudest
    }
}

/// A UAC2 DAC at high speed: one clock (0x29) with 44.1, 48, and 96 kHz, a master volume, and
/// 16-bit and 24-in-32-bit stereo settings with async feedback.
pub fn descriptors() -> Vec<u8> {
    let mut d = vec![];
    d.extend([18, 1, 0, 2, 0xEF, 2, 1, 64, 0, 0, 0, 0, 0, 1, 1, 2, 3, 1]);
    d.extend([9, 2, 0, 0, 2, 1, 0, 0x80, 50]);
    d.extend([9, 4, 0, 0, 0, 1, 1, 0x20, 0]);
    d.extend([9, 0x24, 1, 0x00, 0x02, 0, 0, 0, 0]);
    d.extend([8, 0x24, 0x0A, 0x29, 3, 7, 0, 0]);
    d.extend([
        17, 0x24, 2, 2, 0x01, 0x01, 0, 0x29, 2, 3, 0, 0, 0, 0, 0, 0, 0,
    ]);
    d.extend([18, 0x24, 6, 10, 2, 0x0C, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    d.extend([9, 4, 1, 0, 0, 1, 2, 0x20, 0]);
    d.extend([9, 4, 1, 1, 2, 1, 2, 0x20, 0]);
    d.extend([16, 0x24, 1, 2, 0, 1, 1, 0, 0, 0, 2, 3, 0, 0, 0, 0]);
    d.extend([6, 0x24, 2, 1, 2, 16]);
    d.extend([7, 5, 0x01, 0x05, 0x88, 0x01, 1]);
    d.extend([7, 5, 0x81, 0x11, 4, 0, 4]);
    d.extend([9, 4, 1, 2, 2, 1, 2, 0x20, 0]);
    d.extend([16, 0x24, 1, 2, 0, 1, 1, 0, 0, 0, 2, 3, 0, 0, 0, 0]);
    d.extend([6, 0x24, 2, 1, 4, 24]);
    d.extend([7, 5, 0x01, 0x05, 0x10, 0x03, 1]);
    d.extend([7, 5, 0x81, 0x11, 4, 0, 4]);
    d
}

impl Bus for FakeDac {
    fn high_speed(&self) -> bool {
        true
    }

    fn claim(&self, _interface: u8) {}

    fn set_interface(&self, interface: u8, alt: u8) -> Result<()> {
        if self.trouble == Trouble::Panic && alt != 0 {
            panic!("a pretend bug");
        }
        self.state.lock().unwrap().alts.push((interface, alt));
        Ok(())
    }

    fn control(
        &self,
        request_type: u8,
        request: u8,
        value: u16,
        _index: u16,
        data: &mut [u8],
    ) -> Result<usize> {
        let selector = (value >> 8) as u8;
        match (request_type, request, selector) {
            // The clock's rates: three subranges of one rate each.
            (0xA1, 0x02, 0x01) => {
                let mut answer = vec![3, 0];
                for r in [44_100u32, 48_000, 96_000] {
                    answer.extend(r.to_le_bytes());
                    answer.extend(r.to_le_bytes());
                    answer.extend(0u32.to_le_bytes());
                }
                let n = data.len().min(answer.len());
                data[..n].copy_from_slice(&answer[..n]);
                Ok(n)
            }
            // The volume's range: -100 dB to 0.
            (0xA1, 0x02, 0x02) => {
                let mut answer = vec![1, 0];
                answer.extend((-100i16 * 256).to_le_bytes());
                answer.extend(0i16.to_le_bytes());
                answer.extend(256i16.to_le_bytes());
                let n = data.len().min(answer.len());
                data[..n].copy_from_slice(&answer[..n]);
                Ok(n)
            }
            // Setting the rate.
            (0x21, 0x01, 0x01) => {
                self.state
                    .lock()
                    .unwrap()
                    .rates
                    .push(u32::from_le_bytes([data[0], data[1], data[2], data[3]]));
                Ok(data.len())
            }
            _ => Ok(data.len()),
        }
    }

    fn submit(&self, id: usize, endpoint: u8, bytes: &[u8], lengths: &[usize]) -> Result<()> {
        let mut s = self.state.lock().unwrap();
        if let Trouble::GoneAfter(n) = self.trouble {
            if s.done >= n {
                bail!("The DAC took no more music");
            }
        }
        let now = Instant::now();
        let due = if endpoint & 0x80 != 0 {
            // Feedback, every few milliseconds.
            now + Duration::from_millis(8)
        } else {
            // A transfer of 32 high-speed packets is 4 ms of music, played one after another.
            let start = s.free_at.filter(|t| *t > now).unwrap_or(now);
            let end = start + Duration::from_micros(125 * lengths.len() as u64);
            s.free_at = Some(end);
            s.bytes += bytes.len();
            // 24-bit music in 32-bit slots: how loud it got.
            for c in bytes.chunks_exact(4) {
                let v = i32::from_le_bytes([c[0], c[1], c[2], c[3]]) >> 8;
                s.loudest = s.loudest.max(v.abs());
            }
            end
        };
        s.queue.push_back((due, id, endpoint));
        Ok(())
    }

    fn reap(&self, wait_ms: u32) -> Result<Option<Reaped>> {
        let until = Instant::now() + Duration::from_millis(wait_ms as u64);
        loop {
            {
                let mut s = self.state.lock().unwrap();
                if let Trouble::GoneAfter(n) = self.trouble {
                    if s.done >= n {
                        bail!("The DAC is gone");
                    }
                }
                let stalled = matches!(self.trouble, Trouble::StallAfter(n) if s.done >= n);
                let now = Instant::now();
                if !stalled {
                    if let Some(i) = s.queue.iter().position(|(due, ..)| *due <= now) {
                        let (_, id, endpoint) = s.queue.remove(i).unwrap();
                        s.done += 1;
                        // Feedback: the nominal 44.1 kHz in 16.16 per microframe.
                        let data = if endpoint & 0x80 != 0 {
                            super::packets::nominal(44_100, 8000).to_le_bytes().to_vec()
                        } else {
                            vec![]
                        };
                        return Ok(Some(Reaped {
                            id,
                            status: 0,
                            data,
                        }));
                    }
                }
            }
            if Instant::now() >= until {
                return Ok(None);
            }
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    fn discard(&self, id: usize) {
        let mut s = self.state.lock().unwrap();
        // A stalled device does not give back even what is discarded.
        if matches!(self.trouble, Trouble::StallAfter(n) if s.done >= n) {
            return;
        }
        let now = Instant::now();
        for q in s.queue.iter_mut().filter(|q| q.1 == id) {
            q.0 = now;
        }
    }

    fn busy(&self) -> usize {
        self.state.lock().unwrap().queue.len()
    }
}

/// Checks for the driver with the real player (run from `platform_output_tests`: the output is
/// one for the whole program, so one player runs every check).
pub mod checks {
    use super::*;
    use crate::{Needle, usb_dac::driver::UsbDac};
    use needle_core::platform_output::set_platform_output;
    use std::sync::Arc;

    fn wait(what: &str, limit: Duration, mut done: impl FnMut() -> bool) {
        let start = Instant::now();
        while !done() {
            assert!(start.elapsed() < limit, "timed out waiting for {what}");
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    /// The driver gave up (its own report, fresh) and the core said so in its notice.
    fn said(needle: &Needle, words: &str, limit: Duration) -> bool {
        let start = Instant::now();
        while start.elapsed() < limit {
            let reported = super::super::STATUS.lock().unwrap().1.is_some();
            if reported
                && needle
                    .player
                    .state()
                    .output_notice
                    .is_some_and(|n| n.contains(words))
            {
                return true;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        false
    }

    fn driver(trouble: Trouble) -> (Arc<FakeDac>, Arc<UsbDac>) {
        let fake = Arc::new(FakeDac::new(trouble));
        let dac =
            Arc::new(UsbDac::new(fake.clone(), &descriptors(), "Pretend DAC".into()).unwrap());
        (fake, dac)
    }

    /// The driver on a pretend DAC: it plays, rests while paused, and every trouble (a stall,
    /// the DAC gone, a panic, a rate it does not play) ends in an error within a second or two,
    /// with the song going on through the system's output. `ids` are songs at 44.1 kHz.
    pub fn the_driver_plays_and_every_trouble_ends_in_an_error(
        needle: &Needle,
        music: &std::path::Path,
        ids: Vec<String>,
        fallback: bool,
    ) {
        // Silent: after a trouble the system's output takes over.
        needle.player.send(needle_core::audio::Command::Volume(0.));
        let (fake, dac) = driver(Trouble::None);
        let info = dac.info();
        assert_eq!(
            (info.uac, info.bits, info.rates.clone(), info.volume),
            (2, 24, vec![44_100, 48_000, 96_000], true)
        );
        set_platform_output(Some(dac));

        // It plays: 24-bit in 4-byte slots at the song's rate, in real time, and not silence.
        needle.play(ids.clone(), 0).unwrap();
        // (The song playing before moves over first, at its own rate.)
        wait(
            "the 44.1 kHz song at the DAC",
            Duration::from_secs(10),
            || fake.rates().last() == Some(&44_100),
        );
        wait("music at the DAC", Duration::from_secs(10), || {
            fake.bytes() > 100_000
        });
        assert_eq!(fake.alts().last(), Some(&(1, 2)));
        assert!(fake.loudest() > 1000, "music, not silence");
        let before = fake.bytes();
        std::thread::sleep(Duration::from_secs(1));
        let second = fake.bytes() - before;
        assert!(
            (250_000..450_000).contains(&second),
            "{second} bytes a second (44.1 kHz × 8 = 352 800)"
        );
        assert!(super::super::STATUS.lock().unwrap().0.as_deref() == Some("44.1 kHz, 24-bit"));

        // Paused: the DAC rests (setting 0), and plays again after.
        needle.toggle();
        wait("the DAC resting", Duration::from_secs(3), || {
            fake.alts().last() == Some(&(1, 0))
        });
        needle.toggle();
        wait("playing again", Duration::from_secs(3), || {
            fake.alts().last() == Some(&(1, 2))
        });

        // Each trouble: an error within a second, and the song goes on through the system.
        for (trouble, words) in [
            (Trouble::StallAfter(40), &["stopped taking music"][..]),
            (Trouble::GoneAfter(40), &["gone", "no more music"][..]),
            (Trouble::Panic, &["driver stopped"][..]),
        ] {
            let (_, dac) = driver(trouble);
            let at = needle.playback().position;
            set_platform_output(Some(dac));
            let start = Instant::now();
            assert!(
                said(needle, "Pretend DAC", Duration::from_secs(5)),
                "{trouble:?}: the failure is said"
            );
            assert!(
                start.elapsed() < Duration::from_secs(3),
                "{trouble:?}: it took {:?}",
                start.elapsed()
            );
            let why = super::super::STATUS
                .lock()
                .unwrap()
                .1
                .take()
                .unwrap_or_default();
            assert!(words.iter().any(|w| why.contains(w)), "{trouble:?}: {why}");
            // Without the system's output (see FALLBACK), the first trouble is the last check.
            if !fallback {
                return;
            }
            std::thread::sleep(Duration::from_millis(800));
            assert!(
                needle.playback().position > at,
                "{trouble:?}: the song went on"
            );
        }

        // A song at a rate the DAC does not play: an error, not silence.
        let (_, dac) = driver(Trouble::None);
        set_platform_output(Some(dac));
        let odd = music.join("05 - Odd.wav");
        let frames = 32_000 * 3;
        let mut wav = vec![];
        wav.extend(b"RIFF");
        wav.extend((36 + frames * 4u32).to_le_bytes());
        wav.extend(b"WAVEfmt ");
        wav.extend(16u32.to_le_bytes());
        wav.extend(1u16.to_le_bytes());
        wav.extend(2u16.to_le_bytes());
        wav.extend(32_000u32.to_le_bytes());
        wav.extend((32_000u32 * 4).to_le_bytes());
        wav.extend(4u16.to_le_bytes());
        wav.extend(16u16.to_le_bytes());
        wav.extend(b"data");
        wav.extend((frames * 4).to_le_bytes());
        wav.extend(vec![0u8; (frames * 4) as usize]);
        std::fs::write(&odd, wav).unwrap();
        needle.rescan();
        wait("the odd song", Duration::from_secs(30), || {
            needle
                .songs()
                .unwrap_or_default()
                .iter()
                .any(|s| s.title.contains("Odd"))
        });
        let odd_id = needle
            .songs()
            .unwrap()
            .into_iter()
            .find(|s| s.title.contains("Odd"))
            .unwrap()
            .id;
        needle.play(vec![odd_id], 0).unwrap();
        assert!(
            said(needle, "Pretend DAC", Duration::from_secs(5)),
            "32 kHz: the failure is said"
        );
        let why = super::super::STATUS
            .lock()
            .unwrap()
            .1
            .take()
            .unwrap_or_default();
        assert!(why.contains("32000 Hz"), "{why}");

        set_platform_output(None);
        needle.toggle();
    }
}
