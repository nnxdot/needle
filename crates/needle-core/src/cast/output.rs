//! A speaker as an audio output: Needle's player writes into a mixer as it would for a sound
//! card, and a pump takes the mixed audio at real-time speed and sends it to the speaker.
use super::{Kind, Meta, Remote, Speaker, http};
use anyhow::{Context, Result};
use rodio::Sink;
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

pub const RATE: u32 = 44_100;
pub const CHANNELS: u16 = 2;
/// How far ahead of real time the pump runs, so small hiccups do not starve the speaker.
const LEAD: f64 = 0.4;

/// Where the pump's audio goes.
pub trait Destination: Send {
    /// Start (or start again after a flush) with this description of what plays.
    fn start(&mut self, meta: &Meta) -> Result<()>;
    /// 16-bit interleaved samples.
    fn push(&mut self, samples: &[i16]) -> Result<()>;
    fn pause(&mut self) -> Result<()>;
    fn resume(&mut self) -> Result<()>;
    /// Drop what the speaker holds and start fresh, after a seek or a jump.
    fn flush(&mut self, meta: &Meta) -> Result<()>;
    fn tick(&mut self) -> Result<()>;
    fn stop(&mut self);
    /// Roughly how many seconds the speaker plays behind what was sent.
    fn lag(&self) -> f64;
    /// New extra delays for the members of a group, in milliseconds by address.
    fn set_delays(&mut self, _delays: &std::collections::BTreeMap<String, i32>) {}
}

/// Shared between the player and the pump.
#[derive(Default)]
pub struct Control {
    flush: AtomicBool,
    stop: AtomicBool,
    meta: Mutex<Meta>,
    described: AtomicBool,
    lag: Mutex<f64>,
    delays: Mutex<Option<std::collections::BTreeMap<String, i32>>>,
}

impl Control {
    /// Make the speaker drop what it has buffered (after a seek).
    pub fn flush(&self) {
        self.flush.store(true, Ordering::Relaxed);
    }
    /// Describe what is playing now; the speaker shows it from its next start.
    pub fn set_meta(&self, meta: Meta) {
        *self.meta.lock().unwrap() = meta;
        self.described.store(true, Ordering::Relaxed);
    }
    /// Seconds the speaker plays behind the player.
    pub fn lag(&self) -> f64 {
        *self.lag.lock().unwrap()
    }
    /// Line up the members of a group again, while playing.
    pub fn set_delays(&self, delays: std::collections::BTreeMap<String, i32>) {
        *self.delays.lock().unwrap() = Some(delays);
    }
}

/// Stops the pump when the output is dropped.
pub struct Guard(Arc<Control>);
impl Drop for Guard {
    fn drop(&mut self) {
        self.0.stop.store(true, Ordering::Relaxed);
    }
}

pub struct Network {
    pub sink: Arc<Sink>,
    pub name: String,
    pub rate: u32,
    pub channels: u16,
    pub failure: Arc<Mutex<Option<String>>>,
    pub guard: Guard,
    pub control: Arc<Control>,
}

// ---------------------------------------------------------------- URL speakers

/// DLNA and Chromecast: the speaker fetches a live WAV stream from Needle's server.
struct UrlSpeaker {
    speaker: Speaker,
    remote: Option<Box<dyn Remote>>,
    server: Arc<http::Server>,
    host: std::net::IpAddr,
    stream: Option<(String, Arc<http::Stream>)>,
    serial: u64,
    session: u64,
    bytes: Vec<u8>,
}

/// Connections parked between outputs: speaker address → (parking number, connection).
type Parked = Mutex<std::collections::HashMap<String, (u64, Box<dyn Remote>)>>;
static PARKED: std::sync::LazyLock<Parked> = std::sync::LazyLock::new(Default::default);

fn park(address: String, remote: Box<dyn Remote>) {
    let ticket: u64 = rand::random();
    PARKED
        .lock()
        .unwrap()
        .insert(address.clone(), (ticket, remote));
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(4));
        let mut parked = PARKED.lock().unwrap();
        if parked.get(&address).is_some_and(|(t, _)| *t == ticket)
            && let Some((_, mut remote)) = parked.remove(&address)
        {
            drop(parked);
            let _ = remote.stop();
        }
    });
}

