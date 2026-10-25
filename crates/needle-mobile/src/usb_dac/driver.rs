//! The driver: the music from the core's feed to the DAC, through a [`Bus`]. Any trouble ends
//! `render` with an error at once (a DAC gone, refusing, or no longer taking music within half
//! a second; even a panic), so the core moves the music to Android's output with a notice.
use super::{
    UsbDacInfo,
    bus::Bus,
    descriptors::*,
    packets::*,
};
use anyhow::{Result, anyhow, bail};
use needle_core::platform_output::{Feed, FeedFormat, PlatformOutput};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicU32, Ordering},
    },
    time::{Duration, Instant},
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
/// The id of the feedback endpoint's transfer.
const FEEDBACK: usize = TRANSFERS;
/// How long the DAC may take nothing before Needle gives up on it.
const STALL: Duration = Duration::from_millis(500);

pub struct UsbDac {
    name: String,
    dac: Dac,
    bus: Arc<dyn Bus>,
    high_speed: bool,
    /// UAC2: each clock's rates, by clock id.
    clock_rates: Vec<(u8, Vec<u32>)>,
    /// The volume control's range in 1/256 dB, and the last volume asked (f32 bits).
    volume_range: Option<(i16, i16)>,
    volume: AtomicU32,
    /// One control request at a time (volume from the app, rates from the player).
    control: Mutex<()>,
}

fn wvalue(selector: u8, channel: u8) -> u16 {
    (selector as u16) << 8 | channel as u16
}

