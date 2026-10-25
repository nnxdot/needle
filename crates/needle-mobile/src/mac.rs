//! Needle's core for the Mac app (SwiftUI), beyond what the phones use: every song with all
//! its columns, the settings as the desktop keeps them, stems, Discord, the phone remote,
//! folders watched for changes, updates that install themselves, and the rest of the desktop's
//! tools. Rich data goes to Swift as JSON (the core's own types, which Swift decodes); what is
//! read many times a second ([`MacPlayback`]) is a plain record.
use crate::{Needle, NeedleError, Result, Song, log};
use needle_core::{
    audio::{Command, QueueItem, Repeat},
    cast::{Group, Speaker},
    database::Library,
    discord, import, integrations,
    media::{self, LyricLine as CoreLine, LyricWord},
    model::{Playlist, RuleSet, Settings, Track},
    plugins::PluginEvent,
    query, radio, remote, rules, scan, sources, stems, update,
};
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    sync::{
        Arc, LazyLock, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::Duration,
};

/// The Discord application Needle appears as (as on the desktop).
const DISCORD_APP: &str = "1552097597454295110";
/// The most songs one play action queues, as on the desktop.
const PLAY_LIMIT: usize = 50_000;
/// How many songs a radio station queues.
const STATION: usize = 50;
/// The desktop's notes for each version.
const NOTES: &str = include_str!("../../needle/whats-new.md");

fn failed(text: impl Into<String>) -> NeedleError {
    NeedleError::Failed(text.into())
}

fn to_json<T: serde::Serialize>(value: &T) -> Result<String> {
    serde_json::to_string(value).map_err(|e| failed(e.to_string()))
}

fn quote(text: &str) -> String {
    format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""))
}

/// The settings `patch` changes are written over `base`: objects are merged, anything else is
/// replaced.
fn merge(base: &mut Value, patch: Value) {
    match (base, patch) {
        (Value::Object(base), Value::Object(patch)) => {
            for (key, value) in patch {
                merge(base.entry(key).or_insert(Value::Null), value);
            }
        }
        (base, patch) => *base = patch,
    }
}

/// What the Mac app shares with its background work.
#[derive(Default)]
struct Desk {
    started: AtomicBool,
    remote: Mutex<Option<remote::Server>>,
    measuring: Arc<AtomicBool>,
    measured: Mutex<Option<(usize, usize)>>,
    /// Counts changes the folder watcher saw, so lists load again.
    changes: Arc<AtomicU64>,
    /// Sent to the watcher's thread to watch the music folders again (after one is added).
    rewatch: Mutex<Option<std::sync::mpsc::Sender<()>>>,
    discord_refresh: AtomicBool,
    stems_job: Mutex<Option<StemJob>>,
    stems_cancel: Mutex<Arc<AtomicBool>>,
    stems_done: Mutex<Option<StemsDone>>,
    import_progress: Mutex<Option<String>>,
    import_cancel: Mutex<Arc<AtomicBool>>,
    tags_cancel: Mutex<Arc<AtomicBool>>,
    tags_progress: Mutex<Option<TagProgress>>,
    /// The Last.fm sign-in waiting for the browser.
    lastfm: Mutex<Option<integrations::LastfmPending>>,
    /// Held while a playlist of picked songs changes: each change reads the playlist and saves
    /// it whole, so two at once would lose one.
    playlist_edit: Mutex<()>,
    /// The song playing as the library has it now (its tags and cover may have changed since
    /// it was queued), and whether to read it again.
    playing: Mutex<Option<(String, Option<Track>)>>,
    refresh_playing: AtomicBool,
}

static DESK: LazyLock<Desk> = LazyLock::new(Desk::default);

/// What plays, read many times a second.
#[derive(Clone, uniffi::Record)]
pub struct MacPlayback {
    /// The song playing, and why it is there ("From the album …").
    pub current: Option<String>,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub album_artist: String,
    pub duration: f64,
    pub artwork: Option<String>,
    pub rating: i64,
    pub format: String,
    pub sample_rate: i64,
    pub bit_depth: i64,
    pub bitrate: i64,
    pub channels: i64,
    pub reason: String,
    pub streamed: bool,
    pub missing: bool,
    pub playing: bool,
    pub position: f64,
    pub volume: f32,
    /// "off", "all", or "one".
    pub repeat: String,
    pub queue: u32,
    pub queue_version: u64,
    pub loading: bool,
    pub error: Option<String>,
    pub output_notice: Option<String>,
    /// The output's name, the device chosen, and how it opened.
    pub output: String,
    pub output_device: Option<String>,
    pub output_rate: u32,
    pub output_channels: u16,
    pub exclusive: bool,
    pub replay_gain: bool,
    pub album_gain: bool,
    pub loop_start: Option<f64>,
    pub loop_end: Option<f64>,
    /// The song playing from its stems.
    pub stems: Option<String>,
}

/// One song up next.
#[derive(Clone, uniffi::Record)]
pub struct QueueEntry {
    pub id: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration: f64,
    pub artwork: Option<String>,
    pub reason: String,
}

#[derive(Clone, uniffi::Record)]
pub struct QueryCheck {
    /// What the search does, in words ("Matches rule: …"), when it is understood.
    pub explanation: Option<String>,
    pub error: Option<String>,
}

#[derive(Clone, uniffi::Record)]
pub struct StemJob {
    pub id: String,
    pub stage: String,
    pub fraction: f32,
}

#[derive(Clone, uniffi::Record)]
pub struct StemsDone {
    pub id: String,
    pub title: String,
    /// `None` when it worked; "" when it was stopped.
    pub error: Option<String>,
}

#[derive(Clone, uniffi::Record)]
pub struct StemsInfo {
    /// This song has stems.
    pub ready: bool,
    /// The separation model is downloaded.
    pub model_ready: bool,
    /// Bytes the split songs take.
    pub used: u64,
    pub levels: Vec<f32>,
    pub job: Option<StemJob>,
}

#[derive(Clone, uniffi::Record)]
pub struct TagProgress {
    pub done: u32,
    pub total: u32,
    pub current: String,
}

#[derive(Clone, uniffi::Record)]
pub struct GroupMember {
    /// The member's address (empty for this computer), for [`Needle::set_speaker_delay`].
    pub key: String,
    pub name: String,
    pub delay: i32,
}

#[derive(Clone, uniffi::Record)]
pub struct MacUpdate {
    pub version: String,
    pub page: String,
    /// Needle can download and put in this version by itself.
    pub installable: bool,
}

#[derive(Clone, uniffi::Record)]
pub struct PaletteFound {
    pub songs: Vec<Song>,
    pub albums: Vec<crate::Album>,
    pub artists: Vec<String>,
}

static FOUND_UPDATE: Mutex<Option<update::Release>> = Mutex::new(None);

impl Needle {
    fn mac_track(&self, id: &str) -> Result<Track> {
        self.library
            .track(id)?
            .ok_or_else(|| failed("That song is no longer in the library"))
    }

    fn queue_items(&self, ids: &[String], reason: &str) -> Result<Vec<QueueItem>> {
        Ok(self
            .library
            .tracks_by_ids(ids)?
            .into_iter()
            .filter(|t| !t.missing)
            .map(|track| QueueItem {
                track,
                reason: reason.to_string(),
            })
            .collect())
    }

    /// The tracks a page shows: a playlist of picked songs in its own order (filtered by
    /// `filter`), else whatever `query` matches.
    fn view_tracks(
        &self,
        query: &str,
        playlist: Option<&str>,
        filter: &str,
        offset: usize,
        size: usize,
    ) -> Result<(Vec<Track>, usize)> {
        if let Some(id) = playlist
            && let Some(list) = self.library.playlist_by_id(id)?
            && list.query.is_none()
        {
            let mut tracks = self.library.playlist_tracks(&list)?;
            if !filter.trim().is_empty() {
                let ids: std::collections::HashSet<String> = self
                    .library
                    .search(filter.trim())?
                    .into_iter()
                    .map(|t| t.id)
                    .collect();
                tracks.retain(|t| ids.contains(&t.id));
            }
            sort_by_query(&mut tracks, query);
            let total = tracks.len();
            return Ok((tracks.into_iter().skip(offset).take(size).collect(), total));
        }
        let page = self.library.search_page(query, offset, size)?;
        Ok((page.tracks, page.total))
    }

    fn save_settings_with(&self, change: impl FnOnce(&mut Settings)) -> Result<Settings> {
        Ok(self.library.update_settings(change)?)
    }