impl UrlSpeaker {
    fn new(speaker: &Speaker) -> Result<Self> {
        let peer = speaker
            .socket()
            .context("The speaker's address is not valid")?;
        Ok(Self {
            speaker: speaker.clone(),
            remote: None,
            server: http::server()?,
            host: super::local_ip_for(peer)?,
            stream: None,
            serial: 0,
            session: rand::random::<u32>() as u64,
            bytes: vec![],
        })
    }
    fn open(&mut self, meta: &Meta) -> Result<()> {
        if let Some((id, _)) = self.stream.take() {
            self.server.close_stream(&id);
        }
        self.serial += 1;
        let id = format!("{:x}-{}", self.session, self.serial);
        let stream = self.server.open_stream(&id, RATE, CHANNELS);
        self.stream = Some((id.clone(), stream));
        let url = format!("http://{}:{}/live/{id}.wav", self.host, self.server.port);
        let mut meta = meta.clone();
        // Covers are offered as paths on this computer; the speaker needs a URL.
        if let Some(path) = meta.cover.take().filter(|p| !p.starts_with("http")) {
            let shared = self.server.share_cover(path.into());
            meta.cover = Some(format!("http://{}:{}{shared}", self.host, self.server.port));
        }
        if self.remote.is_none() {
            let parked = PARKED
                .lock()
                .unwrap()
                .remove(&self.speaker.address)
                .map(|(_, r)| r);
            self.remote = Some(match parked {
                Some(remote) => remote,
                None => super::connect(&self.speaker)?,
            });
        }
        let loaded = self.remote.as_mut().unwrap().load(&url, &meta);
        if loaded.is_err() {
            // A parked connection may have gone stale; try once with a fresh one.
            self.remote = Some(super::connect(&self.speaker)?);
            return self.remote.as_mut().unwrap().load(&url, &meta);
        }
        loaded
    }
}

impl Destination for UrlSpeaker {
    fn start(&mut self, meta: &Meta) -> Result<()> {
        self.open(meta)
    }
    fn push(&mut self, samples: &[i16]) -> Result<()> {
        if let Some((_, stream)) = &self.stream {
            self.bytes.clear();
            self.bytes
                .extend(samples.iter().flat_map(|s| s.to_le_bytes()));
            stream.push(&self.bytes);
        }
        Ok(())
    }
    fn pause(&mut self) -> Result<()> {
        self.remote.as_mut().map_or(Ok(()), |r| r.pause())
    }
    fn resume(&mut self) -> Result<()> {
        self.remote.as_mut().map_or(Ok(()), |r| r.resume())
    }
    fn flush(&mut self, meta: &Meta) -> Result<()> {
        self.open(meta)
    }
    fn tick(&mut self) -> Result<()> {
        self.remote.as_mut().map_or(Ok(()), |r| r.tick())
    }
    fn stop(&mut self) {
        if let Some(remote) = self.remote.take() {
            park(self.speaker.address.clone(), remote);
        }
        if let Some((id, _)) = self.stream.take() {
            self.server.close_stream(&id);
        }
    }
    fn lag(&self) -> f64 {
        LEAD + match self.speaker.kind {
            Kind::Chromecast => 1.5,
            _ => 2.0,
        }
    }
}

// ---------------------------------------------------------------- this computer

/// This computer's own speakers as a member of a group: it plays what the pump sends, from
/// a small buffer, so it can wait for the network speakers.
struct LocalSpeaker {
    shared: Arc<LocalShared>,
}

#[derive(Default)]
struct LocalShared {
    buffer: Mutex<std::collections::VecDeque<f32>>,
    paused: AtomicBool,
    stop: AtomicBool,
    failure: Mutex<Option<String>>,
}

struct LocalSource {
    shared: Arc<LocalShared>,
    chunk: Vec<f32>,
    at: usize,
}

