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
    /// Names of the sound effects it adds.
    pub effects: Vec<String>,
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
}

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
        std::thread::Builder::new()
            .name("needle-plugins".into())
            .spawn(move || run(library, rx, infos, effects, actions))
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
    effects: Arc<crate::effects::Registry>,
    actions: Arc<dyn Fn(HostAction) + Send + Sync>,
) {
    let now_playing: Arc<Mutex<Option<Track>>> = Arc::default();
    let mut loaded: Vec<Loaded> = vec![];
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
                publish(&loaded);
                offer(&loaded);
            }
            PluginEvent::TrackStarted(track) => {
                *now_playing.lock().unwrap_or_else(|p| p.into_inner()) = Some((*track).clone());
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
                        effects: vec![],
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
            },
            engine,
            ast: None,
            scope: Scope::new(),
            effects: vec![],
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
        for (_, file, contents) in EXAMPLE_FILES.iter().filter(|(f, ..)| f == name) {
            std::fs::write(dir.join(file), contents)?;
        }
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
