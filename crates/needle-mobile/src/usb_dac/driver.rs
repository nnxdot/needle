//! The driver: the music from the core's feed to the DAC, through a [`Bus`]. Any trouble ends
//! `render` with an error at once (a DAC gone, refusing, or no longer taking music within half
//! a second; even a panic), so the core moves the music to Android's output with a notice.
use super::{UsbDacInfo, bus::Bus, descriptors::*, packets::*};
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
const GET_ENDPOINT: u8 = 0xA2;
const CUR: u8 = 0x01;
const RANGE: u8 = 0x02;
const UAC1_GET_CUR: u8 = 0x81;
const UAC1_GET_MIN: u8 = 0x82;
const UAC1_GET_MAX: u8 = 0x83;
/// Transfers in flight, and how long each holds (ms): about 32 ms of music queued.
const TRANSFERS: usize = 8;
const TRANSFER_MS: u32 = 4;
/// The id of the feedback endpoint's transfer.
const FEEDBACK: usize = TRANSFERS;
/// How long the DAC may take nothing before Needle gives up on it.
const STALL: Duration = Duration::from_millis(500);
/// A clock may take a moment to move: how often it is asked, and how long between.
const RATE_CHECKS: usize = 6;
const RATE_CHECK_WAIT: Duration = Duration::from_millis(30);

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

/// A rate as people say it: "48" or "44.1".
fn khz(rate: u32) -> String {
    if rate % 1000 == 0 {
        format!("{}", rate / 1000)
    } else {
        format!("{:.1}", rate as f64 / 1000.)
    }
}