    /// Starts or stops the phone remote to match the settings.
    fn apply_remote(&self) {
        let Ok(mut settings) = self.library.settings() else {
            return;
        };
        let mut server = DESK.remote.lock().unwrap();
        if !settings.remote {
            *server = None;
            return;
        }
        if settings.remote_key.len() < 16 {
            let key = remote::new_key();
            settings = match self.save_settings_with(|s| s.remote_key = key) {
                Ok(s) => s,
                Err(_) => return,
            };
        }
        if server.is_some() {
            return;
        }
        match remote::start(
            self.player.clone(),
            self.library.clone(),
            settings.remote_key.clone(),
        ) {
            Ok(started) => *server = Some(started),
            Err(error) => {
                needle_core::logfile::error(format!("Phone remote: {error:#}"));
                let _ = self.save_settings_with(|s| s.remote = false);
            }
        }
    }
}

/// Watches the music folders for new, changed, and removed files, and watches them again when
/// asked (a folder was added).
fn watch_folders(library: Library) {
    let (tx, rx) = std::sync::mpsc::channel::<()>();
    *DESK.rewatch.lock().unwrap() = Some(tx);
    std::thread::spawn(move || {
        loop {
            let changes = DESK.changes.clone();
            let watcher = scan::watch(library.clone(), move || {
                changes.fetch_add(1, Ordering::SeqCst);
            });
            if let Err(error) = &watcher {
                log(true, &format!("Watching the music folders: {error:#}"));
            }
            if rx.recv().is_err() {
                break;
            }
            while rx.try_recv().is_ok() {}
            drop(watcher);
        }
    });
}

/// Measures unmeasured songs, a few at a time, while the setting is on (for radio).
fn measure(library: Library) {
    std::thread::spawn(move || {
        loop {
            if !DESK.measuring.load(Ordering::Relaxed) {
                std::thread::sleep(Duration::from_secs(5));
                continue;
            }
            let batch = library.unmeasured(8).unwrap_or_default();
            if batch.is_empty() {
                *DESK.measured.lock().unwrap() = library.measured_count().ok();
                std::thread::sleep(Duration::from_secs(60));
                continue;
            }
            for track in batch {
                if !DESK.measuring.load(Ordering::Relaxed) {
                    break;
                }
                let features = radio::analyze(&track).unwrap_or(radio::Features {
                    version: radio::VERSION,
                    ..Default::default()
                });
                let _ = library.save_features(&track.id, &features);
                std::thread::sleep(Duration::from_millis(40));
            }
            *DESK.measured.lock().unwrap() = library.measured_count().ok();
        }
    });
}

/// Keeps Discord's "Listening to" status and Last.fm's and ListenBrainz's "now playing" in
/// step with playback, as the desktop does.
fn follow_status(player: needle_core::audio::Player, library: Library) {
    std::thread::spawn(move || {
        let mut presence: Option<discord::Presence> = None;
        let mut sent: Option<(String, bool, i64)> = None;
        let mut idle_since: Option<i64> = None;
        let mut now_playing: Option<String> = None;
        let mut settings = library.settings().unwrap_or_default();
        let mut ticks = 0u32;
        loop {
            std::thread::sleep(Duration::from_millis(1000));
            ticks += 1;
            let refresh = DESK.discord_refresh.swap(false, Ordering::SeqCst);
            if refresh || ticks.is_multiple_of(5) {
                settings = library.settings().unwrap_or(settings);
            }
            let state = player.state();
            // Last.fm and ListenBrainz: once per song, when it plays.
            let scrobbling = settings.lastfm_enabled || settings.listenbrainz_enabled;
            let song = state
                .current
                .as_ref()
                .filter(|_| scrobbling && state.playing)
                .map(|c| &c.track);
            let key = song.map(|t| t.id.clone());
            if key != now_playing {
                now_playing = key;
                if let Some(track) = song {
                    let song = integrations::NowPlaying {
                        artist: track.display_artist().to_string(),
                        title: track.title.clone(),
                        album: track.album.clone(),
                        duration: track.duration,
                    };
                    let settings = settings.clone();
                    std::thread::spawn(move || {
                        let credentials = integrations::Credentials::load();
                        if let Err(error) =
                            integrations::send_now_playing(&settings, &credentials, &song)
                        {
                            needle_core::logfile::error(format!("Now playing: {error:#}"));
                        }
                    });
                }
            }
            // Discord.
            if !settings.discord_presence {
                presence = None;
                sent = None;
                continue;
            }
            if state.loading {
                continue;
            }
            let presence =
                presence.get_or_insert_with(|| discord::Presence::start(DISCORD_APP.into()));
            let now = chrono::Utc::now().timestamp();
            if state.playing {
                idle_since = None;
            } else if idle_since.is_none() {
                idle_since = Some(now);
            }
            let idle = settings.discord_idle_minutes > 0
                && idle_since
                    .is_some_and(|since| now - since >= settings.discord_idle_minutes as i64 * 60);
            let hidden = idle || (!state.playing && !settings.discord_paused);
            let activity = state.current.as_ref().filter(|_| !hidden).map(|item| {
                let track = &item.track;
                let started = now - state.position.round() as i64;
                discord::Activity {
                    title: track.title.clone(),
                    artist: track.display_artist().to_string(),
                    album: track.album.clone(),
                    started,
                    ends: (track.duration > 0.).then(|| started + track.duration.round() as i64),
                    paused: !state.playing,
                    find_cover: settings.discord_covers,
                    cover: None,
                    layout: discord::Layout {
                        title: discord::Field::from_name(&settings.discord_title),
                        top: discord::Field::from_name(&settings.discord_top),
                        middle: discord::Field::from_name(&settings.discord_middle),
                        bottom: discord::Field::from_name(&settings.discord_bottom),
                        logo: settings.discord_logo,
                    },
                }
            });
            let key = activity
                .as_ref()
                .map(|a| (format!("{}\u{1}{}", a.title, a.artist), a.paused, a.started));
            let changed = refresh
                || match (&sent, &key) {
                    (None, None) => false,
                    (Some((song, paused, started)), Some((new_song, new_paused, new_started))) => {
                        song != new_song
                            || paused != new_paused
                            || (!new_paused && (started - new_started).abs() > 2)
                    }
                    _ => true,
                };
            if changed {
                presence.set(activity);
                sent = key;
            }
        }
    });
}

#[uniffi::export]
impl Needle {
    // ---------- Starting

    /// Starts what the Mac app runs in the background: the folder watcher, measuring songs
    /// for radio, Discord and "now playing", and the phone remote. Once.
    pub fn start_desktop(&self) {
        if DESK.started.swap(true, Ordering::SeqCst) {
            return;
        }
        let settings = self.library.settings().unwrap_or_default();
        DESK.measuring
            .store(settings.sound_analysis, Ordering::SeqCst);
        watch_folders(self.library.clone());
        measure(self.library.clone());
        follow_status(self.player.clone(), self.library.clone());
        self.apply_remote();
    }

    /// Changes whenever the folder watcher brought in changes.
    pub fn library_changes(&self) -> u64 {
        DESK.changes.load(Ordering::SeqCst)
    }

    /// Watch the music folders again, after one is added or removed.
    pub fn rewatch(&self) {
        if let Some(tx) = DESK.rewatch.lock().unwrap().as_ref() {
            let _ = tx.send(());
        }
    }

    /// Needle's own folder: the library, covers, plugins, themes, and logs.
    pub fn data_directory(&self) -> String {
        self.library.directory.to_string_lossy().to_string()
    }

    /// Stops the background work and saves what plays, for quitting.
    pub fn shutdown_desktop(&self) {
        *DESK.remote.lock().unwrap() = None;
        self.plugins.send(PluginEvent::Shutdown);
        self.player.shutdown();
    }

    // ---------- Songs, with every column

    /// The songs a page shows: `{"tracks": [...], "total": n}`, with every field of each
    /// song. A playlist of picked songs (`playlist`) keeps its own order, filtered by
    /// `filter`; otherwise `query` picks them.
    pub fn view_page(
        &self,
        query: String,
        playlist: Option<String>,
        filter: String,
        offset: u32,
        size: u32,
    ) -> Result<String> {
        let (tracks, total) = self.view_tracks(
            &query,
            playlist.as_deref(),
            &filter,
            offset as usize,
            size.min(5000) as usize,
        )?;
        to_json(&json!({ "tracks": tracks, "total": total }))
    }

    /// Whether a search is understood, and what it does in words.
    pub fn check_query(&self, query: String) -> QueryCheck {
        match query::compile(&query, chrono::Utc::now().timestamp()) {
            Ok(q) => QueryCheck {
                explanation: Some(q.explanation),
                error: None,
            },
            Err(e) => QueryCheck {
                explanation: None,
                error: Some(e.to_string()),
            },
        }
    }

    /// A song with every field, as JSON.
    pub fn track_json(&self, id: String) -> Result<Option<String>> {
        match self.library.track(&id)? {
            Some(track) => Ok(Some(to_json(&track)?)),
            None => Ok(None),
        }
    }

    pub fn tracks_json(&self, ids: Vec<String>) -> Result<String> {
        to_json(&self.library.tracks_by_ids(&ids)?)
    }