impl Iterator for LocalSource {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        if self.shared.stop.load(Ordering::Relaxed) {
            return None;
        }
        if self.shared.paused.load(Ordering::Relaxed) {
            return Some(0.);
        }
        if self.at >= self.chunk.len() {
            self.chunk.clear();
            self.at = 0;
            let mut buffer = self.shared.buffer.lock().unwrap();
            let take = buffer.len().min(1024);
            self.chunk.extend(buffer.drain(..take));
            if self.chunk.is_empty() {
                return Some(0.);
            }
        }
        self.at += 1;
        Some(self.chunk[self.at - 1])
    }
}

impl rodio::Source for LocalSource {
    fn current_span_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> u16 {
        CHANNELS
    }
    fn sample_rate(&self) -> u32 {
        RATE
    }
    fn total_duration(&self) -> Option<Duration> {
        None
    }
}

impl LocalSpeaker {
    fn new() -> Self {
        Self {
            shared: Arc::default(),
        }
    }
}

impl Destination for LocalSpeaker {
    fn start(&mut self, _meta: &Meta) -> Result<()> {
        let shared = self.shared.clone();
        std::thread::Builder::new()
            .name("needle-local-speaker".into())
            .spawn(move || {
                let stream = match rodio::OutputStreamBuilder::open_default_stream() {
                    Ok(mut stream) => {
                        stream.log_on_drop(false);
                        stream
                    }
                    Err(error) => {
                        *shared.failure.lock().unwrap() = Some(format!("{error}"));
                        return;
                    }
                };
                stream.mixer().add(LocalSource {
                    shared: shared.clone(),
                    chunk: Vec::with_capacity(1024),
                    at: 0,
                });
                while !shared.stop.load(Ordering::Relaxed) {
                    std::thread::sleep(Duration::from_millis(50));
                }
                drop(stream);
            })
            .context("Could not start this computer's output")?;
        Ok(())
    }
    fn push(&mut self, samples: &[i16]) -> Result<()> {
        if let Some(error) = self.shared.failure.lock().unwrap().clone() {
            anyhow::bail!("This computer's output failed: {error}");
        }
        let mut buffer = self.shared.buffer.lock().unwrap();
        buffer.extend(samples.iter().map(|s| *s as f32 / 32768.));
        // Never hold more than ten seconds.
        let limit = RATE as usize * CHANNELS as usize * 10;
        if buffer.len() > limit {
            let extra = buffer.len() - limit;
            buffer.drain(..extra);
        }
        Ok(())
    }
    fn pause(&mut self) -> Result<()> {
        self.shared.paused.store(true, Ordering::Relaxed);
        Ok(())
    }
    fn resume(&mut self) -> Result<()> {
        self.shared.paused.store(false, Ordering::Relaxed);
        Ok(())
    }
    fn flush(&mut self, _meta: &Meta) -> Result<()> {
        self.shared.buffer.lock().unwrap().clear();
        Ok(())
    }
    fn tick(&mut self) -> Result<()> {
        Ok(())
    }
    fn stop(&mut self) {
        self.shared.stop.store(true, Ordering::Relaxed);
    }
    fn lag(&self) -> f64 {
        LEAD + 0.05
    }
}

// ---------------------------------------------------------------- groups

/// One output of a group.
struct Member {
    key: String,
    name: String,
    destination: Box<dyn Destination>,
    alive: bool,
    /// Frames of delay this member has now.
    delay: i64,
    /// Frames of silence to add (or, when negative, of sound to skip) before the next push.
    pending: i64,
}

/// Several outputs at once. Each gets extra delay so that all of them play in step with
/// the slowest, plus any delay the listener set to line them up by ear.
struct GroupOut {
    members: Vec<Member>,
    delays: std::collections::BTreeMap<String, i32>,
    silence: Vec<i16>,
}

