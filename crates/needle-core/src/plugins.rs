//! Plugins: small Rhai scripts that react to playback, add commands, and use a
//! permission-checked host API.
//!
//! A plugin is a folder in `<library>/plugins/` holding `plugin.toml` and a script:
//!
//! ```toml
//! id = "now-playing-file"
//! name = "Now playing to a file"
//! version = "1.0.0"
//! description = "Writes the current song to now-playing.txt"
//! author = "nnx"
//! entry = "main.rhai"
//! permissions = ["files"]
//! ```
//!
//! Scripts may define `on_load()`, `on_track_start(track)`, `on_listen(listen)`,
//! `on_pause()`, `on_resume()`, `commands()` (an array of `#{ id, title, scope }` where scope
//! is "track" or "global"), `run(command, track_ids)`, and `effects()` (sound effects, see
//! [`crate::effects`]). Plugins start disabled; enabling one grants the permissions it declares.
//! Every call is bounded in operations, depth, and size, and runs on the plugin thread, never
//! the interface or audio threads. Effects run on the audio thread as native blocks or as
//! sandboxed WebAssembly, never as script.
use crate::{
    database::Library,
    integrations::client,
    model::{Listen, Playlist, Track},
};
use anyhow::{Context, Result, bail};
use crossbeam_channel::{Receiver, Sender};
use rhai::{AST, Array, Dynamic, Engine, EvalAltResult, Map, Scope};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeSet, HashMap},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Permission {
    /// Search and read tracks, playlists, and listening history.
    #[serde(rename = "library.read")]
    LibraryRead,
    /// Change ratings and create or edit playlists.
    #[serde(rename = "library.write")]
    LibraryWrite,
    /// Start, pause, skip, and queue music.
    Playback,
    /// Make web requests.
    Network,
    /// Read and write files inside the plugin's own folder.
    Files,
    /// Change its effects' sliders and turn its effects on or off.
    Audio,
    /// Ask you to choose a file, or to type something.
    Ask,
}