    /// Albums matching a search (all with an empty one), each with its rule.
    pub fn albums_json(&self, query: String) -> Result<String> {
        to_json(&self.library.albums(&query)?)
    }

    pub fn artists_json(&self, query: String) -> Result<String> {
        to_json(&self.library.artists(&query)?)
    }

    pub fn artist_albums_json(&self, name: String) -> Result<String> {
        to_json(&self.library.artist_albums(&name)?)
    }

    /// Home: albums played lately, added lately, loved, and played most, and the artists
    /// played most.
    pub fn home_json(&self) -> Result<String> {
        let home = self.library.home(24)?;
        to_json(&json!({
            "recent": home.recent,
            "added": home.added,
            "favorites": home.favorites,
            "most_played": home.most_played,
            "artists": home.artists,
        }))
    }

    /// The rule that shows a music source's songs.
    pub fn source_rule(&self, plugin: String) -> String {
        sources::rule(&plugin)
    }

    /// Whether a song from a music server is kept on this computer, or played lately and so
    /// still here: `[kept, cached]`.
    pub fn kept_state(&self, id: String) -> Vec<bool> {
        match self.library.track(&id).ok().flatten() {
            Some(t) => vec![sources::is_kept(&t), sources::is_cached(&t)],
            None => vec![false, false],
        }
    }

    /// An artist's photo already on this computer; never looks online (for grids).
    pub fn artist_photo_saved(&self, name: String) -> Option<String> {
        media::artist_image(&self.library, &name, false)
            .ok()
            .flatten()
            .map(|p| p.to_string_lossy().to_string())
    }

    /// Songs, albums, and artists for the command palette.
    pub fn palette_search(&self, text: String) -> PaletteFound {
        let text = text.trim();
        if text.is_empty() {
            return PaletteFound {
                songs: vec![],
                albums: vec![],
                artists: vec![],
            };
        }
        let q = quote(text);
        let songs = self
            .library
            .search_page(&format!("title contains {q} or artist contains {q}"), 0, 8)
            .map(|p| p.tracks.iter().map(Song::from).collect())
            .unwrap_or_default();
        let albums = self
            .library
            .albums(text)
            .unwrap_or_default()
            .into_iter()
            .take(6)
            .map(crate::Album::from)
            .collect();
        let artists = self
            .library
            .artists(text)
            .unwrap_or_default()
            .into_iter()
            .take(6)
            .map(|a| a.name)
            .collect();
        PaletteFound {
            songs,
            albums,
            artists,
        }
    }

    // ---------- Playing

    /// Plays everything a page shows, from its song `start` (counted from the first match),
    /// or all of it shuffled. Returns how many songs were queued.
    pub fn play_view(
        &self,
        query: String,
        playlist: Option<String>,
        filter: String,
        start: u32,
        shuffle: bool,
        reason: String,
    ) -> Result<u32> {
        let (mut tracks, _) =
            self.view_tracks(&query, playlist.as_deref(), &filter, 0, PLAY_LIMIT)?;
        let reason = if shuffle {
            format!("Shuffled · {reason}")
        } else {
            reason
        };
        let start = start as usize;
        if shuffle {
            use rand::seq::SliceRandom;
            let first = (start > 0).then(|| tracks.get(start).cloned()).flatten();
            tracks.shuffle(&mut rand::thread_rng());
            if let Some(first) = first {
                tracks.retain(|t| t.id != first.id);
                tracks.insert(0, first);
            }
            tracks.retain(|t| !t.missing);
            let count = tracks.len();
            let items = tracks
                .into_iter()
                .map(|track| QueueItem {
                    track,
                    reason: reason.clone(),
                })
                .collect();
            self.player.send(Command::Play(items));
            return Ok(count as u32);
        }
        // Earlier songs stay in the list, so Repeat all comes back round to them. A chosen song
        // whose file is gone starts the list at the next one that is there.
        let first = tracks
            .iter()
            .skip(start)
            .find(|t| !t.missing)
            .map(|t| t.id.clone());
        tracks.retain(|t| !t.missing);
        let start = first
            .and_then(|id| tracks.iter().position(|t| t.id == id))
            .unwrap_or(tracks.len());
        let count = tracks.len();
        if start >= count {
            return Ok(0);
        }
        let items = tracks
            .into_iter()
            .map(|track| QueueItem {
                track,
                reason: reason.clone(),
            })
            .collect();
        self.player.send(Command::PlayAt(items, start));
        Ok(count as u32)
    }

    /// Plays these songs in order, from `start`, saying why.
    pub fn play_songs(&self, ids: Vec<String>, start: u32, reason: String) -> Result<u32> {
        let items = self.queue_items(&ids, &reason)?;
        let count = items.len() as u32;
        if count > 0 {
            let start = (start as usize).min(items.len() - 1);
            self.player.send(Command::PlayAt(items, start));
        }
        Ok(count)
    }

    pub fn enqueue_songs(&self, ids: Vec<String>, reason: String) -> Result<u32> {
        let items = self.queue_items(&ids, &reason)?;
        let count = items.len() as u32;
        self.player.send(Command::Enqueue(items));
        Ok(count)
    }

    /// Reads the song playing from the library again on the next `mac_playback` (after its
    /// tags, cover, or rating changed).
    pub fn refresh_playing(&self) {
        DESK.refresh_playing.store(true, Ordering::Relaxed);
    }

    pub fn play_next_songs(&self, ids: Vec<String>, reason: String) -> Result<u32> {
        let items = self.queue_items(&ids, &reason)?;
        let count = items.len() as u32;
        self.player.send(Command::PlayNext(items));
        Ok(count)
    }

    pub fn mac_playback(&self) -> MacPlayback {
        let s = self.player.state();
        // The library's copy of the song playing, read again when the song changes or after
        // a change to the library (see `refresh_playing`); the queue's copy as it was queued.
        let fresh = {
            let mut playing = DESK.playing.lock().unwrap_or_else(|p| p.into_inner());
            let id = s.current.as_ref().map(|c| c.track.id.clone());
            let refresh = DESK.refresh_playing.swap(false, Ordering::Relaxed);
            match (&id, playing.as_ref()) {
                (Some(id), Some((known, _))) if known == id && !refresh => {}
                (Some(id), _) => {
                    *playing = Some((id.clone(), self.library.track(id).ok().flatten()))
                }
                (None, _) => *playing = None,
            }
            playing.as_ref().and_then(|(_, track)| track.clone())
        };
        let track = fresh.as_ref().or(s.current.as_ref().map(|c| &c.track));
        let text = |f: fn(&Track) -> String| track.map(f).unwrap_or_default();
        MacPlayback {
            current: track.map(|t| t.id.clone()),
            title: text(|t| t.title.clone()),
            artist: text(|t| t.display_artist().to_string()),
            album: text(|t| t.album.clone()),
            album_artist: text(|t| t.album_artist.clone()),
            duration: track.map_or(0., |t| t.duration),
            artwork: track.and_then(|t| t.artwork.clone()),
            rating: track.map_or(0, |t| t.rating),
            format: text(|t| t.format.clone()),
            sample_rate: track.map_or(0, |t| t.sample_rate),
            bit_depth: track.map_or(0, |t| t.bit_depth),
            bitrate: track.map_or(0, |t| t.bitrate),
            channels: track.map_or(0, |t| t.channels),
            reason: s
                .current
                .as_ref()
                .map(|c| c.reason.clone())
                .unwrap_or_default(),
            streamed: track.is_some_and(|t| t.is_streamed()),
            missing: track.is_some_and(|t| t.missing),
            playing: s.playing,
            position: s.position,
            volume: s.volume,
            repeat: match s.repeat {
                Repeat::Off => "off",
                Repeat::All => "all",
                Repeat::One => "one",
            }
            .into(),
            queue: s.queue.len() as u32,
            queue_version: s.queue_version,
            loading: s.loading,
            error: s.error.clone(),
            output_notice: s.output_notice.clone(),
            output: s.output.clone(),
            output_device: s.output_device.clone(),
            output_rate: s.output_rate,
            output_channels: s.output_channels,
            exclusive: s.exclusive,
            replay_gain: s.replay_gain,
            album_gain: s.album_gain,
            loop_start: s.loop_range.map(|r| r.0),
            loop_end: s.loop_range.map(|r| r.1),
            stems: s.stems.clone(),
        }
    }

    /// The songs up next, from `offset`, at most `limit`.
    pub fn queue_entries(&self, offset: u32, limit: u32) -> Vec<QueueEntry> {
        self.player
            .state()
            .queue
            .iter()
            .skip(offset as usize)
            .take(limit as usize)
            .map(|q| QueueEntry {
                id: q.track.id.clone(),
                title: q.track.title.clone(),
                artist: q.track.display_artist().to_string(),
                album: q.track.album.clone(),
                duration: q.track.duration,
                artwork: q.track.artwork.clone(),
                reason: q.reason.clone(),
            })
            .collect()
    }

