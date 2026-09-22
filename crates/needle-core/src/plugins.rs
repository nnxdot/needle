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
//! is "track" or "global"), and `run(command, track_ids)`. Plugins start disabled; enabling one
//! grants the permissions it declares. Every call is bounded in operations, depth, and size,
//! and runs on the plugin thread, never the interface or audio threads.
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
}

impl Permission {
    pub fn describe(self) -> &'static str {
        match self {
            Self::LibraryRead => "Read your library and history",
            Self::LibraryWrite => "Change ratings and playlists",
            Self::Playback => "Control playback",
            Self::Network => "Use the internet",
            Self::Files => "Use files in its own folder",
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
}

pub enum PluginEvent {
    TrackStarted(Track),
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
}

const ENABLED_KEY: &str = "plugins_enabled";

struct Loaded {
    info: PluginInfo,
    engine: Engine,
    ast: Option<AST>,
    scope: Scope<'static>,
}

/// Runs plugins on their own thread. Clone freely; all clones talk to the same thread.
#[derive(Clone)]
pub struct PluginHost {
    tx: Sender<PluginEvent>,
    infos: Arc<Mutex<Vec<PluginInfo>>>,
    folder: PathBuf,
}

impl PluginHost {
    pub fn start(library: Library, actions: impl Fn(HostAction) + Send + Sync + 'static) -> Self {
        let (tx, rx) = crossbeam_channel::unbounded();
        let infos = Arc::new(Mutex::new(vec![]));
        let folder = library.directory.join("plugins");
        let host = Self {
            tx,
            infos: infos.clone(),
            folder,
        };
        let actions: Arc<dyn Fn(HostAction) + Send + Sync> = Arc::new(actions);
        std::thread::Builder::new()
            .name("needle-plugins".into())
            .spawn(move || run(library, rx, infos, actions))
            .expect("start plugin thread");
        let _ = host.tx.send(PluginEvent::Reload);
        host
    }
    pub fn send(&self, event: PluginEvent) {
        let _ = self.tx.send(event);
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
    infos: Arc<Mutex<Vec<PluginInfo>>>,
    actions: Arc<dyn Fn(HostAction) + Send + Sync>,
) {
    let now_playing: Arc<Mutex<Option<Track>>> = Arc::default();
    let mut loaded: Vec<Loaded> = vec![];
    let publish = |loaded: &Vec<Loaded>| {
        *infos.lock().unwrap_or_else(|p| p.into_inner()) =
            loaded.iter().map(|l| l.info.clone()).collect();
    };
    while let Ok(event) = rx.recv() {
        match event {
            PluginEvent::Shutdown => break,
            PluginEvent::Reload => {
                loaded = load_all(&library, &actions, &now_playing);
                publish(&loaded);
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
                publish(&loaded);
            }
            PluginEvent::TrackStarted(track) => {
                *now_playing.lock().unwrap_or_else(|p| p.into_inner()) = Some(track.clone());
                let value = track_map(&track);
                for plugin in loaded.iter_mut() {
                    let _ = call(plugin, "on_track_start", vec![value.clone().into()]);
                }
                publish(&loaded);
            }
            PluginEvent::Listen(listen) => {
                let value = listen_map(&listen);
                for plugin in loaded.iter_mut() {
                    let _ = call(plugin, "on_listen", vec![value.clone().into()]);
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
                        },
                        folder: dir,
                        enabled: false,
                        error: Some(format!("{error:#}")),
                        commands: vec![],
                    },
                    engine: Engine::new_raw(),
                    ast: None,
                    scope: Scope::new(),
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
            },
            engine,
            ast: None,
            scope: Scope::new(),
        };
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
    engine.set_max_array_size(50_000);
    engine.set_max_map_size(10_000);
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
    engine.register_fn("log", |text: &str| eprintln!("[plugin] {text}"));
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
                std::fs::write(inside(&folder, name)?, text).map_err(|e| fail(e.to_string()))
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
        .map_err(|e| fail(e.to_string()))?;
    let status = response.status();
    let text = response.text().map_err(|e| fail(e.to_string()))?;
    if !status.is_success() {
        return Err(fail(format!("The server answered {status}")));
    }
    Ok(text.chars().take(1 << 20).collect())
}

fn inside(folder: &Path, name: &str) -> Result<PathBuf, Fail> {
    let path = Path::new(name);
    if name.is_empty()
        || path.is_absolute()
        || path
            .components()
            .any(|c| !matches!(c, std::path::Component::Normal(_)))
    {
        return Err(fail(
            "Plugins can only use plain file names inside their own folder",
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
];

/// Copy the example plugins into the plugin folder, leaving any that already exist.
pub fn install_examples(library: &Library) -> Result<usize> {
    let folder = library.directory.join("plugins");
    let mut installed = 0;
    for (name, manifest, script) in EXAMPLES {
        let dir = folder.join(name);
        if dir.exists() {
            continue;
        }
        std::fs::create_dir_all(&dir)?;
        std::fs::write(dir.join("plugin.toml"), manifest)?;
        std::fs::write(dir.join("main.rhai"), script)?;
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
        let host = PluginHost::start(library.clone(), move |a| sink.lock().unwrap().push(a));
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
        host.send(PluginEvent::TrackStarted(Track {
            id: "t1".into(),
            duration: 12.,
            ..Default::default()
        }));
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
        host.send(PluginEvent::TrackStarted(Track {
            title: "Song".into(),
            artist: "Artist".into(),
            ..Default::default()
        }));
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
        let host = PluginHost::start(library.clone(), move |a| sink.lock().unwrap().push(a));
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