impl UsbDac {
    pub fn new(bus: Arc<dyn Bus>, raw: &[u8], name: String) -> Result<Self> {
        let Some(dac) = parse(raw) else {
            bail!("{name} is not a USB audio device Needle can play to");
        };
        bus.claim(dac.control);
        for f in &dac.formats {
            bus.claim(f.interface);
        }
        let high_speed = bus.high_speed();
        let mut out = Self {
            name,
            volume_range: None,
            clock_rates: vec![],
            high_speed,
            bus,
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

    fn index(&self, entity: u8) -> u16 {
        (entity as u16) << 8 | self.dac.control as u16
    }

    fn one_at_a_time(&self) -> std::sync::MutexGuard<'_, ()> {
        self.control.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// A UAC2 clock's rates (RANGE of its sampling frequency control).
    fn clock_range(&self, clock: u8) -> Result<Vec<u32>> {
        let _one = self.one_at_a_time();
        let mut head = [0u8; 2];
        self.bus.control(GET, RANGE, wvalue(0x01, 0), self.index(clock), &mut head)?;
        let count = u16::from_le_bytes(head) as usize;
        let mut all = vec![0u8; 2 + count.min(32) * 12];
        self.bus.control(GET, RANGE, wvalue(0x01, 0), self.index(clock), &mut all)?;
        Ok(rates_in_ranges(&all))
    }

    fn read_volume_range(&self) -> Option<(i16, i16)> {
        let v = self.dac.volume?;
        let channel = if v.master { 0 } else { 1 };
        let _one = self.one_at_a_time();
        let (min, max) = match self.dac.uac {
            Uac::Two => {
                let mut answer = [0u8; 8];
                self.bus.control(GET, RANGE, wvalue(0x02, channel), self.index(v.unit), &mut answer).ok()?;
                (i16::from_le_bytes([answer[2], answer[3]]), i16::from_le_bytes([answer[4], answer[5]]))
            }
            Uac::One => {
                let (mut low, mut high) = ([0u8; 2], [0u8; 2]);
                self.bus.control(GET, UAC1_GET_MIN, wvalue(0x02, channel), self.index(v.unit), &mut low).ok()?;
                self.bus.control(GET, UAC1_GET_MAX, wvalue(0x02, channel), self.index(v.unit), &mut high).ok()?;
                (i16::from_le_bytes(low), i16::from_le_bytes(high))
            }
        };
        (min < max).then_some((min, max))
    }

    fn apply_volume(&self) {
        let (Some(v), Some((min, max))) = (self.dac.volume, self.volume_range) else {
            return;
        };
        let steps = volume_steps(f32::from_bits(self.volume.load(Ordering::Relaxed)), min, max);
        let _one = self.one_at_a_time();
        let channels: &[u8] = if v.master { &[0] } else { &[1, 2] };
        for c in channels {
            let mut data = steps.to_le_bytes();
            let _ = self.bus.control(SET, CUR, wvalue(0x02, *c), self.index(v.unit), &mut data);
        }
    }

    /// Sets the DAC's rate for `f`.
    fn set_rate(&self, f: &Format, rate: u32) -> Result<()> {
        let _one = self.one_at_a_time();
        match self.dac.uac {
            Uac::Two => {
                let clock = self.dac.clock_for(f.terminal).ok_or_else(|| anyhow!("The DAC's clock was not found"))?;
                let mut data = rate.to_le_bytes();
                self.bus.control(SET, CUR, wvalue(0x01, 0), self.index(clock), &mut data)?;
            }
            Uac::One => {
                let mut data = [rate as u8, (rate >> 8) as u8, (rate >> 16) as u8];
                self.bus.control(SET_ENDPOINT, CUR, wvalue(0x01, 0), f.endpoint as u16, &mut data)?;
            }
        }
        Ok(())
    }

    /// Streams music of one format until the format changes, the music pauses, or it stops.
    fn stream(&self, feed: &mut Feed, format: FeedFormat, f: &Format) -> Result<()> {
        let per_second = packets_per_second(self.high_speed, f.interval);
        let per_transfer = ((per_second * TRANSFER_MS / 1000) as usize).max(1);
        let frame = f.channels as usize * f.subslot as usize;
        // The most a packet may carry: the DAC's limit (with high speed's extra transactions).
        let limit = ((f.max_packet & 0x7FF) as usize) * (1 + ((f.max_packet >> 11) & 0b11) as usize);
        let nominal = nominal(format.rate, per_second);
        if ((nominal >> 16) as usize + 2) * frame > limit {
            bail!("{} Hz is too fast for this setting of the DAC", format.rate);
        }
        let mut pacer = Pacer::new(format.rate, per_second);
        let feedback_endpoint = f.feedback.filter(|_| f.sync == Sync::Async);
        let feedback_size = if self.high_speed { 4 } else { 3 };
        let channels = format.channels as usize;
        let mut samples = vec![0f32; ((nominal >> 16) as usize + 2) * channels];
        let mut bytes = vec![0u8; per_transfer * limit];
        // Fill one transfer's packets from the feed; false once the music in this format ends.
        let mut fill = |pacer: &mut Pacer, feed: &mut Feed, bytes: &mut [u8]| -> (Vec<usize>, usize, bool) {
            let mut lengths = Vec::with_capacity(per_transfer);
            let mut at = 0;
            let mut more = true;
            for _ in 0..per_transfer {
                let wanted = pacer.next() * channels;
                let got = if more { feed.read(&mut samples[..wanted]) } else { 0 };
                if got < wanted {
                    samples[got..wanted].fill(0.);
                    more = false;
                }
                let n = pack(&samples[..wanted], channels, f.channels as usize, f.subslot as usize, f.bits, &mut bytes[at..]);
                at += n;
                lengths.push(n);
            }
            (lengths, at, more)
        };
        let mut ending = false;
        let mut result = Ok(());
        for id in 0..TRANSFERS {
            let (lengths, used, more) = fill(&mut pacer, feed, &mut bytes);
            if let Err(e) = self.bus.submit(id, f.endpoint, &bytes[..used], &lengths) {
                result = Err(e);
                ending = true;
                break;
            }
            if !more {
                ending = true;
                break;
            }
        }
        if let (Some(ep), false) = (feedback_endpoint, ending) {
            let _ = self.bus.submit(FEEDBACK, ep, &[], &[feedback_size]);
        }
        let mut last_back = Instant::now();
        while self.bus.busy() > 0 {
            let back = match self.bus.reap(100) {
                Ok(Some(back)) => back,
                Ok(None) => {
                    // Music handed over and nothing back for too long: the DAC stopped.
                    if last_back.elapsed() > STALL {
                        result = Err(anyhow!("{} stopped taking music", self.name));
                        break;
                    }
                    continue;
                }
                Err(e) => {
                    result = Err(e);
                    break;
                }
            };
            last_back = Instant::now();
            if back.id == FEEDBACK {
                if back.status == 0 {
                    if let Some(rate) = feedback(&back.data, nominal) {
                        pacer.set(rate);
                    }
                }
                if !ending {
                    if let Some(ep) = feedback_endpoint {
                        let _ = self.bus.submit(FEEDBACK, ep, &[], &[feedback_size]);
                    }
                }
                continue;
            }
            if back.status == -libc_enodev() || back.status == -libc_eshutdown() {
                result = Err(anyhow!("{} is gone", self.name));
                break;
            }
            if ending || feed.stopped() || !feed.playing() {
                ending = true;
                continue;
            }
            let (lengths, used, more) = fill(&mut pacer, feed, &mut bytes);
            if let Err(e) = self.bus.submit(back.id, f.endpoint, &bytes[..used], &lengths) {
                result = Err(e);
                break;
            }
            if !more {
                ending = true;
            }
        }
        // Everything asked back before leaving; the bus keeps what the kernel still holds.
        for id in 0..=FEEDBACK {
            self.bus.discard(id);
        }
        let until = Instant::now() + Duration::from_millis(500);
        while self.bus.busy() > 0 && Instant::now() < until {
            if self.bus.reap(20).is_err() {
                break;
            }
        }
        result
    }

    fn render_inner(&self, feed: &mut Feed) -> Result<()> {
        self.apply_volume();
        let mut open: Option<(u8, u8, u32, u16)> = None;
        let result = loop {
            if feed.stopped() {
                break Ok(());
            }
            if !feed.playing() {
                // Paused: let the DAC rest (setting 0 frees the bus), and let seeks finish.
                if let Some((interface, ..)) = open.take() {
                    let _ = self.bus.set_interface(interface, 0);
                    super::status_playing(None);
                }
                feed.idle();
                continue;
            }
            let format = feed.format();
            let Some(f) = choose(&self.dac.formats, format.channels, 24) else {
                break Err(anyhow!("{} takes no {}-channel music", self.name, format.channels));
            };
            if !self.rates(f).contains(&format.rate) {
                break Err(anyhow!("{} does not play {} Hz", self.name, format.rate));
            }
            if open != Some((f.interface, f.alt, format.rate, format.channels)) {
                if let Some((interface, ..)) = open.take() {
                    let _ = self.bus.set_interface(interface, 0);
                }
                if let Err(e) = self.bus.set_interface(f.interface, f.alt).and_then(|_| self.set_rate(f, format.rate)) {
                    break Err(e);
                }
                open = Some((f.interface, f.alt, format.rate, format.channels));
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
        if let Some((interface, ..)) = open {
            let _ = self.bus.set_interface(interface, 0);
        }
        result
    }
}

fn libc_enodev() -> i32 {
    19
}

fn libc_eshutdown() -> i32 {
    108
}

impl PlatformOutput for UsbDac {
    fn name(&self) -> String {
        format!("{} (USB)", self.name)
    }

    fn dop(&self) -> bool {
        self.takes_dop()
    }

    fn render(&self, feed: &mut Feed) -> Result<()> {
        // A panic is an error too: the core then plays through Android at once.
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| self.render_inner(feed)))
            .unwrap_or_else(|panic| {
                let why = panic.downcast_ref::<&str>().map(|s| s.to_string()).or_else(|| panic.downcast_ref::<String>().cloned()).unwrap_or_default();
                Err(anyhow!("the driver stopped ({why})"))
            });
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