    /// Seconds of music up next.
    pub fn queue_seconds(&self) -> f64 {
        self.player
            .state()
            .queue
            .iter()
            .map(|q| q.track.duration)
            .sum()
    }

    pub fn clear_queue(&self) {
        self.player.send(Command::ClearQueue);
    }

    pub fn set_volume(&self, value: f32) {
        self.player.send(Command::Volume(value.clamp(0., 1.)));
    }

    /// The seek bar is held (silent) or let go.
    pub fn scrub(&self, held: bool) {
        self.player.send(Command::Scrub(held));
    }

    pub fn stop(&self) {
        self.player.send(Command::Stop);
    }

    /// Plays this song up next: the one at `index` if it is still there.
    pub fn jump_to(&self, id: String, index: u32) {
        self.player.send(Command::JumpTo(id, index as usize));
    }

    // ---------- Settings, as the desktop keeps them

    /// All of Needle's settings, as JSON (the desktop's own names).
    pub fn settings_json(&self) -> Result<String> {
        to_json(&self.library.settings()?)
    }

    /// Changes the settings `patch` names (a JSON object, merged into them), saves them, and
    /// tells the player and the background work what changed.
    pub fn update_settings(&self, patch: String) -> Result<()> {
        let patch: Value = serde_json::from_str(&patch).map_err(|e| failed(e.to_string()))?;
        let keys: Vec<String> = patch
            .as_object()
            .map(|o| o.keys().cloned().collect())
            .unwrap_or_default();
        let mut error = None;
        let saved = self.library.update_settings(|s| {
            let Ok(mut value) = serde_json::to_value(&*s) else {
                return;
            };
            merge(&mut value, patch);
            match serde_json::from_value::<Settings>(value) {
                Ok(next) => *s = next,
                Err(e) => error = Some(e.to_string()),
            }
        })?;
        if let Some(error) = error {
            return Err(failed(error));
        }
        let has = |names: &[&str]| keys.iter().any(|k| names.contains(&k.as_str()));
        if has(&[
            "output_device",
            "exclusive",
            "replay_gain",
            "album_gain",
            "crossfade",
            "smart_mix",
            "autoplay_query",
        ]) {
            self.player
                .send(Command::Configure(Box::new(saved.clone())));
        } else if has(&["dsp"]) {
            self.player.send(Command::Dsp(saved.dsp.clone()));
        }
        if has(&["remote", "remote_key"]) {
            if keys.iter().any(|k| k == "remote_key") {
                *DESK.remote.lock().unwrap() = None;
            }
            self.apply_remote();
        }
        if has(&["sound_analysis"]) {
            DESK.measuring.store(saved.sound_analysis, Ordering::SeqCst);
        }
        if keys
            .iter()
            .any(|k| k.starts_with("discord") || k.ends_with("_enabled"))
        {
            DESK.discord_refresh.store(true, Ordering::SeqCst);
        }
        Ok(())
    }

    /// Checks a layout file before it is used.
    pub fn check_layout(&self, json: String) -> Result<()> {
        let layout: needle_core::model::Layout =
            serde_json::from_str(&json).map_err(|e| failed(e.to_string()))?;
        Ok(layout.validate()?)
    }

    /// The sound outputs of this Mac.
    pub fn output_devices(&self) -> Vec<String> {
        needle_core::audio::devices().unwrap_or_default()
    }

    /// Writes the equalizer as an Equalizer APO file ("ParametricEQ.txt").
    pub fn save_eq_file(&self, path: String) -> Result<()> {
        use needle_core::dsp::{BANDS, ParamBand, format_parametric};
        let dsp = self.library.settings()?.dsp;
        let bands: Vec<ParamBand> = if dsp.parametric_mode() {
            dsp.parametric.clone()
        } else {
            BANDS
                .iter()
                .zip(dsp.bands)
                .filter(|(_, g)| *g != 0.)
                .map(|(f, g)| ParamBand {
                    frequency: *f as f32,
                    gain: g,
                    q: 1.41,
                    ..Default::default()
                })
                .collect()
        };
        std::fs::write(&path, format_parametric(dsp.preamp_db, &bands))
            .map_err(anyhow::Error::from)?;
        Ok(())
    }

    /// How many songs are measured for radio, of how many: `[done, total]`.
    pub fn measured_count(&self) -> Vec<u64> {
        let found = *DESK.measured.lock().unwrap();
        let (done, total) = found
            .or_else(|| self.library.measured_count().ok())
            .unwrap_or_default();
        vec![done as u64, total as u64]
    }

    /// A song's sound in words (tempo, key, energy), once measured.
    pub fn sound_summary(&self, id: String) -> Option<String> {
        self.library
            .features(&id)
            .ok()
            .flatten()
            .filter(|f| f.usable())
            .map(|f| f.summary())
    }

    // ---------- Phone remote

    /// The phone remote's address, while it runs.
    pub fn remote_address(&self) -> Option<String> {
        DESK.remote.lock().unwrap().as_ref().map(|s| s.address())
    }

    /// A new address for the phone remote: phones with the old one no longer work.
    pub fn new_remote_address(&self) -> Result<()> {
        let key = remote::new_key();
        self.save_settings_with(|s| s.remote_key = key)?;
        *DESK.remote.lock().unwrap() = None;
        self.apply_remote();
        Ok(())
    }

    // ---------- Playlists

    /// Every playlist with all it holds (its rule, its rule builder's rules, its songs' ids).
    pub fn playlists_json(&self) -> Result<String> {
        to_json(&self.library.playlists()?)
    }

    pub fn playlist_json(&self, id: String) -> Result<Option<String>> {
        match self.library.playlist_by_id(&id)? {
            Some(p) => Ok(Some(to_json(&p)?)),
            None => Ok(None),
        }
    }

    /// Saves a playlist (a new one when its id is empty); a rule builder's rules become its
    /// rule. Returns its id.
    pub fn save_playlist_json(&self, json: String) -> Result<String> {
        let mut playlist: Playlist =
            serde_json::from_str(&json).map_err(|e| failed(e.to_string()))?;
        playlist.name = playlist.name.trim().to_string();
        if playlist.name.is_empty() {
            return Err(failed("Give the playlist a name."));
        }
        if playlist.id.is_empty() {
            playlist.id = uuid::Uuid::new_v4().to_string();
        }
        if let Some(set) = &playlist.rules {
            playlist.query = Some(rules::to_query(set)?);
        }
        if let Some(rule) = &playlist.query {
            query::compile(rule, chrono::Utc::now().timestamp())?;
        }
        playlist.updated_at = chrono::Utc::now().timestamp();
        self.library.save_playlist(&playlist)?;
        Ok(playlist.id)
    }

    /// The rule builder's kinds of rule and orders: `{"kinds": [{label, ops, value,
    /// placeholder}], "orders": [...]}`.
    pub fn rule_kinds_json(&self) -> String {
        let kinds: Vec<Value> = rules::KINDS
            .iter()
            .map(|k| {
                let (value, placeholder) = match k.value {
                    rules::Value::Text(p) => ("text", p),
                    rules::Value::Number(p) => ("number", p),
                    rules::Value::None => ("none", ""),
                };
                json!({
                    "label": k.label,
                    "ops": k.ops.iter().map(|(label, _)| *label).collect::<Vec<_>>(),
                    "value": value,
                    "placeholder": placeholder,
                })
            })
            .collect();
        let orders: Vec<&str> = rules::ORDERS.iter().map(|(label, _)| *label).collect();
        json!({ "kinds": kinds, "orders": orders }).to_string()
    }

    /// The rule a rule builder's rules make.
    pub fn rules_query(&self, json: String) -> Result<String> {
        let set: RuleSet = serde_json::from_str(&json).map_err(|e| failed(e.to_string()))?;
        Ok(rules::to_query(&set)?)
    }

    /// How many songs a rule picks now.
    pub fn count_matching(&self, query: String) -> Result<u32> {
        Ok(self.library.search_page(&query, 0, 1)?.total as u32)
    }

    /// Adds songs to a playlist of picked songs, each once. Returns how many were added.
    pub fn add_songs_to_playlist(&self, id: String, ids: Vec<String>) -> Result<u32> {
        let _edit = DESK.playlist_edit.lock().unwrap_or_else(|p| p.into_inner());
        let mut playlist = self.playlist(&id)?;
        if playlist.query.is_some() {
            return Err(failed("A smart playlist picks its own songs"));
        }
        // Only songs in the library: any text dropped on a playlist is not a song.
        let known: std::collections::HashSet<String> = self
            .library
            .tracks_by_ids(&ids)?
            .into_iter()
            .map(|t| t.id)
            .collect();
        let mut new: Vec<String> = vec![];
        for song in ids {
            if known.contains(&song) && !playlist.track_ids.contains(&song) && !new.contains(&song)
            {
                new.push(song);
            }
        }
        let count = new.len() as u32;
        if count > 0 {
            playlist.track_ids.extend(new);
            playlist.updated_at = chrono::Utc::now().timestamp();
            self.library.save_playlist(&playlist)?;
        }
        Ok(count)
    }