impl GroupOut {
    /// Seconds each member should be delayed by.
    fn targets(&self) -> Vec<f64> {
        let alive = || self.members.iter().filter(|m| m.alive);
        let slowest = alive().map(|m| m.destination.lag()).fold(0., f64::max);
        let wanted: Vec<f64> = self
            .members
            .iter()
            .map(|m| {
                slowest - m.destination.lag()
                    + *self.delays.get(&m.key).unwrap_or(&0) as f64 / 1000.
            })
            .collect();
        // A negative delay for one member means everyone else waits a little longer.
        let shift = alive()
            .zip(wanted.iter())
            .map(|(_, w)| -w)
            .fold(0., f64::max);
        wanted.iter().map(|w| w + shift).collect()
    }
    /// Work out each member's delay again; `fresh` after the members dropped what they held.
    fn align(&mut self, fresh: bool) {
        let targets = self.targets();
        for (member, target) in self.members.iter_mut().zip(targets) {
            let frames = (target * RATE as f64).round() as i64;
            if fresh {
                member.delay = 0;
                member.pending = 0;
            }
            member.pending += frames - member.delay;
            member.delay = frames;
        }
    }
    /// Run `action` on every member still playing. A member that fails is dropped; the group
    /// fails only when none is left.
    fn each(&mut self, mut action: impl FnMut(&mut Member) -> Result<()>) -> Result<()> {
        let mut last = None;
        for member in self.members.iter_mut().filter(|m| m.alive) {
            if let Err(error) = action(member) {
                crate::logfile::warn(format!("{} stopped playing: {error:#}", member.name));
                member.alive = false;
                member.destination.stop();
                last = Some(error);
            }
        }
        // The others keep their timing: lining them up again would make them jump.
        match (self.members.iter().any(|m| m.alive), last) {
            (false, Some(error)) => Err(error),
            (false, None) => anyhow::bail!("No speaker in the group is playing"),
            _ => Ok(()),
        }
    }
}

impl Destination for GroupOut {
    fn start(&mut self, meta: &Meta) -> Result<()> {
        self.each(|m| m.destination.start(meta))?;
        self.align(true);
        Ok(())
    }
    fn push(&mut self, samples: &[i16]) -> Result<()> {
        let silence = &mut self.silence;
        let result = {
            let mut last = None;
            for member in self.members.iter_mut().filter(|m| m.alive) {
                let mut sound = samples;
                if member.pending > 0 {
                    let count = member.pending as usize * CHANNELS as usize;
                    if silence.len() < count {
                        silence.resize(count, 0);
                    }
                    if let Err(e) = member.destination.push(&silence[..count]) {
                        last = Some((member.name.clone(), e));
                        member.alive = false;
                        continue;
                    }
                    member.pending = 0;
                } else if member.pending < 0 {
                    let skip = ((-member.pending) as usize * CHANNELS as usize).min(sound.len());
                    sound = &sound[skip..];
                    member.pending += (skip / CHANNELS as usize) as i64;
                }
                if !sound.is_empty()
                    && let Err(e) = member.destination.push(sound)
                {
                    last = Some((member.name.clone(), e));
                    member.alive = false;
                }
            }
            last
        };
        if let Some((name, error)) = result {
            crate::logfile::warn(format!("{name} stopped playing: {error:#}"));
            for member in self.members.iter_mut().filter(|m| !m.alive) {
                member.destination.stop();
            }
            if !self.members.iter().any(|m| m.alive) {
                return Err(error);
            }
        }
        Ok(())
    }
    fn pause(&mut self) -> Result<()> {
        self.each(|m| m.destination.pause())
    }
    fn resume(&mut self) -> Result<()> {
        self.each(|m| m.destination.resume())
    }
    fn flush(&mut self, meta: &Meta) -> Result<()> {
        self.each(|m| m.destination.flush(meta))?;
        self.align(true);
        Ok(())
    }
    fn tick(&mut self) -> Result<()> {
        self.each(|m| m.destination.tick())
    }
    fn stop(&mut self) {
        for member in self.members.iter_mut().filter(|m| m.alive) {
            member.destination.stop();
        }
    }
    fn lag(&self) -> f64 {
        // When the members are heard, with the delays they have now.
        self.members
            .iter()
            .filter(|m| m.alive)
            .map(|m| m.destination.lag() + (m.delay - m.pending.min(0)) as f64 / RATE as f64)
            .fold(0., f64::max)
    }
    fn set_delays(&mut self, delays: &std::collections::BTreeMap<String, i32>) {
        self.delays = delays.clone();
        self.align(false);
    }
}