impl Permission {
    pub fn describe(self) -> &'static str {
        match self {
            Self::LibraryRead => "Read your library and history",
            Self::LibraryWrite => "Change ratings and playlists",
            Self::Playback => "Control playback",
            Self::Network => "Use the internet",
            Self::Files => "Use files in its own folder",
            Self::Audio => "Change its sound effects",
            Self::Ask => "Ask you to choose a file or type something",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Manifest {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub author: String,
    #[serde(default = "default_entry")]
    pub entry: String,
    #[serde(default)]
    pub permissions: BTreeSet<Permission>,
    /// File types (extensions, such as "json" or "lrc") it opens when they are dropped on
    /// Needle's window. It gets them through `on_file_dropped(file)`.
    #[serde(default)]
    pub opens: Vec<String>,
}
fn default_entry() -> String {
    "main.rhai".into()
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Command {
    pub plugin: String,
    pub id: String,
    pub title: String,
    /// True when the command acts on selected tracks.
    pub for_tracks: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct PluginInfo {
    pub manifest: Manifest,
    pub folder: PathBuf,
    pub enabled: bool,
    pub error: Option<String>,
    pub commands: Vec<Command>,
    /// Names of the sound effects it adds.
    pub effects: Vec<String>,
    /// Set when the plugin is a music source (it defines `source()`).
    pub source: Option<SourceInfo>,
    /// The plugin finds lyrics (it defines `lyrics(song)`).
    pub lyrics: bool,
    /// A bundled plugin someone changed: the newer version this build of Needle has.
    pub update: Option<String>,
    /// One of the plugins that come with Needle (in its own folder, under its own id).
    pub official: bool,
    /// Official, and its files exactly as Needle wrote them.
    pub verified: bool,
}

/// A music source a plugin brings: what to ask for to sign in, and how it is doing.
#[derive(Clone, Debug, Default, Serialize)]
pub struct SourceInfo {
    /// The name people see, such as "Navidrome".
    pub name: String,
    pub fields: Vec<SourceField>,
    pub signed_in: bool,
    /// Songs from it in the library.
    pub songs: usize,
    /// When it was last synced (Unix seconds).
    pub synced_at: Option<i64>,
    pub syncing: bool,
    /// What went wrong signing in or syncing.
    pub error: Option<String>,
}

/// One box of a source's sign-in form.
#[derive(Clone, Debug, Default, Serialize)]
pub struct SourceField {
    pub id: String,
    pub label: String,
    pub placeholder: String,
    /// Typed text is hidden, and the plugin should keep it with `set_secret`.
    pub secret: bool,
}

/// What a plugin asks the application to do.
#[derive(Clone, Debug, PartialEq)]
pub enum HostAction {
    Notify(String),
    Play(Vec<String>),
    Enqueue(Vec<String>),
    PlayNext(Vec<String>),
    Toggle,
    Next,
    Previous,
    /// Ratings or playlists changed; refresh views.
    LibraryChanged,
    /// Set a slider of one of the plugin's effects, wherever it is in the listener's chain.
    EffectParam {
        plugin: String,
        effect: String,
        param: String,
        value: f32,
    },
    /// Turn one of the plugin's effects on (adding it to the chain if needed) or off.
    EffectOn {
        plugin: String,
        effect: String,
        on: bool,
    },
    /// Ask the person something for a plugin; give the reply with `answer(id, …)`.
    Ask {
        id: u64,
        /// The plugin's name, to show who is asking.
        plugin: String,
        question: Question,
    },
}

/// What a plugin asks.
#[derive(Clone, Debug, PartialEq)]
pub enum Question {
    /// Type something: a title, what to ask, and the text already in the box.
    Text {
        title: String,
        prompt: String,
        default: String,
    },
    /// Choose a file, of one of these types (none: any).
    File {
        title: String,
        extensions: Vec<String>,
    },
}

/// The most text a plugin gets from a chosen or dropped file.
pub const MAX_FILE_BYTES: u64 = 1 << 20;

/// Questions waiting for the person's reply, by id.
static ASKED: std::sync::LazyLock<
    Mutex<HashMap<u64, crossbeam_channel::Sender<Option<serde_json::Value>>>>,
> = std::sync::LazyLock::new(Default::default);
static NEXT_QUESTION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

/// The person's reply to a plugin's question (`None`: cancelled). For a file, the reply is a
/// map from `file_map`.
pub fn answer(id: u64, reply: Option<serde_json::Value>) {
    if let Some(waiting) = ASKED.lock().unwrap_or_else(|e| e.into_inner()).remove(&id) {
        let _ = waiting.send(reply);
    }
}

/// A file as a plugin gets it: its name, type, and text. Files over `MAX_FILE_BYTES` are
/// refused, and text that is not UTF-8 is read as well as it can be.
pub fn file_map(path: &Path) -> Result<serde_json::Value> {
    use std::io::Read;
    let mut bytes = vec![];
    std::fs::File::open(path)?
        .take(MAX_FILE_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_FILE_BYTES {
        bail!("{} is over 1 MB, too big for a plugin", path.display());
    }
    Ok(serde_json::json!({
        "name": path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(),
        "extension": path
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_default(),
        "text": String::from_utf8_lossy(&bytes),
    }))
}

/// Ask the person and wait for the reply (at most ten minutes).
fn ask(
    actions: &Arc<dyn Fn(HostAction) + Send + Sync>,
    plugin: &str,
    question: Question,
) -> Option<serde_json::Value> {
    let id = NEXT_QUESTION.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let (tx, rx) = crossbeam_channel::bounded(1);
    ASKED
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(id, tx);
    actions(HostAction::Ask {
        id,
        plugin: plugin.into(),
        question,
    });
    let reply = rx.recv_timeout(Duration::from_secs(600)).ok().flatten();
    ASKED.lock().unwrap_or_else(|e| e.into_inner()).remove(&id);
    reply
}

pub enum PluginEvent {
    TrackStarted(Box<Track>),
    Listen(Listen),
    Paused,
    Resumed,
    Run {
        plugin: String,
        command: String,
        track_ids: Vec<String>,
    },
    Enable(String, bool),
    Reload,
    Shutdown,
    /// Sign in to a source with what was typed in its form.
    SourceSignIn {
        plugin: String,
        fields: std::collections::BTreeMap<String, String>,
    },
    SourceSignOut(String),
    /// Fetch a source's whole list of songs again.
    SourceSync(String),
    /// One page of a sync (sent by the plugin thread to itself, so streams are not held up).
    SourceSyncPage {
        plugin: String,
        page: i64,
    },
    /// A link to stream one of a source's songs.
    Stream {
        plugin: String,
        id: String,
        reply: Sender<Result<String, String>>,
    },
    /// Replace a changed bundled plugin with this build's version (the changes are kept as
    /// `.mine` files).
    TakeUpdate(String),
    /// Keep a changed bundled plugin, and stop offering this version.
    KeepChanged(String),
    /// A file of a type the plugin opens was dropped on the window: `file_map` of it.
    FileDropped {
        plugin: String,
        file: serde_json::Value,
    },
    /// A song was rated in Needle.
    Rated {
        track_id: String,
        stars: i64,
    },
    /// Ask the plugins that find lyrics for a song's.
    Lyrics {
        song: Box<Track>,
        /// `Err` when a plugin failed and none had lyrics: then "no lyrics" is not known.
        reply: Sender<Result<Option<PluginLyrics>, String>>,
    },
}

/// Lyrics a plugin found.
#[derive(Clone, Debug, PartialEq)]
pub struct PluginLyrics {
    /// The plugin's name, to say where they came from.
    pub provider: String,
    /// LRC (timed) or plain text; empty for an instrumental.
    pub text: String,
    pub timed: bool,
    pub instrumental: bool,
}

/// What a plugin's `lyrics(song)` returned: `#{ synced: "[00:01.00]…" }`, `#{ plain: "…" }`,
/// `#{ instrumental: true }`, or a string (LRC or plain); anything else means none.
fn read_lyrics(value: &Dynamic, provider: &str) -> Option<PluginLyrics> {
    let found = |text: String, instrumental: bool| {
        let timed = !crate::media::parse_lrc(&text).is_empty();
        (instrumental || !text.trim().is_empty()).then(|| PluginLyrics {
            provider: provider.to_string(),
            text,
            timed,
            instrumental,
        })
    };
    if let Some(text) = value.clone().try_cast::<String>() {
        return found(text, false);
    }
    let map = value.clone().try_cast::<Map>()?;
    let text = |key: &str| {
        map.get(key)
            .and_then(|v| v.clone().try_cast::<String>())
            .filter(|t| !t.trim().is_empty())
    };
    if map
        .get("instrumental")
        .and_then(|v| v.as_bool().ok())
        .unwrap_or(false)
    {
        return found(String::new(), true);
    }
    text("synced")
        .or_else(|| text("plain"))
        .and_then(|t| found(t, false))
}

/// What a lyrics plugin is told about a song: enough to find it, nothing about the files.
fn lyrics_song_map(track: &Track) -> Map {
    let mut map = Map::new();
    map.insert("title".into(), track.title.clone().into());
    map.insert("artist".into(), track.display_artist().to_string().into());
    map.insert("album".into(), track.album.clone().into());
    map.insert("album_artist".into(), track.album_artist.clone().into());
    map.insert("duration".into(), track.duration.into());
    map
}

/// A source's list stops after this many pages, and nothing is changed: only a plugin
/// that keeps sending the same page gets there.
const MAX_SYNC_PAGES: i64 = 100_000;
/// The most songs one source's list may hold (far past any real library), so a runaway
/// plugin cannot fill the memory.
const MAX_SYNC_SONGS: usize = 2_000_000;

/// Sources are synced again on start when their last sync is older than this. A sync of a
/// large server takes a while (about 40 s for 7,000 songs on Navidrome, most of it the
/// server answering), so it is not done on every start; Sync in Settings does it at once.
const RESYNC_AFTER: i64 = 6 * 60 * 60;

/// How long after start a due sync waits, so it does not compete with Needle starting.
const SYNC_AFTER_START: Duration = Duration::from_secs(20);

const ENABLED_KEY: &str = "plugins_enabled";

struct Loaded {
    info: PluginInfo,
    engine: Engine,
    ast: Option<AST>,
    scope: Scope<'static>,
    effects: Vec<Arc<crate::effects::EffectDef>>,
}

/// Runs plugins on their own thread. Clone freely; all clones talk to the same thread.
#[derive(Clone)]
pub struct PluginHost {
    tx: Sender<PluginEvent>,
    infos: Arc<Mutex<Vec<PluginInfo>>>,
    folder: PathBuf,
}

impl PluginHost {
    pub fn start(
        library: Library,
        effects: Arc<crate::effects::Registry>,
        actions: impl Fn(HostAction) + Send + Sync + 'static,
    ) -> Self {
        let (tx, rx) = crossbeam_channel::unbounded();
        let infos = Arc::new(Mutex::new(vec![]));
        let folder = library.directory.join("plugins");
        let host = Self {
            tx,
            infos: infos.clone(),
            folder,
        };
        let actions: Arc<dyn Fn(HostAction) + Send + Sync> = Arc::new(actions);
        crate::sources::set_cache_dir(library.directory.join("stream-cache"));
        {
            let host = host.clone();
            crate::sources::set_resolver(move |plugin, id| host.stream_link(plugin, id));
        }
        let own = host.tx.clone();
        std::thread::Builder::new()
            .name("needle-plugins".into())
            .spawn(move || run(library, rx, own, infos, effects, actions))
            .expect("start plugin thread");
        let _ = host.tx.send(PluginEvent::Reload);
        host
    }
    pub fn send(&self, event: PluginEvent) {
        let _ = self.tx.send(event);
    }
    /// Ask the plugins that find lyrics (the turned-on ones, in order) for a song's: the first
    /// timed lyrics, or else the first plain ones. `None` at once when no plugin finds lyrics.
    ///
    /// `Err` when the plugins could not be asked, took too long, or failed without any of them
    /// finding lyrics: then it is not known that the song has none.
    pub fn lyrics(&self, track: &Track) -> Result<Option<PluginLyrics>> {
        if !self.plugins().iter().any(|p| p.enabled && p.lyrics) {
            return Ok(None);
        }
        let (reply, answer) = crossbeam_channel::bounded(1);
        self.tx
            .send(PluginEvent::Lyrics {
                song: Box::new(track.clone()),
                reply,
            })
            .map_err(|_| anyhow::anyhow!("Plugins are not running"))?;
        match answer.recv_timeout(Duration::from_secs(30)) {
            Ok(Ok(found)) => Ok(found),
            Ok(Err(problem)) => bail!("{problem}"),
            Err(_) => bail!("The lyrics plugins did not answer in time"),
        }
    }
    /// The turned-on plugins that find lyrics, by id: part of the key lyrics are kept under.
    pub fn lyrics_plugins(&self) -> Vec<String> {
        self.plugins()
            .into_iter()
            .filter(|p| p.enabled && p.lyrics)
            .map(|p| p.manifest.id)
            .collect()
    }
    /// Ask a source plugin for a link to stream one of its songs.
    pub fn stream_link(&self, plugin: &str, id: &str) -> Result<String> {
        let (reply, answer) = crossbeam_channel::bounded(1);
        self.tx
            .send(PluginEvent::Stream {
                plugin: plugin.into(),
                id: id.into(),
                reply,
            })
            .map_err(|_| anyhow::anyhow!("Plugins are not running"))?;
        // Syncs go a page at a time, so a stream waits at most for one page.
        match answer.recv_timeout(Duration::from_secs(45)) {
            Ok(Ok(link)) => Ok(link),
            Ok(Err(error)) => bail!("{error}"),
            Err(_) => bail!("The music source did not answer in time"),
        }
    }
    pub fn plugins(&self) -> Vec<PluginInfo> {
        self.infos.lock().unwrap_or_else(|p| p.into_inner()).clone()
    }
    pub fn commands(&self) -> Vec<Command> {
        self.plugins()
            .into_iter()
            .filter(|p| p.enabled)
            .flat_map(|p| p.commands)
            .collect()
    }
    pub fn folder(&self) -> &Path {
        &self.folder
    }
}

fn run(
    library: Library,
    rx: Receiver<PluginEvent>,
    tx: Sender<PluginEvent>,
    infos: Arc<Mutex<Vec<PluginInfo>>>,
    effects: Arc<crate::effects::Registry>,
    actions: Arc<dyn Fn(HostAction) + Send + Sync>,
) {
    let now_playing: Arc<Mutex<Option<Track>>> = Arc::default();
    let mut loaded: Vec<Loaded> = vec![];
    // Songs of syncs under way, by plugin.
    let mut syncing: HashMap<String, Vec<crate::sources::Song>> = HashMap::new();
    // After loading: read each source's state, and sync the ones not synced lately.
    let start_sources =
        |loaded: &mut Vec<Loaded>, syncing: &HashMap<String, Vec<crate::sources::Song>>| {
            let now = chrono::Utc::now().timestamp();
            for plugin in loaded.iter_mut() {
                refresh_source(plugin, &library);
                let id = plugin.info.manifest.id.clone();
                if let Some(source) = &mut plugin.info.source {
                    source.syncing = syncing.contains_key(&id);
                    if source.signed_in
                        && !source.syncing
                        // A time in the future (the clock went back) counts as stale too.
                        && source
                            .synced_at
                            .is_none_or(|at| at > now || now - at > RESYNC_AFTER)
                    {
                        let tx = tx.clone();
                        std::thread::spawn(move || {
                            std::thread::sleep(SYNC_AFTER_START);
                            let _ = tx.send(PluginEvent::SourceSync(id));
                        });
                    }
                }
            }
        };
    let publish = |loaded: &Vec<Loaded>| {
        *infos.lock().unwrap_or_else(|p| p.into_inner()) =
            loaded.iter().map(|l| l.info.clone()).collect();
    };
    let offer = |loaded: &Vec<Loaded>| {
        effects.set(
            loaded
                .iter()
                .filter(|l| l.info.enabled)
                .flat_map(|l| l.effects.iter().cloned())
                .collect(),
        );
    };
    while let Ok(event) = rx.recv() {
        match event {
            PluginEvent::Shutdown => break,
            PluginEvent::Reload => {
                loaded = load_all(&library, &actions, &now_playing);
                start_sources(&mut loaded, &syncing);
                publish(&loaded);
                offer(&loaded);
            }
            PluginEvent::Enable(id, on) => {
                let mut enabled: BTreeSet<String> = library
                    .get_json(ENABLED_KEY)
                    .ok()
                    .flatten()
                    .unwrap_or_default();
                if on {
                    enabled.insert(id);
                } else {
                    enabled.remove(&id);
                }
                let _ = library.set_json(ENABLED_KEY, &enabled);
                loaded = load_all(&library, &actions, &now_playing);
                start_sources(&mut loaded, &syncing);
                publish(&loaded);
                offer(&loaded);
            }
            PluginEvent::TrackStarted(track) => {
                *now_playing.lock().unwrap_or_else(|p| p.into_inner()) = Some((*track).clone());
                let value = track_map(&track);
                for plugin in loaded.iter_mut() {
                    let _ = call(plugin, "on_track_start", vec![value.clone().into()]);
                }
                // Tell a source's server what is playing.
                if let Some((owner, id)) = track.source()
                    && let Some(plugin) = source_plugin(&mut loaded, owner)
                {
                    let _ = call_source(plugin, "playing", vec![id.into()]);
                }
                publish(&loaded);
            }
            PluginEvent::Listen(listen) => {
                let value = listen_map(&listen);
                for plugin in loaded.iter_mut() {
                    let _ = call(plugin, "on_listen", vec![value.clone().into()]);
                }
                // A played song from a source counts on its server too.
                if listen.qualified
                    && let Ok(Some(track)) = library.track(&listen.track_id)
                    && let Some((owner, id)) = track.source()
                    && let Some(plugin) = source_plugin(&mut loaded, owner)
                {
                    let _ =
                        call_source(plugin, "played", vec![id.into(), listen.started_at.into()]);
                }
                publish(&loaded);
            }
            PluginEvent::TakeUpdate(ref id) | PluginEvent::KeepChanged(ref id) => {
                let take = matches!(event, PluginEvent::TakeUpdate(_));
                if let Some(plugin) = loaded.iter().find(|p| p.info.manifest.id == *id) {
                    let dir = plugin.info.folder.clone();
                    let result = if take {
                        take_bundled_update(&dir, id)
                    } else {
                        keep_changed_plugin(&dir, id)
                    };
                    match result {
                        Ok(()) if take => actions(HostAction::Notify(format!(
                            "Updated {}. Your changes are kept next to it as .mine files.",
                            plugin.info.manifest.name
                        ))),
                        Ok(()) => {}
                        Err(error) => actions(HostAction::Notify(format!("{error:#}"))),
                    }
                }
                loaded = load_all(&library, &actions, &now_playing);
                start_sources(&mut loaded, &syncing);
                publish(&loaded);
                offer(&loaded);
            }
            PluginEvent::FileDropped { plugin, file } => {
                if let Some(target) = loaded
                    .iter_mut()
                    .find(|p| p.info.manifest.id == plugin && p.info.enabled)
                    && let Ok(file) = rhai::serde::to_dynamic(&file)
                    && let Err(error) = call(target, "on_file_dropped", vec![file])
                {
                    actions(HostAction::Notify(format!(
                        "{}: {error}",
                        target.info.manifest.name
                    )));
                }
                publish(&loaded);
            }
            PluginEvent::Lyrics { song, reply } => {
                let map = lyrics_song_map(&song);
                let mut plain = None;
                let mut timed = None;
                let mut failed = None;
                for plugin in loaded
                    .iter_mut()
                    .filter(|p| p.info.enabled && p.info.lyrics)
                {
                    let name = plugin.info.manifest.name.clone();
                    let value = match call_source(plugin, "lyrics", vec![map.clone().into()]) {
                        Ok(value) => value,
                        Err(problem) => {
                            failed.get_or_insert(format!("{name}: {problem}"));
                            continue;
                        }
                    };
                    match read_lyrics(&value, &name) {
                        Some(found) if found.timed || found.instrumental => {
                            timed = Some(found);
                            break;
                        }
                        Some(found) => {
                            plain.get_or_insert(found);
                        }
                        None => {}
                    }
                }
                let _ = reply.send(match (timed.or(plain), failed) {
                    (Some(found), _) => Ok(Some(found)),
                    (None, Some(problem)) => Err(problem),
                    (None, None) => Ok(None),
                });
            }
            PluginEvent::Rated { track_id, stars } => {
                if let Ok(Some(track)) = library.track(&track_id)
                    && let Some((owner, id)) = track.source()
                    && let Some(plugin) = source_plugin(&mut loaded, owner)
                    && let Err(error) = call_source(plugin, "rate", vec![id.into(), stars.into()])
                {
                    actions(HostAction::Notify(format!(
                        "{}: the rating was not saved on the server: {error}",
                        plugin.info.manifest.name
                    )));
                }
            }
            PluginEvent::Stream { plugin, id, reply } => {
                let answer = match source_plugin(&mut loaded, &plugin) {
                    None => Err(format!(
                        "Turn on the plugin {plugin} in Settings › Plugins to play this song"
                    )),
                    Some(target) => match call_source(target, "stream", vec![id.into()]) {
                        Ok(link) => link
                            .into_string()
                            .map_err(|_| "The plugin gave no stream link".to_string()),
                        Err(error) => Err(error),
                    },
                };
                let _ = reply.send(answer);
            }
            PluginEvent::SourceSignIn { plugin, fields } => {
                if let Some(target) = source_plugin(&mut loaded, &plugin) {
                    let map: Map = fields
                        .into_iter()
                        .map(|(k, v)| (k.into(), Dynamic::from(v)))
                        .collect();
                    let result = call_source(target, "sign_in", vec![map.into()]);
                    refresh_source(target, &library);
                    if let Some(source) = &mut target.info.source {
                        match result {
                            Ok(_) if source.signed_in => {
                                source.error = None;
                                let _ = tx.send(PluginEvent::SourceSync(plugin.clone()));
                            }
                            Ok(_) => source.error = Some("Signing in did not work".into()),
                            Err(error) => source.error = Some(error),
                        }
                    }
                }
                publish(&loaded);
            }
            PluginEvent::SourceSignOut(plugin) => {
                if let Some(target) = source_plugin(&mut loaded, &plugin) {
                    let _ = call_source(target, "sign_out", vec![]);
                    let _ = crate::sources::forget(&library, &plugin);
                    refresh_source(target, &library);
                    actions(HostAction::LibraryChanged);
                }
                syncing.remove(&plugin);
                publish(&loaded);
            }
            PluginEvent::SourceSync(plugin) => {
                if !syncing.contains_key(&plugin)
                    && let Some(target) = source_plugin(&mut loaded, &plugin)
                    && let Some(source) = &mut target.info.source
                    && source.signed_in
                {
                    source.syncing = true;
                    source.error = None;
                    syncing.insert(plugin.clone(), vec![]);
                    let _ = tx.send(PluginEvent::SourceSyncPage { plugin, page: 0 });
                }
                publish(&loaded);
            }
            PluginEvent::SourceSyncPage { plugin, page } => {
                let Some(target) = source_plugin(&mut loaded, &plugin) else {
                    syncing.remove(&plugin);
                    continue;
                };
                let name = target.info.manifest.name.clone();
                let result = call_source(target, "songs", vec![page.into()]);
                // Only a list counts as a page; anything else (or no songs() at all) is an
                // error, never "the server has no songs".
                let (list, mut error) = match result {
                    Ok(value) => match serde_json::to_value(&value)
                        .ok()
                        .and_then(|v| v.as_array().cloned())
                    {
                        Some(list) => (list, None),
                        None => (
                            vec![],
                            Some("The plugin's songs() did not give a list of songs".to_string()),
                        ),
                    },
                    Err(error) => (vec![], Some(error)),
                };
                let songs = syncing.entry(plugin.clone()).or_default();
                if songs.len() + list.len() > MAX_SYNC_SONGS {
                    error = Some(format!(
                        "The server has more than {MAX_SYNC_SONGS} songs; nothing was changed"
                    ));
                } else {
                    songs.extend(list.iter().filter_map(crate::sources::Song::from_value));
                }
                // Pages until an empty one. The limit only stops a plugin that never ends.
                let endless = page >= MAX_SYNC_PAGES;
                if error.is_none() && !list.is_empty() && !endless {
                    let _ = tx.send(PluginEvent::SourceSyncPage {
                        plugin,
                        page: page + 1,
                    });
                    continue;
                }
                let songs = syncing.remove(&plugin).unwrap_or_default();
                // A server with no songs is a valid answer: its songs are marked missing (not
                // deleted), and come back with their ratings when it lists them again.
                let outcome = match error {
                    Some(error) => Err(error),
                    None if endless && !list.is_empty() => Err(format!(
                        "The plugin sent more than {MAX_SYNC_PAGES} pages; nothing was changed"
                    )),
                    None => crate::sources::apply(&library, &plugin, &songs)
                        .map_err(|e| format!("{e:#}")),
                };
                if let Some(source) = &mut target.info.source {
                    source.syncing = false;
                    source.error = outcome.as_ref().err().cloned();
                }
                refresh_source(target, &library);
                match outcome {
                    Ok(synced) => {
                        if synced.added + synced.gone > 0 {
                            actions(HostAction::Notify(format!(
                                "{name}: {} new songs{}.",
                                synced.added,
                                if synced.gone > 0 {
                                    format!(", {} no longer on the server", synced.gone)
                                } else {
                                    String::new()
                                }
                            )));
                        }
                        actions(HostAction::LibraryChanged);
                        if !synced.covers.is_empty() {
                            let (library, actions) = (library.clone(), actions.clone());
                            std::thread::spawn(move || {
                                let shown = || actions(HostAction::LibraryChanged);
                                if crate::sources::fetch_covers(&library, &synced.covers, shown) > 0
                                {
                                    actions(HostAction::LibraryChanged);
                                }
                            });
                        }
                    }
                    Err(error) => actions(HostAction::Notify(format!(
                        "{name} could not sync: {error}"
                    ))),
                }
                publish(&loaded);
            }
            PluginEvent::Paused | PluginEvent::Resumed => {
                let name = if matches!(event, PluginEvent::Paused) {
                    "on_pause"
                } else {
                    "on_resume"
                };
                for plugin in loaded.iter_mut() {
                    let _ = call(plugin, name, vec![]);
                }
                publish(&loaded);
            }
            PluginEvent::Run {
                plugin,
                command,
                track_ids,
            } => {
                if let Some(target) = loaded
                    .iter_mut()
                    .find(|p| p.info.manifest.id == plugin && p.info.enabled)
                {
                    let ids: Array = track_ids.into_iter().map(Dynamic::from).collect();
                    if let Err(error) = call(target, "run", vec![command.into(), ids.into()]) {
                        actions(HostAction::Notify(format!(
                            "{}: {error}",
                            target.info.manifest.name
                        )));
                    }
                }
                publish(&loaded);
            }
        }
    }
}

/// The turned-on plugin with this id, if it is a music source.
fn source_plugin<'a>(loaded: &'a mut [Loaded], id: &str) -> Option<&'a mut Loaded> {
    loaded
        .iter_mut()
        .find(|p| p.info.manifest.id == id && p.info.enabled && p.info.source.is_some())
}

/// Read a source plugin's description and whether it is signed in.
fn refresh_source(plugin: &mut Loaded, library: &Library) {
    let defines = |plugin: &Loaded, name: &str| {
        plugin
            .ast
            .as_ref()
            .is_some_and(|ast| ast.iter_functions().any(|f| f.name == name))
    };
    plugin.info.lyrics = plugin.ast.as_ref().is_some_and(|ast| {
        ast.iter_functions()
            .any(|f| f.name == "lyrics" && f.params.len() == 1)
    });
    // Needle fetches the links a source gives it, so being one needs the network permission:
    // otherwise a plugin could send library data out inside a link.
    if !plugin.info.enabled
        || !defines(plugin, "source")
        || !plugin
            .info
            .manifest
            .permissions
            .contains(&Permission::Network)
    {
        plugin.info.source = None;
        return;
    }
    let previous = plugin.info.source.take().unwrap_or_default();
    let about = call_source(plugin, "source", vec![]).ok();
    let about = about
        .and_then(|d| serde_json::to_value(&d).ok())
        .unwrap_or_default();
    let text = |v: &serde_json::Value| v.as_str().unwrap_or_default().to_string();
    let fields = about["fields"]
        .as_array()
        .map(|list| {
            list.iter()
                .filter(|f| f["id"].is_string())
                .map(|f| SourceField {
                    id: text(&f["id"]),
                    label: Some(text(&f["label"]))
                        .filter(|l| !l.is_empty())
                        .unwrap_or_else(|| text(&f["id"])),
                    placeholder: text(&f["placeholder"]),
                    secret: f["secret"].as_bool().unwrap_or(false),
                })
                .collect()
        })
        .unwrap_or_default();
    let signed_in = call_source(plugin, "signed_in", vec![])
        .ok()
        .and_then(|d| d.as_bool().ok())
        .unwrap_or(false);
    let (songs, synced_at) = crate::sources::status(library, &plugin.info.manifest.id);
    plugin.info.source = Some(SourceInfo {
        name: Some(text(&about["name"]))
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| plugin.info.manifest.name.clone()),
        fields,
        signed_in,
        songs,
        synced_at,
        ..previous
    });
}

/// The message a script threw, without the engine's wrapping.
fn plain_error(error: &EvalAltResult) -> String {
    match error {
        EvalAltResult::ErrorRuntime(value, _) => value.to_string(),
        EvalAltResult::ErrorInFunctionCall(_, _, inner, _) => plain_error(inner),
        other => other.to_string(),
    }
}

/// Call a source function: like `call`, but a failure (a wrong password, a server that is
/// down) is returned as its plain message and does not mark the plugin as broken.
fn call_source(plugin: &mut Loaded, name: &str, args: Vec<Dynamic>) -> Result<Dynamic, String> {
    let Some(ast) = &plugin.ast else {
        return Ok(Dynamic::UNIT);
    };
    if !plugin.info.enabled
        || !ast
            .iter_functions()
            .any(|f| f.name == name && f.params.len() == args.len())
    {
        return Ok(Dynamic::UNIT);
    }
    plugin
        .engine
        .call_fn::<Dynamic>(&mut plugin.scope, ast, name, args)
        .map_err(|error| {
            let message = plain_error(&error);
            crate::logfile::warn(format!(
                "Source {} failed in {name}: {message}",
                plugin.info.manifest.id
            ));
            message
        })
}

/// Call `name` if the plugin defines it; failures are recorded on the plugin, not raised.
fn call(plugin: &mut Loaded, name: &str, args: Vec<Dynamic>) -> Result<Dynamic, String> {
    let Some(ast) = &plugin.ast else {
        return Ok(Dynamic::UNIT);
    };
    if !plugin.info.enabled
        || !ast
            .iter_functions()
            .any(|f| f.name == name && f.params.len() == args.len())
    {
        return Ok(Dynamic::UNIT);
    }
    match plugin
        .engine
        .call_fn::<Dynamic>(&mut plugin.scope, ast, name, args)
    {
        Ok(value) => Ok(value),
        Err(error) => {
            let message = error.to_string();
            crate::logfile::warn(format!(
                "Plugin {} failed in {name}: {message}",
                plugin.info.manifest.id
            ));
            plugin.info.error = Some(format!("{name}: {message}"));
            Err(message)
        }
    }
}

/// Find, read, and (for enabled plugins) start every plugin folder.
fn load_all(
    library: &Library,
    actions: &Arc<dyn Fn(HostAction) + Send + Sync>,
    now_playing: &Arc<Mutex<Option<Track>>>,
) -> Vec<Loaded> {
    let folder = library.directory.join("plugins");
    let _ = std::fs::create_dir_all(&folder);
    update_examples(&folder);
    let enabled: BTreeSet<String> = library
        .get_json(ENABLED_KEY)
        .ok()
        .flatten()
        .unwrap_or_default();
    let mut folders: Vec<PathBuf> = std::fs::read_dir(&folder)
        .map(|entries| {
            entries
                .filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| p.join("plugin.toml").is_file())
                .collect()
        })
        .unwrap_or_default();
    folders.sort();
    let mut loaded = vec![];
    let mut seen = BTreeSet::new();
    for dir in folders {
        let manifest = match read_manifest(&dir) {
            Ok(m) => m,
            Err(error) => {
                let name = dir
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                loaded.push(Loaded {
                    info: PluginInfo {
                        manifest: Manifest {
                            id: name.clone(),
                            name,
                            version: String::new(),
                            description: String::new(),
                            author: String::new(),
                            entry: default_entry(),
                            permissions: BTreeSet::new(),
                            opens: vec![],
                        },
                        folder: dir,
                        enabled: false,
                        error: Some(format!("{error:#}")),
                        commands: vec![],
                        effects: vec![],
                        source: None,
                        lyrics: false,
                        update: None,
                        official: false,
                        verified: false,
                    },
                    engine: Engine::new_raw(),
                    ast: None,
                    scope: Scope::new(),
                    effects: vec![],
                });
                continue;
            }
        };
        if !seen.insert(manifest.id.clone()) {
            continue;
        }
        let is_enabled = enabled.contains(&manifest.id);
        let engine = engine_for(
            &manifest,
            &dir,
            library.clone(),
            actions.clone(),
            now_playing.clone(),
        );
        let mut plugin = Loaded {
            info: PluginInfo {
                manifest: manifest.clone(),
                folder: dir.clone(),
                enabled: is_enabled,
                error: None,
                commands: vec![],
                effects: vec![],
                source: None,
                lyrics: false,
                update: bundled_update(&dir, &manifest.id),
                official: is_official(&dir, &manifest.id),
                verified: is_official(&dir, &manifest.id)
                    && unedited_version(&dir, &manifest.id).is_some(),
            },
            engine,
            ast: None,
            scope: Scope::new(),
            effects: vec![],
        };
        // A plugin that only brings themes (a `themes` folder) needs no script.
        if !dir.join(&manifest.entry).exists() && dir.join("themes").is_dir() {
            loaded.push(plugin);
            continue;
        }
        match std::fs::read_to_string(dir.join(&manifest.entry))
            .context("Cannot read the script")
            .and_then(|source| {
                plugin
                    .engine
                    .compile(&source)
                    .map_err(|e| anyhow::anyhow!("{e}"))
            }) {
            Ok(ast) => {
                plugin.ast = Some(ast);
                if is_enabled {
                    if let Some(ast) = &plugin.ast
                        && let Err(error) = plugin.engine.run_ast_with_scope(&mut plugin.scope, ast)
                    {
                        plugin.info.error = Some(error.to_string());
                    }
                    let _ = call(&mut plugin, "on_load", vec![]);
                    if let Ok(list) = call(&mut plugin, "commands", vec![]) {
                        plugin.info.commands = commands_from(&manifest.id, list);
                    }
                    if let Ok(list) = call(&mut plugin, "effects", vec![])
                        && !list.is_unit()
                    {
                        let effects = serde_json::to_value(&list)
                            .map_err(anyhow::Error::from)
                            .and_then(|value| {
                                crate::effects::load(&manifest.id, &manifest.name, &dir, value)
                            });
                        match effects {
                            Ok(effects) => {
                                plugin.info.effects =
                                    effects.iter().map(|e| e.name.clone()).collect();
                                plugin.effects = effects.into_iter().map(Arc::new).collect();
                            }
                            Err(error) => {
                                plugin.info.error = Some(format!("effects: {error:#}"));
                            }
                        }
                    }
                }
            }
            Err(error) => plugin.info.error = Some(format!("{error:#}")),
        }
        loaded.push(plugin);
    }
    loaded
}

pub fn read_manifest(dir: &Path) -> Result<Manifest> {
    let text =
        std::fs::read_to_string(dir.join("plugin.toml")).context("Cannot read plugin.toml")?;
    let manifest: Manifest = toml::from_str(&text).context("plugin.toml is not valid")?;
    if manifest.id.is_empty()
        || !manifest
            .id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        bail!("The plugin id must use only letters, digits, - and _");
    }
    if manifest.entry.contains("..") || Path::new(&manifest.entry).is_absolute() {
        bail!("The entry script must be inside the plugin folder");
    }
    Ok(manifest)
}

