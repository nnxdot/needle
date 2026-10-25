//! Needle's own USB DAC driver for Android (behind Settings › Sound › Bit-perfect USB): the
//! music goes to the DAC as it is, at each song's own rate and depth, without Android's mixer.
//! USB Audio Class 1 and 2; 16, 24, and 32-bit music in 2, 3, or 4-byte slots; the DAC's
//! feedback for its own clock; its own volume; DSD as DoP for a DAC that takes 24-bit at
//! 176.4 kHz or more. Android 8 and newer. When the DAC refuses, the core plays through
//! Android again (the core's `platform_output`).
// Used only on Android; tested everywhere.
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub mod descriptors;
#[cfg(target_os = "android")]
mod device;
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub mod packets;

use crate::{Needle, Result};
use std::sync::Mutex;

/// What the driver plays now ("96 kHz, 24-bit"), and why it last stopped, for the app.
static STATUS: Mutex<(Option<String>, Option<String>)> = Mutex::new((None, None));

pub(crate) fn status_playing(text: Option<String>) {
    STATUS.lock().unwrap_or_else(|e| e.into_inner()).0 = text;
}

#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub(crate) fn status_failed(why: String) {
    let mut s = STATUS.lock().unwrap_or_else(|e| e.into_inner());
    s.0 = None;
    s.1 = Some(why);
}

/// How Needle's USB driver is doing, for Settings and for the app to fall back.
#[derive(Clone, uniffi::Record)]
pub struct UsbDacStatus {
    /// What it plays now, such as "96 kHz, 24-bit".
    pub playing: Option<String>,
    /// Why it stopped, once; Android's output plays meanwhile.
    pub problem: Option<String>,
}

/// What Needle found in a USB DAC, for Settings to say.
#[derive(Clone, uniffi::Record)]
pub struct UsbDacInfo {
    pub name: String,
    /// 1 or 2: USB Audio Class.
    pub uac: u8,
    /// The rates it plays, lowest first.
    pub rates: Vec<u32>,
    /// The deepest music it takes, in bits.
    pub bits: u8,
    /// It takes DSD as DoP.
    pub dop: bool,
    /// Needle sets its volume.
    pub volume: bool,
}

#[cfg(target_os = "android")]
mod driver {
    use super::{UsbDacInfo, descriptors::*, device::*, packets::*};
    use anyhow::{Result, bail};
    use needle_core::platform_output::{Feed, FeedFormat, PlatformOutput};
    use std::sync::{
        Arc, Mutex,
        atomic::{AtomicU32, Ordering},
    };

    const GET: u8 = 0xA1;
    const SET: u8 = 0x21;
    const SET_ENDPOINT: u8 = 0x22;
    const CUR: u8 = 0x01;
    const RANGE: u8 = 0x02;
    const UAC1_GET_MIN: u8 = 0x82;
    const UAC1_GET_MAX: u8 = 0x83;
    /// Transfers in flight, and how long each holds (ms): about 32 ms of music queued.
    const TRANSFERS: usize = 8;
    const TRANSFER_MS: u32 = 4;

    pub struct UsbDac {
        name: String,
        dac: Dac,
        device: Arc<Device>,
        high_speed: bool,
        /// UAC2: each clock's rates, by clock id.
        clock_rates: Vec<(u8, Vec<u32>)>,
        /// The volume control's range in 1/256 dB, and the last volume asked (f32 bits).
        volume_range: Option<(i16, i16)>,
        volume: AtomicU32,
        /// One control request at a time (volume from the app, rates from the player).
        control: Mutex<()>,
    }

    impl UsbDac {
        pub fn open(fd: i32, raw: &[u8], name: String) -> Result<Self> {
            let Some(dac) = parse(raw) else {
                bail!("{name} is not a USB audio device Needle can play to");
            };
            let device = Arc::new(Device::open(fd)?);
            device.claim(dac.control);
            for f in &dac.formats {
                device.claim(f.interface);
            }
            let high_speed = device.high_speed();
            let mut out = Self {
                name,
                volume_range: None,
                clock_rates: vec![],
                high_speed,
                device,
                volume: AtomicU32::new(1f32.to_bits()),
                control: Mutex::new(()),
                dac,
            };
            if out.dac.uac == Uac::Two {
                let clocks: Vec<u8> = out.dac.formats.iter().filter_map(|f| out.dac.clock_for(f.terminal)).collect();
                for clock in clocks {
                    if out.clock_rates.iter().any(|(c, _)| *c == clock) {
                        continue;
                    }
                    let rates = out.clock_range(clock).unwrap_or_default();
                    out.clock_rates.push((clock, rates));
                }
            }
            out.volume_range = out.read_volume_range();
            Ok(out)
        }