/// Open several speakers (and maybe this computer) as one output.
pub fn open_group(group: &super::Group) -> Result<Network> {
    let mut members = vec![];
    let mut problems = vec![];
    for speaker in &group.speakers {
        let destination: Result<Box<dyn Destination>> = match speaker.kind {
            Kind::AirPlay => super::airplay::Receiver::new(speaker).map(|r| Box::new(r) as _),
            _ => UrlSpeaker::new(speaker).map(|s| Box::new(s) as _),
        };
        match destination {
            Ok(destination) => members.push(Member {
                key: speaker.address.clone(),
                name: speaker.name.clone(),
                destination,
                alive: true,
                delay: 0,
                pending: 0,
            }),
            Err(error) => problems.push(format!("{}: {error:#}", speaker.name)),
        }
    }
    if group.this_computer {
        members.push(Member {
            key: String::new(),
            name: "This computer".into(),
            destination: Box::new(LocalSpeaker::new()),
            alive: true,
            delay: 0,
            pending: 0,
        });
    }
    for problem in &problems {
        crate::logfile::warn(format!("Left out of the group: {problem}"));
    }
    if members.is_empty() {
        anyhow::bail!(
            "None of the speakers could be reached. {}",
            problems.join(" ")
        );
    }
    Ok(start(
        group.name(),
        Box::new(GroupOut {
            members,
            delays: group.delays.clone(),
            silence: vec![],
        }),
    ))
}

// ---------------------------------------------------------------- the pump

/// Open a speaker as an output.
pub fn open(speaker: &Speaker) -> Result<Network> {
    let destination: Box<dyn Destination> = match speaker.kind {
        Kind::AirPlay => Box::new(super::airplay::Receiver::new(speaker)?),
        _ => Box::new(UrlSpeaker::new(speaker)?),
    };
    Ok(start(speaker.name.clone(), destination))
}