    /// Moves songs (already in it, or dropped from elsewhere) to just before the song
    /// `before`, or to the end.
    pub fn place_in_playlist(
        &self,
        id: String,
        ids: Vec<String>,
        before: Option<String>,
    ) -> Result<()> {
        let _edit = DESK.playlist_edit.lock().unwrap_or_else(|p| p.into_inner());
        let mut playlist = self.playlist(&id)?;
        if before.as_ref().is_some_and(|b| ids.contains(b)) {
            return Ok(());
        }
        let moving: Vec<String> = playlist
            .track_ids
            .iter()
            .filter(|t| ids.contains(t))
            .cloned()
            .collect();
        let mut adding = if moving.is_empty() {
            // New to the list: only songs in the library.
            let known: std::collections::HashSet<String> = self
                .library
                .tracks_by_ids(&ids)?
                .into_iter()
                .map(|t| t.id)
                .collect();
            ids.into_iter().filter(|id| known.contains(id)).collect()
        } else {
            moving
        };
        playlist.track_ids.retain(|t| !adding.contains(t));
        adding.dedup();
        let at = before
            .and_then(|b| playlist.track_ids.iter().position(|t| *t == b))
            .unwrap_or(playlist.track_ids.len());
        playlist.track_ids.splice(at..at, adding);
        playlist.updated_at = chrono::Utc::now().timestamp();
        Ok(self.library.save_playlist(&playlist)?)
    }

    /// Takes songs out of a playlist of picked songs. Returns how many went.
    pub fn remove_songs_from_playlist(&self, id: String, ids: Vec<String>) -> Result<u32> {
        let _edit = DESK.playlist_edit.lock().unwrap_or_else(|p| p.into_inner());
        let mut playlist = self.playlist(&id)?;
        let before = playlist.track_ids.len();
        playlist.track_ids.retain(|t| !ids.contains(t));
        let removed = (before - playlist.track_ids.len()) as u32;
        playlist.updated_at = chrono::Utc::now().timestamp();
        self.library.save_playlist(&playlist)?;
        Ok(removed)
    }

    /// Reads an M3U or M3U8 file into a new playlist. Returns its name and songs, in words.
    pub fn import_playlist_file(&self, path: String) -> Result<String> {
        let p = self.library.import_playlist(Path::new(&path))?;
        Ok(format!(
            "Imported {} tracks into “{}”.",
            p.track_ids.len(),
            p.name
        ))
    }

    // ---------- Radio

    /// The artists that sound nearest to `artist`, for a blend.
    pub fn similar_artists(&self, artist: String) -> Vec<String> {
        self.library
            .similar_artists(&artist, 10)
            .unwrap_or_default()
    }

    /// Plays songs from both artists that flow from one to the other. Returns how many.
    pub fn start_blend(&self, first: String, second: String) -> Result<u32> {
        let station = self.library.blend(&first, &second, STATION)?;
        if station.len() < 2 {
            return Err(failed(
                "Not enough songs are measured yet to make a station. Needle measures them in the background; try again soon.",
            ));
        }
        let reason = format!("Blend: {first} × {second}");
        let count = station.len() as u32;
        self.player.send(Command::Play(
            station
                .into_iter()
                .map(|track| QueueItem {
                    track,
                    reason: reason.clone(),
                })
                .collect(),
        ));
        Ok(count)
    }

    /// Plays this song, then songs that sound like it. Returns how many.
    pub fn radio_from(&self, id: String) -> Result<u32> {
        let track = self.mac_track(&id)?;
        let station = self
            .library
            .radio(std::slice::from_ref(&track), STATION - 1)?;
        if station.is_empty() {
            return Err(failed(
                "Not enough songs are measured yet to make a station. Needle measures them in the background; try again soon.",
            ));
        }
        let reason = format!("Radio: like “{}”", track.title);
        let mut items = vec![QueueItem {
            track,
            reason: reason.clone(),
        }];
        items.extend(station.into_iter().map(|track| QueueItem {
            track,
            reason: reason.clone(),
        }));
        let count = items.len() as u32;
        self.player.send(Command::Play(items));
        Ok(count)
    }

    /// Plays an artist's station. Returns how many songs.
    pub fn artist_radio(&self, artist: String) -> Result<u32> {
        let station = self.library.artist_radio(&artist, STATION)?;
        if station.len() < 2 {
            return Err(failed(
                "Not enough songs are measured yet to make a station. Needle measures them in the background; try again soon.",
            ));
        }
        let reason = format!("Radio: {artist}");
        let count = station.len() as u32;
        self.player.send(Command::Play(
            station
                .into_iter()
                .map(|track| QueueItem {
                    track,
                    reason: reason.clone(),
                })
                .collect(),
        ));
        Ok(count)
    }

    // ---------- Stems

    /// The song being split, if any: cheap, for the window to follow often (`stems_info` also
    /// adds up every split song's files).
    pub fn stems_job(&self) -> Option<StemJob> {
        DESK.stems_job
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }

    pub fn stems_info(&self, id: String) -> StemsInfo {
        StemsInfo {
            ready: stems::stems_for(&self.library, &id).is_some(),
            model_ready: stems::model_ready(&self.library),
            used: stems::cache_size(&self.library),
            levels: (0..stems::STEMS.len())
                .map(|i| self.player.stem_mix().get(i))
                .collect(),
            job: DESK.stems_job.lock().unwrap().clone(),
        }
    }

    /// Splits a song into drums, bass, other, and vocals, downloading the model first when
    /// needed. Runs in the background; see [`Needle::stems_info`] and
    /// [`Needle::take_stems_done`].
    pub fn split_stems(&self, id: String) -> Result<()> {
        let track = self.mac_track(&id)?;
        if track.is_streamed() {
            return Err(failed(
                "Songs on a music server cannot be split. Save it to your music first.",
            ));
        }
        let mut job = DESK.stems_job.lock().unwrap();
        if job.is_some() {
            return Err(failed(
                "Another song is being split. Wait for it, or stop it first.",
            ));
        }
        *job = Some(StemJob {
            id: id.clone(),
            stage: "Starting…".into(),
            fraction: 0.,
        });
        let cancel = Arc::new(AtomicBool::new(false));
        *DESK.stems_cancel.lock().unwrap() = cancel.clone();
        let library = self.library.clone();
        std::thread::spawn(move || {
            let progress = |stage: String, fraction: f32| {
                *DESK.stems_job.lock().unwrap() = Some(StemJob {
                    id: track.id.clone(),
                    stage,
                    fraction,
                });
            };
            let result = (|| -> anyhow::Result<()> {
                if !stems::model_ready(&library) {
                    stems::download_model(&library, &cancel, |done, total| {
                        progress(
                            format!(
                                "Downloading the stem model · {} of {} MB",
                                done / 1_048_576,
                                total.max(1) / 1_048_576
                            ),
                            if total > 0 {
                                done as f32 / total as f32
                            } else {
                                0.
                            },
                        )
                    })?;
                }
                stems::separate(&library, &track, &cancel, |fraction| {
                    progress("Splitting into stems".into(), fraction)
                })?;
                Ok(())
            })();
            *DESK.stems_job.lock().unwrap() = None;
            *DESK.stems_done.lock().unwrap() = Some(StemsDone {
                id: track.id.clone(),
                title: track.title.clone(),
                error: match result {
                    Ok(()) => None,
                    Err(e) => {
                        let text = format!("{e:#}");
                        Some(if text == "Stopped" || text == "Download stopped" {
                            String::new()
                        } else {
                            text
                        })
                    }
                },
            });
        });
        Ok(())
    }

    pub fn stop_stems(&self) {
        DESK.stems_cancel
            .lock()
            .unwrap()
            .store(true, Ordering::SeqCst);
    }

    /// A split that finished since the last call.
    pub fn take_stems_done(&self) -> Option<StemsDone> {
        DESK.stems_done.lock().unwrap().take()
    }

    /// Plays a song from its stems (with the live mix), or from its file again.
    pub fn play_stems(&self, id: String, on: bool) -> Result<()> {
        let dir = stems::stems_for(&self.library, &id);
        if on {
            let playing = self
                .player
                .state()
                .current
                .is_some_and(|c| c.track.id == id);
            if !playing {
                let track = self.mac_track(&id)?;
                self.player.send(Command::Play(vec![QueueItem {
                    track,
                    reason: "Played from its stems".into(),
                }]));
            }
        }
        self.player
            .send(Command::Stems(if on { dir.map(|d| (id, d)) } else { None }));
        Ok(())
    }

    /// A stem's level, 0 to 1.5 (drums, bass, other, vocals).
    pub fn set_stem_level(&self, stem: u32, level: f32) {
        self.player
            .stem_mix()
            .set(stem as usize, level.clamp(0., 1.5));
    }

