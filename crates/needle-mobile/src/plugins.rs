//! Plugins on the phone, run by Needle's own plugin host as on desktop: music servers
//! (Navidrome, Subsonic), lyrics finders (NetEase), sound effects, and commands. What a plugin
//! asks of the app (a message, a question) waits in a list the app reads.
use crate::{Needle, Result, Song, log};
use needle_core::{
    audio::{Command, Player, QueueItem},
    database::Library,
    effects::EffectSlot,
    model::Track,
    plugins::{HostAction, PluginEvent, PluginHost, Question},
};
use std::{
    collections::{BTreeMap, HashMap, VecDeque},
    sync::{Arc, Mutex},
};

#[derive(Clone, uniffi::Record)]
pub struct PluginField {
    pub id: String,
    pub label: String,
    pub placeholder: String,
    pub secret: bool,
}

#[derive(Clone, uniffi::Record)]
pub struct PluginSwitch {
    pub id: String,
    pub label: String,
    pub detail: String,
    pub on: bool,
}

#[derive(Clone, uniffi::Record)]
pub struct MusicSource {
    pub name: String,
    pub fields: Vec<PluginField>,
    pub switches: Vec<PluginSwitch>,
    pub signed_in: bool,
    pub songs: u32,
    pub synced_at: Option<i64>,
    pub syncing: bool,
    pub error: Option<String>,
}

#[derive(Clone, uniffi::Record)]
pub struct PluginCommand {
    pub id: String,
    pub title: String,
    /// It acts on the songs chosen (from a song's menu), not on its own.
    pub for_songs: bool,
}

#[derive(Clone, uniffi::Record)]
pub struct PluginItem {
    pub id: String,
    pub name: String,
    pub version: String,
    pub author: String,
    pub description: String,
    pub enabled: bool,
    pub error: Option<String>,
    /// Comes with Needle, and its files are as Needle wrote them.
    pub official: bool,
    pub verified: bool,
    /// What it may do, as people read it.
    pub permissions: Vec<String>,
    pub finds_lyrics: bool,
    pub effects: Vec<String>,
    pub commands: Vec<PluginCommand>,
    pub source: Option<MusicSource>,
}

#[derive(Clone, uniffi::Record)]
pub struct EffectParam {
    pub id: String,
    pub name: String,
    pub min: f32,
    pub max: f32,
    pub step: f32,
    pub value: f32,
    /// The value as people read it, such as "3.0 dB".
    pub shown: String,
}

#[derive(Clone, uniffi::Record)]
pub struct Effect {
    pub plugin: String,
    pub plugin_name: String,
    pub id: String,
    pub name: String,
    pub description: String,
    /// In the listener's chain and turned on.
    pub on: bool,
    pub params: Vec<EffectParam>,
    pub failure: Option<String>,
}

/// Something a plugin asks of the app.
#[derive(Clone, uniffi::Enum)]
pub enum Notice {
    /// A short message to show.
    Message { text: String },
    /// A question to answer with [`Needle::answer`]: some text, or (with `extensions`) a
    /// file to choose.
    Question {
        id: u64,
        plugin: String,
        title: String,
        prompt: String,
        default: String,
        file: bool,
        extensions: Vec<String>,
    },
    /// Ratings or playlists changed: lists should load again.
    LibraryChanged,
}

/// What the plugin host shares with the app.
#[derive(Default)]
pub(crate) struct PluginState {
    notices: Mutex<VecDeque<Notice>>,
    /// The latest songs the music servers found for a search, by its number.
    server_songs: Mutex<(u64, Vec<Track>)>,
    search_number: std::sync::atomic::AtomicU64,
}

fn items(library: &Library, ids: Vec<String>) -> Vec<QueueItem> {
    library
        .tracks_by_ids(&ids)
        .unwrap_or_default()
        .into_iter()
        .map(|track| QueueItem {
            track,
            reason: "From a plugin".into(),
        })
        .collect()
}