        pub fn info(&self) -> UsbDacInfo {
            let mut rates: Vec<u32> = self.dac.formats.iter().flat_map(|f| self.rates(f)).collect();
            rates.sort_unstable();
            rates.dedup();
            UsbDacInfo {
                name: self.name.clone(),
                uac: if self.dac.uac == Uac::Two { 2 } else { 1 },
                rates,
                bits: self.dac.formats.iter().map(|f| f.bits).max().unwrap_or(16),
                dop: self.takes_dop(),
                volume: self.volume_range.is_some(),
            }
        }

        fn rates(&self, f: &Format) -> Vec<u32> {
            match self.dac.uac {
                Uac::Two => self
                    .dac
                    .clock_for(f.terminal)
                    .and_then(|c| self.clock_rates.iter().find(|(id, _)| *id == c))
                    .map(|(_, r)| r.clone())
                    .unwrap_or_default(),
                Uac::One => listed_rates(f),
            }
        }

        fn takes_dop(&self) -> bool {
            self.dac.formats.iter().any(|f| f.bits >= 24 && f.channels == 2 && self.rates(f).iter().any(|r| *r >= 176_400))
        }

        fn wvalue(selector: u8, channel: u8) -> u16 {
            (selector as u16) << 8 | channel as u16
        }

        fn index(&self, entity: u8) -> u16 {
            (entity as u16) << 8 | self.dac.control as u16
        }

        /// A UAC2 clock's rates (RANGE of its sampling frequency control).
        fn clock_range(&self, clock: u8) -> Result<Vec<u32>> {
            let _one = self.control.lock().unwrap_or_else(|e| e.into_inner());
            let mut head = [0u8; 2];
            self.device.control(GET, RANGE, Self::wvalue(0x01, 0), self.index(clock), &mut head)?;
            let count = u16::from_le_bytes(head) as usize;
            let mut all = vec![0u8; 2 + count.min(32) * 12];
            self.device.control(GET, RANGE, Self::wvalue(0x01, 0), self.index(clock), &mut all)?;
            Ok(rates_in_ranges(&all))
        }

        fn read_volume_range(&self) -> Option<(i16, i16)> {
            let v = self.dac.volume?;
            let channel = if v.master { 0 } else { 1 };
            let _one = self.control.lock().unwrap_or_else(|e| e.into_inner());
            match self.dac.uac {
                Uac::Two => {
                    let mut answer = [0u8; 8];
                    self.device.control(GET, RANGE, Self::wvalue(0x02, channel), self.index(v.unit), &mut answer).ok()?;
                    let min = i16::from_le_bytes([answer[2], answer[3]]);
                    let max = i16::from_le_bytes([answer[4], answer[5]]);
                    (min < max).then_some((min, max))
                }
                Uac::One => {
                    let mut low = [0u8; 2];
                    let mut high = [0u8; 2];
                    self.device.control(GET, UAC1_GET_MIN, Self::wvalue(0x02, channel), self.index(v.unit), &mut low).ok()?;
                    self.device.control(GET, UAC1_GET_MAX, Self::wvalue(0x02, channel), self.index(v.unit), &mut high).ok()?;
                    let (min, max) = (i16::from_le_bytes(low), i16::from_le_bytes(high));
                    (min < max).then_some((min, max))
                }
            }
        }

        fn apply_volume(&self) {
            let (Some(v), Some((min, max))) = (self.dac.volume, self.volume_range) else {
                return;
            };
            let steps = volume_steps(f32::from_bits(self.volume.load(Ordering::Relaxed)), min, max);
            let _one = self.control.lock().unwrap_or_else(|e| e.into_inner());
            let channels: Vec<u8> = if v.master { vec![0] } else { vec![1, 2] };
            for c in channels {
                let mut data = steps.to_le_bytes();
                let _ = self.device.control(SET, CUR, Self::wvalue(0x02, c), self.index(v.unit), &mut data);
            }
        }

        /// Sets the DAC's rate for `f`.
        fn set_rate(&self, f: &Format, rate: u32) -> Result<()> {
            let _one = self.control.lock().unwrap_or_else(|e| e.into_inner());
            match self.dac.uac {
                Uac::Two => {
                    let Some(clock) = self.dac.clock_for(f.terminal) else { bail!("The DAC's clock was not found") };
                    let mut data = rate.to_le_bytes();
                    self.device.control(SET, CUR, Self::wvalue(0x01, 0), self.index(clock), &mut data)?;
                }
                Uac::One => {
                    let mut data = [rate as u8, (rate >> 8) as u8, (rate >> 16) as u8];
                    self.device.control(SET_ENDPOINT, CUR, Self::wvalue(0x01, 0), f.endpoint as u16, &mut data)?;
                }
            }
            Ok(())
        }