    pub fn delete_stems(&self, id: String) -> Result<()> {
        if self.player.state().stems.as_deref() == Some(id.as_str()) {
            self.player.send(Command::Stems(None));
        }
        Ok(stems::delete_stems(&self.library, &id)?)
    }

    pub fn delete_all_stems(&self) {
        self.player.send(Command::Stems(None));
        let _ = std::fs::remove_dir_all(self.library.directory.join("stems"));
    }

    pub fn delete_stem_model(&self) {
        let _ = std::fs::remove_file(stems::model_path(&self.library));
    }

    // ---------- Tags, lookups, and loudness

    /// What the chosen songs have in common, field by field (`{"state": "same", "value": …}`
    /// or `{"state": "mixed"}`).
    pub fn common_tags_json(&self, ids: Vec<String>) -> Result<String> {
        to_json(&scan::common_tags(&self.library.tracks_by_ids(&ids)?))
    }

    /// Writes tags (a JSON `TagEdit`: only the fields given change) into the songs' files.
    /// Each file is backed up first and its sound checked after.
    pub fn write_tags_json(
        &self,
        ids: Vec<String>,
        edit: String,
    ) -> Result<crate::tools::TagReport> {
        let edit: scan::TagEdit = serde_json::from_str(&edit).map_err(|e| failed(e.to_string()))?;
        let cancel = Arc::new(AtomicBool::new(false));
        *DESK.tags_cancel.lock().unwrap() = cancel.clone();
        let report = scan::write_tags_batch(&self.library, &ids, &edit, cancel, |p| {
            *DESK.tags_progress.lock().unwrap() = Some(TagProgress {
                done: p.done as u32,
                total: p.total as u32,
                current: p.current.clone(),
            });
        });
        *DESK.tags_progress.lock().unwrap() = None;
        let report = report?;
        Ok(crate::tools::TagReport {
            saved: report.saved.len() as u32,
            failed: report
                .failed
                .into_iter()
                .map(|f| format!("{}: {}", f.path.trim_start_matches("\\\\?\\"), f.error))
                .collect(),
        })
    }

    pub fn tags_progress(&self) -> Option<TagProgress> {
        DESK.tags_progress.lock().unwrap().clone()
    }

    pub fn stop_tags(&self) {
        DESK.tags_cancel
            .lock()
            .unwrap()
            .store(true, Ordering::SeqCst);
    }

    /// Recordings on MusicBrainz that may be this song, by its artist and title (JSON).
    pub fn musicbrainz_lookup(&self, id: String) -> Result<String> {
        let track = self.mac_track(&id)?;
        to_json(&integrations::musicbrainz_search(
            &self.library,
            &track.artist,
            &track.title,
        )?)
    }

    /// Recordings that sound like this song, from AcoustID (JSON).
    pub fn acoustid_lookup_json(&self, id: String) -> Result<String> {
        let track = self.mac_track(&id)?;
        if track.cue.is_some() {
            return Err(failed(
                "Songs from a CUE sheet can't be identified by sound.",
            ));
        }
        let key = integrations::acoustid_key().unwrap_or_default();
        to_json(&integrations::acoustid_lookup(
            &self.library,
            Path::new(&track.path),
            &key,
        )?)
    }

    /// Saves a release's cover from the Cover Art Archive for a song.
    pub fn use_release_cover(&self, id: String, release: String) -> Result<()> {
        integrations::cover_art(&self.library, &id, &release)?;
        Ok(())
    }

    pub fn measure_loudness(&self, id: String) -> Result<String> {
        let result = needle_core::analysis::scan_loudness(&self.library, &id)?;
        Ok(format!(
            "Measured {:.1} LUFS · ReplayGain {:+.2} dB. Turn on ReplayGain in Settings to use it.",
            result.integrated_lufs, result.replay_gain_db
        ))
    }

    pub fn measure_album_loudness(&self, id: String) -> Result<String> {
        let album = needle_core::analysis::scan_album_loudness(&self.library, &id)?;
        Ok(format!(
            "Album measured: {:.1} LUFS · gain {:+.2} dB across {} tracks.",
            album.integrated_lufs,
            album.replay_gain_db,
            album.tracks.len()
        ))
    }

    /// Looks up covers for every album without one. Returns how many got one.
    pub fn find_all_covers(&self) -> Result<u32> {
        let mut found = 0;
        for album in self
            .library
            .albums("")?
            .into_iter()
            .filter(|a| a.artwork.is_none())
        {
            if let Some(track) = self.library.album_tracks(&album.key)?.into_iter().next() {
                found += media::fetch_album_art(&self.library, &track)
                    .unwrap_or(0)
                    .min(1);
            }
        }
        Ok(found as u32)
    }

    /// Keeps `keep` and merges the others into it; their files go to the Trash.
    /// Folds `others` into `keep`. The app has moved their files to the Trash first (the
    /// Finder's own way, so Put Back works), so here they only leave the library.
    pub fn merge_into(&self, keep: String, others: Vec<String>) -> Result<()> {
        // With the other playlist edits: two merges at once each saved a playlist from before
        // the other's change.
        let _edit = DESK.playlist_edit.lock().unwrap_or_else(|p| p.into_inner());
        let keep = self.mac_track(&keep)?;
        let others = self.library.tracks_by_ids(&others)?;
        Ok(self.library.merge_duplicates(&keep, &others, false)?)
    }

    // ---------- Lyrics beside the song

    /// Saves lyrics timed in the timing editor as a `.lrc` file beside the song. Returns its
    /// name.
    pub fn save_lyrics_file(
        &self,
        id: String,
        lines: Vec<crate::media::LyricLine>,
    ) -> Result<String> {
        let track = self.mac_track(&id)?;
        if track.cue.is_some() {
            return Err(failed(
                "This song is part of a CUE sheet, so it has no file of its own to put lyrics beside.",
            ));
        }
        let lines: Vec<CoreLine> = lines
            .into_iter()
            .map(|l| CoreLine {
                time: l.time,
                text: l.text,
                words: l
                    .words
                    .into_iter()
                    .map(|w| LyricWord {
                        time: w.time,
                        text: w.text,
                    })
                    .collect(),
            })
            .collect();
        let path = media::save_lrc(&track, &lines)?;
        Ok(path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default())
    }

    // ---------- Speakers

    /// The members of the speaker group playing now, with their delays.
    pub fn group_members(&self) -> Vec<GroupMember> {
        let Some(group) = self
            .library
            .settings()
            .ok()
            .and_then(|s| s.output_device)
            .as_deref()
            .and_then(Group::from_device_name)
        else {
            return vec![];
        };
        group
            .members()
            .into_iter()
            .map(|(key, name)| GroupMember {
                delay: *group.delays.get(&key).unwrap_or(&0),
                key,
                name,
            })
            .collect()
    }

    /// Moves one member of the group playing now earlier or later, in ms.
    pub fn set_speaker_delay(&self, key: String, ms: i32) -> Result<()> {
        let settings = self.library.settings()?;
        let Some(mut group) = settings
            .output_device
            .as_deref()
            .and_then(Group::from_device_name)
        else {
            return Ok(());
        };
        group.delays.insert(key, ms.clamp(-1000, 1000));
        group.delays.retain(|_, ms| *ms != 0);
        let device = group.device_name();
        self.save_settings_with(|s| s.output_device = Some(device))?;
        self.player
            .send(Command::SpeakerDelays(group.delays.clone()));
        Ok(())
    }

    /// Speakers on the network as JSON (`[{name, kind, address, port, …}]`), after a few
    /// seconds of looking.
    pub fn find_speakers_json(&self) -> String {
        let found = needle_core::cast::discover::discover(Duration::from_secs(3));
        serde_json::to_string(&found).unwrap_or_else(|_| "[]".into())
    }

    /// Plays on one speaker (a JSON speaker from [`Needle::find_speakers_json`]), several at
    /// once (a JSON array, with `this_computer`), or this Mac (`None`).
    pub fn play_on(&self, speakers: Option<String>, this_computer: bool) -> Result<String> {
        let list: Vec<Speaker> = match speakers {
            Some(json) => serde_json::from_str(&json).map_err(|e| failed(e.to_string()))?,
            None => vec![],
        };
        let (device, name) = match (list.len(), this_computer) {
            (0, _) => (None, "this Mac".to_string()),
            (1, false) => (Some(list[0].device_name()), list[0].name.clone()),
            _ => {
                let group = Group {
                    speakers: list,
                    this_computer,
                    delays: Default::default(),
                };
                (Some(group.device_name()), group.name())
            }
        };
        let saved = self.save_settings_with(|s| {
            s.output_device = device;
            s.exclusive = false;
        })?;
        self.player.send(Command::Configure(Box::new(saved)));
        Ok(name)
    }

    // ---------- Online services