fn commands_from(plugin: &str, value: Dynamic) -> Vec<Command> {
    let Some(list) = value.try_cast::<Array>() else {
        return vec![];
    };
    list.into_iter()
        .filter_map(|item| {
            let map = item.try_cast::<Map>()?;
            let text = |k: &str| map.get(k).and_then(|v| v.clone().into_string().ok());
            Some(Command {
                plugin: plugin.into(),
                id: text("id")?,
                title: text("title")?,
                for_tracks: text("scope").as_deref() != Some("global"),
            })
        })
        .take(32)
        .collect()
}

fn track_map(track: &Track) -> Map {
    let mut map = Map::new();
    map.insert("id".into(), track.id.clone().into());
    map.insert("title".into(), track.title.clone().into());
    map.insert("artist".into(), track.artist.clone().into());
    map.insert("album".into(), track.album.clone().into());
    map.insert("album_artist".into(), track.album_artist.clone().into());
    map.insert("genre".into(), track.genre.clone().into());
    map.insert("year".into(), track.year.into());
    map.insert("duration".into(), track.duration.into());
    map.insert("rating".into(), track.rating.into());
    map.insert("play_count".into(), track.play_count.into());
    map.insert("format".into(), track.format.clone().into());
    map.insert(
        "path".into(),
        track.path.trim_start_matches("\\\\?\\").to_string().into(),
    );
    if let Some((source, id)) = track.source() {
        map.insert("source".into(), source.to_string().into());
        map.insert("source_id".into(), id.into());
    }
    map
}
fn listen_map(listen: &Listen) -> Map {
    let mut map = Map::new();
    map.insert("track_id".into(), listen.track_id.clone().into());
    map.insert("title".into(), listen.title.clone().into());
    map.insert("artist".into(), listen.artist.clone().into());
    map.insert("album".into(), listen.album.clone().into());
    map.insert("started_at".into(), listen.started_at.into());
    map.insert("seconds".into(), listen.listened_seconds.into());
    map.insert("qualified".into(), listen.qualified.into());
    map
}

type Fail = Box<EvalAltResult>;
fn fail(message: impl Into<String>) -> Fail {
    message.into().into()
}
fn ids(array: Array) -> Vec<String> {
    array
        .into_iter()
        .filter_map(|v| v.into_string().ok())
        .collect()
}

