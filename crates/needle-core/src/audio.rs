use crate::{
    database::Library,
    model::{Listen, Settings, Track},
};
use anyhow::{Context, Result, bail};
use crossbeam_channel::{Receiver, Sender};
use rodio::{
    OutputStreamBuilder, Sink, Source,
    cpal::traits::{DeviceTrait, HostTrait},
};
use serde::{Deserialize, Serialize};
use std::{
    any::Any,
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
        self.resolve_with(|id| library.track(id).ok().flatten())
    }
    fn resolve_with(&self, lookup: impl Fn(&str) -> Option<Track>) -> Vec<QueueItem> {
        self.current
            .iter()
            .chain(self.queue.iter())
            .filter_map(|(id, reason)| {
                lookup(id).filter(|t| !t.missing).map(|track| QueueItem {
                    track,
                    reason: reason.clone(),
                })
            })
            .collect()
    }
}
/// Written every few seconds; the full session is rewritten only when the queue changes.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct PositionMark {
    track_id: String,
    position: f64,
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
    /// Shared snapshot; cloning the state does not copy the queue.
    pub queue: Arc<Vec<QueueItem>>,
    /// Changes whenever `queue` changes.
    pub queue_version: u64,
    pub playing: bool,
    pub position: f64,
    pub volume: f32,
    pub output: String,
    /// The output device the player is set to now (it drops a failed speaker).
    pub output_device: Option<String>,
    pub output_rate: u32,
    pub output_channels: u16,
    pub error: Option<String>,
    /// Non-fatal output events, e.g. a missing configured device or a reconnect.
    pub output_notice: Option<String>,
    pub repeat: Repeat,
    pub exclusive: bool,
    pub replay_gain: bool,
    pub album_gain: bool,
    pub loop_range: Option<(f64, f64)>,
    /// The track currently playing from its separated stems.
    pub stems: Option<String>,
}

pub enum Command {
    Play(Vec<QueueItem>),
    /// Play the list starting at the given index; earlier items are not queued.
    PlayAt(Vec<QueueItem>, usize),
    Enqueue(Vec<QueueItem>),
    /// Insert at the front of the upcoming queue.
    PlayNext(Vec<QueueItem>),
    /// Skip directly to an index of the upcoming queue.
    Jump(usize),
    Toggle,
    Next,
    Previous,
    Seek(f64),
    Volume(f32),
    Configure(Box<Settings>),
    Stop,
    ClearQueue,
    Remove(usize),
    Move(usize, usize),
    Repeat(Repeat),
    Loop(Option<(f64, f64)>),
    /// Change the equalizer and sound tools; applies to the playing track without a restart.
    Dsp(crate::dsp::Dsp),
    /// Play this track from its stem folder (with the live mix), or `None` to go back to the file.
    Stems(Option<(String, std::path::PathBuf)>),
    Shutdown,
}

