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
}

/// Shared between the player and the pump.
#[derive(Default)]
pub struct Control {
    flush: AtomicBool,
    stop: AtomicBool,
    meta: Mutex<Meta>,
    described: AtomicBool,
    lag: Mutex<f64>,
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