/// An engine with safety limits and the host functions this plugin's permissions allow.
/// Functions outside its permissions exist but fail with a clear message.
fn engine_for(
    manifest: &Manifest,
    folder: &Path,
    library: Library,
    actions: Arc<dyn Fn(HostAction) + Send + Sync>,
    now_playing: Arc<Mutex<Option<Track>>>,
) -> Engine {
    let mut engine = Engine::new();
    engine.set_max_operations(2_000_000);
    engine.set_max_call_levels(48);
    engine.set_max_expr_depths(64, 32);
    engine.set_max_string_size(1 << 20);
    // Sizes count everything inside a value: a page of songs from a server, each with a few
    // dozen fields, is one value.
    engine.set_max_array_size(50_000);
    engine.set_max_map_size(200_000);
    engine.disable_symbol("eval");
    let permissions = manifest.permissions.clone();
    let name = manifest.name.clone();
    let id = manifest.id.clone();
    let allowed = move |p: Permission| -> Result<(), Fail> {
        if permissions.contains(&p) {
            Ok(())
        } else {
            Err(fail(format!(
                "{name} did not ask for permission to {}",
                p.describe().to_lowercase()
            )))
        }
    };

    {
        let (label, actions) = (manifest.name.clone(), actions.clone());
        engine.register_fn("notify", move |text: &str| {
            actions(HostAction::Notify(format!("{label}: {text}")))
        });
    }
    {
        let id = manifest.id.clone();
        engine.register_fn("log", move |text: &str| {
            crate::logfile::info(format!("[plugin {id}] {text}"))
        });
    }
    engine.register_fn("now", || chrono::Utc::now().timestamp());
    {
        let now_playing = now_playing.clone();
        engine.register_fn("now_playing", move || -> Dynamic {
            match &*now_playing.lock().unwrap_or_else(|p| p.into_inner()) {
                Some(track) => track_map(track).into(),
                None => Dynamic::UNIT,
            }
        });
    }
    // Library
    {
        let (library, allowed) = (library.clone(), allowed.clone());
        engine.register_fn("library_search", move |rule: &str| -> Result<Array, Fail> {
            allowed(Permission::LibraryRead)?;
            let tracks = library.search(rule).map_err(|e| fail(format!("{e:#}")))?;
            Ok(tracks
                .iter()
                .take(10_000)
                .map(|t| Dynamic::from(track_map(t)))
                .collect())
        });
    }
    {
        let (library, allowed) = (library.clone(), allowed.clone());
        engine.register_fn("recent_listens", move |count: i64| -> Result<Array, Fail> {
            allowed(Permission::LibraryRead)?;
            let listens = library
                .history(count.clamp(1, 1000) as usize)
                .map_err(|e| fail(format!("{e:#}")))?;
            Ok(listens
                .iter()
                .map(|l| Dynamic::from(listen_map(l)))
                .collect())
        });
    }
    {
        let (library, allowed, actions) = (library.clone(), allowed.clone(), actions.clone());
        engine.register_fn(
            "set_rating",
            move |track: &str, stars: i64| -> Result<(), Fail> {
                allowed(Permission::LibraryWrite)?;
                library
                    .rate(track, stars.clamp(0, 5))
                    .map_err(|e| fail(format!("{e:#}")))?;
                actions(HostAction::LibraryChanged);
                Ok(())
            },
        );
    }
    {
        let (library, allowed, actions, plugin) = (
            library.clone(),
            allowed.clone(),
            actions.clone(),
            id.clone(),
        );
        engine.register_fn(
            "save_playlist",
            move |name: &str, tracks: Array| -> Result<(), Fail> {
                allowed(Permission::LibraryWrite)?;
                // One playlist per plugin and name, so running a command again updates it.
                let id = format!(
                    "plugin-{plugin}-{}",
                    &blake3::hash(name.as_bytes()).to_hex()[..12]
                );
                library
                    .save_playlist(&Playlist {
                        id,
                        name: name.to_string(),
                        query: None,
                        track_ids: ids(tracks),
                        updated_at: chrono::Utc::now().timestamp(),
                        ..Default::default()
                    })
                    .map_err(|e| fail(format!("{e:#}")))?;
                actions(HostAction::LibraryChanged);
                Ok(())
            },
        );
    }
    // Playback
    for (function, make) in [
        ("play", HostAction::Play as fn(Vec<String>) -> HostAction),
        ("enqueue", HostAction::Enqueue),
        ("play_next", HostAction::PlayNext),
    ] {
        let (allowed, actions) = (allowed.clone(), actions.clone());
        engine.register_fn(function, move |tracks: Array| -> Result<(), Fail> {
            allowed(Permission::Playback)?;
            actions(make(ids(tracks)));
            Ok(())
        });
    }
    for (function, action) in [
        ("toggle_playback", HostAction::Toggle),
        ("next_track", HostAction::Next),
        ("previous_track", HostAction::Previous),
    ] {
        let (allowed, actions) = (allowed.clone(), actions.clone());
        engine.register_fn(function, move || -> Result<(), Fail> {
            allowed(Permission::Playback)?;
            actions(action.clone());
            Ok(())
        });
    }
    // Sound effects
    {
        let (allowed, actions, plugin) = (allowed.clone(), actions.clone(), id.clone());
        engine.register_fn(
            "set_effect",
            move |effect: &str, param: &str, value: f64| -> Result<(), Fail> {
                allowed(Permission::Audio)?;
                if !value.is_finite() {
                    return Err(fail("The value must be a number"));
                }
                actions(HostAction::EffectParam {
                    plugin: plugin.clone(),
                    effect: effect.into(),
                    param: param.into(),
                    value: value as f32,
                });
                Ok(())
            },
        );
    }
    {
        let (allowed, actions, plugin) = (allowed.clone(), actions.clone(), id.clone());
        engine.register_fn(
            "effect_on",
            move |effect: &str, on: bool| -> Result<(), Fail> {
                allowed(Permission::Audio)?;
                actions(HostAction::EffectOn {
                    plugin: plugin.clone(),
                    effect: effect.into(),
                    on,
                });
                Ok(())
            },
        );
    }
    // Network
    {
        let allowed = allowed.clone();
        engine.register_fn("http_get", move |url: &str| -> Result<String, Fail> {
            allowed(Permission::Network)?;
            web(client().map_err(|e| fail(e.to_string()))?.get(url))
        });
    }
    {
        let allowed = allowed.clone();
        engine.register_fn(
            "http_post_json",
            move |url: &str, body: Dynamic| -> Result<String, Fail> {
                allowed(Permission::Network)?;
                let json = serde_json::to_string(&body).map_err(|e| fail(e.to_string()))?;
                web(client()
                    .map_err(|e| fail(e.to_string()))?
                    .post(url)
                    .header("Content-Type", "application/json")
                    .body(json))
            },
        );
    }
    // Asking the person: type something, or choose a file.
    {
        let (allowed, actions, name) = (allowed.clone(), actions.clone(), manifest.name.clone());
        engine.register_fn(
            "ask_text",
            move |title: &str, prompt: &str| -> Result<Dynamic, Fail> {
                allowed(Permission::Ask)?;
                let question = Question::Text {
                    title: title.into(),
                    prompt: prompt.into(),
                    default: String::new(),
                };
                Ok(
                    match ask(&actions, &name, question).and_then(|v| v.as_str().map(String::from))
                    {
                        Some(text) => text.into(),
                        None => Dynamic::UNIT,
                    },
                )
            },
        );
    }
    {
        let (allowed, actions, name) = (allowed.clone(), actions.clone(), manifest.name.clone());
        engine.register_fn(
            "ask_text",
            move |title: &str, prompt: &str, default: &str| -> Result<Dynamic, Fail> {
                allowed(Permission::Ask)?;
                let question = Question::Text {
                    title: title.into(),
                    prompt: prompt.into(),
                    default: default.into(),
                };
                Ok(
                    match ask(&actions, &name, question).and_then(|v| v.as_str().map(String::from))
                    {
                        Some(text) => text.into(),
                        None => Dynamic::UNIT,
                    },
                )
            },
        );
    }
    {
        let (allowed, actions, name) = (allowed.clone(), actions.clone(), manifest.name.clone());
        engine.register_fn(
            "pick_file",
            move |title: &str, extensions: Array| -> Result<Dynamic, Fail> {
                allowed(Permission::Ask)?;
                let extensions = extensions
                    .into_iter()
                    .filter_map(|e| e.into_string().ok())
                    .map(|e| e.trim_start_matches('.').to_lowercase())
                    .collect();
                let question = Question::File {
                    title: title.into(),
                    extensions,
                };
                match ask(&actions, &name, question) {
                    Some(file) => rhai::serde::to_dynamic(&file).map_err(|e| fail(e.to_string())),
                    None => Ok(Dynamic::UNIT),
                }
            },
        );
    }
    // Helpers for talking to servers (a source's sign-in, for example).
    engine.register_fn("md5", |text: &str| {
        format!("{:x}", md5::compute(text.as_bytes()))
    });
    engine.register_fn("random_text", |length: i64| {
        use rand::Rng;
        rand::thread_rng()
            .sample_iter(&rand::distributions::Alphanumeric)
            .take(length.clamp(1, 256) as usize)
            .map(char::from)
            .collect::<String>()
    });
    engine.register_fn("url_encode", |text: &str| {
        let mut out = String::new();
        for byte in text.bytes() {
            if byte.is_ascii_alphanumeric() || b"-_.~".contains(&byte) {
                out.push(byte as char);
            } else {
                out += &format!("%{byte:02X}");
            }
        }
        out
    });
    // Secrets (a password), kept in Windows' Credential Manager under the plugin's name.
    {
        let plugin = id.clone();
        engine.register_fn("secret", move |key: &str| -> Result<Dynamic, Fail> {
            match crate::secrets::plugin::get(&plugin, key).map_err(|e| fail(format!("{e:#}")))? {
                Some(value) => Ok(value.into()),
                None => Ok(Dynamic::UNIT),
            }
        });
    }
    {
        let plugin = id.clone();
        engine.register_fn(
            "set_secret",
            move |key: &str, value: &str| -> Result<(), Fail> {
                crate::secrets::plugin::set(&plugin, key, value).map_err(|e| fail(format!("{e:#}")))
            },
        );
    }
    {
        let plugin = id.clone();
        engine.register_fn("delete_secret", move |key: &str| -> Result<(), Fail> {
            crate::secrets::plugin::delete(&plugin, key).map_err(|e| fail(format!("{e:#}")))
        });
    }
    engine.register_fn("parse_json", |text: &str| -> Result<Dynamic, Fail> {
        serde_json::from_str::<Dynamic>(text).map_err(|e| fail(format!("Not JSON: {e}")))
    });
    // Files, confined to the plugin folder
    {
        let (allowed, folder) = (allowed.clone(), folder.to_path_buf());
        engine.register_fn("read_file", move |name: &str| -> Result<String, Fail> {
            allowed(Permission::Files)?;
            std::fs::read_to_string(inside(&folder, name)?).map_err(|e| fail(e.to_string()))
        });
    }
    {
        let (allowed, folder) = (allowed.clone(), folder.to_path_buf());
        engine.register_fn(
            "write_file",
            move |name: &str, text: &str| -> Result<(), Fail> {
                allowed(Permission::Files)?;
                let path = inside(&folder, name)?;
                // A folder inside the plugin's own (such as "themes") is made when needed.
                if let Some(parent) = path.parent() {
                    std::fs::create_dir_all(parent).map_err(|e| fail(e.to_string()))?;
                }
                std::fs::write(path, text).map_err(|e| fail(e.to_string()))
            },
        );
    }
    // Per-plugin settings, stored in the library
    {
        let (library, plugin) = (library.clone(), id.clone());
        engine.register_fn("setting", move |key: &str| -> Dynamic {
            let all: HashMap<String, serde_json::Value> = library
                .get_json(&format!("plugin:{plugin}"))
                .ok()
                .flatten()
                .unwrap_or_default();
            all.get(key)
                .and_then(|v| serde_json::from_value::<Dynamic>(v.clone()).ok())
                .unwrap_or(Dynamic::UNIT)
        });
    }
    {
        let (library, plugin) = (library.clone(), id.clone());
        engine.register_fn(
            "set_setting",
            move |key: &str, value: Dynamic| -> Result<(), Fail> {
                let storage = format!("plugin:{plugin}");
                let mut all: HashMap<String, serde_json::Value> = library
                    .get_json(&storage)
                    .ok()
                    .flatten()
                    .unwrap_or_default();
                all.insert(
                    key.into(),
                    serde_json::to_value(&value).map_err(|e| fail(e.to_string()))?,
                );
                library
                    .set_json(&storage, &all)
                    .map_err(|e| fail(format!("{e:#}")))
            },
        );
    }
    engine
}

fn web(request: reqwest::blocking::RequestBuilder) -> Result<String, Fail> {
    let response = request
        .timeout(Duration::from_secs(15))
        .send()
        .map_err(|e| fail(crate::sources::http_error(&e)))?;
    let status = response.status();
    let text = response
        .text()
        .map_err(|e| fail(crate::sources::http_error(&e)))?;
    if !status.is_success() {
        return Err(fail(format!("The server answered {status}")));
    }
    Ok(text.chars().take(1 << 20).collect())
}

fn inside(folder: &Path, name: &str) -> Result<PathBuf, Fail> {
    let path = Path::new(name);
    // `\` and `:` are refused everywhere, so a plugin names the same files on every system.
    if name.is_empty()
        || name.contains(['\\', ':'])
        || path.is_absolute()
        || path
            .components()
            .any(|c| !matches!(c, std::path::Component::Normal(_)))
    {
        return Err(fail(
            "Plugins can only use files inside their own folder, such as \"notes.txt\" or \"themes/dark.toml\"",
        ));
    }
    Ok(folder.join(path))
}