    /// Last.fm, ListenBrainz, and AcoustID: signed in or not, and the listens waiting (JSON).
    pub fn services_json(&self) -> String {
        let status = integrations::secret_status();
        let summary = integrations::scrobble_summary(&self.library).ok();
        json!({ "status": status, "summary": summary }).to_string()
    }

    /// Starts signing in to Last.fm (with a key and secret when this build has none): returns
    /// the page to approve Needle on.
    pub fn lastfm_sign_in(&self, key: String, secret: String) -> Result<String> {
        let credentials = integrations::Credentials::load();
        let key = if key.trim().is_empty() {
            credentials.lastfm_api_key.clone()
        } else {
            key.trim().to_string()
        };
        let secret = if secret.trim().is_empty() {
            credentials.lastfm_secret.clone()
        } else {
            secret.trim().to_string()
        };
        if key.is_empty() || secret.is_empty() {
            return Err(failed(
                "Enter your Last.fm API key and shared secret first.",
            ));
        }
        let pending = integrations::lastfm_begin(&key, &secret)?;
        let url = pending.auth_url.clone();
        *DESK.lastfm.lock().unwrap() = Some(pending);
        Ok(url)
    }

    /// Finishes signing in to Last.fm once it is approved in the browser. Returns the user.
    pub fn lastfm_continue(&self) -> Result<String> {
        let pending = DESK
            .lastfm
            .lock()
            .unwrap()
            .clone()
            .ok_or_else(|| failed("Sign in to Last.fm again."))?;
        match integrations::lastfm_complete(&pending) {
            Ok(user) => {
                *DESK.lastfm.lock().unwrap() = None;
                self.save_settings_with(|s| s.lastfm_enabled = true)?;
                DESK.discord_refresh.store(true, Ordering::SeqCst);
                Ok(user)
            }
            Err(e) if integrations::is_not_yet_authorized(&e) => Err(failed(
                "Last.fm hasn't received your approval yet. Approve Needle in the browser, then press Continue.",
            )),
            Err(e) => Err(failed(format!("Last.fm: {e:#}"))),
        }
    }

    pub fn lastfm_pending(&self) -> Option<String> {
        DESK.lastfm
            .lock()
            .unwrap()
            .as_ref()
            .map(|p| p.auth_url.clone())
    }

    pub fn lastfm_cancel(&self) {
        *DESK.lastfm.lock().unwrap() = None;
    }

    /// Sends failed listens again (`lastfm`, `listenbrainz`). Returns how many.
    pub fn retry_scrobbles(&self, service: String) -> Result<u32> {
        Ok(integrations::retry_failed_scrobbles(&self.library, Some(&service))? as u32)
    }

    pub fn clear_acoustid_key(&self) -> Result<()> {
        Ok(integrations::clear_secret(
            integrations::SecretKind::AcoustidKey,
        )?)
    }

    // ---------- Importing from other apps

    /// Libraries found on this computer to import with one click (JSON: kind, path, label,
    /// detail).
    pub fn import_sources_json(&self) -> String {
        serde_json::to_string(&import::detect()).unwrap_or_else(|_| "[]".into())
    }

    /// Imports from another app: "itunes" (library XML), "spotify" (data download, ZIP or
    /// folder), "folder" (a folder of M3U playlists) from `path`; "lastfm" or "listenbrainz"
    /// for the user `path`. Returns what came across, in words. See
    /// [`Needle::import_progress`].
    pub fn import_from(&self, kind: String, path: String) -> Result<String> {
        let cancel = Arc::new(AtomicBool::new(false));
        *DESK.import_cancel.lock().unwrap() = cancel.clone();
        *DESK.import_progress.lock().unwrap() = Some("Starting…".into());
        let mut progress = |text: &str| {
            *DESK.import_progress.lock().unwrap() = Some(text.to_string());
        };
        let file = PathBuf::from(&path);
        let result = match kind.as_str() {
            "itunes" => import::import_itunes(&self.library, &file, &mut progress),
            "spotify" => import::import_spotify(&self.library, &file, &mut progress),
            "folder" => import::import_playlist_folder(&self.library, &file, &mut progress),
            "lastfm" => {
                let key = integrations::Credentials::load().lastfm_api_key;
                import::import_lastfm(&self.library, &path, &key, cancel, &mut progress)
            }
            "listenbrainz" => {
                import::import_listenbrainz(&self.library, &path, cancel, &mut progress)
            }
            _ => Err(anyhow::anyhow!("Unknown kind of import")),
        };
        *DESK.import_progress.lock().unwrap() = None;
        Ok(result?.summary())
    }

    pub fn import_progress(&self) -> Option<String> {
        DESK.import_progress.lock().unwrap().clone()
    }

    pub fn cancel_import(&self) {
        DESK.import_cancel
            .lock()
            .unwrap()
            .store(true, Ordering::SeqCst);
    }

    // ---------- Files opened with Needle, and dropped on it