/// Starts the plugin host, answering what plugins ask that needs no screen here.
pub(crate) fn start(library: &Library, player: &Player, state: Arc<PluginState>) -> PluginHost {
    let (library, player) = (library.clone(), player.clone());
    let host_library = library.clone();
    PluginHost::start(host_library, player.effects().clone(), move |action| {
        let push = |notice| state.notices.lock().unwrap().push_back(notice);
        match action {
            HostAction::Notify(text) => push(Notice::Message { text }),
            HostAction::Ask {
                id,
                plugin,
                question,
            } => push(match question {
                Question::Text {
                    title,
                    prompt,
                    default,
                } => Notice::Question {
                    id,
                    plugin,
                    title,
                    prompt,
                    default,
                    file: false,
                    extensions: vec![],
                },
                Question::File { title, extensions } => Notice::Question {
                    id,
                    plugin,
                    title,
                    prompt: String::new(),
                    default: String::new(),
                    file: true,
                    extensions,
                },
            }),
            HostAction::Play(ids) => player.send(Command::Play(items(&library, ids))),
            HostAction::Enqueue(ids) => player.send(Command::Enqueue(items(&library, ids))),
            HostAction::PlayNext(ids) => player.send(Command::PlayNext(items(&library, ids))),
            HostAction::Toggle => player.send(Command::Toggle),
            HostAction::Next => player.send(Command::Next),
            HostAction::Previous => player.send(Command::Previous),
            HostAction::LibraryChanged => push(Notice::LibraryChanged),
            HostAction::ServerSongs { generation, tracks } => {
                let mut found = state.server_songs.lock().unwrap();
                if generation >= found.0 {
                    *found = (generation, tracks);
                }
            }
            HostAction::EffectParam {
                plugin,
                effect,
                param,
                value,
            } => {
                let Ok(settings) = library.settings() else {
                    return;
                };
                let mut dsp = settings.dsp;
                let mut changed = false;
                for slot in dsp
                    .effects
                    .iter_mut()
                    .filter(|s| s.plugin == plugin && s.effect == effect)
                {
                    slot.params.insert(param.clone(), value);
                    changed = true;
                }
                if changed {
                    player.send(Command::Dsp(dsp));
                }
            }
            HostAction::EffectOn { plugin, effect, on } => {
                let Ok(settings) = library.settings() else {
                    return;
                };
                let mut dsp = settings.dsp;
                let mut found = false;
                for slot in dsp
                    .effects
                    .iter_mut()
                    .filter(|s| s.plugin == plugin && s.effect == effect)
                {
                    slot.on = on;
                    found = true;
                }
                if !found && on {
                    dsp.effects.push(EffectSlot::new(&plugin, &effect));
                }
                if found || on {
                    player.send(Command::Dsp(dsp));
                }
            }
        }
    })
}

/// Tells the plugins what plays: a new song, a pause, a listen that counted.
pub(crate) fn follow_playback(
    player: Player,
    library: Library,
    host: PluginHost,
    stop: Arc<std::sync::atomic::AtomicBool>,
) {
    std::thread::spawn(move || {
        let (mut song, mut playing, mut last_listen) = (None::<String>, false, None::<String>);
        while !stop.load(std::sync::atomic::Ordering::Relaxed) {
            let state = player.state();
            let id = state.current.as_ref().map(|c| c.track.id.clone());
            if id != song {
                if let Some(current) = &state.current {
                    host.send(PluginEvent::TrackStarted(Box::new(current.track.clone())));
                }
                song = id;
                playing = state.playing;
                // A song ended or was left: its listen may have been recorded.
                if let Ok(latest) = library.history(1)
                    && let Some(listen) = latest.into_iter().next()
                {
                    if last_listen.is_some() && last_listen.as_ref() != Some(&listen.id) {
                        host.send(PluginEvent::Listen(listen.clone()));
                    }
                    last_listen = Some(listen.id);
                }
            } else if state.playing != playing {
                playing = state.playing;
                host.send(if playing {
                    PluginEvent::Resumed
                } else {
                    PluginEvent::Paused
                });
            }
            std::thread::sleep(std::time::Duration::from_millis(500));
        }
    });
}