pub(crate) fn start(name: String, mut destination: Box<dyn Destination>) -> Network {
    let (mixer, mut source) = rodio::mixer::mixer(CHANNELS, RATE);
    let sink = Arc::new(Sink::connect_new(&mixer));
    let control = Arc::new(Control::default());
    let failure = Arc::new(Mutex::new(None));
    let (weak, shared, slot) = (Arc::downgrade(&sink), control.clone(), failure.clone());
    std::thread::Builder::new()
        .name("needle-speaker".into())
        .spawn(move || {
            // Keep the mixer's input alive as long as the pump runs.
            let _mixer = mixer;
            let fail = |error: anyhow::Error| {
                slot.lock()
                    .unwrap()
                    .get_or_insert_with(|| format!("Speaker error: {error:#}"));
            };
            // The player describes the first song right after opening.
            for _ in 0..50 {
                if shared.described.load(Ordering::Relaxed) {
                    break;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            let meta = shared.meta.lock().unwrap().clone();
            if let Err(e) = destination.start(&meta) {
                fail(e);
                return;
            }
            *shared.lag.lock().unwrap() = destination.lag();
            let mut samples: Vec<i16> = Vec::with_capacity(RATE as usize);
            let mut budget = LEAD * RATE as f64;
            let mut last = Instant::now();
            let mut last_tick = Instant::now();
            let mut paused = false;
            while !shared.stop.load(Ordering::Relaxed) {
                let Some(sink) = weak.upgrade() else { break };
                let now_paused = sink.is_paused();
                drop(sink);
                if now_paused != paused {
                    paused = now_paused;
                    let result = if paused {
                        destination.pause()
                    } else {
                        destination.resume()
                    };
                    if let Err(e) = result {
                        fail(e);
                    }
                }
                if let Some(delays) = shared.delays.lock().unwrap().take() {
                    destination.set_delays(&delays);
                    *shared.lag.lock().unwrap() = destination.lag();
                }
                if shared.flush.swap(false, Ordering::Relaxed) {
                    let meta = shared.meta.lock().unwrap().clone();
                    if let Err(e) = destination.flush(&meta) {
                        fail(e);
                    }
                    budget = LEAD * RATE as f64;
                    last = Instant::now();
                }
                let now = Instant::now();
                if !paused {
                    budget += now.duration_since(last).as_secs_f64() * RATE as f64;
                    let frames = budget.floor() as usize;
                    budget -= frames as f64;
                    samples.clear();
                    for _ in 0..frames * CHANNELS as usize {
                        let sample = source.next().unwrap_or(0.);
                        samples.push((sample.clamp(-1., 1.) * i16::MAX as f32) as i16);
                    }
                    if !samples.is_empty()
                        && let Err(e) = destination.push(&samples)
                    {
                        fail(e);
                    }
                }
                last = now;
                if last_tick.elapsed() > Duration::from_secs(1) {
                    last_tick = Instant::now();
                    if let Err(e) = destination.tick() {
                        fail(e);
                    }
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            destination.stop();
        })
        .expect("spawn speaker thread");
    Network {
        sink,
        name,
        rate: RATE,
        channels: CHANNELS,
        failure,
        guard: Guard(control.clone()),
        control,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Records what the pump does.
    #[derive(Clone, Default)]
    struct Probe(Arc<Mutex<(Vec<String>, usize)>>);
    impl Destination for Probe {
        fn start(&mut self, meta: &Meta) -> Result<()> {
            self.0
                .lock()
                .unwrap()
                .0
                .push(format!("start {}", meta.title));
            Ok(())
        }
        fn push(&mut self, samples: &[i16]) -> Result<()> {
            self.0.lock().unwrap().1 += samples.len();
            Ok(())
        }
        fn pause(&mut self) -> Result<()> {
            self.0.lock().unwrap().0.push("pause".into());
            Ok(())
        }
        fn resume(&mut self) -> Result<()> {
            self.0.lock().unwrap().0.push("resume".into());
            Ok(())
        }
        fn flush(&mut self, meta: &Meta) -> Result<()> {
            self.0
                .lock()
                .unwrap()
                .0
                .push(format!("flush {}", meta.title));
            Ok(())
        }
        fn tick(&mut self) -> Result<()> {
            Ok(())
        }
        fn stop(&mut self) {
            self.0.lock().unwrap().0.push("stop".into());
        }
        fn lag(&self) -> f64 {
            1.
        }
    }

    /// A member that records what reaches it, with a fixed lag.
    #[derive(Clone)]
    struct Timed(Arc<Mutex<Vec<i16>>>, f64, Arc<AtomicBool>);
    impl Destination for Timed {
        fn start(&mut self, _: &Meta) -> Result<()> {
            Ok(())
        }
        fn push(&mut self, samples: &[i16]) -> Result<()> {
            if self.2.load(Ordering::Relaxed) {
                anyhow::bail!("gone");
            }
            self.0.lock().unwrap().extend_from_slice(samples);
            Ok(())
        }
        fn pause(&mut self) -> Result<()> {
            Ok(())
        }
        fn resume(&mut self) -> Result<()> {
            Ok(())
        }
        fn flush(&mut self, _: &Meta) -> Result<()> {
            self.0.lock().unwrap().clear();
            Ok(())
        }
        fn tick(&mut self) -> Result<()> {
            Ok(())
        }
        fn stop(&mut self) {}
        fn lag(&self) -> f64 {
            self.1
        }
    }

    #[test]
    fn groups_line_up_their_members_and_survive_one_failing() {
        let slow = Timed(Arc::default(), 2.0, Arc::default());
        let fast = Timed(Arc::default(), 0.5, Arc::default());
        let member = |key: &str, t: &Timed| Member {
            key: key.into(),
            name: key.into(),
            destination: Box::new(t.clone()),
            alive: true,
            delay: 0,
            pending: 0,
        };
        let mut group = GroupOut {
            members: vec![member("slow", &slow), member("fast", &fast)],
            delays: Default::default(),
            silence: vec![],
        };
        group.start(&Meta::default()).unwrap();
        let sound = vec![1000i16; 2 * 100];
        group.push(&sound).unwrap();
        // The fast member waits 1.5 s so both are heard at 2 s.
        let fast_got = fast.0.lock().unwrap().clone();
        let lead = (1.5 * RATE as f64) as usize * 2;
        assert_eq!(fast_got.len(), lead + sound.len());
        assert!(fast_got[..lead].iter().all(|s| *s == 0));
        assert_eq!(slow.0.lock().unwrap().len(), sound.len());
        assert!((group.lag() - 2.0).abs() < 1e-9);
        // The listener delays the slow one by 100 ms: it gets 100 ms of silence.
        let before = slow.0.lock().unwrap().len();
        group.set_delays(&[("slow".to_string(), 100)].into());
        group.push(&sound).unwrap();
        let added = slow.0.lock().unwrap().len() - before;
        assert_eq!(added, (0.1 * RATE as f64) as usize * 2 + sound.len());
        assert!((group.lag() - 2.1).abs() < 1e-9);
        // A negative delay on the fast one: it skips sound instead.
        group.set_delays(&[("slow".to_string(), 100), ("fast".to_string(), -50)].into());
        let before = fast.0.lock().unwrap().len();
        group.push(&vec![1i16; 2 * 10_000]).unwrap();
        let added = fast.0.lock().unwrap().len() - before;
        assert_eq!(
            added,
            2 * 10_000 - (0.05 * RATE as f64).round() as usize * 2
        );
        // One member fails: the other plays on; when both fail, the group fails.
        slow.2.store(true, Ordering::Relaxed);
        group.push(&sound).unwrap();
        assert!(!group.members[0].alive && group.members[1].alive);
        fast.2.store(true, Ordering::Relaxed);
        assert!(group.push(&sound).is_err());
    }

    #[test]
    fn groups_round_trip_through_device_names() {
        let group = crate::cast::Group {
            speakers: vec![Speaker {
                kind: Kind::Chromecast,
                name: "Kitchen".into(),
                address: "192.168.1.30:8009".into(),
            }],
            this_computer: true,
            delays: [("".to_string(), 120)].into(),
        };
        let name = group.device_name();
        assert!(crate::cast::is_network(&name));
        assert_eq!(
            crate::cast::Group::from_device_name(&name),
            Some(group.clone())
        );
        assert_eq!(Speaker::from_device_name(&name), None);
        assert_eq!(group.name(), "Kitchen + This computer");
        assert_eq!(group.len(), 2);
    }

    #[test]
    fn pumps_in_real_time_and_follows_pause_flush_and_stop() {
        let probe = Probe::default();
        let network = start("Test".into(), Box::new(probe.clone()));
        // As the player does right after opening.
        network.control.set_meta(Meta {
            title: "First".into(),
            ..Default::default()
        });
        network.sink.append(rodio::source::SineWave::new(440.));
        std::thread::sleep(Duration::from_millis(600));
        let sent = probe.0.lock().unwrap().1;
        // About 0.6 s plus the lead, in stereo samples.
        let expected = ((0.6 + LEAD) * RATE as f64 * 2.) as usize;
        assert!(
            sent > expected / 2 && sent < expected * 3 / 2,
            "sent {sent}, expected about {expected}"
        );
        network.sink.pause();
        std::thread::sleep(Duration::from_millis(100));
        let while_paused = probe.0.lock().unwrap().1;
        std::thread::sleep(Duration::from_millis(200));
        assert_eq!(
            probe.0.lock().unwrap().1,
            while_paused,
            "nothing is sent while paused"
        );
        network.sink.play();
        network.control.set_meta(Meta {
            title: "Next".into(),
            ..Default::default()
        });
        network.control.flush();
        std::thread::sleep(Duration::from_millis(100));
        assert_eq!(network.control.lag(), 1.);
        drop(network);
        std::thread::sleep(Duration::from_millis(100));
        let events = probe.0.lock().unwrap().0.clone();
        assert_eq!(
            events,
            vec!["start First", "pause", "resume", "flush Next", "stop"]
        );
    }
}