        /// Streams music of one format until the format changes, the music pauses, or it stops.
        fn stream(&self, feed: &mut Feed, format: FeedFormat, f: &Format) -> Result<()> {
            let device = &*self.device;
            let per_second = packets_per_second(self.high_speed, f.interval);
            let per_transfer = ((per_second * TRANSFER_MS / 1000) as usize).max(1);
            let frame = f.channels as usize * f.subslot as usize;
            // The most a packet may carry: the DAC's limit (with high speed's extra transactions).
            let limit = ((f.max_packet & 0x7FF) as usize) * (1 + ((f.max_packet >> 11) & 0b11) as usize);
            let nominal = nominal(format.rate, per_second);
            let biggest = ((nominal >> 16) as usize + 2) * frame;
            if biggest > limit {
                bail!("{} Hz is too fast for this setting of the DAC", format.rate);
            }
            let mut pacer = Pacer::new(format.rate, per_second);
            let mut transfers: Vec<Transfer> = (0..TRANSFERS).map(|id| Transfer::new(f.endpoint, per_transfer, limit, id)).collect();
            // The feedback endpoint's transfer: one packet at a time.
            let feedback_id = TRANSFERS;
            let mut back = f
                .feedback
                .filter(|_| f.sync == Sync::Async)
                .map(|ep| Transfer::new(ep, 1, if self.high_speed { 4 } else { 3 }, feedback_id));
            let channels = format.channels as usize;
            let mut samples = vec![0f32; ((nominal >> 16) as usize + 2) * channels];
            let mut ending = false;
            // Fill one transfer's packets from the feed; false once the music in this format ends.
            let mut fill = |t: &mut Transfer, pacer: &mut Pacer, feed: &mut Feed| -> (Vec<usize>, bool) {
                let mut lengths = Vec::with_capacity(t.packets);
                let mut at = 0;
                let mut more = true;
                for _ in 0..t.packets {
                    let frames = pacer.next();
                    let wanted = frames * channels;
                    let got = if more { feed.read(&mut samples[..wanted]) } else { 0 };
                    if got < wanted {
                        samples[got..wanted].fill(0.);
                        more = false;
                    }
                    let n = pack(&samples[..wanted], channels, f.channels as usize, f.subslot as usize, f.bits, &mut t.buffer[at..]);
                    at += n;
                    lengths.push(n);
                }
                // Packets are laid out one after another in the buffer.
                (lengths, more)
            };
            for t in transfers.iter_mut() {
                let (lengths, more) = fill(t, &mut pacer, feed);
                t.submit(device, &lengths)?;
                if !more {
                    ending = true;
                    break;
                }
            }
            if let Some(b) = back.as_mut() {
                b.submit(device, &[b.buffer.len()])?;
            }
            let result = loop {
                if transfers.iter().all(|t| !t.busy) {
                    break Ok(());
                }
                let id = match reap(device, true) {
                    Ok(Some(id)) => id,
                    Ok(None) => continue,
                    Err(e) => break Err(e),
                };
                if id == feedback_id {
                    if let Some(b) = back.as_mut() {
                        b.busy = false;
                        let (status, got) = b.result();
                        if status == 0 {
                            if let Some(rate) = feedback(&b.buffer[..got.min(b.buffer.len())], nominal) {
                                pacer.set(rate);
                            }
                        }
                        if !ending && b.submit(device, &[b.buffer.len()]).is_err() {
                            ending = true;
                        }
                    }
                    continue;
                }
                let Some(t) = transfers.get_mut(id) else { continue };
                t.busy = false;
                let (status, _) = t.result();
                if status == -libc::ENODEV || status == -libc::ESHUTDOWN {
                    break Err(anyhow::anyhow!("The DAC is gone"));
                }
                if ending || feed.stopped() || !feed.playing() {
                    ending = true;
                    continue;
                }
                let (lengths, more) = fill(t, &mut pacer, feed);
                if let Err(e) = t.submit(device, &lengths) {
                    break Err(e);
                }
                if !more {
                    ending = true;
                }
            };
            // Everything back from the kernel before its memory goes.
            for t in transfers.iter_mut() {
                t.discard(device);
            }
            if let Some(b) = back.as_mut() {
                b.discard(device);
            }
            let mut left = transfers.iter().filter(|t| t.busy).count() + back.as_ref().is_some_and(|b| b.busy) as usize;
            let mut tries = 0;
            while left > 0 && tries < 500 {
                match reap(device, false) {
                    Ok(Some(id)) => {
                        if id == feedback_id {
                            if let Some(b) = back.as_mut() {
                                b.busy = false;
                            }
                        } else if let Some(t) = transfers.get_mut(id) {
                            t.busy = false;
                        }
                        left -= 1;
                    }
                    Ok(None) => {
                        tries += 1;
                        std::thread::sleep(std::time::Duration::from_millis(2));
                    }
                    Err(_) => break,
                }
            }
            if left > 0 {
                // The kernel kept some: their memory must outlive this, so it is let go.
                std::mem::forget(transfers);
                std::mem::forget(back);
            }
            result
        }
    }