#[uniffi::export]
impl Needle {
    pub fn plugins(&self) -> Vec<PluginItem> {
        self.plugins
            .plugins()
            .into_iter()
            .map(|p| PluginItem {
                id: p.manifest.id.clone(),
                name: p.manifest.name.clone(),
                version: p.manifest.version.clone(),
                author: p.manifest.author.clone(),
                description: p.manifest.description.clone(),
                enabled: p.enabled,
                error: p.error.clone(),
                official: p.official,
                verified: p.verified,
                permissions: p
                    .manifest
                    .permissions
                    .iter()
                    .map(|perm| perm.describe().to_string())
                    .collect(),
                finds_lyrics: p.lyrics,
                effects: p.effects.clone(),
                commands: p
                    .commands
                    .iter()
                    .map(|c| PluginCommand {
                        id: c.id.clone(),
                        title: c.title.clone(),
                        for_songs: c.for_tracks,
                    })
                    .collect(),
                source: p.source.map(|s| MusicSource {
                    name: s.name,
                    fields: s
                        .fields
                        .into_iter()
                        .map(|f| PluginField {
                            id: f.id,
                            label: f.label,
                            placeholder: f.placeholder,
                            secret: f.secret,
                        })
                        .collect(),
                    switches: s
                        .switches
                        .into_iter()
                        .map(|w| PluginSwitch {
                            id: w.id,
                            label: w.label,
                            detail: w.detail,
                            on: w.on,
                        })
                        .collect(),
                    signed_in: s.signed_in,
                    songs: s.songs as u32,
                    synced_at: s.synced_at,
                    syncing: s.syncing,
                    error: s.error,
                }),
            })
            .collect()
    }

    pub fn set_plugin_enabled(&self, id: String, on: bool) {
        self.plugins.send(PluginEvent::Enable(id, on));
    }

    /// Adds the plugins that come with Needle (NetEase lyrics, the Navidrome / Subsonic
    /// source, and the examples), leaving any already there.
    pub fn install_bundled_plugins(&self) -> Result<u32> {
        let mut added = needle_core::plugins::install_examples(&self.library)? as u32;
        for name in ["netease-lyrics", "subsonic"] {
            if needle_core::plugins::install_example(&self.library, name).unwrap_or(false) {
                added += 1;
            }
        }
        self.plugins.send(PluginEvent::Reload);
        Ok(added)
    }

    pub fn reload_plugins(&self) {
        self.plugins.send(PluginEvent::Reload);
    }

    pub fn source_sign_in(&self, plugin: String, fields: HashMap<String, String>) {
        self.plugins.send(PluginEvent::SourceSignIn {
            plugin,
            fields: fields.into_iter().collect::<BTreeMap<_, _>>(),
        });
    }

    pub fn source_sign_out(&self, plugin: String) {
        self.plugins.send(PluginEvent::SourceSignOut(plugin));
    }

    pub fn source_sync(&self, plugin: String) {
        self.plugins.send(PluginEvent::SourceSync(plugin));
    }

    pub fn source_switch(&self, plugin: String, id: String, on: bool) {
        self.plugins
            .send(PluginEvent::SourceSwitch { plugin, id, on });
    }

    /// Runs a plugin's command, on `songs` when it acts on songs.
    pub fn run_plugin_command(&self, plugin: String, command: String, songs: Vec<String>) {
        self.plugins.send(PluginEvent::Run {
            plugin,
            command,
            track_ids: songs,
        });
    }

    /// Messages and questions from plugins since the last call.
    pub fn take_notices(&self) -> Vec<Notice> {
        self.plugin_state
            .notices
            .lock()
            .unwrap()
            .drain(..)
            .collect()
    }