    /// Plays files opened with Needle (Finder, another app), adding them to the library first;
    /// a CUE sheet plays its songs. Returns how many play.
    pub fn open_files(&self, paths: Vec<String>) -> Result<u32> {
        let mut items = vec![];
        for file in paths {
            let Ok(full) = Path::new(&file).canonicalize() else {
                continue;
            };
            let text = full.to_string_lossy().to_string();
            if full
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("cue"))
            {
                let _ = needle_core::cue::import_sheet(&self.library, &full);
                let mut tracks = self
                    .library
                    .search(&format!("path starts with {}", quote(&format!("{text}#"))))
                    .unwrap_or_default();
                tracks.sort_by_key(|t| t.track_number);
                items.extend(tracks.into_iter().map(|track| QueueItem {
                    track,
                    reason: "Opened".into(),
                }));
                continue;
            }
            let _ = scan::import_one(&self.library, &full);
            if let Ok(Some(track)) = self.library.track_by_path(&text) {
                items.push(QueueItem {
                    track,
                    reason: "Opened".into(),
                });
            }
        }
        let count = items.len() as u32;
        if count > 0 {
            self.player.send(Command::Play(items));
        }
        Ok(count)
    }

    /// Gives a dropped file to the first turned-on plugin that opens its type. Returns what
    /// happened, in words, or `None` when no plugin opens it.
    pub fn drop_to_plugin(&self, path: String) -> Result<Option<String>> {
        let file = Path::new(&path);
        let kind = file
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        if kind.is_empty() {
            return Ok(None);
        }
        let Some(plugin) = self.plugins.plugins().into_iter().find(|p| {
            p.enabled
                && p.manifest
                    .opens
                    .iter()
                    .any(|o| o.trim_start_matches('.').eq_ignore_ascii_case(&kind))
        }) else {
            return Ok(None);
        };
        let map = needle_core::plugins::file_map(file)?;
        self.plugins.send(PluginEvent::FileDropped {
            plugin: plugin.manifest.id.clone(),
            file: map,
        });
        Ok(Some(format!(
            "Gave {} to {}.",
            file.file_name().unwrap_or_default().to_string_lossy(),
            plugin.manifest.name
        )))
    }

    // ---------- Plugins

    /// Plugins that came with Needle and were changed, with the newer version this build has
    /// (JSON: `[{id, update}]`).
    pub fn plugin_updates_json(&self) -> String {
        let found: Vec<Value> = self
            .plugins
            .plugins()
            .into_iter()
            .filter_map(|p| Some(json!({ "id": p.manifest.id, "update": p.update? })))
            .collect();
        Value::Array(found).to_string()
    }

    /// Takes a bundled plugin's newer version (keeping the changed files beside it), or keeps
    /// the changed one.
    pub fn plugin_update(&self, id: String, take: bool) {
        self.plugins.send(if take {
            PluginEvent::TakeUpdate(id)
        } else {
            PluginEvent::KeepChanged(id)
        });
    }

    pub fn plugins_folder(&self) -> String {
        self.library
            .directory
            .join("plugins")
            .to_string_lossy()
            .to_string()
    }

    // ---------- Themes

    /// Custom themes with all a theme file holds (JSON: id, name, base, author, read_only,
    /// music_colors, colors, grain, font, path).
    pub fn themes_json(&self) -> String {
        let mut folders = vec![(self.library.directory.join("themes"), String::new())];
        folders.extend(
            self.plugins
                .plugins()
                .into_iter()
                .filter(|p| p.enabled)
                .map(|p| (p.folder.join("themes"), format!("{}/", p.manifest.id))),
        );
        let mut found: Vec<(String, Value)> = vec![];
        for (folder, prefix) in folders {
            for entry in std::fs::read_dir(&folder).into_iter().flatten().flatten() {
                let path = entry.path();
                if !path.extension().is_some_and(|e| e == "toml") {
                    continue;
                }
                if std::fs::metadata(&path).map_or(true, |m| m.len() > 64 * 1024) {
                    continue;
                }
                let Ok(text) = std::fs::read_to_string(&path) else {
                    continue;
                };
                let Ok(value) = toml::from_str::<toml::Value>(&text) else {
                    continue;
                };
                let stem = path
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                let get = |key: &str| value.get(key).and_then(|v| v.as_str()).map(str::to_string);
                let colors: serde_json::Map<String, Value> = value
                    .get("colors")
                    .and_then(|c| c.as_table())
                    .map(|t| {
                        t.iter()
                            .filter_map(|(k, v)| {
                                let v = v.as_str()?;
                                v.starts_with('#')
                                    .then(|| (k.clone(), Value::String(v.into())))
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                let extras = value.get("extras");
                let name = get("name").unwrap_or_else(|| stem.clone());
                found.push((
                    name.to_lowercase(),
                    json!({
                        "id": format!("custom:{prefix}{stem}"),
                        "name": name,
                        "base": get("base").unwrap_or_else(|| "dark".into()),
                        "author": get("author").unwrap_or_default(),
                        "read_only": !prefix.is_empty(),
                        "music_colors": value.get("music_colors").and_then(|v| v.as_bool()).unwrap_or(true),
                        "colors": colors,
                        "grain": extras.and_then(|e| e.get("grain")).and_then(|v| v.as_float().or_else(|| v.as_integer().map(|i| i as f64))),
                        "font": extras.and_then(|e| e.get("font")).and_then(|v| v.as_str()),
                        "path": path.to_string_lossy(),
                    }),
                ));
            }
        }
        found.sort_by(|a, b| a.0.cmp(&b.0));
        Value::Array(found.into_iter().map(|(_, v)| v).collect()).to_string()
    }

    /// Saves a theme of the listener's own (JSON: id (empty for a new one), name, base,
    /// author, music_colors, colors). Returns its id.
    pub fn save_theme_json(&self, json: String) -> Result<String> {
        let value: Value = serde_json::from_str(&json).map_err(|e| failed(e.to_string()))?;
        let text = |key: &str| value[key].as_str().unwrap_or_default().trim().to_string();
        let name = text("name");
        if name.is_empty() {
            return Err(failed("Give the theme a name"));
        }
        let mut table = toml::map::Map::new();
        table.insert("name".into(), toml::Value::String(name.clone()));
        let base = text("base");
        table.insert(
            "base".into(),
            toml::Value::String(if base.is_empty() { "dark".into() } else { base }),
        );
        let author = text("author");
        if !author.is_empty() {
            table.insert("author".into(), toml::Value::String(author));
        }
        if let Some(on) = value["music_colors"].as_bool() {
            table.insert("music_colors".into(), toml::Value::Boolean(on));
        }
        let mut slots = toml::map::Map::new();
        if let Some(colors) = value["colors"].as_object() {
            for (slot, color) in colors {
                let hex = color.as_str().unwrap_or_default().trim_start_matches('#');
                if hex.len() == 6 && hex.chars().all(|c| c.is_ascii_hexdigit()) {
                    slots.insert(slot.clone(), toml::Value::String(format!("#{hex}")));
                }
            }
        }
        table.insert("colors".into(), toml::Value::Table(slots));
        // The grain and the font the theme had: the editor changes only its colours.
        let mut extras = toml::map::Map::new();
        if let Some(grain) = value["grain"].as_f64() {
            extras.insert("grain".into(), toml::Value::Float(grain));
        }
        let font = text("font");
        if !font.is_empty() {
            extras.insert("font".into(), toml::Value::String(font));
        }
        if !extras.is_empty() {
            table.insert("extras".into(), toml::Value::Table(extras));
        }
        let file = toml::to_string(&toml::Value::Table(table)).map_err(anyhow::Error::from)?;
        let id = text("id");
        crate::media::save_theme_file(
            &self.library,
            (!id.is_empty()).then_some(id.as_str()),
            &name,
            &file,
        )
    }

    pub fn themes_folder(&self) -> String {
        self.library
            .directory
            .join("themes")
            .to_string_lossy()
            .to_string()
    }

    // ---------- Updates

    /// Asks needle.nnx.fyi whether a newer Needle for Mac is out. Sends nothing but the request.
    pub fn check_mac_update(&self) -> Result<Option<MacUpdate>> {
        let found = update::check(env!("CARGO_PKG_VERSION"))?;
        let _ = self.save_settings_with(|s| s.last_update_check = chrono::Utc::now().timestamp());
        let info = found.as_ref().map(|r| MacUpdate {
            version: r.version.clone(),
            page: r.page.clone(),
            installable: r.package.is_some() && r.checksums_url.is_some(),
        });
        *FOUND_UPDATE.lock().unwrap() = found;
        Ok(info)
    }

    /// Downloads the update found, checks it against the published checksums, and puts it in
    /// place of this Needle.app. Needle opens again a moment after it quits: quit straight
    /// after this returns.
    pub fn install_mac_update(&self) -> Result<()> {
        let release = FOUND_UPDATE
            .lock()
            .unwrap()
            .clone()
            .ok_or_else(|| failed("Check for updates first"))?;
        let file = update::download(&release, &std::env::temp_dir())?;
        Ok(update::install(&release, &file)?)
    }

    /// What's new in each version of Needle for the desktop, newest first.
    pub fn desktop_whats_new(&self) -> Vec<crate::media::Notes> {
        let mut notes = vec![];
        for section in NOTES.split("\n# ").map(|s| s.trim_start_matches("# ")) {
            let (head, text) = section.split_once('\n').unwrap_or((section, ""));
            // `# 1.7.1 [windows, linux]` is for those systems only; a section without names
            // is for all.
            let (version, ours) = match head.split_once('[') {
                Some((version, systems)) => (
                    version,
                    systems
                        .trim_end_matches(']')
                        .split(',')
                        .any(|s| s.trim().eq_ignore_ascii_case("macos")),
                ),
                None => (head, true),
            };
            if ours && !version.trim().is_empty() {
                notes.push(crate::media::Notes {
                    version: version.trim().to_string(),
                    text: text.trim().to_string(),
                });
            }
        }
        notes
    }

    // ---------- The log

    pub fn log_folder(&self) -> String {
        needle_core::logfile::log_folder(&self.library.directory)
            .to_string_lossy()
            .to_string()
    }

    /// An error report to paste in an email: the recent log and the last crash, without file
    /// paths or names.
    pub fn error_report(&self) -> String {
        let data = &self.library.directory;
        let crash = needle_core::logfile::pending(data)
            .last()
            .and_then(|f| std::fs::read_to_string(f).ok())
            .map(|t| format!("\n\nLast crash:\n{t}"))
            .unwrap_or_default();
        needle_core::logfile::prepare(&format!(
            "Needle {} on {} {}\n\nRecent log:\n{}{crash}",
            env!("CARGO_PKG_VERSION"),
            std::env::consts::OS,
            std::env::consts::ARCH,
            needle_core::logfile::recent(data, 80)
        ))
    }
}

/// Where Needle keeps its library on this Mac: the same folder the desktop app used, so the
/// library, playlists, and history carry over.
/// Puts a playlist's own songs in the order a page's column sort asks for (`… order by year
/// desc`): its query picks no songs, so the library's search cannot sort them.
fn sort_by_query(tracks: &mut [Track], query: &str) {
    let Some((_, order)) = query.rsplit_once("order by ") else {
        return;
    };
    let mut words = order.split_whitespace();
    let Some(field) = words.next() else { return };
    let descending = words.next().is_some_and(|w| w.eq_ignore_ascii_case("desc"));
    let text = |a: &str, b: &str| a.to_lowercase().cmp(&b.to_lowercase());
    let number = |a: f64, b: f64| a.partial_cmp(&b).unwrap_or(std::cmp::Ordering::Equal);
    tracks.sort_by(|a, b| {
        let order = match field {
            "title" => text(&a.title, &b.title),
            "artist" => text(a.display_artist(), b.display_artist()),
            "album" => text(&a.album, &b.album),
            "album_artist" => text(&a.album_artist, &b.album_artist),
            "genre" => text(&a.genre, &b.genre),
            "format" => text(&a.format, &b.format),
            "year" => a.year.cmp(&b.year),
            "track_number" => (a.disc, a.track_number).cmp(&(b.disc, b.track_number)),
            "bitrate" => a.bitrate.cmp(&b.bitrate),
            "sample_rate" => a.sample_rate.cmp(&b.sample_rate),
            "play_count" => a.play_count.cmp(&b.play_count),
            "added_at" => a.added_at.cmp(&b.added_at),
            "rating" => a.rating.cmp(&b.rating),
            "last_played" => a.last_played.cmp(&b.last_played),
            "bpm" => number(a.bpm.unwrap_or(0.), b.bpm.unwrap_or(0.)),
            "duration" => number(a.duration, b.duration),
            _ => std::cmp::Ordering::Equal,
        };
        if descending { order.reverse() } else { order }
    });
}

#[uniffi::export]
pub fn default_data_directory() -> String {
    Library::default_directory().to_string_lossy().to_string()
}
