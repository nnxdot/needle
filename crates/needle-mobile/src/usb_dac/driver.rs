//! The driver: the music from the core's feed to the DAC, through a [`Bus`]. Any trouble ends
//! `render` with an error at once (a DAC gone, refusing, or no longer taking music within half
//! a second; even a panic), so the core moves the music to Android's output with a notice.
use super::{UsbDacInfo, bus::Bus, descriptors::*, details, packets::*};
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
    /// The volume control's range in 1/256 dB, and the last volume asked (f32 bits): the
    /// player's own (its fades), and the phone's, which Android keeps.
    volume_range: Option<(i16, i16)>,
    volume: AtomicU32,
    phone_volume: AtomicU32,
    /// What it plays with now, and what the DAC said of the rate, for a report.
    chosen: Mutex<Option<String>>,
    /// Whether the DAC is muted now, when that is known.
    muted: Mutex<Option<bool>>,
    /// What happened as it opened, for a report.
    opened: String,
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
    /// Takes the DAC in the configuration it runs now. `android_rates` are the rates Android
    /// lists for it, for a clock that does not say its own.
    pub fn new(
        bus: Arc<dyn Bus>,
        raw: &[u8],
        name: String,
        phone_volume: f32,
        android_rates: &[u32],
    ) -> Result<Self> {
        // What happened, for "Copy DAC details", even when it does not open.
        let mut notes = vec![];
        let mut active = [0u8];
        let active = match bus.control(0x80, 0x08, 0, 0, &mut active) {
            Ok(_) if active[0] > 0 => {
                notes.push(format!("Active configuration: {}", active[0]));
                Some(active[0])
            }
            outcome => {
                let guess = likely_configuration(raw);
                notes.push(format!(
                    "Active configuration: the device did not say ({}); taking {guess:?}",
                    outcome.err().map_or("0".to_string(), |e| e.to_string()),
                ));
                guess
            }
        };
        let Some(dac) = parse_active(raw, active) else {
            super::remember_open(notes.join("\n"));
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
            phone_volume: AtomicU32::new(phone_volume.clamp(0., 1.).to_bits()),
            chosen: Mutex::new(None),
            muted: Mutex::new(None),
            opened: String::new(),
            control: Mutex::new(()),
            dac,
        };
        out.wake(&mut notes);
        out.read_channels(&mut notes);
        if out.dac.uac != Uac::One {
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
                let rates = out.rates_of_clock(clock, android_rates, &mut notes);
                out.clock_rates.push((clock, rates));
            }
        }
        // Just plugged in, a DAC may not answer yet: the app tries again.
        if out.dac.formats.iter().all(|f| out.rates(f).is_empty()) {
            super::remember_open(notes.join("\n"));
            bail!("{} did not say which rates it plays", out.name);
        }
        out.volume_range = out.read_volume_range();
        out.opened = notes.join("\n");
        super::remember_open(out.opened.clone());
        out.log_what_it_says();
        Ok(out)
    }

    /// UAC3: the way's power domain awake before anything else, as Linux has it before a stream
    /// starts. Its own driver leaves it idle (D1), so it may answer little until then.
    fn wake(&self, notes: &mut Vec<String>) {
        let Some(power) = self.dac.power else {
            return;
        };
        const POWER_DOMAIN: u16 = 0x0200;
        let index = self.index(power.id);
        let mut state = [0u8];
        let was = self.bus.control(GET, CUR, POWER_DOMAIN, index, &mut state);
        if let Err(e) = &was {
            notes.push(format!(
                "Power domain {}: its state cannot be read ({e})",
                power.id
            ));
        }
        if was.is_ok() && state[0] == 0 {
            notes.push(format!("Power domain {}: awake (D0)", power.id));
            return;
        }
        let from = state[0];
        let mut awake = [0u8];
        match self.bus.control(SET, CUR, POWER_DOMAIN, index, &mut awake) {
            Ok(_) => {
                notes.push(format!("Power domain {}: D{from} to D0", power.id));
                let wait = if from == 2 {
                    power.from_d2
                } else {
                    power.from_d1
                };
                std::thread::sleep(Duration::from_micros(wait as u64 * 50));
            }
            Err(e) => notes.push(format!("Power domain {}: D0 refused ({e})", power.id)),
        }
    }

    /// UAC3: the channels of each format, from its cluster descriptor. As Linux asks; the
    /// descriptor's seventh byte is the count.
    fn read_channels(&mut self, notes: &mut Vec<String>) {
        const HIGH_CAPABILITY: u8 = 0x06;
        let control = self.dac.control as u16;
        for f in self.dac.formats.iter_mut() {
            let Some(cluster) = f.cluster else {
                continue;
            };
            let mut head = [0u8; 7];
            match self
                .bus
                .control(GET, HIGH_CAPABILITY, cluster, control, &mut head)
            {
                Ok(7) if head[6] > 0 => f.channels = head[6],
                outcome => notes.push(format!(
                    "Cluster {cluster:#06x}: no answer ({:?}), taken as stereo",
                    outcome.map_err(|e| e.to_string())
                )),
            }
        }
        let chosen = self.dac.formats.clone();
        self.dac.paths[self.dac.chosen].formats = chosen;
    }

    /// A clock's rates: from itself, else its rate now and the rates Android lists, each proved
    /// by the read-back when it is played.
    fn rates_of_clock(&self, clock: u8, android: &[u32], notes: &mut Vec<String>) -> Vec<u32> {
        match self.clock_range(clock) {
            Ok(rates) if !rates.is_empty() => return rates,
            Ok(_) => notes.push(format!("Clock {clock}: RANGE gave no rates")),
            Err(e) => notes.push(format!("Clock {clock}: RANGE failed ({e:#})")),
        }
        let mut now = [0u8; 4];
        let said = self
            .bus
            .control(GET, CUR, wvalue(0x01, 0), self.index(clock), &mut now);
        let mut rates = android.to_vec();
        match said {
            Ok(4) if u32::from_le_bytes(now) > 0 => {
                notes.push(format!(
                    "Clock {clock}: runs at {} Hz now",
                    u32::from_le_bytes(now)
                ));
                rates.push(u32::from_le_bytes(now));
            }
            Ok(n) => notes.push(format!("Clock {clock}: its rate now came as {n} bytes")),
            Err(e) => notes.push(format!(
                "Clock {clock}: its rate now cannot be read ({e:#})"
            )),
        }
        notes.push(format!("Android lists {android:?}"));
        rates.sort_unstable();
        rates.dedup();
        rates
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
            self.dac.uac.number(),
            if self.high_speed { "high" } else { "full" },
            if self.volume_range.is_some() {
                "yes"
            } else {
                "no"
            },
        ));
        for line in details::paths(&self.dac) {
            say(format!("{}: {}", self.name, line.trim_end()));
        }
        for f in &self.dac.formats {
            say(format!(
                "{}: setting {} plays {:?}",
                self.name,
                f.alt,
                self.rates(f)
            ));
        }
    }

    /// What only the running driver knows, for a report.
    pub fn details(&self) -> String {
        let mut text = format!(
            "The driver sends {} speed packets\n",
            if self.high_speed { "high" } else { "full" },
        );
        for (clock, rates) in &self.clock_rates {
            text += &format!("Clock {clock} says it plays {rates:?}\n");
        }
        for f in &self.dac.formats {
            text += &format!("Setting {} plays {:?}\n", f.alt, self.rates(f));
        }
        text += &match self.volume_range {
            Some((min, max)) => format!(
                "Volume: the DAC's own, {:.1} to {:.1} dB\n",
                min as f32 / 256.,
                max as f32 / 256.
            ),
            None => "Volume: Needle scales the samples; the DAC has none it can set\n".into(),
        };
        text += &format!("Volume now: {:.0} %\n", self.loudness() * 100.);
        let chosen = self.chosen.lock().unwrap_or_else(|e| e.into_inner());
        text += &format!(
            "Chosen: {}\n",
            chosen
                .as_deref()
                .unwrap_or("nothing yet: play a song first")
        );
        text
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
            uac: self.dac.uac.number(),
            rates,
            bits: self.dac.formats.iter().map(|f| f.bits).max().unwrap_or(16),
            dop: self.takes_dop(),
            volume: self.volume_range.is_some(),
        }
    }

    fn rates(&self, f: &Format) -> Vec<u32> {
        if self.dac.uac == Uac::One || !f.rates.is_empty() {
            return listed_rates(f);
        }
        self.dac
            .clock_for(f.terminal)
            .and_then(|c| self.clock_rates.iter().find(|(id, _)| *id == c))
            .map(|(_, r)| r.clone())
            .unwrap_or_default()
    }

    /// DoP cannot be scaled, so a DAC with no volume of its own gets DSD as PCM: else DSD would
    /// be the one thing whose volume cannot change.
    fn takes_dop(&self) -> bool {
        self.volume_range.is_some()
            && self.dac.formats.iter().any(|f| {
                f.bits >= 24 && f.channels == 2 && self.rates(f).iter().any(|r| *r >= 176_400)
            })
    }

    /// Needle's volume for this DAC, 0 to 1: the player's and the phone's together.
    fn loudness(&self) -> f32 {
        let (player, phone) = (
            f32::from_bits(self.volume.load(Ordering::Relaxed)),
            f32::from_bits(self.phone_volume.load(Ordering::Relaxed)),
        );
        player * phone
    }

    /// What the samples are scaled by: 1 with a DAC that has its own volume.
    fn gain(&self) -> f32 {
        match self.volume_range {
            Some(_) => 1.,
            None => software_gain(self.loudness()),
        }
    }

    /// The samples reach the DAC as they are.
    pub fn bit_perfect(&self) -> bool {
        self.gain() >= 1.
    }

    /// The phone's music volume moved (0 to 1).
    pub fn set_phone_volume(&self, volume: f32) {
        self.phone_volume
            .store(volume.clamp(0., 1.).to_bits(), Ordering::Relaxed);
        self.apply_volume();
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
            Uac::Two | Uac::Three => {
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
        let steps = volume_steps(self.loudness(), min, max);
        let _one = self.one_at_a_time();
        // 0 is real silence: the bottom of a narrow range can still be heard. And a DAC found
        // muted plays. Only when that changes, as this runs through every fade.
        let silent = self.loudness() <= 0.001;
        let mut muted = self.muted.lock().unwrap_or_else(|e| e.into_inner());
        if v.mute && *muted != Some(silent) {
            *muted = Some(silent);
            let mut data = [silent as u8];
            let _ = self
                .bus
                .control(SET, CUR, wvalue(0x01, 0), self.index(v.unit), &mut data);
        }
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
            Uac::Two | Uac::Three => (GET, CUR, self.index(self.dac.clock_for(f.terminal)?), 4),
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

    fn stayed_at(&self, now: u32, rate: u32) -> String {
        format!(
            "{} stayed at {} kHz, so it cannot play {} kHz bit-perfect",
            self.name,
            khz(now),
            khz(rate)
        )
    }

    /// Sets the DAC's rate for `f`, and reads it back: the rate the DAC said it took, or None
    /// when it cannot say. A DAC that stays at another rate is an error: 44.1 kHz music on a
    /// 48 kHz clock is 9 % fast and high where the DAC sets the pace, and has gaps in every packet
    /// where the phone does, at the right pitch.
    fn set_rate(&self, f: &Format, rate: u32) -> Result<Option<u32>> {
        let _one = self.one_at_a_time();
        // No sampling frequency control and one rate (UAC1, a UAC3 BADD profile): the setting is
        // the rate. One that listed more was left with the one it is trusted for: no request,
        // but it may say.
        if !f.rate_control && listed_rates(f).len() <= 1 {
            let said = if f.rates.len() > 1 || f.range.is_some() {
                self.read_rate(f)
            } else {
                None
            };
            if let Some(now) = said.filter(|now| !same_rate(*now, rate)) {
                bail!(self.stayed_at(now, rate));
            }
            return Ok(said);
        }
        let set = match self.dac.uac {
            Uac::Two | Uac::Three => {
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
            bail!(self.stayed_at(now, rate));
        }
        // A range of rates and no answer: nothing shows that it left its own clock.
        if f.range.is_some() && rate != 48_000 {
            bail!(
                "{} gives a range of rates and cannot say which it plays, so {} kHz cannot be trusted",
                self.name,
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
        let in_a_range = |f: &Format| {
            f.range
                .is_some_and(|(low, high)| (low..=high).contains(&rate))
        };
        if self.dac.formats.iter().any(in_a_range) {
            bail!(
                "{} gives a range of rates, so {} kHz cannot be trusted",
                self.name,
                khz(rate)
            );
        }
        let listed_only = |f: &Format| !f.rate_control && f.rates.contains(&rate);
        if self.dac.formats.iter().any(listed_only) {
            bail!(
                "{} cannot be told its rate, so {} kHz cannot be trusted",
                self.name,
                khz(rate)
            );
        }
        let mut all: Vec<u32> = self
            .dac
            .formats
            .iter()
            .flat_map(|f| self.rates(f))
            .collect();
        all.sort_unstable();
        all.dedup();
        if let [only] = all[..] {
            bail!(
                "{} plays only {} kHz, so a {} kHz song cannot be bit-perfect",
                self.name,
                khz(only),
                khz(rate)
            );
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
        if !rate_fits(format.rate, per_second, frame, limit) {
            bail!("{} Hz is too fast for this setting of the DAC", format.rate);
        }
        // The most frames a packet may carry; a DAC's feedback cannot ask for more.
        let most = limit / frame;
        let mut pacer = Pacer::new(format.rate, per_second);
        let feedback_endpoint = f.feedback.filter(|_| f.sync == Sync::Async);
        let feedback_size = if self.high_speed { 4 } else { 3 };
        let channels = format.channels as usize;
        let mut samples = vec![0f32; most * channels];
        let mut bytes = vec![0u8; per_transfer * limit];
        let mut gain = self.gain();
        // Fill one transfer's packets from the feed; false once the music in this format ends.
        let mut fill =
            |pacer: &mut Pacer, feed: &mut Feed, bytes: &mut [u8]| -> (Vec<usize>, usize, bool) {
                let mut lengths = Vec::with_capacity(per_transfer);
                let mut at = 0;
                let mut more = true;
                for _ in 0..per_transfer {
                    let wanted = pacer.next().min(most) * channels;
                    let got = if more {
                        feed.read(&mut samples[..wanted])
                    } else {
                        0
                    };
                    if got < wanted {
                        samples[got..wanted].fill(0.);
                        more = false;
                    }
                    // With no volume of its own: never at full volume against the listener's
                    // will, and bit-perfect only at 100 %.
                    let wanted_gain = self.gain();
                    if gain < 1. || wanted_gain < 1. {
                        ramp(&mut samples[..wanted], channels, gain, wanted_gain);
                    }
                    gain = wanted_gain;
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
                let chosen = format!(
                    "{} Hz, {} bits in {}-byte slots, {} channels (setting {} of interface {}), the DAC says {}",
                    format.rate,
                    f.bits,
                    f.subslot,
                    f.channels,
                    f.alt,
                    f.interface,
                    said.map_or("nothing".to_string(), |r| format!("{r} Hz")),
                );
                needle_core::logfile::warn(format!("{}: {chosen}", self.name));
                *self.chosen.lock().unwrap_or_else(|e| e.into_inner()) = Some(chosen);
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
    use super::super::fake::{
        APPLE_DAC, DOCK, Dock, FakeDac, Trouble, descriptors, dock, hex, uac1_descriptors,
    };
    use super::*;

    fn dac(trouble: Trouble, raw: &[u8]) -> (Arc<FakeDac>, UsbDac) {
        let fake = Arc::new(FakeDac::new(trouble));
        let dac = UsbDac::new(fake.clone(), raw, "Pretend DAC".into(), 1., &[]).unwrap();
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
    fn a_dac_that_does_not_answer_yet_is_an_error_to_try_again() {
        let fake = Arc::new(FakeDac::new(Trouble::NoClockAnswer));
        let raw = descriptors();
        let why = UsbDac::new(fake, &raw, "Pretend DAC".into(), 1., &[])
            .err()
            .unwrap();
        assert_eq!(
            why.to_string(),
            "Pretend DAC did not say which rates it plays"
        );
    }

    fn in_configuration(
        config: u8,
        trouble: Trouble,
        android: &[u32],
    ) -> (Arc<FakeDac>, Result<UsbDac>) {
        let fake = Arc::new(FakeDac::in_configuration(trouble, config));
        let raw = hex(APPLE_DAC);
        let dac = UsbDac::new(fake.clone(), &raw, "Adapter".into(), 1., android);
        (fake, dac)
    }

    #[test]
    fn apples_adapter_in_its_badd_configuration_plays_48_khz_and_asks_nothing_of_a_clock() {
        // Its clock answers nothing, as the real one did not.
        let (fake, dac) = in_configuration(2, Trouble::NoClockAnswer, &[]);
        let dac = dac.unwrap();
        assert_eq!((dac.info().uac, dac.info().rates), (3, vec![48_000]));
        assert_eq!(dac.info().bits, 24);
        // Awake before any setting is chosen, as Linux has it.
        assert_eq!(fake.power(), Some((10, 0)));
        assert!(fake.alts().is_empty());
        let f = dac.format_for(2, 48_000).unwrap();
        assert_eq!((f.alt, f.subslot, f.bits), (1, 3, 24));
        assert_eq!(dac.set_rate(&f, 48_000).unwrap(), None);
        assert!(fake.rates().is_empty(), "no request for a fixed rate");
        // Packets of the real adapter's size: 288 bytes is 48 frames.
        let why = dac.format_for(2, 44_100).unwrap_err().to_string();
        assert_eq!(
            why,
            "Adapter plays only 48 kHz, so a 44.1 kHz song cannot be bit-perfect"
        );
    }

    #[test]
    fn apples_adapter_in_its_uac2_configuration_with_a_silent_clock_uses_androids_rates() {
        // As the real report: RANGE stalls; Android lists 48 kHz.
        let (fake, dac) = in_configuration(1, Trouble::NoClockAnswer, &[48_000]);
        let dac = dac.unwrap();
        assert_eq!(dac.info().rates, [48_000]);
        assert!(fake.power().is_none(), "UAC2 has no power domains");
        // The read-back still judges it.
        let f = dac.format_for(2, 48_000).unwrap();
        assert_eq!(dac.set_rate(&f, 48_000).unwrap(), Some(48_000));
        let notes = &dac.opened;
        assert!(notes.contains("Active configuration: 1"), "{notes}");
        assert!(
            notes.contains("Clock 9: RANGE failed (The DAC refused the request)"),
            "{notes}"
        );
    }

    #[test]
    fn a_clock_that_says_only_its_rate_now_plays_that_rate() {
        let fake = Arc::new(FakeDac::in_configuration(Trouble::NoClockAnswer, 1));
        fake.set_clock(48_000);
        let raw = hex(APPLE_DAC);
        let dac = UsbDac::new(fake, &raw, "Adapter".into(), 1., &[]).unwrap();
        assert_eq!(dac.info().rates, [48_000]);
    }

    #[test]
    fn with_no_rates_from_anywhere_it_is_refused_and_says_why() {
        let (_, dac) = in_configuration(1, Trouble::NoClockAnswer, &[]);
        let why = dac.err().unwrap().to_string();
        assert_eq!(why, "Adapter did not say which rates it plays");
    }

    #[test]
    fn apples_adapter_in_its_full_uac3_configuration_wakes_and_asks_its_clock() {
        let (fake, dac) = in_configuration(3, Trouble::None, &[]);
        let dac = dac.unwrap();
        assert_eq!(fake.power(), Some((0x28, 0)));
        assert_eq!(
            (dac.info().uac, dac.info().rates),
            (3, vec![44_100, 48_000, 96_000])
        );
        let f = dac.format_for(2, 44_100).unwrap();
        assert_eq!((f.alt, f.channels), (1, 2));
        assert_eq!(dac.set_rate(&f, 44_100).unwrap(), Some(44_100));
    }

    #[test]
    fn a_power_domain_already_awake_is_left_alone() {
        let fake = Arc::new(FakeDac::in_configuration(Trouble::None, 2));
        fake.set_power_state(0);
        let raw = hex(APPLE_DAC);
        UsbDac::new(fake.clone(), &raw, "Adapter".into(), 1., &[]).unwrap();
        assert_eq!(fake.power(), None);
    }

    #[test]
    fn the_real_dock_uses_its_speakers_volume_by_channel_and_mutes_at_0() {
        let fake = Arc::new(FakeDac::new(Trouble::None));
        fake.set_volume_floor(-45);
        let raw = hex(DOCK);
        let dock = UsbDac::new(fake.clone(), &raw, "Dock".into(), 1., &[]).unwrap();
        assert!(dock.info().volume);
        dock.set_phone_volume(1. / 16.);
        let (unit, steps) = fake.volume().unwrap();
        assert_eq!(unit, 6);
        assert!((-31 * 256..-29 * 256).contains(&(steps as i32)), "{steps}");
        assert_eq!(fake.muted(), Some(false));
        dock.set_phone_volume(0.);
        assert_eq!(fake.muted(), Some(true));
        dock.set_phone_volume(0.5);
        assert_eq!(fake.muted(), Some(false));
    }

    #[test]
    fn a_dock_plays_through_its_speakers_not_its_headset() {
        let (_, dock) = dac(Trouble::None, &dock(Dock::default()));
        // The speakers' interface and formats only: not the headset's before it.
        assert_eq!(dock.dac.paths.len(), 2);
        let f = dock.format_for(2, 48_000).unwrap();
        assert_eq!((f.interface, f.alt, f.bits), (3, 2, 24));
        let f = dock.format_for(2, 44_100).unwrap();
        assert_eq!((f.interface, f.alt, f.bits), (3, 1, 16));
        // Not the microphone's volume, which comes first, nor the headset's.
        assert_eq!(dock.dac.volume.map(|v| v.unit), Some(22));
        // A rate only the headset lists is not played by the speakers.
        assert!(dock.format_for(2, 8_000).is_err());
    }

    #[test]
    fn a_range_of_rates_plays_48_khz_and_refuses_what_it_cannot_prove() {
        let raw = dock(Dock {
            speakers: false,
            ..Dock::default()
        });
        let (_, headset) = dac(Trouble::None, &raw);
        assert_eq!(headset.info().rates, [48_000]);
        let why = headset.format_for(2, 44_100).unwrap_err().to_string();
        assert_eq!(
            why,
            "Pretend DAC gives a range of rates, so 44.1 kHz cannot be trusted"
        );
        assert_eq!(headset.format_for(2, 48_000).unwrap().alt, 1);
    }

    #[test]
    fn a_range_that_cannot_say_its_rate_is_refused() {
        let raw = dock(Dock {
            speakers: false,
            ..Dock::default()
        });
        let (_, headset) = dac(Trouble::NoRateAnswer, &raw);
        let mut f = headset.format_for(2, 48_000).unwrap();
        assert_eq!(headset.set_rate(&f, 48_000).unwrap(), None);
        f.range = Some((8_000, 44_100));
        let why = headset.set_rate(&f, 44_100).unwrap_err().to_string();
        assert!(why.contains("cannot say which it plays"), "{why}");
    }

    #[test]
    fn with_no_volume_of_its_own_needle_scales_the_samples() {
        let raw = dock(Dock {
            volume: false,
            ..Dock::default()
        });
        let (_, dac) = dac(Trouble::None, &raw);
        assert!(!dac.info().volume);
        assert!(dac.bit_perfect());
        dac.set_phone_volume(0.5);
        let half = dac.gain();
        assert!((0.3..0.6).contains(&half), "{half}");
        assert!(!dac.bit_perfect());
        // The player's own fade counts too.
        dac.set_volume(0.5);
        assert!(dac.gain() < half);
        dac.set_phone_volume(1.);
        dac.set_volume(1.);
        assert!(dac.bit_perfect());
    }

    #[test]
    fn the_phones_volume_reaches_a_dacs_own_volume_and_leaves_the_samples_alone() {
        let (fake, dac) = dac(Trouble::None, &dock(Dock::default()));
        assert!(dac.info().volume);
        dac.set_phone_volume(0.5);
        // The speakers' feature unit, at about -12 dB of its 0 dB top.
        let (unit, steps) = fake.volume().unwrap();
        assert_eq!(unit, 22);
        assert!((-13 * 256..-11 * 256).contains(&(steps as i32)), "{steps}");
        assert_eq!(fake.muted(), Some(false), "a DAC found muted plays");
        assert_eq!(dac.gain(), 1.);
        assert!(dac.bit_perfect());
    }

    #[test]
    fn dsd_is_not_sent_as_dop_when_only_scaling_can_change_the_volume() {
        let plain = uac1_descriptors(&[(1, &[192_000], true)]);
        assert!(!dac(Trouble::None, &plain).1.dop());
        // The same DAC with a feature unit after its header, with a volume.
        let at = plain.windows(3).position(|w| w == [9, 0x24, 1]).unwrap() + 9;
        let mut with_volume = plain.clone();
        with_volume.splice(at..at, [9, 0x24, 6, 2, 1, 1, 0x03, 0, 0]);
        assert!(dac(Trouble::None, &with_volume).1.dop());
    }

    #[test]
    fn a_setting_that_cannot_be_told_its_rate_keeps_one_of_those_it_lists() {
        let raw = uac1_descriptors(&[(1, &[44_100, 48_000], false)]);
        let (fake, told) = dac(Trouble::None, &raw);
        assert_eq!(told.info().rates, [48_000]);
        let why = told.format_for(2, 44_100).unwrap_err().to_string();
        assert_eq!(
            why,
            "Pretend DAC cannot be told its rate, so 44.1 kHz cannot be trusted"
        );
        let f = told.format_for(2, 48_000).unwrap();
        assert_eq!(told.set_rate(&f, 48_000).unwrap(), None);
        assert!(
            fake.rates().is_empty(),
            "no request for a rate it cannot set"
        );

        // It says it runs at another rate: refused too.
        let (_, other) = dac(Trouble::StaysAt(44_100), &raw);
        let why = other.set_rate(&f, 48_000).unwrap_err().to_string();
        assert!(why.contains("stayed at 44.1 kHz"), "{why}");

        // With no 48 kHz among them, its highest.
        let raw = uac1_descriptors(&[(1, &[32_000, 44_100], false)]);
        assert_eq!(dac(Trouble::None, &raw).1.info().rates, [44_100]);

        // Another setting that can be told the rate still plays it.
        let raw = uac1_descriptors(&[(1, &[44_100, 48_000], false), (2, &[44_100], true)]);
        let (_, both) = dac(Trouble::None, &raw);
        assert_eq!(both.format_for(2, 44_100).unwrap().alt, 2);
        assert_eq!(both.format_for(2, 48_000).unwrap().alt, 1);
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