/// The DAC says the rate asked for: a few chips answer a hair off (44 099), and 44.1 and 48 kHz
/// are 8 % apart.
fn same_rate(said: u32, asked: u32) -> bool {
    said.abs_diff(asked) <= asked / 200
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
            let clocks: Vec<u8> = out
                .dac
                .formats
                .iter()
                .filter_map(|f| out.dac.clock_for(f.terminal))
                .collect();
            for clock in clocks {
                if out.clock_rates.iter().any(|(c, _)| *c == clock) {
                    continue;
                }
                let rates = out.clock_range(clock).unwrap_or_default();
                out.clock_rates.push((clock, rates));
            }
        }
        out.volume_range = out.read_volume_range();
        out.log_what_it_says();
        Ok(out)
    }

    /// What the DAC says of itself, in the log: a report of a DAC that plays wrongly can then
    /// show what it claimed.
    fn log_what_it_says(&self) {
        let say = |line: String| {
            needle_core::logfile::info(&line);
            crate::log(false, &line);
        };
        say(format!(
            "{}: USB Audio Class {}, {} speed, volume {}",
            self.name,
            if self.dac.uac == Uac::Two { 2 } else { 1 },
            if self.high_speed { "high" } else { "full" },
            if self.volume_range.is_some() {
                "yes"
            } else {
                "no"
            },
        ));
        for f in &self.dac.formats {
            say(format!(
                "{}: interface {} setting {}: {} bits in {}-byte slots, {} channels, rates {:?}, {:?} sync, feedback {}, rate control {}, packets up to {} bytes",
                self.name,
                f.interface,
                f.alt,
                f.bits,
                f.subslot,
                f.channels,
                self.rates(f),
                f.sync,
                if f.feedback.is_some() { "yes" } else { "no" },
                if f.rate_control { "yes" } else { "no" },
                f.max_packet,
            ));
        }
    }

    pub fn info(&self) -> UsbDacInfo {
        let mut rates: Vec<u32> = self
            .dac
            .formats
            .iter()
            .flat_map(|f| self.rates(f))
            .collect();
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
        self.dac
            .formats
            .iter()
            .any(|f| f.bits >= 24 && f.channels == 2 && self.rates(f).iter().any(|r| *r >= 176_400))
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
        self.bus
            .control(GET, RANGE, wvalue(0x01, 0), self.index(clock), &mut head)?;
        let count = u16::from_le_bytes(head) as usize;
        let mut all = vec![0u8; 2 + count.min(32) * 12];
        self.bus
            .control(GET, RANGE, wvalue(0x01, 0), self.index(clock), &mut all)?;
        Ok(rates_in_ranges(&all))
    }

    fn read_volume_range(&self) -> Option<(i16, i16)> {
        let v = self.dac.volume?;
        let channel = if v.master { 0 } else { 1 };
        let _one = self.one_at_a_time();
        let (min, max) = match self.dac.uac {
            Uac::Two => {
                let mut answer = [0u8; 8];
                self.bus
                    .control(
                        GET,
                        RANGE,
                        wvalue(0x02, channel),
                        self.index(v.unit),
                        &mut answer,
                    )
                    .ok()?;
                (
                    i16::from_le_bytes([answer[2], answer[3]]),
                    i16::from_le_bytes([answer[4], answer[5]]),
                )
            }
            Uac::One => {
                let (mut low, mut high) = ([0u8; 2], [0u8; 2]);
                self.bus
                    .control(
                        GET,
                        UAC1_GET_MIN,
                        wvalue(0x02, channel),
                        self.index(v.unit),
                        &mut low,
                    )
                    .ok()?;
                self.bus
                    .control(
                        GET,
                        UAC1_GET_MAX,
                        wvalue(0x02, channel),
                        self.index(v.unit),
                        &mut high,
                    )
                    .ok()?;
                (i16::from_le_bytes(low), i16::from_le_bytes(high))
            }
        };
        (min < max).then_some((min, max))
    }

    fn apply_volume(&self) {
        let (Some(v), Some((min, max))) = (self.dac.volume, self.volume_range) else {
            return;
        };
        let steps = volume_steps(
            f32::from_bits(self.volume.load(Ordering::Relaxed)),
            min,
            max,
        );
        let _one = self.one_at_a_time();
        let channels: &[u8] = if v.master { &[0] } else { &[1, 2] };
        for c in channels {
            let mut data = steps.to_le_bytes();
            let _ = self
                .bus
                .control(SET, CUR, wvalue(0x02, *c), self.index(v.unit), &mut data);
        }
    }

    /// The rate the DAC says it runs at for `f`: None when it cannot say.
    fn read_rate(&self, f: &Format) -> Option<u32> {
        let (request_type, request, index, size) = match self.dac.uac {
            Uac::Two => (GET, CUR, self.index(self.dac.clock_for(f.terminal)?), 4),
            Uac::One => (GET_ENDPOINT, UAC1_GET_CUR, f.endpoint as u16, 3),
        };
        let mut data = [0u8; 4];
        let got = self
            .bus
            .control(
                request_type,
                request,
                wvalue(0x01, 0),
                index,
                &mut data[..size],
            )
            .ok()?;
        let rate = u32::from_le_bytes(data);
        (got >= size && rate > 0).then_some(rate)
    }

    /// Sets the DAC's rate for `f`, and reads it back: the rate the DAC said it took, or None
    /// when it cannot say. A DAC that stays at another rate is an error, as its clock decides
    /// how fast the music plays: 44.1 kHz music on a 48 kHz clock is 9 % fast and high.
    fn set_rate(&self, f: &Format, rate: u32) -> Result<Option<u32>> {
        // UAC1 with no sampling frequency control and one rate: the setting is the rate.
        if self.dac.uac == Uac::One && !f.rate_control && listed_rates(f).len() <= 1 {
            return Ok(None);
        }
        let _one = self.one_at_a_time();
        let set = match self.dac.uac {
            Uac::Two => {
                let clock = self
                    .dac
                    .clock_for(f.terminal)
                    .ok_or_else(|| anyhow!("The DAC's clock was not found"))?;
                let mut data = rate.to_le_bytes();
                self.bus
                    .control(SET, CUR, wvalue(0x01, 0), self.index(clock), &mut data)
            }
            Uac::One => {
                let mut data = [rate as u8, (rate >> 8) as u8, (rate >> 16) as u8];
                self.bus.control(
                    SET_ENDPOINT,
                    CUR,
                    wvalue(0x01, 0),
                    f.endpoint as u16,
                    &mut data,
                )
            }
        };
        let mut said = None;
        for check in 0..RATE_CHECKS {
            said = self.read_rate(f);
            match said {
                Some(now) if same_rate(now, rate) => return Ok(Some(now)),
                // It cannot say: no use asking again.
                None => break,
                Some(_) if check + 1 < RATE_CHECKS => std::thread::sleep(RATE_CHECK_WAIT),
                Some(_) => {}
            }
        }
        if let Some(now) = said {
            bail!(
                "{} stayed at {} kHz, so it cannot play {} kHz bit-perfect",
                self.name,
                khz(now),
                khz(rate)
            );
        }
        // It cannot say (as before these checks). The request only counts where the DAC claims
        // the control: without it, it was a try.
        if f.rate_control {
            set?;
        } else {
            needle_core::logfile::warn(format!(
                "{}: no rate control and no answer: {} Hz is not confirmed",
                self.name, rate
            ));
        }
        Ok(None)
    }

    /// The setting for music of `channels` at `rate`: only those that play the rate (a DAC may
    /// have one rate per setting), and of those the best for the music.
    fn format_for(&self, channels: u16, rate: u32) -> Result<Format> {
        let fitting: Vec<Format> = self
            .dac
            .formats
            .iter()
            .filter(|f| self.rates(f).contains(&rate))
            .cloned()
            .collect();
        if let Some(f) = choose(&fitting, channels, 24) {
            return Ok(f.clone());
        }
        if choose(&self.dac.formats, channels, 24).is_none() {
            bail!("{} takes no {}-channel music", self.name, channels);
        }
        bail!("{} does not play {} Hz", self.name, rate)
    }

    /// Streams music of one format until the format changes, the music pauses, or it stops.
    fn stream(&self, feed: &mut Feed, format: FeedFormat, f: &Format) -> Result<()> {
        let per_second = packets_per_second(self.high_speed, f.interval);
        let per_transfer = ((per_second * TRANSFER_MS / 1000) as usize).max(1);
        let frame = f.channels as usize * f.subslot as usize;
        // The most a packet may carry: the DAC's limit (with high speed's extra transactions).
        let limit =
            ((f.max_packet & 0x7FF) as usize) * (1 + ((f.max_packet >> 11) & 0b11) as usize);
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
        let mut fill =
            |pacer: &mut Pacer, feed: &mut Feed, bytes: &mut [u8]| -> (Vec<usize>, usize, bool) {
                let mut lengths = Vec::with_capacity(per_transfer);
                let mut at = 0;
                let mut more = true;
                for _ in 0..per_transfer {
                    let wanted = pacer.next() * channels;
                    let got = if more {
                        feed.read(&mut samples[..wanted])
                    } else {
                        0
                    };
                    if got < wanted {
                        samples[got..wanted].fill(0.);
                        more = false;
                    }
                    let n = pack(
                        &samples[..wanted],
                        channels,
                        f.channels as usize,
                        f.subslot as usize,
                        f.bits,
                        &mut bytes[at..],
                    );
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
            if let Err(e) = self
                .bus
                .submit(back.id, f.endpoint, &bytes[..used], &lengths)
            {
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
            let f = match self.format_for(format.channels, format.rate) {
                Ok(f) => f,
                Err(e) => break Err(e),
            };
            let f = &f;
            if open != Some((f.interface, f.alt, format.rate, format.channels)) {
                if let Some((interface, ..)) = open.take() {
                    let _ = self.bus.set_interface(interface, 0);
                }
                let said = match self
                    .bus
                    .set_interface(f.interface, f.alt)
                    .and_then(|_| self.set_rate(f, format.rate))
                {
                    Ok(said) => said,
                    Err(e) => {
                        // Left open, so the DAC is set back to rest below.
                        open = Some((f.interface, f.alt, format.rate, format.channels));
                        break Err(e);
                    }
                };
                open = Some((f.interface, f.alt, format.rate, format.channels));
                feed.playing_at(format);
                super::status_playing(Some(format!("{} kHz, {}-bit", khz(format.rate), f.bits)));
                needle_core::logfile::warn(format!(
                    "{}: {} Hz, {} bits in {}-byte slots, {} channels (setting {} of interface {}), the DAC says {}",
                    self.name,
                    format.rate,
                    f.bits,
                    f.subslot,
                    f.channels,
                    f.alt,
                    f.interface,
                    said.map_or("nothing".to_string(), |r| format!("{r} Hz")),
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
        let result =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| self.render_inner(feed)))
                .unwrap_or_else(|panic| {
                    let why = panic
                        .downcast_ref::<&str>()
                        .map(|s| s.to_string())
                        .or_else(|| panic.downcast_ref::<String>().cloned())
                        .unwrap_or_default();
                    Err(anyhow!("the driver stopped ({why})"))
                });
        match &result {
            Ok(()) => super::status_playing(None),
            Err(e) => super::status_failed(e.to_string()),
        }
        result
    }

    fn set_volume(&self, volume: f32) {
        self.volume
            .store(volume.clamp(0., 1.).to_bits(), Ordering::Relaxed);
        self.apply_volume();
    }
}

#[cfg(test)]
mod tests {
    use super::super::fake::{FakeDac, Trouble, descriptors, uac1_descriptors};
    use super::*;

    fn dac(trouble: Trouble, raw: &[u8]) -> (Arc<FakeDac>, UsbDac) {
        let fake = Arc::new(FakeDac::new(trouble));
        let dac = UsbDac::new(fake.clone(), raw, "Pretend DAC".into()).unwrap();
        (fake, dac)
    }

    fn first(dac: &UsbDac) -> Format {
        dac.dac.formats.last().unwrap().clone()
    }

    #[test]
    fn a_normal_dac_takes_the_rate_and_says_so() {
        let (fake, dac) = dac(Trouble::None, &descriptors());
        assert_eq!(dac.set_rate(&first(&dac), 44_100).unwrap(), Some(44_100));
        assert_eq!(fake.rates(), [44_100]);
    }

    #[test]
    fn a_dac_that_ignores_the_rate_is_refused() {
        let (_, dac) = dac(Trouble::StaysAt(48_000), &descriptors());
        let f = first(&dac);
        let why = dac.set_rate(&f, 44_100).unwrap_err().to_string();
        assert_eq!(
            why,
            "Pretend DAC stayed at 48 kHz, so it cannot play 44.1 kHz bit-perfect"
        );
        // The rate it runs at is fine.
        assert_eq!(dac.set_rate(&f, 48_000).unwrap(), Some(48_000));
    }

    #[test]
    fn a_dac_that_cannot_say_goes_on_as_before() {
        let (fake, dac) = dac(Trouble::NoRateAnswer, &descriptors());
        assert_eq!(dac.set_rate(&first(&dac), 44_100).unwrap(), None);
        assert_eq!(fake.rates(), [44_100]);
    }

    #[test]
    fn a_fixed_clock_that_refuses_the_request_is_judged_by_what_it_says() {
        let (_, dac) = dac(Trouble::FixedAt(48_000), &descriptors());
        let f = first(&dac);
        assert_eq!(dac.set_rate(&f, 48_000).unwrap(), Some(48_000));
        assert!(dac.set_rate(&f, 44_100).is_err());
    }

    #[test]
    fn uac1_reads_the_rate_back_from_the_endpoint() {
        let raw = uac1_descriptors(&[(1, &[44_100, 48_000], true)]);
        let (_, good) = dac(Trouble::None, &raw);
        assert_eq!(good.set_rate(&first(&good), 44_100).unwrap(), Some(44_100));
        let (_, stuck) = dac(Trouble::StaysAt(48_000), &raw);
        let why = stuck.set_rate(&first(&stuck), 44_100).unwrap_err();
        assert!(why.to_string().contains("stayed at 48 kHz"), "{why}");
    }

    #[test]
    fn uac1_with_no_rate_control_and_one_rate_offers_that_rate_and_asks_nothing() {
        let raw = uac1_descriptors(&[(1, &[48_000], false)]);
        let (fake, dac) = dac(Trouble::None, &raw);
        assert!(!first(&dac).rate_control);
        assert_eq!(dac.info().rates, [48_000]);
        assert_eq!(dac.set_rate(&first(&dac), 48_000).unwrap(), None);
        assert!(
            fake.rates().is_empty(),
            "no request for a rate it cannot set"
        );
        assert!(dac.format_for(2, 44_100).is_err());
    }

    #[test]
    fn the_setting_is_chosen_by_the_rate_too() {
        let raw = uac1_descriptors(&[(1, &[44_100], false), (2, &[48_000], false)]);
        let (_, dac) = dac(Trouble::None, &raw);
        assert_eq!(dac.info().rates, [44_100, 48_000]);
        assert_eq!(dac.format_for(2, 44_100).unwrap().alt, 1);
        assert_eq!(dac.format_for(2, 48_000).unwrap().alt, 2);
        let why = dac.format_for(2, 96_000).unwrap_err().to_string();
        assert!(why.contains("does not play 96000 Hz"), "{why}");
        let why = dac.format_for(6, 48_000).unwrap_err().to_string();
        assert!(why.contains("no 6-channel music"), "{why}");
    }
}