#[derive(Clone)]
pub struct Player {
    stem_mix: Arc<crate::stems::StemMix>,
    effects: Arc<crate::effects::Registry>,
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
        let stem_mix = Arc::new(crate::stems::StemMix::default());
        let mix = stem_mix.clone();
        let effects = Arc::new(crate::effects::Registry::default());
        let registry = effects.clone();
        let worker = std::thread::Builder::new()
            .name("needle-playback".into())
            .spawn(move || {
                let mut worker =
                    Worker::new(library, worker_state, settings, Box::new(SystemOutput));
                worker.stem_mix = mix;
                worker.dsp = crate::dsp::DspControl::new(worker.settings.dsp.clone(), registry);
                worker.run(rx)
            })
            .expect("start audio worker");
        Self {
            stem_mix,
            effects,
            tx,
            state,
            worker: Arc::new(Mutex::new(Some(worker))),
        }
    }
    pub fn send(&self, command: Command) {
        let _ = self.tx.send(command);
    }
    /// The effects plugins offer; the plugin host fills it.
    pub fn effects(&self) -> &Arc<crate::effects::Registry> {
        &self.effects
    }
    /// Live stem volumes for the track playing from stems.
    pub fn stem_mix(&self) -> &Arc<crate::stems::StemMix> {
        &self.stem_mix
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

/// Names of the connected output devices, enumerated afresh on each call.
pub fn devices() -> Result<Vec<String>> {
    let mut names: Vec<String> = vec![];
    for name in rodio::cpal::default_host()
        .output_devices()?
        .filter_map(|d| d.name().ok())
    {
        if !names.contains(&name) {
            names.push(name);
        }
    }
    Ok(names)
}
/// The current system default output, if any.
pub fn default_device() -> Option<String> {
    rodio::cpal::default_host()
        .default_output_device()?
        .name()
        .ok()
}

/// Linear gain applied before volume. Album gain is used when enabled and
/// measured, otherwise track gain; the matching peak limits the gain so the
/// result does not clip, and the boost is capped at +24 dB.
pub fn replay_gain_factor(track: &Track, settings: &Settings) -> f64 {
    if !settings.replay_gain || settings.exclusive {
        return 1.0;
    }
    let (db, peak) = match (settings.album_gain, track.album_replay_gain) {
        (true, Some(db)) => (db, track.album_peak),
        _ => match track.replay_gain {
            Some(db) => (db, track.replay_peak),
            None => return 1.0,
        },
    };
    let mut gain = 10f64.powf(db / 20.0);
    if let Some(peak) = peak.filter(|p| *p > 0.0) {
        gain = gain.min(1.0 / peak);
    }
    gain.min(16.0)
}
/// Local listening qualification: half the track or four minutes.
/// How a speaker should describe a song.
fn speaker_meta(track: &Track) -> crate::cast::Meta {
    crate::cast::Meta {
        title: track.title.clone(),
        artist: track.display_artist().to_string(),
        album: track.album.clone(),
        cover: track.artwork.clone(),
    }
}

fn qualifies(listened: f64, duration: f64) -> bool {
    duration > 0.0 && listened >= (duration * 0.5).min(240.0)
}
fn validate_loop(range: Option<(f64, f64)>, duration: f64) -> Result<Option<(f64, f64)>> {
    if let Some((a, b)) = range
        && (a < 0.0 || b <= a || b > duration)
    {
        bail!("Loop end must follow its start and stay within the track")
    }
    Ok(range)
}
/// Where to jump back to when an A–B loop reaches its end.
fn loop_restart(range: Option<(f64, f64)>, position: f64) -> Option<f64> {
    range.filter(|(_, b)| position >= *b).map(|(a, _)| a)
}
/// Previous restarts the current track after this many seconds.
const RESTART_AFTER: f64 = 3.0;

/// The play queue, independent of any output device. `staged` items are
/// already appended to the output; `pending` items are not. Items held aside
/// while repeating one track live in `repeat_tail`.
#[derive(Default)]
struct Queue {
    active: Option<QueueItem>,
    pending: VecDeque<QueueItem>,
    staged: VecDeque<QueueItem>,
    previous: Vec<QueueItem>,
    cycle: Vec<QueueItem>,
    repeat_tail: Vec<QueueItem>,
    repeat: Repeat,
    version: u64,
}
enum Edit {
    Unchanged,
    Done,
    /// Items already sent to the output changed; rebuild with this tail.
    Rebuild(Vec<QueueItem>),
}
impl Queue {
    fn restore(session: &Session, items: Vec<QueueItem>) -> (Self, f64) {
        let mut items = items.into_iter();
        let active = items.next();
        let mut pending: VecDeque<_> = items.collect();
        let repeat_tail = if session.repeat == Repeat::One {
            pending.drain(..).collect()
        } else {
            vec![]
        };
        let position = if active
            .as_ref()
            .is_some_and(|i| session.position >= i.track.duration)
        {
            0.
        } else {
            session.position.max(0.)
        };
        let queue = Self {
            active,
            pending,
            repeat_tail,
            repeat: session.repeat,
            version: 1,
            ..Default::default()
        };
        (queue, position)
    }
    fn session(&self, position: f64) -> Session {
        let identity = |i: &QueueItem| (i.track.id.clone(), i.reason.clone());
        Session {
            current: self.active.as_ref().map(identity),
            queue: self
                .staged
                .iter()
                .chain(self.pending.iter())
                .chain(self.repeat_tail.iter())
                .map(identity)
                .collect(),
            position,
            repeat: self.repeat,
        }
    }
    fn touch(&mut self) {
        self.version += 1;
    }
    /// Items after the current one, in playing order.
    fn tail(&self) -> Vec<QueueItem> {
        self.staged
            .iter()
            .chain(self.pending.iter())
            .cloned()
            .collect()
    }
    /// What the listener sees as "up next", including items held by Repeat One.
    fn upcoming(&self) -> Vec<QueueItem> {
        self.staged
            .iter()
            .chain(self.pending.iter())
            .chain(self.repeat_tail.iter())
            .cloned()
            .collect()
    }
    fn load(&mut self, items: Vec<QueueItem>) {
        self.cycle = items.clone();
        self.pending = items.into();
        self.touch();
    }
    fn clear_playing(&mut self) {
        self.pending.clear();
        self.staged.clear();
        self.active = None;
        self.touch();
    }
    /// Holds a stopped queue: `items[0]` becomes current, the rest wait.
    fn hold(&mut self, items: Vec<QueueItem>) {
        let mut items = items.into_iter();
        self.active = items.next();
        self.staged.clear();
        self.pending = items.collect();
        self.touch();
    }
    /// Splits a play request; Repeat One plays the first item and holds the rest.
    fn play_request(&mut self, items: Vec<QueueItem>) -> Vec<QueueItem> {
        self.repeat_tail.clear();
        self.touch();
        if self.repeat == Repeat::One {
            let mut items = items.into_iter();
            let first = items.next().into_iter().collect();
            self.repeat_tail = items.collect();
            first
        } else {
            items
        }
    }
    fn append(&mut self, items: Vec<QueueItem>) {
        if self.repeat == Repeat::One && self.active.is_some() {
            self.repeat_tail.extend(items);
        } else {
            self.pending.extend(items);
        }
        self.touch();
    }
    fn insert_next(&mut self, items: Vec<QueueItem>) -> Edit {
        self.touch();
        if self.repeat == Repeat::One && self.active.is_some() {
            self.repeat_tail.splice(0..0, items);
            Edit::Done
        } else if self.staged.is_empty() {
            for item in items.into_iter().rev() {
                self.pending.push_front(item);
            }
            Edit::Done
        } else {
            let mut tail = items;
            tail.extend(self.tail());
            Edit::Rebuild(tail)
        }
    }
    fn remove(&mut self, index: usize) -> Edit {
        if self.repeat == Repeat::One {
            if index < self.repeat_tail.len() {
                self.repeat_tail.remove(index);
                self.touch();
                return Edit::Done;
            }
            return Edit::Unchanged;
        }
        if index < self.staged.len() {
            let mut queue = self.tail();
            queue.remove(index);
            Edit::Rebuild(queue)
        } else if self.pending.remove(index - self.staged.len()).is_some() {
            self.touch();
            Edit::Done
        } else {
            Edit::Unchanged
        }
    }
    fn move_item(&mut self, from: usize, to: usize) -> Edit {
        if self.repeat == Repeat::One {
            if from < self.repeat_tail.len() && to < self.repeat_tail.len() {
                let item = self.repeat_tail.remove(from);
                self.repeat_tail.insert(to, item);
                self.touch();
                return Edit::Done;
            }
            return Edit::Unchanged;
        }
        let staged = self.staged.len();
        let len = staged + self.pending.len();
        if from >= len || to >= len {
            Edit::Unchanged
        } else if from >= staged && to >= staged {
            let item = self.pending.remove(from - staged).unwrap();
            self.pending.insert(to - staged, item);
            self.touch();
            Edit::Done
        } else {
            let mut queue = self.tail();
            let item = queue.remove(from);
            queue.insert(to, item);
            Edit::Rebuild(queue)
        }
    }
    /// Changes repeat mode; entering or leaving Repeat One moves the queue.
    fn set_repeat(&mut self, repeat: Repeat) -> Option<Vec<QueueItem>> {
        let previous = self.repeat;
        self.repeat = repeat;
        self.touch();
        if repeat == Repeat::One && previous != Repeat::One {
            self.repeat_tail = self.tail();
            Some(vec![])
        } else if previous == Repeat::One && repeat != Repeat::One {
            Some(std::mem::take(&mut self.repeat_tail))
        } else {
            None
        }
    }
    /// Next under Repeat One takes the first held item.
    fn next_held(&mut self) -> Option<QueueItem> {
        (self.repeat == Repeat::One && !self.repeat_tail.is_empty()).then(|| {
            self.touch();
            self.repeat_tail.remove(0)
        })
    }
    /// Removes and returns upcoming item `index`, discarding the ones before it.
    fn jump(&mut self, index: usize) -> Option<Vec<QueueItem>> {
        let items = if self.repeat == Repeat::One {
            if index >= self.repeat_tail.len() {
                return None;
            }
            let item = self.repeat_tail.drain(..=index).next_back();
            item.into_iter().collect()
        } else {
            let mut tail = self.tail();
            if index >= tail.len() {
                return None;
            }
            tail.split_off(index)
        };
        if let Some(active) = self.active.take() {
            self.previous.push(active);
        }
        self.touch();
        Some(items)
    }
    /// The list to play for Previous: the last played item, then everything after.
    fn back(&mut self) -> Option<Vec<QueueItem>> {
        let previous = self.previous.pop()?;
        let mut items = vec![previous];
        items.extend(self.active.clone());
        items.extend(self.staged.iter().cloned());
        items.extend(self.pending.iter().cloned());
        Some(items)
    }
    /// Records that the output started `item`.
    fn started(&mut self, item: QueueItem) {
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
        self.active = Some(item);
        self.touch();
    }
    /// What to play when everything has finished.
    fn after_end(&self) -> Vec<QueueItem> {
        match self.repeat {
            Repeat::One => self.active.clone().into_iter().collect(),
            Repeat::All => self.cycle.clone(),
            Repeat::Off => vec![],
        }
    }
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

/// A shared-mode output: a sink the device pulls from, and a slot the device
/// fills when it fails. `_stream` keeps the device stream open.
struct OpenOutput {
    sink: Arc<Sink>,
    name: String,
    rate: u32,
    channels: u16,
    failure: Arc<Mutex<Option<String>>>,
    _stream: Box<dyn Any>,
    /// For a speaker on the network.
    network: Option<Arc<crate::cast::output::Control>>,
}
trait OutputOpener {
    /// Opens the named device, or the default for `None`. `Ok(None)` means the
    /// named device is not connected.
    fn open(&mut self, device: Option<&str>) -> Result<Option<OpenOutput>>;
    fn default_name(&mut self) -> Option<String>;
}
struct SystemOutput;
impl OutputOpener for SystemOutput {
    fn open(&mut self, device: Option<&str>) -> Result<Option<OpenOutput>> {
        if let Some(speaker) = device.and_then(crate::cast::Speaker::from_device_name) {
            let network = crate::cast::output::open(&speaker)?;
            return Ok(Some(OpenOutput {
                sink: network.sink,
                name: network.name,
                rate: network.rate,
                channels: network.channels,
                failure: network.failure,
                _stream: Box::new(network.guard),
                network: Some(network.control),
            }));
        }
        let host = rodio::cpal::default_host();
        let device = match device {
            Some(name) => match host
                .output_devices()?
                .find(|d| d.name().is_ok_and(|n| n == name))
            {
                Some(device) => device,
                None => return Ok(None),
            },
            None => host
                .default_output_device()
                .context("No audio output device is available")?,
        };
        let name = device.name().unwrap_or_else(|_| "System output".into());
        let failure = Arc::new(Mutex::new(None));
        let slot = failure.clone();
        let mut stream = OutputStreamBuilder::from_device(device)?
            .with_error_callback(move |error| {
                if let Ok(mut slot) = slot.lock() {
                    slot.get_or_insert_with(|| format!("Audio device error: {error}"));
                }
            })
            .open_stream()?;
        stream.log_on_drop(false);
        let sink = Sink::connect_new(stream.mixer());
        Ok(Some(OpenOutput {
            sink: Arc::new(sink),
            name,
            rate: stream.config().sample_rate(),
            channels: stream.config().channel_count(),
            failure,
            _stream: Box::new(stream),
            network: None,
        }))
    }
    fn default_name(&mut self) -> Option<String> {
        default_device()
    }
}

struct Worker {
    library: Library,
    state: Arc<Mutex<PlaybackState>>,
    settings: Settings,
    opener: Box<dyn OutputOpener + Send>,
    output: Option<OpenOutput>,
    sink: Option<Arc<Sink>>,
    queue: Queue,
    #[cfg(windows)]
    exclusive: Option<crate::exclusive::Output>,
    started_tx: Sender<(u64, QueueItem)>,
    started_rx: Receiver<(u64, QueueItem)>,
    epoch: u64,
    listen: Option<Listen>,
    last_tick: Instant,
    playing: bool,
    loop_range: Option<(f64, f64)>,
    resume_position: f64,
    last_session_save: Instant,
    saved_version: u64,
    published_version: u64,
    lost: Option<String>,
    progress: (Duration, Instant),
    stall_timeout: Duration,
    seek_timeout: Duration,
    default_check: Duration,
    last_default_check: Instant,
    dsp: Arc<crate::dsp::DspControl>,
    stem_mix: Arc<crate::stems::StemMix>,
    stems: Option<(String, std::path::PathBuf)>,
    /// The last queued song's ending, which the next song may crossfade over.
    ending: Option<(Track, Arc<crate::crossfade::Ending>)>,
}
impl Worker {
    fn new(
        library: Library,
        state: Arc<Mutex<PlaybackState>>,
        settings: Settings,
        opener: Box<dyn OutputOpener + Send>,
    ) -> Self {
        let (started_tx, started_rx) = crossbeam_channel::unbounded();
        let mut session = library
            .get_json::<Session>("playback_session")
            .ok()
            .flatten()
            .unwrap_or_default();
        if let Some(mark) = library
            .get_json::<PositionMark>("playback_position")
            .ok()
            .flatten()
            && session
                .current
                .as_ref()
                .is_some_and(|(id, _)| *id == mark.track_id)
        {
            session.position = mark.position;
        }
        let items = session.resolve(&library);
        let (queue, resume_position) = Queue::restore(&session, items);
        let saved_version = queue.version;
        let dsp = crate::dsp::DspControl::new(settings.dsp.clone(), Default::default());
        Self {
            dsp,
            stem_mix: Arc::default(),
            stems: None,
            ending: None,
            library,
            state,
            settings,
            opener,
            output: None,
            sink: None,
            queue,
            started_tx,
            started_rx,
            epoch: 0,
            listen: None,
            last_tick: Instant::now(),
            playing: false,
            loop_range: None,
            resume_position,
            last_session_save: Instant::now(),
            saved_version,
            published_version: 0,
            lost: None,
            progress: (Duration::ZERO, Instant::now()),
            stall_timeout: Duration::from_secs(3),
            seek_timeout: Duration::from_secs(2),
            default_check: Duration::from_secs(2),
            last_default_check: Instant::now(),
            #[cfg(windows)]
            exclusive: None,
        }
    }
    fn position(&self) -> f64 {
        self.sink
            .as_ref()
            .map(|s| s.get_pos().as_secs_f64())
            .unwrap_or(self.resume_position)
    }
    fn save_session(&mut self) -> Result<()> {
        let position = self
            .sink
            .as_ref()
            .map(|s| {
                if s.empty() {
                    0.
                } else {
                    s.get_pos().as_secs_f64()
                }
            })
            .unwrap_or(self.resume_position);
        if self.saved_version != self.queue.version {
            self.library
                .set_json("playback_session", &self.queue.session(position))?;
            self.saved_version = self.queue.version;
        }
        self.library.set_json(
            "playback_position",
            &PositionMark {
                track_id: self
                    .queue
                    .active
                    .as_ref()
                    .map(|i| i.track.id.clone())
                    .unwrap_or_default(),
                position,
            },
        )
    }
    fn fail(&mut self, error: anyhow::Error) {
        self.state.lock().unwrap().error = Some(format!("{error:#}"));
    }
    fn finish_listen(&mut self) {
        if let Some(mut listen) = self.listen.take() {
            listen.qualified = qualifies(listen.listened_seconds, listen.duration);
            if listen.listened_seconds > 0.5
                && let Err(e) = self.library.record_listen(&listen)
            {
                self.fail(e);
            }
        }
    }
    /// Drops the output device without touching the queue.
    fn release(&mut self) {
        self.epoch += 1;
        self.ending = None;
        if let Some(sink) = self.sink.take() {
            sink.stop();
        }
        #[cfg(windows)]
        {
            self.exclusive = None;
        }
        self.output = None;
    }
    fn close(&mut self) {
        self.finish_listen();
        self.release();
        self.queue.clear_playing();
        self.playing = false;
    }
    /// Keeps `items` as a paused queue at `position` with no output open.
    /// Toggle resumes from there.
    fn suspend(&mut self, items: Vec<QueueItem>, position: f64) {
        self.release();
        self.queue.hold(items);
        self.resume_position = position;
        self.playing = false;
    }
    /// Whether the output is a speaker on the network.
    fn speaker(&self) -> bool {
        self.settings
            .output_device
            .as_deref()
            .is_some_and(|d| d.starts_with(crate::cast::PREFIX))
    }
    fn network(&self) -> Option<Arc<crate::cast::output::Control>> {
        self.output.as_ref().and_then(|o| o.network.clone())
    }
    fn open(&mut self) -> Result<()> {
        if self.settings.exclusive && !self.speaker() {
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
        if self.output.is_some() {
            return Ok(());
        }
        let requested = self.settings.output_device.clone();
        let mut notice = None;
        let output = match self.opener.open(requested.as_deref())? {
            Some(output) => output,
            None => {
                let output = self
                    .opener
                    .open(None)?
                    .context("No audio output device is available")?;
                notice = Some(format!(
                    "{} is not connected. Playing through {} instead.",
                    requested.unwrap_or_default(),
                    output.name
                ));
                output
            }
        };
        output.sink.set_volume(self.settings.volume);
        {
            let mut state = self.state.lock().unwrap();
            state.output = output.name.clone();
            state.output_rate = output.rate;
            state.output_channels = output.channels;
            state.exclusive = false;
            state.output_notice = notice;
        }
        self.sink = Some(output.sink.clone());
        self.progress = (Duration::ZERO, Instant::now());
        if let (Some(control), Some(item)) = (&output.network, self.queue.pending.front()) {
            control.set_meta(speaker_meta(&item.track));
        }
        self.output = Some(output);
        Ok(())
    }
    fn play(&mut self, items: Vec<QueueItem>) -> Result<()> {
        self.close();
        self.queue.load(items);
        if self.queue.pending.is_empty() {
            return Ok(());
        }
        if let Err(error) = self.open() {
            // Keep the request as a paused queue so Toggle can retry it.
            let items = self.queue.pending.drain(..).collect();
            self.queue.hold(items);
            return Err(error);
        }
        self.playing = true;
        self.state.lock().unwrap().error = None;
        self.fill()?;
        Ok(())
    }
    /// Plays `items` from `position` in the given play/pause state, keeping the
    /// current listening session. If no output opens, the queue is kept paused.
    fn restart(&mut self, items: Vec<QueueItem>, position: f64, playing: bool) -> Result<()> {
        let listen = self.listen.take();
        if let Err(error) = self.play(items.clone()) {
            self.listen = listen;
            self.suspend(items, position);
            return Err(error);
        }
        self.listen = listen;
        if let Some(sink) = self.sink.clone() {
            if !playing {
                sink.pause();
                self.playing = false;
            }
            if position > 0.0 {
                let _ = self.seek(position);
            }
        }
        Ok(())
    }
    /// Seeks without trusting the device: a dead stream never acknowledges
    /// Rodio's seek, so give up after a timeout and treat the output as lost.
    fn seek(&mut self, position: f64) -> Result<()> {
        let Some(sink) = self.sink.clone() else {
            return Ok(());
        };
        let (tx, rx) = crossbeam_channel::bounded(1);
        std::thread::spawn(move || {
            let _ = tx.send(sink.try_seek(Duration::from_secs_f64(position.max(0.0))));
        });
        match rx.recv_timeout(self.seek_timeout) {
            Ok(result) => {
                // A speaker holds seconds of audio; start it fresh from the new place.
                if result.is_ok()
                    && let Some(control) = self.network()
                {
                    control.flush();
                }
                result.map_err(|e| anyhow::anyhow!("Seek failed: {e}"))
            }
            Err(_) => {
                self.lost = Some("Audio output stopped responding".into());
                bail!("Audio output stopped responding")
            }
        }
    }
    fn fill(&mut self) -> Result<()> {
        if self.sink.is_none() {
            return Ok(());
        }
        while self.sink.as_ref().unwrap().len() < 2 {
            let Some(item) = self.queue.pending.pop_front() else {
                break;
            };
            let stems = self
                .stems
                .as_ref()
                .filter(|(id, _)| *id == item.track.id && !self.settings.exclusive)
                .and_then(|(_, dir)| {
                    crate::stems::StemSource::open(dir, self.stem_mix.clone()).ok()
                });
            let decoded = match stems {
                Some(stems) => Ok(Box::new(stems) as Box<dyn Source + Send>),
                None => crate::audio_file::decode_track(&item.track),
            };
            let source = match decoded {
                Ok(source) => source,
                Err(error) => {
                    self.queue.touch();
                    self.fail(anyhow::anyhow!(
                        "Cannot decode {}: {error}",
                        item.track.title
                    ));
                    continue;
                }
            };
            let gain = replay_gain_factor(&item.track, &self.settings);
            // Exclusive output stays bit-exact; the sound tools only apply to shared output.
            let processed: Box<dyn Source + Send> = if self.settings.exclusive
                || self.settings.dsp.is_transparent() && !self.settings.dsp.eq
            {
                Box::new(source.amplify(gain as f32))
            } else {
                Box::new(crate::dsp::Processed::new(
                    source.amplify(gain as f32),
                    self.dsp.clone(),
                ))
            };
            let processed = self.crossfade(&item.track, processed);
            let marked = Marked {
                inner: processed,
                item: item.clone(),
                started: false,
                epoch: self.epoch,
                tx: self.started_tx.clone(),
            };
            self.queue.staged.push_back(item);
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
    /// Blend the start of `track` over the previous song's ending, and set up its own ending
    /// for the next song. Off on exclusive output, which stays bit-exact, and between
    /// consecutive tracks of one album, which are often meant to run into each other.
    fn crossfade(
        &mut self,
        track: &Track,
        source: Box<dyn Source + Send>,
    ) -> Box<dyn Source + Send> {
        let fade = self.settings.crossfade as f64;
        if fade <= 0. || self.settings.exclusive {
            self.ending = None;
            return source;
        }
        let mut source = source;
        if let Some((previous, ending)) = self.ending.take() {
            let same_album = previous.album == track.album
                && previous.album_artist == track.album_artist
                && !track.album.is_empty()
                && track.track_number == previous.track_number + 1;
            if !same_album && ending.claim() {
                source = Box::new(crate::crossfade::blend(ending, source));
            }
        }
        if track.duration > fade * 3. {
            let (head, ending) = crate::crossfade::split(source, track.duration, fade);
            self.ending = Some((track.clone(), ending));
            Box::new(head)
        } else {
            source
        }
    }
    fn handle(&mut self, command: Command) -> Result<bool> {
        match command {
            Command::Shutdown => {
                self.save_session()?;
                self.close();
                return Ok(false);
            }
            Command::Play(items) => {
                let items = self.queue.play_request(items);
                self.play(items)?;
            }
            Command::PlayAt(items, index) => {
                if index >= items.len() {
                    bail!("Nothing to play at position {}", index + 1)
                }
                let start = self.queue.play_request(items[index..].to_vec());
                self.play(start)?;
                self.queue.cycle = items;
            }
            Command::Enqueue(items) => {
                if self.sink.is_none() && self.queue.active.is_none() {
                    self.play(items)?
                } else {
                    self.queue.append(items);
                    self.fill()?
                }
            }
            Command::PlayNext(items) => {
                if self.sink.is_none() && self.queue.active.is_none() {
                    self.play(items)?
                } else if let Edit::Rebuild(tail) = self.queue.insert_next(items) {
                    self.rebuild_tail(tail)?
                } else {
                    self.fill()?
                }
            }
            Command::Jump(index) => {
                let cycle = self.queue.cycle.clone();
                if let Some(items) = self.queue.jump(index) {
                    self.loop_range = None;
                    self.play(items)?;
                    self.queue.cycle = cycle;
                }
            }
            Command::Toggle => {
                if self.sink.is_none() {
                    if let Some(active) = self.queue.active.clone() {
                        let mut items = vec![active];
                        items.extend(self.queue.pending.iter().cloned());
                        self.restart(items, self.resume_position, true)?;
                    }
                } else if self.sink.as_ref().is_some_and(|sink| sink.empty()) {
                    if let Some(active) = self.queue.active.clone() {
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
                self.queue.repeat_tail.clear();
                self.save_session()?;
            }
            Command::Next => {
                self.loop_range = None;
                if let Some(next) = self.queue.next_held() {
                    self.play(vec![next])?;
                } else if let Some(sink) = &self.sink {
                    sink.skip_one();
                }
            }
            Command::Previous => {
                // After the queue has finished, the sink is empty and seeking cannot restart
                // anything, so start the last track again instead.
                let finished = self.sink.as_ref().is_some_and(|sink| sink.empty());
                if finished && self.position() > RESTART_AFTER {
                    if let Some(active) = self.queue.active.clone() {
                        self.play(vec![active])?;
                    }
                } else if self.sink.is_some() && !finished && self.position() > RESTART_AFTER {
                    self.seek(0.0)?;
                } else if let Some(items) = self.queue.back() {
                    self.play(items)?;
                }
            }
            Command::Seek(seconds) => {
                let duration = self
                    .queue
                    .active
                    .as_ref()
                    .map(|i| i.track.duration)
                    .unwrap_or(0.0);
                let seconds = seconds.max(0.0).min(duration);
                if self.sink.is_some() {
                    self.seek(seconds)?;
                } else {
                    self.resume_position = seconds;
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
                let current = self.queue.active.clone();
                let position = self.position();
                let was_playing = self.playing;
                let queue = self.queue.tail();
                let listen = self.listen.take();
                self.close();
                self.settings = *settings;
                self.dsp.set(self.settings.dsp.clone());
                self.library.save_settings(&self.settings)?;
                if let Some(current) = current {
                    let mut items = vec![current];
                    items.extend(queue);
                    self.listen = listen;
                    self.restart(items, position, was_playing)?;
                }
            }
            Command::Dsp(dsp) => {
                // Tracks already queued without processing pick it up from the next one onwards.
                let was_off = self.settings.dsp.is_transparent() && !self.settings.dsp.eq;
                self.dsp.set(dsp.clone());
                self.settings.dsp = dsp;
                self.library.save_settings(&self.settings)?;
                if was_off && !self.settings.exclusive && self.queue.active.is_some() {
                    let current = self.queue.active.clone();
                    let position = self.position();
                    let was_playing = self.playing;
                    let queue = self.queue.tail();
                    let listen = self.listen.take();
                    self.close();
                    if let Some(current) = current {
                        let mut items = vec![current];
                        items.extend(queue);
                        self.listen = listen;
                        self.restart(items, position, was_playing)?;
                    }
                }
            }
            Command::Stems(stems) => {
                // Restart the current track at the same place from the new source.
                self.stems = stems;
                let current = self.queue.active.clone();
                let position = self.position();
                let was_playing = self.playing;
                let queue = self.queue.tail();
                let listen = self.listen.take();
                self.close();
                if let Some(current) = current {
                    let mut items = vec![current];
                    items.extend(queue);
                    self.listen = listen;
                    self.restart(items, position, was_playing)?;
                }
            }
            Command::ClearQueue => {
                self.queue.repeat_tail.clear();
                self.queue.pending.clear();
                self.queue.touch();
                self.rebuild_tail(vec![])?;
            }
            Command::Remove(index) => {
                if let Edit::Rebuild(queue) = self.queue.remove(index) {
                    self.rebuild_tail(queue)?;
                }
            }
            Command::Move(from, to) => {
                if let Edit::Rebuild(queue) = self.queue.move_item(from, to) {
                    self.rebuild_tail(queue)?;
                }
            }
            Command::Repeat(repeat) => {
                if let Some(queue) = self.queue.set_repeat(repeat) {
                    self.rebuild_tail(queue)?;
                }
            }
            Command::Loop(range) => {
                let duration = self
                    .queue
                    .active
                    .as_ref()
                    .map(|i| i.track.duration)
                    .unwrap_or(0.0);
                self.loop_range = validate_loop(range, duration)?;
            }
        }
        Ok(true)
    }
    fn rebuild_tail(&mut self, queue: Vec<QueueItem>) -> Result<()> {
        let position = self.sink.as_ref().map(|s| s.get_pos()).unwrap_or_default();
        let mut items = vec![];
        items.extend(self.queue.active.clone());
        items.extend(queue);
        if items.is_empty() {
            let listen = self.listen.take();
            self.play(items)?;
            self.listen = listen;
            return Ok(());
        }
        // Preserve a listening session when only its future queue changes.
        let playing = self.playing;
        self.restart(items, position.as_secs_f64(), playing)
    }
    /// Reopens the output after it failed and continues at the same position.
    fn recover(&mut self, reason: String) {
        let position = self.position();
        let was_playing = self.playing;
        let mut items = vec![];
        items.extend(self.queue.active.clone());
        items.extend(self.queue.tail());
        if items.is_empty() {
            self.release();
            self.queue.clear_playing();
            self.playing = false;
            return;
        }
        if self.queue.active.is_none() {
            self.suspend(items, 0.0);
            return;
        }
        let cycle = self.queue.cycle.clone();
        self.release();
        let result = self.restart(items, position, was_playing);
        self.queue.cycle = cycle;
        match result {
            Ok(()) => {
                let mut state = self.state.lock().unwrap();
                let name = state.output.clone();
                state.output_notice = Some(match state.output_notice.take() {
                    Some(notice) => format!("{reason}. {notice}"),
                    None => format!("{reason}. Continuing on {name}."),
                });
            }
            Err(error) => {
                self.state.lock().unwrap().error =
                    Some(format!("{reason}. Playback paused: {error:#}"));
            }
        }
    }
    /// A playing, unpaused output whose position does not move has stopped.
    fn stalled(&mut self) -> Option<String> {
        let sink = self.sink.as_ref()?;
        self.output.as_ref()?;
        let position = sink.get_pos();
        if !self.playing || sink.empty() || sink.is_paused() || position != self.progress.0 {
            self.progress = (position, Instant::now());
            return None;
        }
        (self.progress.1.elapsed() > self.stall_timeout).then(|| {
            self.progress.1 = Instant::now();
            "Audio output stopped responding".into()
        })
    }
    fn default_changed(&mut self) -> Option<String> {
        if self.settings.output_device.is_some()
            || self.last_default_check.elapsed() < self.default_check
        {
            return None;
        }
        let current = self.output.as_ref()?.name.clone();
        self.last_default_check = Instant::now();
        let default = self.opener.default_name()?;
        (default != current).then(|| format!("System default output changed to {default}"))
    }
    fn watch_output(&mut self) {
        #[cfg(windows)]
        if self
            .exclusive
            .as_ref()
            .is_some_and(|output| output.failed.load(std::sync::atomic::Ordering::Relaxed))
        {
            // The exclusive thread reports its own error. Keep the queue, paused.
            let position = self.position();
            let mut items = vec![];
            items.extend(self.queue.active.clone());
            items.extend(self.queue.tail());
            self.finish_listen();
            self.suspend(items, position);
            return;
        }
        let device = self
            .output
            .as_ref()
            .and_then(|o| o.failure.lock().ok()?.take());
        if let Some(reason) = device.as_ref()
            && self.speaker()
        {
            // A speaker that fails is left for this computer's own output; retrying would
            // only fail again.
            let name = self
                .output
                .as_ref()
                .map(|o| o.name.clone())
                .unwrap_or_default();
            self.settings.output_device = None;
            let _ = self.library.save_settings(&self.settings);
            self.recover(format!(
                "{name} stopped playing ({reason}). Playing on this computer"
            ));
            return;
        }
        let failure = self
            .lost
            .take()
            .or(device)
            .or_else(|| self.stalled())
            .or_else(|| self.default_changed());
        if let Some(reason) = failure {
            self.recover(reason);
        }
    }
    fn tick(&mut self) -> Result<()> {
        let elapsed = self.last_tick.elapsed().as_secs_f64();
        if elapsed > 1.0 {
            // The process was suspended; don't mistake that for a stalled device.
            self.progress.1 = Instant::now();
        }
        self.watch_output();
        let elapsed = elapsed.min(0.25);
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
                && self.queue.active.is_none();
            if !continuing {
                self.finish_listen();
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
            if let Some(control) = self.network() {
                control.set_meta(speaker_meta(&item.track));
            }
            self.queue.started(item);
        }
        if self.sink.is_some() {
            if self.playing
                && let Some(a) = loop_restart(self.loop_range, self.position())
            {
                self.seek(a)?;
            }
            let sink = self.sink.as_ref().unwrap();
            if sink.empty() && self.queue.pending.is_empty() && self.queue.staged.is_empty() {
                self.finish_listen();
                let next = self.queue.after_end();
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
        let queue =
            (self.published_version != self.queue.version).then(|| Arc::new(self.queue.upcoming()));
        self.published_version = self.queue.version;
        // A speaker plays a little behind; show where it is.
        let lag = self.network().map_or(0., |c| c.lag());
        let position = (self.position() - lag).max(0.);
        let mut state = self.state.lock().unwrap();
        state.output_device = self.settings.output_device.clone();
        state.current = self.queue.active.clone();
        if let Some(queue) = queue {
            state.queue = queue;
            state.queue_version = self.queue.version;
        }
        state.playing = self.playing;
        state.position = position;
        state.volume = self.settings.volume;
        state.repeat = self.queue.repeat;
        state.replay_gain = self.settings.replay_gain && !self.settings.exclusive;
        state.album_gain = state.replay_gain && self.settings.album_gain;
        state.loop_range = self.loop_range;
        state.exclusive = self.settings.exclusive;
        state.stems = self.stems.as_ref().map(|(id, _)| id.clone());
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        path::Path,
        sync::atomic::{AtomicBool, Ordering},
    };

    fn item(id: &str) -> QueueItem {
        QueueItem {
            track: Track {
                id: id.into(),
                title: id.into(),
                duration: 60.0,
                ..Default::default()
            },
            reason: "test".into(),
        }
    }
    fn items(ids: &[&str]) -> Vec<QueueItem> {
        ids.iter().map(|id| item(id)).collect()
    }
    fn ids<'a>(items: impl IntoIterator<Item = &'a QueueItem>) -> Vec<String> {
        items.into_iter().map(|i| i.track.id.clone()).collect()
    }
    fn queue(active: &str, staged: &[&str], pending: &[&str]) -> Queue {
        Queue {
            active: Some(item(active)),
            staged: items(staged).into(),
            pending: items(pending).into(),
            ..Default::default()
        }
    }

    #[test]
    fn replay_gain_uses_track_or_album_gain_with_peak_protection() {
        let mut settings = Settings::default();
        let track = Track {
            replay_gain: Some(-6.0),
            replay_peak: Some(0.5),
            album_replay_gain: Some(-3.0),
            album_peak: Some(0.9),
            ..Default::default()
        };
        assert_eq!(replay_gain_factor(&track, &settings), 1.0);
        settings.replay_gain = true;
        let track_gain = replay_gain_factor(&track, &settings);
        assert!((track_gain - 10f64.powf(-6.0 / 20.0)).abs() < 1e-12);
        settings.album_gain = true;
        let album = replay_gain_factor(&track, &settings);
        assert!((album - 10f64.powf(-3.0 / 20.0)).abs() < 1e-12);
        // Without album measurements, album mode falls back to track gain.
        let track_only = Track {
            album_replay_gain: None,
            ..track.clone()
        };
        assert_eq!(replay_gain_factor(&track_only, &settings), track_gain);
        // Positive gain is limited by the peak so the result stays below full scale.
        let loud = Track {
            album_replay_gain: Some(12.0),
            album_peak: Some(0.8),
            ..track.clone()
        };
        assert!((replay_gain_factor(&loud, &settings) - 1.25).abs() < 1e-12);
        // Missing or zero peaks don't protect; the boost is capped at +24 dB.
        let quiet = Track {
            replay_gain: Some(40.0),
            replay_peak: Some(0.0),
            ..Default::default()
        };
        assert_eq!(replay_gain_factor(&quiet, &settings), 16.0);
        assert_eq!(replay_gain_factor(&Track::default(), &settings), 1.0);
        settings.exclusive = true;
        assert_eq!(replay_gain_factor(&track, &settings), 1.0);
    }

    #[test]
    fn listens_qualify_at_half_or_four_minutes() {
        assert!(!qualifies(10.0, 0.0));
        assert!(!qualifies(29.9, 60.0));
        assert!(qualifies(30.0, 60.0));
        assert!(qualifies(240.0, 3600.0));
        assert!(!qualifies(239.0, 3600.0));
    }

    #[test]
    fn loop_bounds_are_validated_and_wrap_at_the_end() {
        assert!(validate_loop(Some((1.0, 2.0)), 60.0).is_ok());
        assert!(validate_loop(None, 0.0).unwrap().is_none());
        for range in [(-1.0, 2.0), (2.0, 2.0), (3.0, 2.0), (1.0, 61.0)] {
            assert!(validate_loop(Some(range), 60.0).is_err(), "{range:?}");
        }
        assert!(validate_loop(Some((0.0, 60.0)), 60.0).is_ok());
        assert_eq!(loop_restart(Some((1.0, 2.0)), 1.5), None);
        assert_eq!(loop_restart(Some((1.0, 2.0)), 2.0), Some(1.0));
        assert_eq!(loop_restart(None, 100.0), None);
    }

    #[test]
    fn session_restores_queue_position_and_repeat() {
        let session = Session {
            current: Some(("a".into(), "r".into())),
            queue: vec![
                ("b".into(), "r".into()),
                ("gone".into(), "r".into()),
                ("missing".into(), "r".into()),
                ("c".into(), "r".into()),
            ],
            position: 12.5,
            repeat: Repeat::Off,
        };
        let lookup = |id: &str| {
            (id != "gone").then(|| Track {
                id: id.into(),
                duration: 60.0,
                missing: id == "missing",
                ..Default::default()
            })
        };
        let resolved = session.resolve_with(lookup);
        assert_eq!(ids(&resolved), ["a", "b", "c"]);
        let (queue, position) = Queue::restore(&session, resolved.clone());
        assert_eq!(ids(&queue.active), ["a"]);
        assert_eq!(ids(&queue.pending), ["b", "c"]);
        assert_eq!(position, 12.5);
        let saved = queue.session(3.0);
        assert_eq!(saved.current, Some(("a".into(), "r".into())));
        assert_eq!(saved.queue.len(), 2);
        assert_eq!(saved.position, 3.0);
        // A finished track restarts; Repeat One holds the rest aside.
        let ended = Session {
            position: 60.0,
            repeat: Repeat::One,
            ..session.clone()
        };
        let (queue, position) = Queue::restore(&ended, resolved);
        assert_eq!(position, 0.0);
        assert!(queue.pending.is_empty());
        assert_eq!(ids(&queue.repeat_tail), ["b", "c"]);
        assert_eq!(ids(&queue.upcoming()), ["b", "c"]);
        assert_eq!(queue.session(0.0).queue.len(), 2);
        assert_eq!(Queue::restore(&Session::default(), vec![]).1, 0.0);
    }

    #[test]
    fn queue_edits_touch_only_what_changed() {
        let mut q = queue("a", &["b"], &["c", "d", "e"]);
        let version = q.version;
        assert_eq!(ids(&q.upcoming()), ["b", "c", "d", "e"]);
        // Pending items change in place; staged ones need the output rebuilt.
        assert!(matches!(q.remove(2), Edit::Done));
        assert_eq!(ids(&q.upcoming()), ["b", "c", "e"]);
        assert!(q.version > version);
        match q.remove(0) {
            Edit::Rebuild(tail) => assert_eq!(ids(&tail), ["c", "e"]),
            _ => panic!("staged removal must rebuild"),
        }
        assert!(matches!(q.remove(9), Edit::Unchanged));
        assert!(matches!(q.move_item(2, 1), Edit::Done));
        assert_eq!(ids(&q.upcoming()), ["b", "e", "c"]);
        match q.move_item(2, 0) {
            Edit::Rebuild(tail) => assert_eq!(ids(&tail), ["c", "b", "e"]),
            _ => panic!("moving before a staged item must rebuild"),
        }
        assert!(matches!(q.move_item(0, 3), Edit::Unchanged));
        // Play next goes before pending items, or rebuilds when one is staged.
        match q.insert_next(items(&["x", "y"])) {
            Edit::Rebuild(tail) => assert_eq!(ids(&tail), ["x", "y", "b", "e", "c"]),
            _ => panic!("staged items require a rebuild"),
        }
        let mut q = queue("a", &[], &["c"]);
        assert!(matches!(q.insert_next(items(&["x", "y"])), Edit::Done));
        assert_eq!(ids(&q.pending), ["x", "y", "c"]);
        q.append(items(&["z"]));
        assert_eq!(ids(&q.pending), ["x", "y", "c", "z"]);
    }

    #[test]
    fn repeat_one_holds_the_queue_aside() {
        let mut q = queue("a", &["b"], &["c"]);
        assert!(q.set_repeat(Repeat::One).unwrap().is_empty());
        assert_eq!(ids(&q.repeat_tail), ["b", "c"]);
        q.append(items(&["d"]));
        assert!(matches!(q.insert_next(items(&["x"])), Edit::Done));
        assert_eq!(ids(&q.repeat_tail), ["x", "b", "c", "d"]);
        assert!(matches!(q.remove(0), Edit::Done));
        assert!(matches!(q.move_item(2, 0), Edit::Done));
        assert_eq!(ids(&q.repeat_tail), ["d", "b", "c"]);
        assert!(matches!(q.remove(5), Edit::Unchanged));
        assert_eq!(ids(&q.after_end()), ["a"]);
        assert_eq!(q.next_held().unwrap().track.id, "d");
        assert_eq!(ids(&q.set_repeat(Repeat::All).unwrap()), ["b", "c"]);
        assert!(q.repeat_tail.is_empty());
        assert!(q.set_repeat(Repeat::Off).is_none());
        assert!(q.next_held().is_none());
        // Play under Repeat One starts only the first item.
        q.repeat = Repeat::One;
        assert_eq!(ids(&q.play_request(items(&["p", "q", "r"]))), ["p"]);
        assert_eq!(ids(&q.repeat_tail), ["q", "r"]);
        q.repeat = Repeat::Off;
        assert_eq!(ids(&q.play_request(items(&["p", "q"]))), ["p", "q"]);
        assert!(q.repeat_tail.is_empty());
    }

    #[test]
    fn queue_follows_started_items_previous_jump_and_end() {
        let mut q = Queue::default();
        q.load(items(&["a", "b", "c", "d"]));
        let a = q.pending.pop_front().unwrap();
        q.staged.push_back(a.clone());
        q.started(a);
        assert!(q.staged.is_empty());
        let b = q.pending.pop_front().unwrap();
        q.staged.push_back(b.clone());
        q.started(b);
        assert_eq!(ids(&q.active), ["b"]);
        assert_eq!(ids(&q.previous), ["a"]);
        assert_eq!(ids(&q.back().unwrap()), ["a", "b", "c", "d"]);
        assert!(q.back().is_none());
        q.repeat = Repeat::All;
        assert_eq!(ids(&q.after_end()), ["a", "b", "c", "d"]);
        q.repeat = Repeat::Off;
        assert!(q.after_end().is_empty());
        let mut q = queue("a", &["b"], &["c", "d"]);
        assert!(q.jump(3).is_none());
        assert_eq!(ids(&q.jump(1).unwrap()), ["c", "d"]);
        assert_eq!(ids(&q.previous), ["a"]);
        assert!(q.active.is_none());
        let mut q = queue("a", &[], &[]);
        q.repeat = Repeat::One;
        q.repeat_tail = items(&["b", "c", "d"]);
        assert_eq!(ids(&q.jump(1).unwrap()), ["c"]);
        assert_eq!(ids(&q.repeat_tail), ["d"]);
    }

    // A fake output device: a thread pulls samples from a Rodio sink at 20x
    // real time until the device is dropped or "unplugged".
    struct FakeDevice {
        pulling: Arc<AtomicBool>,
        failure: Arc<Mutex<Option<String>>>,
    }
    #[derive(Default)]
    struct Hardware {
        names: Vec<String>,
        default: Option<String>,
        opened: Vec<String>,
        devices: Vec<FakeDevice>,
    }
    #[derive(Clone, Default)]
    struct FakeOpener(Arc<Mutex<Hardware>>);
    struct Running(Arc<AtomicBool>);
    impl Drop for Running {
        fn drop(&mut self) {
            self.0.store(false, Ordering::Relaxed);
        }
    }
    impl OutputOpener for FakeOpener {
        fn open(&mut self, device: Option<&str>) -> Result<Option<OpenOutput>> {
            let mut hardware = self.0.lock().unwrap();
            let name = match device {
                Some(name) if hardware.names.iter().any(|n| n == name) => name.to_string(),
                Some(_) => return Ok(None),
                None => hardware
                    .default
                    .clone()
                    .context("No audio output device is available")?,
            };
            let (sink, mut source) = Sink::new();
            let running = Arc::new(AtomicBool::new(true));
            let pulling = Arc::new(AtomicBool::new(true));
            let failure = Arc::new(Mutex::new(None));
            let (alive, pull) = (running.clone(), pulling.clone());
            std::thread::spawn(move || {
                while alive.load(Ordering::Relaxed) {
                    if pull.load(Ordering::Relaxed) {
                        let n = source.sample_rate() as usize * source.channels() as usize / 50;
                        for _ in 0..n {
                            source.next();
                        }
                    }
                    std::thread::sleep(Duration::from_millis(1));
                }
            });
            hardware.opened.push(name.clone());
            hardware.devices.push(FakeDevice {
                pulling,
                failure: failure.clone(),
            });
            Ok(Some(OpenOutput {
                sink: Arc::new(sink),
                name,
                rate: 48000,
                channels: 2,
                failure,
                _stream: Box::new(Running(running)),
                network: None,
            }))
        }
        fn default_name(&mut self) -> Option<String> {
            self.0.lock().unwrap().default.clone()
        }
    }
    impl FakeOpener {
        fn with(names: &[&str], default: Option<&str>) -> Self {
            let opener = Self::default();
            opener.set(names, default);
            opener
        }
        fn opened(&self) -> Vec<String> {
            self.0.lock().unwrap().opened.clone()
        }
        /// Stops the latest device, optionally reporting an error as a removed endpoint does.
        fn kill(&self, error: Option<&str>) {
            let hardware = self.0.lock().unwrap();
            let device = hardware.devices.last().unwrap();
            device.pulling.store(false, Ordering::Relaxed);
            *device.failure.lock().unwrap() = error.map(String::from);
        }
        fn set(&self, names: &[&str], default: Option<&str>) {
            let mut hardware = self.0.lock().unwrap();
            hardware.names = names.iter().map(|n| n.to_string()).collect();
            hardware.default = default.map(String::from);
        }
    }

    struct Rig {
        worker: Worker,
        _dir: tempfile::TempDir,
        fixture: String,
    }
    fn rig(opener: FakeOpener, settings: Settings) -> Rig {
        let dir = tempfile::tempdir().unwrap();
        let library = Library::open(dir.path().join("library")).unwrap();
        let fixture = dir.path().join("tone.wav");
        let mut writer = hound::WavWriter::create(
            &fixture,
            hound::WavSpec {
                channels: 1,
                sample_rate: 8000,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            },
        )
        .unwrap();
        for i in 0..8000 * 60 {
            writer
                .write_sample(((i as f32 * 0.1).sin() * 3000.0) as i16)
                .unwrap();
        }
        writer.finalize().unwrap();
        let mut worker = Worker::new(
            library,
            Arc::new(Mutex::new(PlaybackState::default())),
            settings,
            Box::new(opener),
        );
        worker.stall_timeout = Duration::from_millis(150);
        worker.seek_timeout = Duration::from_millis(300);
        worker.default_check = Duration::ZERO;
        Rig {
            worker,
            _dir: dir,
            fixture: fixture.to_string_lossy().into(),
        }
    }
    impl Rig {
        fn items(&self, ids: &[&str]) -> Vec<QueueItem> {
            items(ids)
                .into_iter()
                .map(|mut i| {
                    i.track.path = self.fixture.clone();
                    i
                })
                .collect()
        }
        fn run(&mut self, command: Command) {
            self.worker.handle(command).unwrap();
            self.worker.tick().unwrap();
        }
        fn until(&mut self, what: &str, done: impl Fn(&Worker) -> bool) {
            let start = Instant::now();
            while !done(&self.worker) {
                assert!(
                    start.elapsed() < Duration::from_secs(10),
                    "timed out: {what}"
                );
                self.worker.tick().unwrap();
                std::thread::sleep(Duration::from_millis(2));
            }
        }
        fn until_active(&mut self, id: &str) {
            self.until(id, |w| {
                w.queue.active.as_ref().is_some_and(|i| i.track.id == id)
            });
        }
        fn state(&self) -> PlaybackState {
            self.worker.state.lock().unwrap().clone()
        }
        fn active(&self) -> Option<String> {
            self.worker
                .queue
                .active
                .as_ref()
                .map(|i| i.track.id.clone())
        }
    }

    #[test]
    fn plays_skips_jumps_and_inserts_through_an_output() {
        let opener = FakeOpener::with(&["Speakers"], Some("Speakers"));
        let mut rig = rig(opener, Settings::default());
        let list = rig.items(&["a", "b", "c", "d", "e"]);
        rig.run(Command::PlayAt(list, 1));
        rig.until_active("b");
        assert_eq!(ids(rig.state().queue.iter()), ["c", "d", "e"]);
        assert!(rig.worker.queue.previous.is_empty());
        assert_eq!(rig.worker.queue.cycle.len(), 5);
        rig.run(Command::Next);
        rig.until_active("c");
        assert_eq!(ids(&rig.worker.queue.previous), ["b"]);
        let next = rig.items(&["x"]);
        rig.run(Command::PlayNext(next));
        assert_eq!(rig.active().as_deref(), Some("c"));
        assert_eq!(ids(rig.state().queue.iter()), ["x", "d", "e"]);
        rig.run(Command::Jump(2));
        rig.until_active("e");
        assert!(rig.state().queue.is_empty());
        assert_eq!(ids(&rig.worker.queue.previous), ["b", "c"]);
        rig.run(Command::Previous);
        rig.until_active("c");
        assert_eq!(ids(rig.state().queue.iter()), ["e"]);
        assert!(rig.worker.handle(Command::PlayAt(vec![], 0)).is_err());
    }

    #[test]
    fn previous_after_the_queue_ends_plays_the_last_track_again() {
        let mut rig = rig(
            FakeOpener::with(&["Speakers"], Some("Speakers")),
            Settings::default(),
        );
        let list = rig.items(&["a"]);
        rig.run(Command::Play(list));
        rig.until_active("a");
        rig.run(Command::Seek(59.5));
        rig.until("the queue to finish", |w| !w.playing);
        rig.run(Command::Previous);
        rig.until_active("a");
        assert!(rig.worker.playing);
        assert!(rig.worker.position() < 5.0);
    }

    #[test]
    fn crossfade_starts_the_next_song_before_the_last_one_ends() {
        // Time from a seek to 55 s in a 60 s song until the next one starts. The fake output
        // is not real time, so compare against the same run without crossfade.
        let wait = |crossfade: f32| {
            let settings = Settings {
                crossfade,
                ..Settings::default()
            };
            let mut rig = rig(FakeOpener::with(&[], Some("Speakers")), settings);
            let list = rig.items(&["a", "b"]);
            rig.run(Command::Play(list));
            rig.until_active("a");
            rig.run(Command::Seek(55.));
            let seeked = Instant::now();
            rig.until_active("b");
            assert!(rig.state().error.is_none());
            seeked.elapsed().as_secs_f64()
        };
        let (plain, faded) = (wait(0.), wait(4.));
        // Five seconds of song without crossfade, one with a four-second fade.
        assert!(
            faded < plain * 0.5,
            "b started after {faded:.3} s with crossfade, {plain:.3} s without"
        );
    }

    #[test]
    fn opus_tracks_play_back_to_back() {
        let mut rig = rig(FakeOpener::with(&[], Some("Speakers")), Settings::default());
        let testdata = Path::new(env!("CARGO_MANIFEST_DIR")).join("testdata");
        let list: Vec<_> = ["tone.opus", "mono44.opus"]
            .into_iter()
            .map(|name| {
                let mut item = item(name);
                item.track.path = testdata.join(name).to_string_lossy().into();
                item
            })
            .collect();
        rig.run(Command::Play(list));
        rig.until_active("tone.opus");
        rig.until_active("mono44.opus");
        assert!(rig.state().error.is_none());
        rig.until("finished", |w| !w.playing);
    }

    #[test]
    fn large_queues_are_published_once_per_change() {
        let mut rig = rig(FakeOpener::with(&[], Some("Speakers")), Settings::default());
        let mut list = rig.items(&["first"]);
        list.extend((0..50_000).map(|i| QueueItem {
            track: Track {
                id: format!("t{i}"),
                path: rig.fixture.clone(),
                duration: 60.0,
                ..Default::default()
            },
            reason: "library".into(),
        }));
        rig.run(Command::Play(list));
        rig.until_active("first");
        rig.worker.tick().unwrap();
        let before = rig.state();
        assert_eq!(before.queue.len(), 50_000);
        let start = Instant::now();
        for _ in 0..20 {
            rig.worker.tick().unwrap();
        }
        let after = rig.state();
        assert!(
            Arc::ptr_eq(&before.queue, &after.queue),
            "tick copied the queue"
        );
        assert_eq!(before.queue_version, after.queue_version);
        assert!(start.elapsed() < Duration::from_secs(1));
        rig.run(Command::Remove(10));
        let removed = rig.state();
        assert_eq!(removed.queue.len(), 49_999);
        assert!(removed.queue_version > after.queue_version);
        assert_eq!(removed.queue[10].track.id, "t11");
        rig.run(Command::Move(20, 5));
        assert_eq!(rig.state().queue[5].track.id, "t21");
        assert_eq!(rig.active().as_deref(), Some("first"));
    }

    #[test]
    fn repeat_all_restarts_the_list_and_repeat_one_the_track() {
        let mut rig = rig(FakeOpener::with(&[], Some("Speakers")), Settings::default());
        let list = rig.items(&["a", "b"]);
        rig.run(Command::Repeat(Repeat::All));
        rig.run(Command::Play(list));
        rig.until_active("a");
        rig.run(Command::Next);
        rig.until_active("b");
        rig.run(Command::Next);
        rig.until_active("a");
        rig.run(Command::Repeat(Repeat::One));
        assert_eq!(ids(rig.state().queue.iter()), ["b"]);
        rig.run(Command::Next);
        rig.until_active("b");
        assert!(rig.state().queue.is_empty());
    }

    #[test]
    fn unplugged_device_reconnects_at_the_same_position() {
        let opener = FakeOpener::with(&["Speakers", "Headphones"], Some("Speakers"));
        let settings = Settings {
            output_device: Some("Headphones".into()),
            ..Default::default()
        };
        let mut rig = rig(opener.clone(), settings);
        let list = rig.items(&["a", "b"]);
        rig.run(Command::Play(list));
        rig.until("3 s played", |w| w.position() > 3.0);
        opener.set(&["Speakers"], Some("Speakers"));
        opener.kill(Some("Audio device error: device removed"));
        std::thread::sleep(Duration::from_millis(20));
        let position = rig.worker.position();
        rig.worker.tick().unwrap();
        assert_eq!(opener.opened(), ["Headphones", "Speakers"]);
        let state = rig.state();
        assert_eq!(state.output, "Speakers");
        assert!(state.playing);
        assert!(state.error.is_none());
        let notice = state.output_notice.unwrap();
        assert!(
            notice.contains("device removed") && notice.contains("Headphones"),
            "{notice}"
        );
        assert_eq!(rig.active().as_deref(), Some("a"));
        assert_eq!(ids(state.queue.iter()), ["b"]);
        assert!(rig.worker.position() >= position - 0.01, "{position}");
        assert!(rig.worker.position() < position + 3.0);
    }

    #[test]
    fn lost_output_without_replacement_pauses_and_resumes_later() {
        let opener = FakeOpener::with(&["Speakers"], Some("Speakers"));
        let mut rig = rig(opener.clone(), Settings::default());
        let list = rig.items(&["a", "b"]);
        rig.run(Command::Play(list));
        rig.until("2 s played", |w| w.position() > 2.0);
        rig.run(Command::Toggle);
        std::thread::sleep(Duration::from_millis(20));
        let position = rig.worker.position();
        opener.set(&[], None);
        opener.kill(Some(
            "Audio device error: The requested device is no longer available",
        ));
        rig.worker.tick().unwrap();
        let state = rig.state();
        assert!(!state.playing);
        let error = state.error.unwrap();
        assert!(
            error.contains("no longer available") && error.contains("paused"),
            "{error}"
        );
        assert_eq!(state.current.unwrap().track.id, "a");
        assert_eq!(ids(state.queue.iter()), ["b"]);
        assert!(rig.worker.sink.is_none());
        assert!((rig.worker.resume_position - position).abs() < 0.01);
        // Retrying without a device keeps everything in place.
        assert!(rig.worker.handle(Command::Toggle).is_err());
        assert_eq!(rig.active().as_deref(), Some("a"));
        opener.set(&["USB DAC"], Some("USB DAC"));
        rig.run(Command::Toggle);
        assert!(rig.worker.playing);
        assert_eq!(rig.state().output, "USB DAC");
        assert!(rig.worker.position() >= position - 0.01);
        rig.until_active("a");
    }

    #[test]
    fn missing_configured_device_falls_back_to_default() {
        let opener = FakeOpener::with(&["Speakers"], Some("Speakers"));
        let settings = Settings {
            output_device: Some("Old DAC".into()),
            ..Default::default()
        };
        let mut rig = rig(opener, settings);
        let list = rig.items(&["a"]);
        rig.run(Command::Play(list));
        let state = rig.state();
        assert_eq!(state.output, "Speakers");
        assert!(
            state
                .output_notice
                .unwrap()
                .contains("Old DAC is not connected")
        );
        assert!(state.error.is_none());
        let mut none = self::rig(FakeOpener::with(&[], None), Settings::default());
        let list = none.items(&["a"]);
        assert!(none.worker.handle(Command::Play(list)).is_err());
        assert_eq!(none.active().as_deref(), Some("a"));
        assert!(!none.worker.playing);
    }

    #[test]
    fn stalled_output_and_default_changes_reopen_the_device() {
        let opener = FakeOpener::with(&["Speakers"], Some("Speakers"));
        let mut rig = rig(opener.clone(), Settings::default());
        let list = rig.items(&["a"]);
        rig.run(Command::Play(list));
        rig.until("started", |w| w.position() > 0.5);
        opener.kill(None);
        rig.until("stall detected", |_| opener.opened().len() == 2);
        assert!(
            rig.state()
                .output_notice
                .unwrap()
                .contains("stopped responding")
        );
        assert!(rig.worker.playing);
        opener.set(&["Speakers", "USB DAC"], Some("USB DAC"));
        rig.worker.tick().unwrap();
        assert_eq!(opener.opened(), ["Speakers", "Speakers", "USB DAC"]);
        assert_eq!(rig.state().output, "USB DAC");
        assert_eq!(rig.active().as_deref(), Some("a"));
    }

    #[test]
    fn seeking_a_dead_output_does_not_hang() {
        let opener = FakeOpener::with(&["Speakers"], Some("Speakers"));
        let mut rig = rig(opener.clone(), Settings::default());
        let list = rig.items(&["a"]);
        rig.run(Command::Play(list));
        rig.until_active("a");
        rig.run(Command::Seek(20.0));
        assert!((rig.worker.position() - 20.0).abs() < 2.0);
        opener.kill(None);
        let start = Instant::now();
        assert!(rig.worker.handle(Command::Seek(5.0)).is_err());
        assert!(start.elapsed() < Duration::from_secs(2));
        rig.worker.tick().unwrap();
        assert_eq!(opener.opened().len(), 2);
        assert!(rig.worker.playing);
    }

    #[test]
    fn session_survives_a_restart_of_the_worker() {
        let mut rig = rig(FakeOpener::with(&[], Some("Speakers")), Settings::default());
        let list = rig.items(&["a", "b", "c"]);
        for item in &list {
            let track = Track {
                path: item.track.id.clone(),
                ..item.track.clone()
            };
            rig.worker.library.upsert(&track).unwrap();
        }
        rig.run(Command::Play(list));
        rig.until_active("a");
        rig.run(Command::Toggle);
        rig.run(Command::Seek(30.0));
        rig.run(Command::Shutdown);
        let worker = Worker::new(
            rig.worker.library.clone(),
            Arc::new(Mutex::new(PlaybackState::default())),
            Settings::default(),
            Box::new(FakeOpener::default()),
        );
        assert_eq!(ids(&worker.queue.active), ["a"]);
        assert_eq!(ids(&worker.queue.pending), ["b", "c"]);
        assert!((worker.resume_position - 30.0).abs() < 0.5);
    }

    #[test]
    #[ignore = "needs a real audio output device"]
    fn system_output_opens_and_plays_a_short_tone() {
        let dir = tempfile::tempdir().unwrap();
        crate::demo::create(dir.path()).unwrap();
        let path = std::fs::read_dir(dir.path())
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        assert!(!devices().unwrap().is_empty());
        let output = SystemOutput.open(None).unwrap().unwrap();
        assert_eq!(Some(output.name.clone()), default_device());
        output.sink.set_volume(0.05);
        let tone = crate::audio_file::decode(&path).unwrap();
        output
            .sink
            .append(tone.take_duration(Duration::from_millis(300)));
        output.sink.sleep_until_end();
        assert!(output.failure.lock().unwrap().is_none());
    }
}