    impl PlatformOutput for UsbDac {
        fn name(&self) -> String {
            format!("{} (USB)", self.name)
        }

        fn dop(&self) -> bool {
            self.takes_dop()
        }

        fn render(&self, feed: &mut Feed) -> Result<()> {
            self.apply_volume();
            let mut open: Option<(u8, u8, u32)> = None;
            let result = loop {
                if feed.stopped() {
                    break Ok(());
                }
                if !feed.playing() {
                    // Paused: let the DAC rest (setting 0 frees the bus), and let seeks finish.
                    if let Some((interface, _, _)) = open.take() {
                        let _ = self.device.set_interface(interface, 0);
                    }
                    feed.idle();
                    continue;
                }
                let format = feed.format();
                let wanted_bits = 24;
                let Some(f) = choose(&self.dac.formats, format.channels, wanted_bits) else {
                    break Err(anyhow::anyhow!("{} takes no {}-channel music", self.name, format.channels));
                };
                if !self.rates(f).contains(&format.rate) {
                    break Err(anyhow::anyhow!("{} does not play {} Hz", self.name, format.rate));
                }
                if open != Some((f.interface, f.alt, format.rate)) {
                    if let Some((interface, _, _)) = open.take() {
                        let _ = self.device.set_interface(interface, 0);
                    }
                    if let Err(e) = self.device.set_interface(f.interface, f.alt) {
                        break Err(e);
                    }
                    if let Err(e) = self.set_rate(f, format.rate) {
                        break Err(e);
                    }
                    open = Some((f.interface, f.alt, format.rate));
                    feed.playing_at(format);
                    let khz = if format.rate % 1000 == 0 { format!("{}", format.rate / 1000) } else { format!("{:.1}", format.rate as f64 / 1000.) };
                    super::status_playing(Some(format!("{khz} kHz, {}-bit", f.bits)));
                    needle_core::logfile::warn(format!(
                        "{}: {} Hz, {} bits in {}-byte slots, {} channels (setting {} of interface {})",
                        self.name, format.rate, f.bits, f.subslot, f.channels, f.alt, f.interface
                    ));
                }
                if let Err(e) = self.stream(feed, format, f) {
                    break Err(e);
                }
            };
            if let Some((interface, _, _)) = open {
                let _ = self.device.set_interface(interface, 0);
            }
            match &result {
                Ok(()) => super::status_playing(None),
                Err(e) => super::status_failed(e.to_string()),
            }
            result
        }

        fn set_volume(&self, volume: f32) {
            self.volume.store(volume.clamp(0., 1.).to_bits(), Ordering::Relaxed);
            self.apply_volume();
        }
    }
}

#[uniffi::export]
impl Needle {
    /// Use Needle's own driver for a USB DAC (Settings › Sound › Bit-perfect USB): `fd` is the
    /// file of Android's connection to it (its interfaces taken with `claimInterface(force)`),
    /// `descriptors` its raw descriptors. From the next moment on, music goes to it as it is.
    /// An error for a device the driver cannot play to; Android's own output goes on then.
    pub fn usb_dac_open(&self, fd: i32, descriptors: Vec<u8>, name: String) -> Result<UsbDacInfo> {
        #[cfg(target_os = "android")]
        {
            let dac = std::sync::Arc::new(driver::UsbDac::open(fd, &descriptors, name)?);
            let info = dac.info();
            needle_core::platform_output::set_platform_output(Some(dac));
            Ok(info)
        }
        #[cfg(not(target_os = "android"))]
        {
            let _ = (fd, name);
            let dac = descriptors::parse(&descriptors).ok_or_else(|| crate::NeedleError::from(anyhow::anyhow!("Not a USB audio device")))?;
            Ok(UsbDacInfo {
                name: String::new(),
                uac: if dac.uac == descriptors::Uac::Two { 2 } else { 1 },
                rates: dac.formats.iter().flat_map(descriptors::listed_rates).collect(),
                bits: dac.formats.iter().map(|f| f.bits).max().unwrap_or(16),
                dop: false,
                volume: dac.volume.is_some(),
            })
        }
    }

    /// Stop using Needle's driver (the DAC was unplugged, or Bit-perfect USB turned off).
    pub fn usb_dac_close(&self) {
        needle_core::platform_output::set_platform_output(None);
        status_playing(None);
    }

    /// What the driver plays now, and why it stopped (said once).
    pub fn usb_dac_status(&self) -> UsbDacStatus {
        let mut s = STATUS.lock().unwrap_or_else(|e| e.into_inner());
        UsbDacStatus {
            playing: s.0.clone(),
            problem: s.1.take(),
        }
    }
}
