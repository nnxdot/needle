use crate::{
    database::Library,
    model::{Listen, Settings, Track},
};
use anyhow::{Context, Result, bail};
use crossbeam_channel::{Receiver, Sender};
use rodio::{
    OutputStream, OutputStreamBuilder, Sink, Source,
    cpal::traits::{DeviceTrait, HostTrait},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QueueItem {
    pub track: Track,
    pub reason: String,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct Session {
    current: Option<(String, String)>,
    queue: Vec<(String, String)>,
    position: f64,
    repeat: Repeat,
}
impl Session {
    fn resolve(&self, library: &Library) -> Vec<QueueItem> {
        self.current
            .iter()
            .chain(self.queue.iter())
            .filter_map(|(id, reason)| {
                library
                    .track(id)
                    .ok()
                    .flatten()
                    .filter(|t| !t.missing)
                    .map(|track| QueueItem {
                        track,
                        reason: reason.clone(),
                    })
            })
            .collect()
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Repeat {
    #[default]
    Off,
    All,
    One,
}
#[derive(Clone, Debug, Default)]
pub struct PlaybackState {
    pub current: Option<QueueItem>,
    pub queue: Vec<QueueItem>,
    pub playing: bool,
    pub position: f64,
    pub volume: f32,
    pub output: String,
    pub output_rate: u32,
    pub output_channels: u16,
    pub error: Option<String>,
    pub repeat: Repeat,
    pub exclusive: bool,
    pub replay_gain: bool,
    pub loop_range: Option<(f64, f64)>,
}

pub enum Command {
    Play(Vec<QueueItem>),
    Enqueue(Vec<QueueItem>),
    Toggle,
    Next,
    Previous,
    Seek(f64),
    Volume(f32),
    Configure(Settings),
    Stop,
    ClearQueue,
    Remove(usize),
    Move(usize, usize),
    Repeat(Repeat),
    Loop(Option<(f64, f64)>),
    Shutdown,
}

#[derive(Clone)]
pub struct Player {
    tx: Sender<Command>,
    state: Arc<Mutex<PlaybackState>>,
    worker: Arc<Mutex<Option<std::thread::JoinHandle<()>>>>,
}
impl Player {
    pub fn new(library: Library) -> Self {
        let (tx, rx) = crossbeam_channel::unbounded();
        let settings = library.settings().unwrap_or_default();
        let state = Arc::new(Mutex::new(PlaybackState {
            volume: settings.volume,
            ..Default::default()
        }));
        let worker_state = state.clone();
        let worker = std::thread::Builder::new()
            .name("needle-playback".into())
            .spawn(move || Worker::new(library, worker_state, settings).run(rx))
            .expect("start audio worker");
        Self {
            tx,
            state,
            worker: Arc::new(Mutex::new(Some(worker))),
        }
    }
    pub fn send(&self, command: Command) {
        let _ = self.tx.send(command);
    }
    pub fn state(&self) -> PlaybackState {
        self.state.lock().unwrap_or_else(|p| p.into_inner()).clone()
    }
    pub fn shutdown(&self) {
        self.send(Command::Shutdown);
        if let Some(worker) = self.worker.lock().unwrap().take() {
            let _ = worker.join();
        }
    }
}

pub fn devices() -> Result<Vec<String>> {
    Ok(rodio::cpal::default_host()
        .output_devices()?
        .filter_map(|d| d.name().ok())
        .collect())
}

struct Marked<S> {
    inner: S,
    item: QueueItem,
    started: bool,
    epoch: u64,
    tx: Sender<(u64, QueueItem)>,
}
impl<S: Source> Iterator for Marked<S> {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        let sample = self.inner.next();
        if sample.is_some() && !self.started {
            self.started = true;
            let _ = self.tx.send((self.epoch, self.item.clone()));
        }
        sample
    }
}
impl<S: Source> Source for Marked<S> {
    fn current_span_len(&self) -> Option<usize> {
        self.inner.current_span_len()
    }
    fn channels(&self) -> u16 {
        self.inner.channels()
    }
    fn sample_rate(&self) -> u32 {
        self.inner.sample_rate()
    }
    fn total_duration(&self) -> Option<Duration> {
        self.inner.total_duration()
    }
    fn try_seek(&mut self, pos: Duration) -> std::result::Result<(), rodio::source::SeekError> {
        self.inner.try_seek(pos)
    }
}

struct Worker {
    library: Library,
    state: Arc<Mutex<PlaybackState>>,
    settings: Settings,
    stream: Option<OutputStream>,
    sink: Option<Arc<Sink>>,
    pending: VecDeque<QueueItem>,
    staged: VecDeque<QueueItem>,
    #[cfg(windows)]
    exclusive: Option<crate::exclusive::Output>,
    started_tx: Sender<(u64, QueueItem)>,
    started_rx: Receiver<(u64, QueueItem)>,
    epoch: u64,
    active: Option<QueueItem>,
    listen: Option<Listen>,
    last_tick: Instant,
    playing: bool,
    repeat: Repeat,
    loop_range: Option<(f64, f64)>,
    previous: Vec<QueueItem>,
    cycle: Vec<QueueItem>,
    repeat_tail: Vec<QueueItem>,
    resume_position: f64,
    last_session_save: Instant,
}
impl Worker {
    fn new(library: Library, state: Arc<Mutex<PlaybackState>>, settings: Settings) -> Self {
        let (started_tx, started_rx) = crossbeam_channel::unbounded();
        let session = library
            .get_json::<Session>("playback_session")
            .ok()
            .flatten()
            .unwrap_or_default();
        let mut items = session.resolve(&library).into_iter();
        let active = items.next();
        let mut pending: VecDeque<_> = items.collect();
        let repeat_tail = if session.repeat == Repeat::One {
            pending.drain(..).collect()
        } else {
            vec![]
        };
        let resume_position = if active
            .as_ref()
            .is_some_and(|i| session.position >= i.track.duration)
        {
            0.
        } else {
            session.position.max(0.)
        };
        Self {
            library,
            state,
            settings,
            stream: None,
            sink: None,
            pending,
            staged: VecDeque::new(),
            started_tx,
            started_rx,
            epoch: 0,
            active,
            listen: None,
            last_tick: Instant::now(),
            playing: false,
            repeat: session.repeat,
            loop_range: None,
            previous: vec![],
            cycle: vec![],
            repeat_tail,
            resume_position,
            last_session_save: Instant::now(),
            #[cfg(windows)]
            exclusive: None,
        }
    }
    fn save_session(&self) -> Result<()> {
        let identity = |i: &QueueItem| (i.track.id.clone(), i.reason.clone());
        self.library.set_json(
            "playback_session",
            &Session {
                current: self.active.as_ref().map(identity),
                queue: self
                    .staged
                    .iter()
                    .chain(self.pending.iter())
                    .chain(self.repeat_tail.iter())
                    .map(identity)
                    .collect(),
                position: self
                    .sink
                    .as_ref()
                    .map(|s| {
                        if s.empty() {
                            0.
                        } else {
                            s.get_pos().as_secs_f64()
                        }
                    })
                    .unwrap_or(self.resume_position),
                repeat: self.repeat,
            },
        )
    }
    fn fail(&mut self, error: anyhow::Error) {
        self.state.lock().unwrap().error = Some(format!("{error:#}"));
    }
    fn finish_listen(&mut self) {
        if let Some(mut listen) = self.listen.take() {
            listen.qualified = listen.duration > 0.0
                && listen.listened_seconds >= (listen.duration * 0.5).min(240.0);
            if listen.listened_seconds > 0.5
                && let Err(e) = self.library.record_listen(&listen)
            {
                self.fail(e);
            }
        }
    }
    fn close(&mut self) {
        self.finish_listen();
        self.epoch += 1;
        if let Some(sink) = self.sink.take() {
            sink.stop();
        }
        #[cfg(windows)]
        {
            self.exclusive = None;
        }
        self.stream = None;
        self.pending.clear();
        self.staged.clear();
        self.active = None;
        self.playing = false;
    }
    fn open(&mut self) -> Result<()> {
        if self.settings.exclusive {
            #[cfg(windows)]
            {
                if self.exclusive.is_some() {
                    return Ok(());
                }
                let (sink, source) = Sink::new();
                let sink = Arc::new(sink);
                self.exclusive = Some(crate::exclusive::Output::start(
                    source,
                    Arc::downgrade(&sink),
                    self.settings.output_device.clone(),
                    self.state.clone(),
                ));
                self.sink = Some(sink);
                return Ok(());
            }
            #[cfg(not(windows))]
            {
                bail!("Exclusive desktop output is currently available on Windows")
            }
        }
        if self.stream.is_some() {
            return Ok(());
        }
        let host = rodio::cpal::default_host();
        let device = if let Some(name) = &self.settings.output_device {
            host.output_devices()?
                .find(|d| d.name().is_ok_and(|n| &n == name))
                .context("Selected output device is disconnected")?
        } else {
            host.default_output_device()
                .context("No audio output device is available")?
        };
        let name = device.name().unwrap_or_else(|_| "System output".into());
        let state = self.state.clone();
        let mut stream = OutputStreamBuilder::from_device(device)?
            .with_error_callback(move |error| {
                if let Ok(mut state) = state.lock() {
                    state.error = Some(format!("Audio device error: {error}"));
                    state.playing = false;
                }
            })
            .open_stream()?;
        stream.log_on_drop(false);
        let sink = Sink::connect_new(stream.mixer());
        sink.set_volume(self.settings.volume);
        {
            let mut state = self.state.lock().unwrap();
            state.output = name;
            state.output_rate = stream.config().sample_rate();
            state.output_channels = stream.config().channel_count();
            state.exclusive = false;
        }
        self.sink = Some(Arc::new(sink));
        self.stream = Some(stream);
        Ok(())
    }
    fn play(&mut self, items: Vec<QueueItem>) -> Result<()> {
        self.close();
        self.cycle = items.clone();
        self.pending = items.into();
        if self.pending.is_empty() {
            return Ok(());
        }
        self.open()?;
        self.playing = true;
        self.state.lock().unwrap().error = None;
        self.fill()?;
        Ok(())
    }
    fn fill(&mut self) -> Result<()> {
        if self.sink.is_none() {
            return Ok(());
        }
        while self.sink.as_ref().unwrap().len() < 2 {
            let Some(item) = self.pending.pop_front() else {
                break;
            };
            let source = match crate::audio_file::decode(std::path::Path::new(&item.track.path)) {
                Ok(source) => source,
                Err(error) => {
                    self.fail(anyhow::anyhow!(
                        "Cannot decode {}: {error}",
                        item.track.title
                    ));
                    continue;
                }
            };
            let mut gain = 1.0;
            if self.settings.replay_gain
                && !self.settings.exclusive
                && let Some(db) = item.track.replay_gain
            {
                gain = 10f64.powf(db / 20.0);
                if let Some(peak) = item.track.replay_peak.filter(|p| *p > 0.0) {
                    gain = gain.min(1.0 / peak);
                }
                gain = gain.min(16.0);
            }
            let marked = Marked {
                inner: source.amplify(gain as f32),
                item: item.clone(),
                started: false,
                epoch: self.epoch,
                tx: self.started_tx.clone(),
            };
            self.staged.push_back(item);
            self.sink.as_ref().unwrap().append(marked);
        }
        #[cfg(windows)]
        if let Some(output) = &self.exclusive {
            output
                .ready
                .store(true, std::sync::atomic::Ordering::Release);
        }
        Ok(())
    }
    fn handle(&mut self, command: Command) -> Result<bool> {
        match command {
            Command::Shutdown => {
                self.save_session()?;
                self.close();
                return Ok(false);
            }
            Command::Play(items) => {
                self.repeat_tail.clear();
                if self.repeat == Repeat::One {
                    let mut items = items.into_iter();
                    let first = items.next().into_iter().collect();
                    self.repeat_tail = items.collect();
                    self.play(first)?;
                } else {
                    self.play(items)?;
                }
            }
            Command::Enqueue(items) => {
                if self.repeat == Repeat::One && self.active.is_some() {
                    self.repeat_tail.extend(items);
                } else if self.sink.is_none() && self.active.is_some() {
                    self.pending.extend(items);
                } else if self.sink.is_none() {
                    self.play(items)?
                } else {
                    self.pending.extend(items);
                    self.fill()?
                }
            }
            Command::Toggle => {
                if self.sink.is_none() {
                    if let Some(active) = self.active.clone() {
                        let position = self.resume_position;
                        let mut items = vec![active];
                        items.extend(self.pending.iter().cloned());
                        self.play(items)?;
                        if let Some(sink) = &self.sink {
                            let _ = sink.try_seek(Duration::from_secs_f64(position));
                        }
                    }
                } else if self.sink.as_ref().is_some_and(|sink| sink.empty()) {
                    if let Some(active) = self.active.clone() {
                        self.play(vec![active])?;
                    }
                } else if let Some(sink) = &self.sink {
                    self.playing = !self.playing;
                    if self.playing {
                        sink.play()
                    } else {
                        sink.pause()
                    }
                }
            }
            Command::Stop => {
                self.close();
                self.resume_position = 0.;
                self.repeat_tail.clear();
                self.save_session()?;
            }
            Command::Next => {
                self.loop_range = None;
                if self.repeat == Repeat::One && !self.repeat_tail.is_empty() {
                    let next = self.repeat_tail.remove(0);
                    self.play(vec![next])?;
                } else if let Some(sink) = &self.sink {
                    sink.skip_one();
                }
            }
            Command::Previous => {
                if self
                    .sink
                    .as_ref()
                    .is_some_and(|s| s.get_pos().as_secs_f64() > 3.0)
                {
                    if let Some(sink) = &self.sink {
                        sink.try_seek(Duration::ZERO)
                            .map_err(|e| anyhow::anyhow!("Seek failed: {e}"))?;
                    }
                } else if let Some(previous) = self.previous.pop() {
                    let mut items = vec![previous];
                    if let Some(active) = self.active.clone() {
                        items.push(active)
                    }
                    items.extend(self.staged.iter().cloned());
                    items.extend(self.pending.iter().cloned());
                    self.play(items)?;
                }
            }
            Command::Seek(seconds) => {
                if let Some(sink) = &self.sink {
                    let duration = self
                        .active
                        .as_ref()
                        .map(|i| i.track.duration)
                        .unwrap_or(0.0);
                    sink.try_seek(Duration::from_secs_f64(seconds.max(0.0).min(duration)))
                        .map_err(|e| anyhow::anyhow!("Seek failed: {e}"))?;
                }
            }
            Command::Volume(volume) => {
                self.settings.volume = volume.clamp(0.0, 1.0);
                if let Some(sink) = &self.sink {
                    sink.set_volume(if self.settings.exclusive {
                        1.0
                    } else {
                        self.settings.volume
                    });
                }
                let mut persisted = self.library.settings()?;
                persisted.volume = self.settings.volume;
                self.library.save_settings(&persisted)?;
            }
            Command::Configure(settings) => {
                let current = self.active.clone();
                let position = self
                    .sink
                    .as_ref()
                    .map(|s| s.get_pos())
                    .unwrap_or_else(|| Duration::from_secs_f64(self.resume_position));
                let was_playing = self.playing;
                let mut queue: Vec<_> = self
                    .staged
                    .iter()
                    .chain(self.pending.iter())
                    .cloned()
                    .collect();
                let listen = self.listen.take();
                self.close();
                self.settings = settings;
                self.library.save_settings(&self.settings)?;
                if let Some(current) = current {
                    queue.insert(0, current);
                    self.play(queue)?;
                    self.listen = listen;
                    if let Some(sink) = &self.sink {
                        let _ = sink.try_seek(position);
                        if !was_playing {
                            sink.pause();
                            self.playing = false;
                        }
                    }
                }
            }
            Command::ClearQueue => {
                self.repeat_tail.clear();
                self.pending.clear();
                self.rebuild_tail(vec![])?;
            }
            Command::Remove(index) => {
                if self.repeat == Repeat::One {
                    if index < self.repeat_tail.len() {
                        self.repeat_tail.remove(index);
                    }
                    return Ok(true);
                }
                let mut queue: Vec<_> = self
                    .staged
                    .iter()
                    .chain(self.pending.iter())
                    .cloned()
                    .collect();
                if index < queue.len() {
                    queue.remove(index);
                    self.rebuild_tail(queue)?;
                }
            }
            Command::Move(from, to) => {
                if self.repeat == Repeat::One {
                    if from < self.repeat_tail.len() && to < self.repeat_tail.len() {
                        let item = self.repeat_tail.remove(from);
                        self.repeat_tail.insert(to, item);
                    }
                    return Ok(true);
                }
                let mut queue: Vec<_> = self
                    .staged
                    .iter()
                    .chain(self.pending.iter())
                    .cloned()
                    .collect();
                if from < queue.len() && to < queue.len() {
                    let item = queue.remove(from);
                    queue.insert(to, item);
                    self.rebuild_tail(queue)?;
                }
            }
            Command::Repeat(repeat) => {
                let previous = self.repeat;
                self.repeat = repeat;
                if repeat == Repeat::One && previous != Repeat::One {
                    self.repeat_tail = self
                        .staged
                        .iter()
                        .chain(self.pending.iter())
                        .cloned()
                        .collect();
                    self.rebuild_tail(vec![])?;
                } else if previous == Repeat::One && repeat != Repeat::One {
                    let queue = std::mem::take(&mut self.repeat_tail);
                    self.rebuild_tail(queue)?;
                }
            }
            Command::Loop(range) => {
                if let Some((a, b)) = range
                    && (a < 0.0
                        || b <= a
                        || b > self
                            .active
                            .as_ref()
                            .map(|i| i.track.duration)
                            .unwrap_or(0.0))
                {
                    bail!("Loop end must follow its start and stay within the track")
                }
                self.loop_range = range;
            }
        }
        Ok(true)
    }
    fn rebuild_tail(&mut self, queue: Vec<QueueItem>) -> Result<()> {
        let current = self.active.clone();
        let position = self.sink.as_ref().map(|s| s.get_pos()).unwrap_or_default();
        let was_playing = self.playing;
        // Preserve a listening session when only its future queue changes.
        let listen = self.listen.take();
        let mut items = vec![];
        if let Some(current) = current {
            items.push(current);
        }
        items.extend(queue);
        self.play(items)?;
        self.listen = listen;
        if let Some(sink) = &self.sink {
            let _ = sink.try_seek(position);
            if !was_playing {
                sink.pause();
                self.playing = false;
            }
        }
        Ok(())
    }
    fn tick(&mut self) -> Result<()> {
        #[cfg(windows)]
        if self
            .exclusive
            .as_ref()
            .is_some_and(|output| output.failed.load(std::sync::atomic::Ordering::Relaxed))
        {
            self.close();
        }
        let elapsed = self.last_tick.elapsed().as_secs_f64().min(0.25);
        self.last_tick = Instant::now();
        if self.playing
            && let Some(listen) = &mut self.listen
        {
            listen.listened_seconds += elapsed;
        }
        while let Ok((epoch, item)) = self.started_rx.try_recv() {
            if epoch != self.epoch {
                continue;
            }
            let continuing = self
                .listen
                .as_ref()
                .is_some_and(|listen| listen.track_id == item.track.id)
                && self.active.is_none();
            if !continuing {
                self.finish_listen();
            }
            if let Some(active) = self.active.take() {
                self.previous.push(active);
            }
            if self
                .staged
                .front()
                .is_some_and(|s| s.track.id == item.track.id)
            {
                self.staged.pop_front();
            }
            if !continuing {
                self.listen = Some(Listen {
                    id: uuid::Uuid::new_v4().to_string(),
                    track_id: item.track.id.clone(),
                    title: item.track.title.clone(),
                    artist: item.track.artist.clone(),
                    album: item.track.album.clone(),
                    started_at: chrono::Utc::now().timestamp(),
                    listened_seconds: 0.0,
                    duration: item.track.duration,
                    qualified: false,
                });
            }
            self.active = Some(item);
        }
        if let Some(sink) = &self.sink {
            if let Some((a, b)) = self.loop_range
                && self.playing
                && sink.get_pos().as_secs_f64() >= b
            {
                sink.try_seek(Duration::from_secs_f64(a))
                    .map_err(|e| anyhow::anyhow!("Seek failed: {e}"))?;
            }
            if sink.empty() && self.pending.is_empty() && self.staged.is_empty() {
                self.finish_listen();
                let next = match self.repeat {
                    Repeat::One => self.active.clone().into_iter().collect(),
                    Repeat::All => self.cycle.clone(),
                    Repeat::Off => vec![],
                };
                if !next.is_empty() {
                    self.play(next)?;
                } else if self.playing {
                    self.playing = false;
                    if !self.settings.autoplay_query.trim().is_empty() {
                        let tracks = self.library.search(&self.settings.autoplay_query)?;
                        let items = tracks
                            .into_iter()
                            .filter(|t| !t.missing)
                            .take(100)
                            .map(|track| QueueItem {
                                track,
                                reason: format!("Autoplay: {}", self.settings.autoplay_query),
                            })
                            .collect::<Vec<_>>();
                        if !items.is_empty() {
                            self.play(items)?;
                        }
                    }
                }
            }
        }
        self.fill()?;
        if self.last_session_save.elapsed() > Duration::from_secs(10) {
            self.save_session()?;
            self.last_session_save = Instant::now();
        }
        let mut state = self.state.lock().unwrap();
        state.current = self.active.clone();
        state.queue = self
            .staged
            .iter()
            .chain(self.pending.iter())
            .chain(self.repeat_tail.iter())
            .cloned()
            .collect();
        state.playing = self.playing;
        state.position = self
            .sink
            .as_ref()
            .map(|s| s.get_pos().as_secs_f64())
            .unwrap_or(self.resume_position);
        state.volume = self.settings.volume;
        state.repeat = self.repeat;
        state.replay_gain = self.settings.replay_gain && !self.settings.exclusive;
        state.loop_range = self.loop_range;
        state.exclusive = self.settings.exclusive;
        Ok(())
    }
    fn run(mut self, rx: Receiver<Command>) {
        loop {
            match rx.recv_timeout(Duration::from_millis(25)) {
                Ok(command) => match self.handle(command) {
                    Ok(true) => {}
                    Ok(false) => break,
                    Err(e) => self.fail(e),
                },
                Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {
                    let _ = self.save_session();
                    self.close();
                    break;
                }
                Err(crossbeam_channel::RecvTimeoutError::Timeout) => {}
            }
            if let Err(e) = self.tick() {
                self.fail(e);
            }
        }
    }
}

/// Exclusive WASAPI path at the file's native rate. No volume or DSP processing.
#[cfg(windows)]
pub fn play_exclusive(path: &std::path::Path) -> Result<()> {
    use wasapi::*;
    initialize_mta().ok().context("Initialize Windows audio")?;
    let mut decoder = crate::audio_file::decode(path)?;
    let rate = decoder.sample_rate();
    let channels = decoder.channels() as usize;
    let enumerator = DeviceEnumerator::new()?;
    let device = enumerator.get_default_device(&Direction::Render)?;
    let mut client = device.get_iaudioclient()?;
    let format = crate::exclusive::native_format(&client, rate, channels as u16)?;
    let (_, min_period) = client.get_device_period()?;
    let period =
        client.calculate_aligned_period_near(min_period.max(100_000), Some(128), &format)?;
    client.initialize_client(
        &format,
        &Direction::Render,
        &StreamMode::EventsExclusive { period_hns: period },
    )?;
    let event = client.set_get_eventhandle()?;
    let renderer = client.get_audiorenderclient()?;
    let frame_count = client.get_buffer_size()? as usize;
    let bytes_per_sample = format.get_blockalign() as usize / channels;
    let mut buffer = vec![0u8; frame_count * format.get_blockalign() as usize];
    let mut ended = false;
    let mut fill = |buffer: &mut [u8]| {
        for bytes in buffer.chunks_exact_mut(bytes_per_sample) {
            let sample = decoder.next().unwrap_or_else(|| {
                ended = true;
                0.0
            });
            let value = (sample as f64 * 2147483648.0)
                .round()
                .clamp(i32::MIN as f64, i32::MAX as f64) as i32;
            let raw = value.to_le_bytes();
            if bytes_per_sample == 4 {
                bytes.copy_from_slice(&raw);
            } else if bytes_per_sample == 3 {
                bytes.copy_from_slice(&raw[1..]);
            } else if bytes_per_sample == 2 {
                bytes.copy_from_slice(&raw[2..]);
            }
        }
        ended
    };
    let mut final_buffer = fill(&mut buffer);
    renderer.write_to_device(frame_count, &buffer, None)?;
    client.start_stream()?;
    loop {
        if let Err(e) = event.wait_for_event(2000) {
            let _ = client.stop_stream();
            return Err(e.into());
        }
        if final_buffer {
            break;
        }
        final_buffer = fill(&mut buffer);
        renderer.write_to_device(frame_count, &buffer, None)?;
    }
    client.stop_stream()?;
    Ok(())
}
#[cfg(not(windows))]
pub fn play_exclusive(_path: &std::path::Path) -> Result<()> {
    bail!("Exclusive CLI playback is currently implemented for Windows WASAPI")
}