/// Example plugins shipped with Needle: (folder name, manifest, script).
pub const EXAMPLES: &[(&str, &str, &str)] = &[
    (
        "now-playing-file",
        r#"id = "now-playing-file"
name = "Now playing to a file"
version = "1.0.0"
author = "nnx"
description = "Writes the current song to now-playing.txt in this plugin's folder, for stream overlays."
permissions = ["files"]
"#,
        r#"// Keep now-playing.txt up to date for streaming software such as OBS.
fn on_track_start(track) {
    write_file("now-playing.txt", `${track.artist} — ${track.title}`);
}
fn on_pause() {
    write_file("now-playing.txt", "");
}
"#,
    ),
    (
        "this-week",
        r#"id = "this-week"
name = "This week's favourites"
version = "1.0.0"
author = "nnx"
description = "Adds a command that builds a playlist of your 25 most played songs of the last 7 days."
permissions = ["library.read", "library.write"]
"#,
        r#"fn commands() {
    [ #{ id: "build", title: "Build “This week” playlist", scope: "global" } ]
}
fn run(command, ids) {
    let songs = library_search("played(7d) order by play_count desc limit 25");
    if songs.len() == 0 {
        notify("Nothing played in the last 7 days yet.");
        return;
    }
    save_playlist("This week", songs.map(|t| t.id));
    notify(`Made “This week” with ${songs.len()} songs.`);
}
"#,
    ),
    (
        "skip-intros",
        r#"id = "skip-intros"
name = "Skip very short tracks"
version = "1.0.0"
author = "nnx"
description = "Skips tracks shorter than 30 seconds, such as intros and interludes. Change the limit with the command."
permissions = ["playback"]
"#,
        r#"fn on_track_start(track) {
    let limit = setting("seconds");
    if limit == () { limit = 30; }
    if track.duration > 0.0 && track.duration < limit {
        next_track();
    }
}
fn commands() {
    [ #{ id: "toggle", title: "Skip tracks under 60 seconds instead of 30", scope: "global" } ]
}
fn run(command, ids) {
    let now = setting("seconds");
    let next = if now == 60 { 30 } else { 60 };
    set_setting("seconds", next);
    notify(`Now skipping tracks under ${next} seconds.`);
}
"#,
    ),
    (
        "rate-selection",
        r#"id = "rate-selection"
name = "Quick five stars"
version = "1.0.0"
author = "nnx"
description = "Adds “Rate five stars” to the track menu, for rating many songs at once."
permissions = ["library.write"]
"#,
        r#"fn commands() {
    [ #{ id: "five", title: "Rate five stars", scope: "track" } ]
}
fn run(command, ids) {
    for id in ids { set_rating(id, 5); }
    notify(`Rated ${ids.len()} songs.`);
}
"#,
    ),
    (
        "studio-effects",
        r#"id = "studio-effects"
name = "Studio effects"
version = "1.0.0"
author = "nnx"
description = "Adds Room, Echo, Night mode, and Old radio to Sound › Effects, made from Needle's built-in blocks."
permissions = ["audio"]
"#,
        r#"// Each effect is a chain of built-in blocks. "$name" follows the slider with that id.
fn effects() {
    [
        #{ id: "room", name: "Room", description: "Places the music in a room.",
           params: [ #{ id: "size", name: "Size", min: 0.0, max: 1.0, value: 0.5, unit: "%" },
                     #{ id: "mix", name: "Amount", min: 0.0, max: 0.6, value: 0.2, unit: "%" } ],
           blocks: [ #{ kind: "reverb", size: "$size", mix: "$mix", damping: 0.5 } ] },
        #{ id: "echo", name: "Echo", description: "Repeats the sound after a short time.",
           params: [ #{ id: "time", name: "Time", min: 50.0, max: 1000.0, value: 320.0, unit: "ms" },
                     #{ id: "feedback", name: "Repeats", min: 0.0, max: 0.9, value: 0.35, unit: "%" },
                     #{ id: "mix", name: "Amount", min: 0.0, max: 1.0, value: 0.25, unit: "%" } ],
           blocks: [ #{ kind: "delay", time: "$time", feedback: "$feedback", mix: "$mix" } ] },
        #{ id: "night", name: "Night mode", description: "Makes quiet parts louder and loud parts quieter, for listening at low volume.",
           params: [ #{ id: "ratio", name: "Strength", min: 1.0, max: 10.0, value: 4.0, step: 0.5 } ],
           blocks: [ #{ kind: "compressor", threshold: -30.0, ratio: "$ratio", attack: 5.0, release: 250.0, makeup: 8.0 },
                     #{ kind: "limiter", ceiling: -1.0 } ] },
        #{ id: "radio", name: "Old radio", description: "A small, narrow speaker.",
           blocks: [ #{ kind: "filter", shape: "highpass", frequency: 400.0 },
                     #{ kind: "filter", shape: "lowpass", frequency: 3200.0 },
                     #{ kind: "saturate", drive: 9.0 },
                     #{ kind: "width", amount: 0.0 } ] },
    ]
}

// With the "audio" permission, a script can turn its effects on and move their sliders.
fn commands() {
    [ #{ id: "night", title: "Turn on Night mode", scope: "global" } ]
}
fn run(command, ids) {
    effect_on("night", true);
    notify("Night mode is on. Change it in Sound.");
}
"#,
    ),
    (
        "bitcrusher",
        r#"id = "bitcrusher"
name = "Bitcrusher"
version = "1.0.0"
author = "nnx"
description = "A DSP plugin with its own sound code, in WebAssembly (crush.wat): fewer bits and a lower sample rate, for a lo-fi sound."
"#,
        r#"// The sound code is in crush.wat. Needle runs it in a sandbox, a block of sound at a time.
fn effects() {
    [
        #{ id: "crush", name: "Bitcrusher", description: "Fewer bits and a lower sample rate, for a lo-fi sound.",
           wasm: "crush.wat",
           params: [ #{ id: "bits", name: "Bits", min: 2.0, max: 16.0, value: 8.0, step: 1.0 },
                     #{ id: "hold", name: "Rate divider", min: 1.0, max: 16.0, value: 1.0, step: 1.0 },
                     #{ id: "mix", name: "Amount", min: 0.0, max: 1.0, value: 1.0, unit: "%" } ] },
    ]
}
"#,
    ),
    (
        "netease-lyrics",
        r#"id = "netease-lyrics"
name = "NetEase lyrics"
version = "1.0.0"
author = "nnx"
description = "Finds timed lyrics on NetEase Cloud Music when LRCLIB has none. Good for K-pop, J-pop, C-pop, and much more. Sends only a song's title, artist, and length."
permissions = ["network"]
"#,
        r#"// Lyrics from NetEase Cloud Music (music.163.com), which has timed lyrics for a great many
// songs, Asian music above all. Needle asks it only when a song has no lyrics of its own and
// LRCLIB has no timed ones. This uses NetEase's public web API, which is not official and
// may change.

// Credit lines NetEase puts before the lyrics ("作词 : …" is "lyrics by"), left out.
fn is_credit(line) {
    let words = ["作词", "作曲", "编曲", "制作", "混音", "录音", "母带", "和声", "监制", "出品", "企划", "统筹"];
    if !(line.contains(":") || line.contains("：")) {
        return false;
    }
    for word in words {
        if line.contains(word) {
            return true;
        }
    }
    false
}

// The search result closest to the song: about the same length, the same title if it can.
fn closest(candidates, song) {
    let want = song.duration * 1000.0;
    let best = ();
    let best_score = 0.0;
    for candidate in candidates {
        if candidate.duration == () {
            continue;
        }
        let gap = candidate.duration.to_float() - want;
        if gap < 0.0 {
            gap = -gap;
        }
        // More than eight seconds apart is another recording.
        if want > 0.0 && gap > 8000.0 {
            continue;
        }
        let score = gap;
        if candidate.name.to_lower() != song.title.to_lower() {
            score += 5000.0;
        }
        if best == () || score < best_score {
            best = candidate;
            best_score = score;
        }
    }
    best
}

fn lyrics(song) {
    if song.title == "" {
        return ();
    }
    let query = url_encode(song.artist + " " + song.title);
    let found = parse_json(http_get(`https://music.163.com/api/search/get?s=${query}&type=1&limit=10`));
    if found.result == () || found.result.songs == () {
        return ();
    }
    let best = closest(found.result.songs, song);
    if best == () {
        return ();
    }
    let answer = parse_json(http_get(`https://music.163.com/api/song/lyric?id=${best.id}&lv=1&kv=1&tv=-1`));
    if answer.pureMusic == true {
        return #{ instrumental: true };
    }
    if answer.lrc == () || answer.lrc.lyric == () || answer.lrc.lyric == "" {
        return ();
    }
    let kept = [];
    for line in answer.lrc.lyric.split("\n") {
        if !is_credit(line) {
            kept.push(line);
        }
    }
    let text = "";
    for line in kept {
        text += line + "\n";
    }
    #{ synced: text }
}
"#,
    ),
    (
        "subsonic",
        r#"id = "subsonic"
name = "Navidrome / Subsonic"
version = "1.2.1"
author = "nnx"
description = "Plays the music on your Navidrome or other Subsonic server. Songs are listed with your own and streamed; plays and ratings are saved on the server."
permissions = ["network"]
"#,
        r#"// Your music server as a source: Navidrome, and other servers that speak the Subsonic API
// (Airsonic-Advanced, Gonic, Ampache, LMS). Songs are listed in Needle and streamed from the
// server; plays and ratings are saved on the server too.

fn source() {
    #{
        name: "Navidrome / Subsonic",
        fields: [
            #{ id: "server", label: "Server address", placeholder: "https://music.example.com" },
            #{ id: "username", label: "User name", placeholder: "" },
            #{ id: "password", label: "Password", placeholder: "", secret: true },
        ]
    }
}

fn signed_in() {
    let server = setting("server");
    server != () && server != "" && secret("password") != ()
}

// "music.example.com/" becomes "music.example.com".
fn address(typed) {
    let server = typed;
    server.trim();
    while server.ends_with("/") {
        server.pop();
    }
    server
}

// The sign-in part of every request. The password itself is never sent: only a salted hash.
fn auth() {
    let password = secret("password");
    if password == () {
        throw "Sign in to your server first";
    }
    let salt = random_text(12);
    `u=${url_encode(setting("username"))}&t=${md5(password + salt)}&s=${salt}&v=1.16.1&c=Needle&f=json`
}

fn link(method, params, auth) {
    let url = `${setting("server")}/rest/${method}.view?${auth}`;
    for key in params.keys() {
        url += `&${key}=${url_encode(params[key].to_string())}`;
    }
    url
}

fn ask(method, params) {
    let text = "";
    try {
        text = http_get(link(method, params, auth()));
    } catch (error) {
        // Something answered, but not a music server: often the port is missing.
        if type_of(error) == "string" && error.contains("404") {
            throw "No music server at this address. Check it, and the port: Navidrome uses :4533 unless it was changed";
        }
        throw error;
    }
    let answer = parse_json(text);
    let reply = answer["subsonic-response"];
    if reply == () {
        throw "That address did not answer like a Subsonic server";
    }
    if reply.status != "ok" {
        let message = if reply.error != () { reply.error.message } else { () };
        throw if message != () { message } else { "The server refused the request" };
    }
    reply
}

fn sign_in(fields) {
    let typed = address(fields.server);
    if typed == "" {
        throw "Type your server's address";
    }
    // Without http:// or https://, only the secure address is tried: the sign-in part of
    // each request could be copied off a plain-http connection and used again.
    let plain = typed.starts_with("http://");
    let server = if plain || typed.starts_with("https://") { typed } else { "https://" + typed };
    set_setting("server", server);
    set_setting("username", fields.username);
    set_setting("list_by", "");
    set_secret("password", fields.password);
    try {
        ask("ping", #{});
    } catch (error) {
        sign_out();
        if !plain && type_of(error) == "string" && error.contains("Could not connect") {
            throw `${error} over https. If your server only uses plain http (Navidrome on :4533 often does), type http:// in front of the address. Plain http is not encrypted, so only use it at home or through a VPN.`;
        }
        throw error;
    }
}

fn sign_out() {
    set_setting("server", "");
    delete_secret("password");
}

// The songs of a page, as Needle wants them.
fn to_songs(found) {
    // One sign-in for all the cover links of this page, so an album's songs share one link.
    let cover_auth = auth();
    let list = [];
    for song in found {
        list.push(#{
            id: song.id,
            title: song.title,
            artist: song.artist,
            album: song.album,
            album_artist: song.displayAlbumArtist,
            genre: song.genre,
            year: song.year,
            track: song.track,
            disc: song.discNumber,
            duration: song.duration,
            format: song.suffix,
            bitrate: song.bitRate,
            sample_rate: song.samplingRate,
            bit_depth: song.bitDepth,
            channels: song.channelCount,
            size: song.size,
            musicbrainz_id: song.musicBrainzId,
            // One cover per album: songs often carry their own copy of the same picture.
            cover_id: if song.albumId != () { "album:" + song.albumId } else { song.coverArt },
            cover: if song.coverArt != () {
                link("getCoverArt", #{ id: song.coverArt, size: 600 }, cover_auth)
            } else {
                ""
            },
        });
    }
    list
}

// 250 songs a page, until a page comes back empty. Some servers (Ampache) find nothing for
// an empty search; for those, 5 albums a page with their songs (short pages, so songs
// waiting to play are not held up for long).
fn songs(page) {
    if setting("list_by") != "albums" {
        let reply = ask("search3", #{
            query: "", songCount: 250, songOffset: page * 250, artistCount: 0, albumCount: 0
        });
        let found = if reply.searchResult3 != () { reply.searchResult3.song } else { () };
        if found != () && found.len() > 0 {
            return to_songs(found);
        }
        if page > 0 {
            return [];
        }
        set_setting("list_by", "albums");
    }
    let reply = ask("getAlbumList2", #{ type: "alphabeticalByName", size: 5, offset: page * 5 });
    let albums = if reply.albumList2 != () { reply.albumList2.album } else { () };
    if albums == () || albums.len() == 0 {
        return [];
    }
    let found = [];
    for album in albums {
        let full = ask("getAlbum", #{ id: album.id });
        if full.album != () && full.album.song != () {
            for song in full.album.song {
                found.push(song);
            }
        }
    }
    to_songs(found)
}

// The original file, not a smaller copy.
fn stream(id) {
    link("stream", #{ id: id, format: "raw" }, auth())
}

fn playing(id) {
    ask("scrobble", #{ id: id, submission: false });
}

fn played(id, started_at) {
    ask("scrobble", #{ id: id, time: started_at * 1000, submission: true });
}

fn rate(id, stars) {
    ask("setRating", #{ id: id, rating: stars });
}
"#,
    ),
];

/// Extra files for the example plugins: (folder, file name, contents).
pub const EXAMPLE_FILES: &[(&str, &str, &str)] = &[(
    "bitcrusher",
    "crush.wat",
    r#";; A bitcrusher in WebAssembly text. Needle calls:
;;   init(rate, channels, max_frames) -> the address of a buffer of max_frames * channels f32s
;;   param(index, value)              for each slider, in the order effects() lists them
;;   process(frames)                  after filling the buffer; change the samples in place
;;   reset()                          after a seek (optional)
(module
  (memory (export "memory") 1)
  (global $step (mut f32) (f32.const 0.0078125)) ;; 2 / 2^bits
  (global $hold (mut i32) (i32.const 1))
  (global $mix (mut f32) (f32.const 1))
  (global $channels (mut i32) (i32.const 2))
  (global $count (mut i32) (i32.const 0))

  (func (export "init") (param $rate i32) (param $channels i32) (param $max i32) (result i32)
    (global.set $channels (local.get $channels))
    (i32.const 1024))

  (func (export "param") (param $index i32) (param $value f32)
    (if (i32.eqz (local.get $index))
      (then (global.set $step
        (f32.div (f32.const 2)
          (f32.convert_i32_s (i32.shl (i32.const 1) (i32.trunc_f32_s (local.get $value))))))))
    (if (i32.eq (local.get $index) (i32.const 1))
      (then (global.set $hold (i32.trunc_f32_s (local.get $value)))))
    (if (i32.eq (local.get $index) (i32.const 2))
      (then (global.set $mix (local.get $value)))))

  (func (export "reset") (global.set $count (i32.const 0)))

  (func (export "process") (param $frames i32)
    (local $at i32) (local $end i32) (local $c i32) (local $x f32) (local $held i32)
    (local.set $at (i32.const 1024))
    (local.set $end (i32.add (i32.const 1024)
      (i32.shl (i32.mul (local.get $frames) (global.get $channels)) (i32.const 2))))
    (block $done (loop $frame
      (br_if $done (i32.ge_u (local.get $at) (local.get $end)))
      (local.set $c (i32.const 0))
      (block $next (loop $channel
        (br_if $next (i32.ge_u (local.get $c) (global.get $channels)))
        (local.set $x (f32.load (local.get $at)))
        ;; the value held for this channel, at 64 + channel * 4
        (local.set $held (i32.add (i32.const 64)
          (i32.shl (i32.and (local.get $c) (i32.const 7)) (i32.const 2))))
        (if (i32.eqz (global.get $count))
          (then (f32.store (local.get $held)
            (f32.mul (f32.nearest (f32.div (local.get $x) (global.get $step))) (global.get $step)))))
        (f32.store (local.get $at)
          (f32.add (f32.mul (local.get $x) (f32.sub (f32.const 1) (global.get $mix)))
                   (f32.mul (f32.load (local.get $held)) (global.get $mix))))
        (local.set $at (i32.add (local.get $at) (i32.const 4)))
        (local.set $c (i32.add (local.get $c) (i32.const 1)))
        (br $channel)))
      (global.set $count (i32.add (global.get $count) (i32.const 1)))
      (if (i32.ge_s (global.get $count) (global.get $hold))
        (then (global.set $count (i32.const 0))))
      (br $frame))))
)
"#,
)];

/// Scripts of the Navidrome / Subsonic plugin from before bundled plugins kept a record
/// (SHA-256). A copy that still matches one exactly was never edited, so it is updated.
const EARLIER_EXAMPLES: &[(&str, &str)] = &[
    (
        "subsonic",
        "62f1f6b31fb90aa09fd19f8aee0ee31b40437b696474135b3f121de2c812bebd",
    ),
    (
        "subsonic",
        "1c7ea98aca006966d667d45e3192de3ed661e9424896d9404fb60c3e7c9e1e5b",
    ),
    (
        "subsonic",
        "41f1421aee56cf12d4febbab87f2f6216d888fb7d563dfcfb133eb036307818f",
    ),
    (
        "subsonic",
        "57942e2acdcd0a0e376c59d7dc4dabc05c3c990394c10673a8e5c29207881862",
    ),
    (
        "subsonic",
        "6ed2b6a5b1f7c0d6018af622b35ccf394ab7dc083ae6c9ed9e895e9b1f22cce8",
    ),
    (
        "subsonic",
        "f66bd34adddf7ee5561fbe38c18ba3a14cb02ff8532b4b5f68921ab7426ce4b1",
    ),
    (
        "subsonic",
        "ae1bfab837a569a518d79b710ee56031663be2202dca5d33ae60f489190c8ce4",
    ),
];

/// What Needle installed in a bundled plugin's folder, kept in `.bundled.json`: its
/// version, a fingerprint of its files as written, and a newer version the person chose not
/// to take over their own changes.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
struct Bundled {
    version: String,
    hash: String,
    #[serde(default)]
    kept: Option<String>,
    /// The files that fingerprint covers (a later version may add files).
    #[serde(default)]
    files: Vec<String>,
}
const BUNDLED_RECORD: &str = ".bundled.json";

/// The files of a bundled plugin, as Needle ships them.
fn bundled_files(name: &str) -> Vec<(&'static str, &'static str)> {
    let Some((_, manifest, script)) = EXAMPLES.iter().find(|(n, ..)| *n == name) else {
        return vec![];
    };
    let mut files = vec![("plugin.toml", *manifest), ("main.rhai", *script)];
    files.extend(
        EXAMPLE_FILES
            .iter()
            .filter(|(n, ..)| *n == name)
            .map(|(_, file, contents)| (*file, *contents)),
    );
    files
}

/// Whether the plugin in `dir` is one Needle ships: its id is a bundled plugin's, in the
/// folder of that name.
pub fn is_official(dir: &Path, id: &str) -> bool {
    EXAMPLES.iter().any(|(name, ..)| *name == id)
        && dir
            .file_name()
            .is_some_and(|f| f == std::ffi::OsStr::new(id))
}

/// The version a bundled plugin has in this build of Needle.
fn bundled_version(name: &str) -> String {
    bundled_files(name)
        .first()
        .and_then(|(_, manifest)| toml::from_str::<Manifest>(manifest).ok())
        .map(|m| m.version)
        .unwrap_or_default()
}

/// A fingerprint of these files' contents (a missing file counts as empty).
fn fingerprint<'a>(files: impl Iterator<Item = (&'a str, Vec<u8>)>) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    for (name, contents) in files {
        hasher.update(name.as_bytes());
        hasher.update((contents.len() as u64).to_le_bytes());
        hasher.update(&contents);
    }
    format!("{:x}", hasher.finalize())
}

/// The fingerprint of a bundled plugin's files as they are on disk now.
fn fingerprint_on_disk(dir: &Path, name: &str) -> String {
    let files: Vec<&str> = bundled_files(name).into_iter().map(|(f, _)| f).collect();
    fingerprint_files(dir, &files)
}

/// The fingerprint of these files in `dir` as they are now.
fn fingerprint_files(dir: &Path, files: &[&str]) -> String {
    fingerprint(
        files
            .iter()
            .map(|file| (*file, std::fs::read(dir.join(file)).unwrap_or_default())),
    )
}

/// A name for a copy of `file` that no earlier copy has, so no copy is ever replaced:
/// `main.rhai.mine`, then `main.rhai.mine-2`, and so on.
fn free_copy_name(dir: &Path, file: &str) -> PathBuf {
    let mut path = dir.join(format!("{file}.mine"));
    let mut n = 2;
    while path.exists() {
        path = dir.join(format!("{file}.mine-{n}"));
        n += 1;
    }
    path
}

/// Whether a plugin.toml is the bundled one apart from its version (as in every version of
/// Needle before records were kept).
fn manifest_unchanged(dir: &Path, name: &str) -> bool {
    let bundled = bundled_files(name)
        .first()
        .and_then(|(_, m)| toml::from_str::<Manifest>(m).ok());
    let on_disk = read_manifest(dir).ok();
    match (bundled, on_disk) {
        (Some(mut bundled), Some(on_disk)) => {
            bundled.version = on_disk.version.clone();
            bundled == on_disk
        }
        _ => false,
    }
}

/// "1.2.0" is newer than "1.1.9"; missing parts count as 0.
fn newer_version(candidate: &str, than: &str) -> bool {
    let parts = |v: &str| -> Vec<u64> {
        v.trim()
            .trim_start_matches('v')
            .split('.')
            .map(|p| p.parse().unwrap_or(0))
            .collect()
    };
    let (mut a, mut b) = (parts(candidate), parts(than));
    let length = a.len().max(b.len());
    a.resize(length, 0);
    b.resize(length, 0);
    a > b
}

fn read_record(dir: &Path) -> Option<Bundled> {
    serde_json::from_str(&std::fs::read_to_string(dir.join(BUNDLED_RECORD)).ok()?).ok()
}

/// Write this build's version of a bundled plugin into `dir`, and record it. With
/// `keep_copy`, the files it replaces are kept first as `<name>.mine`.
fn write_bundled(dir: &Path, name: &str, keep_copy: bool) -> Result<()> {
    std::fs::create_dir_all(dir)?;
    let files = bundled_files(name);
    for (file, contents) in &files {
        let path = dir.join(file);
        if keep_copy && path.is_file() && std::fs::read(&path)? != contents.as_bytes() {
            std::fs::copy(&path, free_copy_name(dir, file))?;
        }
        std::fs::write(path, contents)?;
    }
    let record = Bundled {
        version: bundled_version(name),
        hash: fingerprint(files.iter().map(|(f, c)| (*f, c.as_bytes().to_vec()))),
        kept: None,
        files: files.iter().map(|(f, _)| f.to_string()).collect(),
    };
    std::fs::write(
        dir.join(BUNDLED_RECORD),
        serde_json::to_string_pretty(&record)?,
    )?;
    Ok(())
}

/// Whether a bundled plugin's files on disk are exactly as some version of Needle wrote
/// them, and which version that was.
fn unedited_version(dir: &Path, name: &str) -> Option<String> {
    if let Some(record) = read_record(dir) {
        // Compare the files that were written then; a later version may have more.
        let on_disk = if record.files.is_empty() {
            fingerprint_on_disk(dir, name)
        } else {
            let files: Vec<&str> = record.files.iter().map(String::as_str).collect();
            fingerprint_files(dir, &files)
        };
        return (on_disk == record.hash).then_some(record.version);
    }
    // Installed before records were kept.
    let script = std::fs::read(dir.join("main.rhai")).ok()?;
    let hash = {
        use sha2::{Digest, Sha256};
        format!("{:x}", Sha256::digest(&script))
    };
    if fingerprint_on_disk(dir, name)
        == fingerprint(
            bundled_files(name)
                .iter()
                .map(|(f, c)| (*f, c.as_bytes().to_vec())),
        )
    {
        Some(bundled_version(name))
    } else if (EARLIER_EXAMPLES.contains(&(name, hash.as_str()))
        || bundled_files(name)
            .iter()
            .any(|(file, contents)| *file == "main.rhai" && script == contents.as_bytes()))
        // Updating writes plugin.toml too, so it must be unchanged as well.
        && manifest_unchanged(dir, name)
    {
        // An earlier script, or this one with an earlier plugin.toml.
        Some("0".into())
    } else {
        None
    }
}

/// Bring bundled plugins up to date: ones nobody edited are replaced by this build's newer
/// version. Edited ones are left alone (see `bundled_update`). Returns how many changed.
pub fn update_examples(folder: &Path) -> usize {
    let mut updated = 0;
    for (name, ..) in EXAMPLES {
        let dir = folder.join(name);
        if !dir.join("plugin.toml").is_file() {
            continue;
        }
        let current = bundled_version(name);
        match unedited_version(&dir, name) {
            Some(version) if newer_version(&current, &version) => {
                if write_bundled(&dir, name, false).is_ok() {
                    crate::logfile::info(format!("Updated the bundled plugin {name} to {current}"));
                    updated += 1;
                }
            }
            // The same version as this build, never edited: make sure it has a record.
            Some(_) if read_record(&dir).is_none() => {
                let _ = write_bundled(&dir, name, false);
            }
            _ => {}
        }
    }
    updated
}

/// For a bundled plugin someone changed: the newer version this build of Needle has, unless
/// they chose to keep theirs over that version.
pub fn bundled_update(dir: &Path, id: &str) -> Option<String> {
    let current = bundled_version(id);
    if current.is_empty() || unedited_version(dir, id).is_some() {
        return None;
    }
    let installed = read_record(dir)
        .map(|r| (r.version, r.kept))
        .or_else(|| read_manifest(dir).ok().map(|m| (m.version, None)))?;
    if installed.1.as_deref() == Some(current.as_str()) {
        return None;
    }
    newer_version(&current, &installed.0).then_some(current)
}

/// Replace a changed bundled plugin with this build's version, keeping the changed files as
/// `<name>.mine`.
pub fn take_bundled_update(dir: &Path, id: &str) -> Result<()> {
    write_bundled(dir, id, true)
}

/// Keep a changed bundled plugin as it is, and stop offering this build's version.
pub fn keep_changed_plugin(dir: &Path, id: &str) -> Result<()> {
    let mut record = read_record(dir).unwrap_or_else(|| Bundled {
        version: read_manifest(dir).map(|m| m.version).unwrap_or_default(),
        ..Default::default()
    });
    record.kept = Some(bundled_version(id));
    std::fs::write(
        dir.join(BUNDLED_RECORD),
        serde_json::to_string_pretty(&record)?,
    )?;
    Ok(())
}

/// Install one bundled plugin, unless its folder is already there. Returns whether it was.
pub fn install_example(library: &Library, name: &str) -> Result<bool> {
    let dir = library.directory.join("plugins").join(name);
    if dir.exists() || bundled_files(name).is_empty() {
        return Ok(false);
    }
    write_bundled(&dir, name, false)?;
    Ok(true)
}

/// Copy the example plugins into the plugin folder, leaving any that already exist.
pub fn install_examples(library: &Library) -> Result<usize> {
    let folder = library.directory.join("plugins");
    let mut installed = 0;
    for (name, ..) in EXAMPLES {
        let dir = folder.join(name);
        if dir.exists() {
            continue;
        }
        write_bundled(&dir, name, false)?;
        installed += 1;
    }
    Ok(installed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    fn wait<T>(what: impl Fn() -> Option<T>) -> T {
        let start = Instant::now();
        loop {
            if let Some(value) = what() {
                return value;
            }
            assert!(start.elapsed() < Duration::from_secs(10), "timed out");
            std::thread::sleep(Duration::from_millis(20));
        }
    }
    fn setup() -> (
        tempfile::TempDir,
        Library,
        PluginHost,
        Arc<Mutex<Vec<HostAction>>>,
    ) {
        let dir = tempfile::tempdir().unwrap();
        let library = Library::open(dir.path()).unwrap();
        library
            .upsert(&Track {
                id: "t1".into(),
                path: "C:/a.flac".into(),
                title: "Interlude".into(),
                artist: "A".into(),
                duration: 12.,
                ..Default::default()
            })
            .unwrap();
        install_examples(&library).unwrap();
        let seen: Arc<Mutex<Vec<HostAction>>> = Arc::default();
        let sink = seen.clone();
        let host = PluginHost::start(library.clone(), Default::default(), move |a| {
            sink.lock().unwrap().push(a)
        });
        (dir, library, host, seen)
    }

    #[test]
    fn examples_load_disabled_then_run_when_enabled() {
        let (_dir, _library, host, seen) = setup();
        let plugins = wait(|| Some(host.plugins()).filter(|p| p.len() == EXAMPLES.len()));
        assert!(
            plugins.iter().all(|p| !p.enabled && p.error.is_none()),
            "{plugins:?}"
        );
        assert!(
            host.commands().is_empty(),
            "disabled plugins contribute nothing"
        );
        host.send(PluginEvent::Enable("skip-intros".into(), true));
        host.send(PluginEvent::Enable("rate-selection".into(), true));
        let commands = wait(|| Some(host.commands()).filter(|c| c.len() == 2));
        assert!(commands.iter().any(|c| c.id == "five" && c.for_tracks));
        host.send(PluginEvent::TrackStarted(Box::new(Track {
            id: "t1".into(),
            duration: 12.,
            ..Default::default()
        })));
        wait(|| {
            seen.lock()
                .unwrap()
                .contains(&HostAction::Next)
                .then_some(())
        });
        host.send(PluginEvent::Run {
            plugin: "rate-selection".into(),
            command: "five".into(),
            track_ids: vec!["t1".into()],
        });
        wait(|| {
            seen.lock()
                .unwrap()
                .contains(&HostAction::LibraryChanged)
                .then_some(())
        });
    }

    /// A pretend Subsonic server on this computer: it checks the salted password, lists two
    /// songs, streams a WAV, and records every request.
    fn fake_subsonic(password: &'static str) -> (String, Arc<Mutex<Vec<String>>>) {
        fake_subsonic_with(password, 0, false)
    }

    /// `fake_subsonic`, with `filler` more songs on the first page, each with the many extra
    /// fields a real Navidrome sends.
    /// With `albums_only`, it finds nothing for an empty search (as Ampache does) and lists
    /// its songs through albums instead.
    fn fake_subsonic_with(
        password: &'static str,
        filler: usize,
        albums_only: bool,
    ) -> (String, Arc<Mutex<Vec<String>>>) {
        use std::io::{BufRead, BufReader, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = format!("http://{}", listener.local_addr().unwrap());
        let seen: Arc<Mutex<Vec<String>>> = Arc::default();
        let log = seen.clone();
        let wav = {
            let mut bytes = std::io::Cursor::new(vec![]);
            let spec = hound::WavSpec {
                channels: 2,
                sample_rate: 44100,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            };
            let mut writer = hound::WavWriter::new(&mut bytes, spec).unwrap();
            for i in 0..44100 {
                let v = ((i as f32 * 0.05).sin() * 8000.) as i16;
                writer.write_sample(v).unwrap();
                writer.write_sample(v).unwrap();
            }
            writer.finalize().unwrap();
            bytes.into_inner()
        };
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut line = String::new();
                if reader.read_line(&mut line).is_err() {
                    continue;
                }
                loop {
                    let mut header = String::new();
                    if reader.read_line(&mut header).is_err() || header.trim().is_empty() {
                        break;
                    }
                }
                let target = line
                    .split_whitespace()
                    .nth(1)
                    .unwrap_or_default()
                    .to_string();
                log.lock().unwrap().push(target.clone());
                let (path, query) = target.split_once('?').unwrap_or((&target, ""));
                let param = |key: &str| {
                    query
                        .split('&')
                        .find_map(|kv| kv.strip_prefix(&format!("{key}=")))
                        .unwrap_or_default()
                        .to_string()
                };
                let authorised = param("t")
                    == format!("{:x}", md5::compute(format!("{password}{}", param("s"))));
                let ok = |body: &str| {
                    format!(
                        "{{\"subsonic-response\":{{\"status\":\"ok\",\"version\":\"1.16.1\"{body}}}}}"
                    )
                };
                let status = if path.starts_with("/rest/") {
                    "200 OK"
                } else {
                    "404 Not Found"
                };
                let (kind, body): (&str, Vec<u8>) = if !authorised {
                    ("application/json", br#"{"subsonic-response":{"status":"failed","error":{"code":40,"message":"Wrong username or password"}}}"#.to_vec())
                } else if path.ends_with("/stream.view") {
                    ("audio/wav", wav.clone())
                } else if path.ends_with("/getCoverArt.view") {
                    ("image/png", vec![7u8; 500])
                } else if path.ends_with("/getAlbumList2.view") {
                    let list = if param("offset") == "0" && param("size") == "5" {
                        r#"[{"id":"al-1","name":"WhyKiiiKiii - EP"}]"#
                    } else {
                        "[]"
                    };
                    (
                        "application/json",
                        ok(&format!(r#","albumList2":{{"album":{list}}}"#)).into_bytes(),
                    )
                } else if path.ends_with("/getAlbum.view") {
                    ("application/json", ok(r#","album":{"id":"al-1","song":[
                        {"id":"s1","title":"Hey Hi","artist":"KiiiKiii","album":"WhyKiiiKiii - EP","albumId":"al-1","suffix":"wav","coverArt":"al-1"},
                        {"id":"s2","title":"Sweet Sour","artist":"KiiiKiii","album":"WhyKiiiKiii - EP","albumId":"al-1","suffix":"wav","coverArt":"al-1"}
                    ]}"#).into_bytes())
                } else if path.ends_with("/search3.view") && albums_only {
                    (
                        "application/json",
                        ok(r#","searchResult3":{}"#).into_bytes(),
                    )
                } else if path.ends_with("/search3.view") {
                    if param("songOffset") == "0" {
                        let mut songs = vec![
                            r#"{"id":"s1","title":"Hey Hi","artist":"KiiiKiii","album":"WhyKiiiKiii - EP","albumId":"1","year":2026,"track":2,"duration":1,"suffix":"wav","bitRate":1411,"coverArt":"mf-s1"}"#.to_string(),
                            r#"{"id":"s2","title":"Sweet Sour","artist":"KiiiKiii","album":"WhyKiiiKiii - EP","albumId":"1","track":4,"duration":1,"suffix":"wav","coverArt":"mf-s2"}"#.to_string(),
                        ];
                        for n in 0..filler {
                            let extra: Vec<String> = (0..40)
                                .map(|f| format!(r#""extra{f}":"value {f}""#))
                                .collect();
                            songs.push(format!(
                                r#"{{"id":"f{n}","title":"Filler {n}","artist":"A","album":"B","suffix":"flac","genres":[{{"name":"Pop"}}],"artists":[{{"id":"a","name":"A"}}],{}}}"#,
                                extra.join(",")
                            ));
                        }
                        (
                            "application/json",
                            ok(&format!(
                                r#","searchResult3":{{"song":[{}]}}"#,
                                songs.join(",")
                            ))
                            .into_bytes(),
                        )
                    } else {
                        (
                            "application/json",
                            ok(r#","searchResult3":{}"#).into_bytes(),
                        )
                    }
                } else {
                    ("application/json", ok("").into_bytes())
                };
                let mut stream = stream;
                let _ = write!(
                    stream,
                    "HTTP/1.1 {status}\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let _ = stream.write_all(&body);
            }
        });
        (address, seen)
    }

    #[test]
    fn subsonic_songs_sync_stream_and_count_on_the_server() {
        let (server, requests) = fake_subsonic("hunter2");
        let dir = tempfile::tempdir().unwrap();
        let library = Library::open(dir.path()).unwrap();
        install_examples(&library).unwrap();
        let host = PluginHost::start(library.clone(), Arc::default(), |_| {});
        wait(|| Some(host.plugins()).filter(|p| p.len() == EXAMPLES.len()));
        host.send(PluginEvent::Enable("subsonic".into(), true));
        let source = || {
            host.plugins()
                .into_iter()
                .find(|p| p.manifest.id == "subsonic")
                .and_then(|p| p.source)
        };
        let info = wait(source);
        assert_eq!(info.name, "Navidrome / Subsonic");
        assert_eq!(info.fields.len(), 3);
        assert!(info.fields[2].secret && !info.signed_in);

        let sign_in = |password: &str| {
            host.send(PluginEvent::SourceSignIn {
                plugin: "subsonic".into(),
                fields: [
                    // Plain http only when typed (the pretend server has no https).
                    ("server".to_string(), format!("{server}/")),
                    ("username".to_string(), "willow".to_string()),
                    ("password".to_string(), password.to_string()),
                ]
                .into(),
            })
        };
        // Without http://, only https is tried, and the message says how to use plain http.
        host.send(PluginEvent::SourceSignIn {
            plugin: "subsonic".into(),
            fields: [
                ("server".to_string(), "127.0.0.1:9".to_string()),
                ("username".to_string(), "willow".to_string()),
                ("password".to_string(), "hunter2".to_string()),
            ]
            .into(),
        });
        let hint = wait(|| source().and_then(|s| s.error));
        assert!(hint.contains("type http:// in front"), "{hint}");
        assert!(
            requests.lock().unwrap().is_empty(),
            "nothing went over plain http"
        );
        // An address with something else on it (a missing port, usually) says so.
        host.send(PluginEvent::SourceSignIn {
            plugin: "subsonic".into(),
            fields: [
                ("server".to_string(), format!("{server}/other")),
                ("username".to_string(), "willow".to_string()),
                ("password".to_string(), "hunter2".to_string()),
            ]
            .into(),
        });
        let lost = wait(|| {
            source()
                .and_then(|s| s.error)
                .filter(|e| !e.contains("https"))
        });
        assert!(
            lost.starts_with("No music server at this address"),
            "{lost}"
        );
        sign_in("wrong");
        let refused = wait(|| {
            source()
                .and_then(|s| s.error)
                .filter(|e| !e.starts_with("No music"))
        });
        assert_eq!(refused, "Wrong username or password");
        assert!(!source().unwrap().signed_in);

        sign_in("hunter2");
        let synced = wait(|| source().filter(|s| s.songs == 2 && !s.syncing));
        assert!(synced.signed_in && synced.error.is_none() && synced.synced_at.is_some());
        let tracks = library.search(&crate::sources::rule("subsonic")).unwrap();
        assert_eq!(tracks.len(), 2);
        let hey = tracks.iter().find(|t| t.title == "Hey Hi").unwrap().clone();
        assert!(hey.is_streamed());
        assert_eq!(
            (hey.artist.as_str(), hey.year, hey.format.as_str()),
            ("KiiiKiii", 2026, "WAV")
        );
        // Both songs share the album's cover, fetched once, though each has its own cover id.
        wait(|| library.track(&hey.id).unwrap().unwrap().artwork);
        let sweet = tracks.iter().find(|t| t.title == "Sweet Sour").unwrap();
        wait(|| library.track(&sweet.id).unwrap().unwrap().artwork);
        let covers = requests
            .lock()
            .unwrap()
            .iter()
            .filter(|r| r.contains("getCoverArt"))
            .count();
        assert_eq!(covers, 1);
        // The password never goes over the network, only a salted hash of it.
        assert!(
            requests
                .lock()
                .unwrap()
                .iter()
                .all(|r| !r.contains("hunter2"))
        );

        // Streaming: samples arrive, and the finished song stays in the cache.
        let cache = dir.path().join("stream-cache");
        let streamer = host.clone();
        let link = move |plugin: &str, id: &str| streamer.stream_link(plugin, id);
        let source_audio = crate::sources::open_with(&hey, &cache, &link).unwrap();
        assert_eq!(source_audio.channels(), 2);
        assert_eq!(source_audio.sample_rate(), 44100);
        let samples: Vec<f32> = source_audio.collect();
        assert_eq!(samples.len(), 44100 * 2);
        assert!(samples.iter().any(|s| s.abs() > 0.1));
        wait(|| {
            walkdir::WalkDir::new(&cache)
                .into_iter()
                .flatten()
                .any(|e| e.path().extension().is_some_and(|x| x == "wav"))
                .then_some(())
        });
        let streams = || {
            requests
                .lock()
                .unwrap()
                .iter()
                .filter(|r| r.contains("stream.view"))
                .count()
        };
        assert_eq!(streams(), 1);
        let again: Vec<f32> = crate::sources::open_with(&hey, &cache, &link)
            .unwrap()
            .collect();
        assert_eq!(again.len(), samples.len());
        assert_eq!(streams(), 1, "played again from the cache");

        // Plays and ratings reach the server.
        host.send(PluginEvent::Listen(Listen {
            id: "l1".into(),
            track_id: hey.id.clone(),
            title: hey.title.clone(),
            artist: hey.artist.clone(),
            album: hey.album.clone(),
            started_at: 1_790_000_000,
            listened_seconds: 1.,
            duration: 1.,
            qualified: true,
        }));
        host.send(PluginEvent::Rated {
            track_id: hey.id.clone(),
            stars: 4,
        });
        wait(|| {
            let seen = requests.lock().unwrap();
            (seen.iter().any(|r| {
                r.contains("scrobble.view")
                    && r.contains("id=s1")
                    && r.contains("submission=true")
                    && r.contains("time=1790000000000")
            }) && seen
                .iter()
                .any(|r| r.contains("setRating.view") && r.contains("rating=4")))
            .then_some(())
        });

        // Signing out keeps the songs (with their ratings) but marks them missing.
        host.send(PluginEvent::SourceSignOut("subsonic".into()));
        wait(|| source().filter(|s| !s.signed_in && s.songs == 0));
        assert!(library.track(&hey.id).unwrap().unwrap().missing);
    }

    /// A server that finds nothing for an empty search (Ampache) is listed album by album.
    #[test]
    fn servers_without_an_empty_search_are_read_by_album() {
        let (server, requests) = fake_subsonic_with("pw", 0, true);
        let dir = tempfile::tempdir().unwrap();
        let library = Library::open(dir.path()).unwrap();
        install_examples(&library).unwrap();
        let host = PluginHost::start(library.clone(), Arc::default(), |_| {});
        wait(|| Some(host.plugins()).filter(|p| p.len() == EXAMPLES.len()));
        host.send(PluginEvent::Enable("subsonic".into(), true));
        let source = || {
            host.plugins()
                .into_iter()
                .find(|p| p.manifest.id == "subsonic")
                .and_then(|p| p.source)
        };
        wait(source);
        host.send(PluginEvent::SourceSignIn {
            plugin: "subsonic".into(),
            fields: [
                ("server".to_string(), server),
                ("username".to_string(), "mei".to_string()),
                ("password".to_string(), "pw".to_string()),
            ]
            .into(),
        });
        let synced = wait(|| source().filter(|s| s.synced_at.is_some() || s.error.is_some()));
        assert_eq!(synced.error, None);
        assert_eq!(synced.songs, 2);
        assert!(
            requests
                .lock()
                .unwrap()
                .iter()
                .any(|r| r.contains("getAlbum.view"))
        );
    }

    /// A full page from a real server (250 songs with dozens of fields each) fits in a plugin.
    #[test]
    fn a_full_page_of_server_songs_fits() {
        let (server, _) = fake_subsonic_with("pw", 248, false);
        let dir = tempfile::tempdir().unwrap();
        let library = Library::open(dir.path()).unwrap();
        install_examples(&library).unwrap();
        let host = PluginHost::start(library.clone(), Arc::default(), |_| {});
        wait(|| Some(host.plugins()).filter(|p| p.len() == EXAMPLES.len()));
        host.send(PluginEvent::Enable("subsonic".into(), true));
        let source = || {
            host.plugins()
                .into_iter()
                .find(|p| p.manifest.id == "subsonic")
                .and_then(|p| p.source)
        };
        wait(source);
        host.send(PluginEvent::SourceSignIn {
            plugin: "subsonic".into(),
            fields: [
                ("server".to_string(), server),
                ("username".to_string(), "mei".to_string()),
                ("password".to_string(), "pw".to_string()),
            ]
            .into(),
        });
        let synced = wait(|| source().filter(|s| s.synced_at.is_some() || s.error.is_some()));
        assert_eq!(synced.error, None);
        assert_eq!(synced.songs, 250);
    }

    /// Unedited bundled plugins update to a newer version; changed ones are offered the
    /// update, which keeps the changes as `.mine`, or can be kept as they are.
    #[test]
    fn bundled_plugins_update_unless_changed() {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path();
        let plugin = folder.join("subsonic");
        let read = |file: &str| std::fs::read_to_string(plugin.join(file)).unwrap();
        let current = bundled_version("subsonic");

        // Installed before records were kept, still as shipped: updated, and recorded.
        std::fs::create_dir_all(&plugin).unwrap();
        // The bundled plugin.toml as it was then: the same but for its version.
        let old_manifest = bundled_files("subsonic")[0]
            .1
            .replace(&format!("version = \"{current}\""), "version = \"1.0.0\"");
        std::fs::write(plugin.join("plugin.toml"), &old_manifest).unwrap();
        std::fs::write(
            plugin.join("main.rhai"),
            include_str!("../testdata/subsonic-1.0.1.rhai"),
        )
        .unwrap();
        assert_eq!(update_examples(folder), 1);
        assert!(read("main.rhai").contains("Navidrome uses :4533"));
        assert_eq!(read_record(&plugin).unwrap().version, current);
        assert_eq!(update_examples(folder), 0, "already up to date");
        assert_eq!(bundled_update(&plugin, "subsonic"), None);

        // Before records, with the current script but an older plugin.toml: updated.
        std::fs::remove_file(plugin.join(BUNDLED_RECORD)).unwrap();
        std::fs::write(plugin.join("plugin.toml"), &old_manifest).unwrap();
        assert_eq!(update_examples(folder), 1);
        assert!(read("plugin.toml").contains(&current));

        // Before records, with a plugin.toml someone changed (its name): left alone, since
        // updating would overwrite it.
        std::fs::remove_file(plugin.join(BUNDLED_RECORD)).unwrap();
        let renamed = old_manifest.replace("Navidrome / Subsonic", "My server");
        std::fs::write(plugin.join("plugin.toml"), &renamed).unwrap();
        assert_eq!(update_examples(folder), 0);
        assert_eq!(read("plugin.toml"), renamed);
        write_bundled(&plugin, "subsonic", false).unwrap();

        // An older recorded version nobody changed: updated.
        let mut record = read_record(&plugin).unwrap();
        record.version = "1.0.5".into();
        std::fs::write(
            plugin.join(BUNDLED_RECORD),
            serde_json::to_string(&record).unwrap(),
        )
        .unwrap();
        assert_eq!(update_examples(folder), 1);

        // Changed by hand, with an older version recorded: left alone, and offered.
        std::fs::write(
            plugin.join("main.rhai"),
            format!("{}\n// mine", read("main.rhai")),
        )
        .unwrap();
        let mut record = read_record(&plugin).unwrap();
        record.version = "1.0.5".into();
        std::fs::write(
            plugin.join(BUNDLED_RECORD),
            serde_json::to_string(&record).unwrap(),
        )
        .unwrap();
        assert_eq!(update_examples(folder), 0);
        assert!(read("main.rhai").ends_with("// mine"));
        assert_eq!(
            bundled_update(&plugin, "subsonic").as_deref(),
            Some(current.as_str())
        );

        // Keeping it stops the offer for this version.
        keep_changed_plugin(&plugin, "subsonic").unwrap();
        assert_eq!(bundled_update(&plugin, "subsonic"), None);
        assert!(read("main.rhai").ends_with("// mine"));

        // Official either way; verified only while unchanged.
        assert!(is_official(&plugin, "subsonic"));
        assert!(!is_official(&folder.join("copycat"), "subsonic"));
        assert!(unedited_version(&plugin, "subsonic").is_none());

        // Taking it replaces the files and keeps the changed script as main.rhai.mine; a
        // second update keeps the first copy and adds another.
        std::fs::write(plugin.join("main.rhai.mine"), "first copy").unwrap();
        take_bundled_update(&plugin, "subsonic").unwrap();
        assert!(!read("main.rhai").ends_with("// mine"));
        assert_eq!(read("main.rhai.mine"), "first copy");
        assert!(read("main.rhai.mine-2").ends_with("// mine"));
        assert_eq!(
            unedited_version(&plugin, "subsonic").as_deref(),
            Some(current.as_str())
        );
    }

    #[test]
    fn versions_compare_by_number() {
        assert!(newer_version("1.10.0", "1.9.9"));
        assert!(newer_version("1.1", "1.0.9"));
        assert!(!newer_version("1.0", "1.0.0"));
        assert!(newer_version("1.0.0", "0"));
    }

    /// Network errors say what happened without the link, which can carry a sign-in token.
    #[test]
    fn network_errors_leave_out_the_link() {
        let error = reqwest::blocking::Client::new()
            .get("http://127.0.0.1:9/rest/ping.view?u=mei&t=secrettoken&s=salt")
            .timeout(Duration::from_secs(5))
            .send()
            .unwrap_err();
        let text = crate::sources::http_error(&error);
        assert_eq!(text, "Could not connect to 127.0.0.1:9");
        assert!(!text.contains("secrettoken"));
    }

    /// A plugin with "ask" asks the person (here answered at once), one without is refused;
    /// a dropped file reaches the plugin that opens its type, which can write into a folder
    /// of its own.
    #[test]
    fn plugins_ask_and_open_dropped_files() {
        let dir = tempfile::tempdir().unwrap();
        let library = Library::open(dir.path()).unwrap();
        let folder = dir.path().join("plugins").join("converter");
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(
            folder.join("plugin.toml"),
            "id = \"converter\"\nname = \"Converter\"\npermissions = [\"ask\", \"files\"]\nopens = [\"json\"]",
        )
        .unwrap();
        std::fs::write(
            folder.join("main.rhai"),
            r#"
fn commands() { [ #{ id: "name", title: "Name", scope: "global" } ] }
fn run(command, ids) {
    let name = ask_text("Name", "What should it be called?", "Mine");
    write_file("answer.txt", `${name}`);
}
fn on_file_dropped(file) {
    let colors = parse_json(file.text);
    write_file(`themes/${file.name}.toml`, `accent = "${colors.accent}"`);
}
"#,
        )
        .unwrap();
        let asked: Arc<Mutex<Vec<Question>>> = Arc::default();
        let seen = asked.clone();
        let host = PluginHost::start(library, Arc::default(), move |action| {
            if let HostAction::Ask { id, question, .. } = action {
                seen.lock().unwrap().push(question);
                answer(id, Some(serde_json::json!("Sakura")));
            }
        });
        host.send(PluginEvent::Enable("converter".into(), true));
        wait(|| host.plugins().into_iter().find(|p| p.enabled));
        assert_eq!(host.plugins()[0].manifest.opens, ["json"]);
        host.send(PluginEvent::Run {
            plugin: "converter".into(),
            command: "name".into(),
            track_ids: vec![],
        });
        wait(|| std::fs::read_to_string(folder.join("answer.txt")).ok());
        assert_eq!(
            std::fs::read_to_string(folder.join("answer.txt")).unwrap(),
            "Sakura"
        );
        assert!(matches!(
            &asked.lock().unwrap()[0],
            Question::Text { default, .. } if default == "Mine"
        ));

        let dropped = dir.path().join("pink.json");
        std::fs::write(&dropped, r##"{"accent": "#ff66cc"}"##).unwrap();
        let file = file_map(&dropped).unwrap();
        assert_eq!(file["extension"], "json");
        host.send(PluginEvent::FileDropped {
            plugin: "converter".into(),
            file,
        });
        let theme = folder.join("themes").join("pink.json.toml");
        wait(|| std::fs::read_to_string(&theme).ok());
        assert_eq!(
            std::fs::read_to_string(&theme).unwrap(),
            "accent = \"#ff66cc\""
        );

        // Too big for a plugin.
        let big = dir.path().join("big.json");
        std::fs::write(&big, vec![b' '; MAX_FILE_BYTES as usize + 1]).unwrap();
        assert!(file_map(&big).is_err());
    }

    /// A plugin that did not ask to use the internet cannot be a source: Needle would fetch
    /// its links for it.
    #[test]
    fn a_source_needs_the_network_permission() {
        let dir = tempfile::tempdir().unwrap();
        let library = Library::open(dir.path()).unwrap();
        let folder = dir.path().join("plugins").join("sneaky");
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(
            folder.join("plugin.toml"),
            "id = \"sneaky\"\nname = \"Sneaky\"",
        )
        .unwrap();
        std::fs::write(
            folder.join("main.rhai"),
            "fn source() { #{ name: \"S\", fields: [] } }\nfn signed_in() { true }",
        )
        .unwrap();
        let host = PluginHost::start(library, Arc::default(), |_| {});
        host.send(PluginEvent::Enable("sneaky".into(), true));
        let plugin = wait(|| host.plugins().into_iter().find(|p| p.enabled));
        assert!(plugin.source.is_none());
    }

    #[test]
    fn a_plugin_with_only_themes_needs_no_script() {
        let dir = tempfile::tempdir().unwrap();
        let library = Library::open(dir.path()).unwrap();
        let folder = dir.path().join("plugins").join("pastels");
        std::fs::create_dir_all(folder.join("themes")).unwrap();
        std::fs::write(
            folder.join("plugin.toml"),
            "id = \"pastels\"\nname = \"Pastels\"",
        )
        .unwrap();
        std::fs::write(folder.join("themes").join("mint.toml"), "name = \"Mint\"").unwrap();
        let host = PluginHost::start(library, Arc::default(), |_| {});
        let plugins = wait(|| Some(host.plugins()).filter(|p| p.len() == 1));
        assert!(plugins[0].error.is_none(), "{:?}", plugins[0].error);
        host.send(PluginEvent::Enable("pastels".into(), true));
        wait(|| host.plugins()[0].enabled.then_some(()));
        assert!(host.plugins()[0].error.is_none());
    }

    #[test]
    fn effect_plugins_offer_their_effects_only_while_on() {
        let dir = tempfile::tempdir().unwrap();
        let library = Library::open(dir.path()).unwrap();
        install_examples(&library).unwrap();
        let registry: Arc<crate::effects::Registry> = Arc::default();
        let seen: Arc<Mutex<Vec<HostAction>>> = Arc::default();
        let sink = seen.clone();
        let host = PluginHost::start(library.clone(), registry.clone(), move |a| {
            sink.lock().unwrap().push(a)
        });
        wait(|| Some(host.plugins()).filter(|p| p.len() == EXAMPLES.len()));
        assert!(registry.all().is_empty());
        host.send(PluginEvent::Enable("studio-effects".into(), true));
        host.send(PluginEvent::Enable("bitcrusher".into(), true));
        wait(|| (registry.all().len() == 5).then_some(()));
        let plugins = host.plugins();
        for id in ["studio-effects", "bitcrusher"] {
            let plugin = plugins.iter().find(|p| p.manifest.id == id).unwrap();
            assert!(plugin.error.is_none(), "{:?}", plugin.error);
        }
        assert!(registry.find("bitcrusher", "crush").unwrap().is_wasm());

        // The bitcrusher's WebAssembly runs: at 2 bits, a quiet ramp becomes a few steps.
        let mut rack = crate::effects::Rack::new(registry.clone(), 48000, 2);
        rack.sync(&[crate::effects::EffectSlot {
            uid: "1".into(),
            plugin: "bitcrusher".into(),
            effect: "crush".into(),
            on: true,
            params: [("bits".to_string(), 2.)].into(),
        }]);
        let mut block: Vec<f32> = (0..512).flat_map(|i| [i as f32 / 512., 0.]).collect();
        rack.process(&mut block);
        let mut levels: Vec<i32> = block.iter().map(|s| (s * 1000.) as i32).collect();
        levels.sort();
        levels.dedup();
        assert!(levels.len() <= 3, "{levels:?}");
        assert!(registry.failure("bitcrusher", "crush").is_none());

        // A script with the audio permission can turn its effect on.
        host.send(PluginEvent::Run {
            plugin: "studio-effects".into(),
            command: "night".into(),
            track_ids: vec![],
        });
        wait(|| {
            seen.lock()
                .unwrap()
                .iter()
                .any(|a| matches!(a, HostAction::EffectOn { effect, on: true, .. } if effect == "night"))
                .then_some(())
        });
        host.send(PluginEvent::Enable("bitcrusher".into(), false));
        wait(|| (registry.all().len() == 4).then_some(()));
    }

    #[test]
    fn files_stay_inside_the_plugin_folder() {
        let (_dir, library, host, _seen) = setup();
        host.send(PluginEvent::Enable("now-playing-file".into(), true));
        wait(|| {
            host.plugins()
                .iter()
                .find(|p| p.manifest.id == "now-playing-file" && p.enabled)
                .map(|_| ())
        });
        host.send(PluginEvent::TrackStarted(Box::new(Track {
            title: "Song".into(),
            artist: "Artist".into(),
            ..Default::default()
        })));
        let file = library
            .directory
            .join("plugins/now-playing-file/now-playing.txt");
        wait(|| {
            std::fs::read_to_string(&file)
                .ok()
                .filter(|t| t == "Artist — Song")
        });
        assert!(inside(Path::new("x"), "../escape.txt").is_err());
        assert!(inside(Path::new("x"), "C:\\escape.txt").is_err());
        assert!(inside(Path::new("x"), "ok.txt").is_ok());
    }

    #[test]
    fn lyrics_plugins_answer_and_timed_lyrics_win() {
        let dir = tempfile::tempdir().unwrap();
        let library = Library::open(dir.path()).unwrap();
        let add = |id: &str, name: &str, script: &str| {
            let folder = library.directory.join("plugins").join(id);
            std::fs::create_dir_all(&folder).unwrap();
            std::fs::write(
                folder.join("plugin.toml"),
                format!(
                    "id = \"{id}\"
name = \"{name}\"
"
                ),
            )
            .unwrap();
            std::fs::write(folder.join("main.rhai"), script).unwrap();
        };
        add(
            "a-plain",
            "Plain words",
            r#"fn lyrics(song) { #{ plain: "Just words" } }"#,
        );
        add(
            "b-timed",
            "Timed words",
            r#"fn lyrics(song) { if song.title == "Song" { `[00:01.00]${song.artist} sings` } }"#,
        );
        library
            .set_json(
                ENABLED_KEY,
                &BTreeSet::from(["a-plain".to_string(), "b-timed".to_string()]),
            )
            .unwrap();
        install_examples(&library).unwrap();
        let host = PluginHost::start(library.clone(), Default::default(), |_| {});
        let plugins =
            wait(|| Some(host.plugins()).filter(|p| p.iter().any(|p| p.lyrics && p.enabled)));
        assert!(
            plugins
                .iter()
                .any(|p| p.manifest.id == "netease-lyrics" && p.lyrics && !p.enabled)
        );
        let song = |title: &str| Track {
            title: title.into(),
            artist: "Artist".into(),
            duration: 60.,
            ..Default::default()
        };
        let found = host.lyrics(&song("Song")).unwrap().unwrap();
        assert_eq!(found.provider, "Timed words");
        assert!(found.timed);
        assert_eq!(found.text, "[00:01.00]Artist sings");
        // Only plain lyrics anywhere: those.
        let plain = host.lyrics(&song("Other")).unwrap().unwrap();
        assert_eq!(
            (plain.provider.as_str(), plain.timed),
            ("Plain words", false)
        );
        assert_eq!(host.lyrics_plugins(), ["a-plain", "b-timed"]);
    }

    #[test]
    fn permissions_limits_and_errors_are_contained() {
        let dir = tempfile::tempdir().unwrap();
        let library = Library::open(dir.path()).unwrap();
        let plugin = library.directory.join("plugins/greedy");
        std::fs::create_dir_all(&plugin).unwrap();
        std::fs::write(
            plugin.join("plugin.toml"),
            "id = \"greedy\"\nname = \"Greedy\"\npermissions = []\n",
        )
        .unwrap();
        std::fs::write(plugin.join("main.rhai"), r#"
fn commands() { [ #{ id: "spin", title: "Spin", scope: "global" }, #{ id: "peek", title: "Peek", scope: "global" } ] }
fn run(command, ids) {
    if command == "spin" { loop { } }
    library_search("rating >= 1");
}
"#).unwrap();
        let broken = library.directory.join("plugins/broken");
        std::fs::create_dir_all(&broken).unwrap();
        std::fs::write(
            broken.join("plugin.toml"),
            "id = \"bad id!\"\nname = \"Broken\"\n",
        )
        .unwrap();
        library
            .set_json(ENABLED_KEY, &BTreeSet::from(["greedy".to_string()]))
            .unwrap();
        let seen: Arc<Mutex<Vec<HostAction>>> = Arc::default();
        let sink = seen.clone();
        let host = PluginHost::start(library.clone(), Default::default(), move |a| {
            sink.lock().unwrap().push(a)
        });
        let plugins = wait(|| Some(host.plugins()).filter(|p| p.len() == 2));
        assert!(plugins.iter().any(|p| {
            p.error
                .as_deref()
                .is_some_and(|e| e.contains("letters, digits"))
        }));
        host.send(PluginEvent::Run {
            plugin: "greedy".into(),
            command: "spin".into(),
            track_ids: vec![],
        });
        host.send(PluginEvent::Run {
            plugin: "greedy".into(),
            command: "peek".into(),
            track_ids: vec![],
        });
        let messages = wait(|| {
            let seen = seen.lock().unwrap();
            let notes: Vec<String> = seen
                .iter()
                .filter_map(|a| {
                    if let HostAction::Notify(m) = a {
                        Some(m.clone())
                    } else {
                        None
                    }
                })
                .collect();
            (notes.len() == 2).then_some(notes)
        });
        assert!(
            messages[0].to_lowercase().contains("too many operations"),
            "{messages:?}"
        );
        assert!(
            messages[1].contains("did not ask for permission"),
            "{messages:?}"
        );
    }
}