    /// Answers a plugin's question: typed text, a chosen file's path, or `None` to cancel.
    pub fn answer(&self, id: u64, text: Option<String>, file: Option<String>) {
        let reply = match (text, file) {
            (_, Some(path)) => match needle_core::plugins::file_map(std::path::Path::new(&path)) {
                Ok(map) => Some(map),
                Err(e) => {
                    log(true, &format!("A file for a plugin: {e:#}"));
                    None
                }
            },
            (Some(text), None) => Some(serde_json::Value::String(text)),
            (None, None) => None,
        };
        needle_core::plugins::answer(id, reply);
    }

    /// Asks the music servers that search (octo-fiesta) for a search; read the songs later
    /// with [`Needle::server_results`] and the number returned.
    pub fn search_servers(&self, query: String) -> u64 {
        let generation = self
            .plugin_state
            .search_number
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
            + 1;
        self.plugins
            .send(PluginEvent::SourceSearch { query, generation });
        generation
    }

    pub fn server_results(&self, generation: u64) -> Vec<Song> {
        let found = self.plugin_state.server_songs.lock().unwrap();
        if found.0 == generation {
            found.1.iter().map(Song::from).collect()
        } else {
            vec![]
        }
    }

    /// Plays songs a music server found that are not in the library.
    pub fn play_server_songs(&self, generation: u64, start: u32) {
        let found = self.plugin_state.server_songs.lock().unwrap();
        if found.0 != generation || found.1.is_empty() {
            return;
        }
        let items: Vec<QueueItem> = found
            .1
            .iter()
            .cloned()
            .map(|track| QueueItem {
                track,
                reason: "From your server".into(),
            })
            .collect();
        let start = (start as usize).min(items.len() - 1);
        self.player.send(Command::PlayAt(items, start));
    }

    /// The sound effects plugins offer, with whether each is on and its sliders.
    pub fn effects(&self) -> Vec<Effect> {
        let chain = self
            .library
            .settings()
            .map(|s| s.dsp.effects)
            .unwrap_or_default();
        let registry = self.player.effects();
        registry
            .all()
            .iter()
            .map(|def| {
                let slot = chain
                    .iter()
                    .find(|s| s.plugin == def.plugin && s.effect == def.id)
                    .cloned()
                    .unwrap_or_else(|| EffectSlot::new(&def.plugin, &def.id));
                let on = chain
                    .iter()
                    .any(|s| s.plugin == def.plugin && s.effect == def.id && s.on);
                let values = def.values(&slot);
                Effect {
                    plugin: def.plugin.clone(),
                    plugin_name: def.plugin_name.clone(),
                    id: def.id.clone(),
                    name: def.name.clone(),
                    description: def.description.clone(),
                    on,
                    params: def
                        .params
                        .iter()
                        .zip(values)
                        .map(|(p, value)| EffectParam {
                            id: p.id.clone(),
                            name: p.name.clone(),
                            min: p.min,
                            max: p.max,
                            step: p.step(),
                            value,
                            shown: p.display(value),
                        })
                        .collect(),
                    failure: registry.failure(&def.plugin, &def.id),
                }
            })
            .collect()
    }

    pub fn set_effect(&self, plugin: String, effect: String, on: bool) -> Result<()> {
        let mut dsp = self.library.settings()?.dsp;
        let mut found = false;
        for slot in dsp
            .effects
            .iter_mut()
            .filter(|s| s.plugin == plugin && s.effect == effect)
        {
            slot.on = on;
            found = true;
        }
        if !found && on {
            dsp.effects.push(EffectSlot::new(&plugin, &effect));
        }
        self.player.send(Command::Dsp(dsp));
        Ok(())
    }

    pub fn set_effect_param(
        &self,
        plugin: String,
        effect: String,
        param: String,
        value: f32,
    ) -> Result<()> {
        let mut dsp = self.library.settings()?.dsp;
        for slot in dsp
            .effects
            .iter_mut()
            .filter(|s| s.plugin == plugin && s.effect == effect)
        {
            slot.params.insert(param.clone(), value);
        }
        self.player.send(Command::Dsp(dsp));
        Ok(())
    }
}
