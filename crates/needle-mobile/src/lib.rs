//! Needle's core for the phone apps. The app makes one [`Needle`] with its data folder and
//! calls it for the library, playback, and lyrics; everything is plain data, so the Kotlin
//! (and later Swift) side only draws it.
use needle_core::{
    audio::{Command, Player, QueueItem, Repeat},
    browse::AlbumSummary,
    database::Library,
    model::Track,
    scan,
};
use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

uniffi::setup_scaffolding!();

#[cfg(target_os = "android")]
mod android;
mod app;
mod connect;
mod media;
mod more;
mod plugins;
mod settings;
mod tools;

/// Android's log (logcat), for crashes on the Rust side, which would otherwise go nowhere.
#[cfg(target_os = "android")]
mod logcat {
    use std::ffi::{CString, c_char, c_int};

    #[link(name = "log")]
    unsafe extern "C" {
        fn __android_log_write(priority: c_int, tag: *const c_char, text: *const c_char) -> c_int;
    }

    pub fn write(error: bool, text: &str) {
        let (Ok(tag), Ok(text)) = (
            CString::new("Needle"),
            CString::new(text.replace(char::MIN, " ")),
        ) else {
            return;
        };
        // 6 is ANDROID_LOG_ERROR, 4 is ANDROID_LOG_INFO.
        // SAFETY: two valid C strings.
        unsafe { __android_log_write(if error { 6 } else { 4 }, tag.as_ptr(), text.as_ptr()) };
    }
}

fn log(error: bool, text: &str) {
    #[cfg(target_os = "android")]
    logcat::write(error, text);
    #[cfg(not(target_os = "android"))]
    eprintln!("{text}");
    let _ = error;
}

#[derive(Debug, thiserror::Error, uniffi::Error)]
#[uniffi(flat_error)]
pub enum NeedleError {
    #[error("{0}")]
    Failed(String),
}

impl From<anyhow::Error> for NeedleError {
    fn from(error: anyhow::Error) -> Self {
        Self::Failed(format!("{error:#}"))
    }
}

type Result<T> = std::result::Result<T, NeedleError>;

#[derive(Clone, uniffi::Record)]
pub struct Song {
    pub id: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration: f64,
    pub track_number: i64,
    pub format: String,
    /// A picture file for the cover, if the song has one.
    pub artwork: Option<String>,
    pub rating: i64,
}

impl From<&Track> for Song {
    fn from(track: &Track) -> Self {
        Self {
            id: track.id.clone(),
            title: track.title.clone(),
            artist: track.display_artist().to_string(),
            album: track.album.clone(),
            duration: track.duration,
            track_number: track.track_number,
            format: track.format.clone(),
            artwork: track.artwork.clone(),
            rating: track.rating,
        }
    }
}

#[derive(Clone, uniffi::Record)]
pub struct Album {
    /// Pass to [`Needle::album_songs`].
    pub key: String,
    pub title: String,
    pub artist: String,
    pub year: i64,
    pub songs: u32,
    pub duration: f64,
    pub artwork: Option<String>,
    /// When it came into the library (Unix seconds).
    pub added_at: i64,
}

impl From<AlbumSummary> for Album {
    fn from(a: AlbumSummary) -> Self {
        Self {
            key: a.key,
            title: a.album,
            artist: a.artist,
            year: a.year,
            songs: a.tracks as u32,
            duration: a.duration,
            artwork: a.artwork,
            added_at: a.added_at,
        }
    }
}

#[derive(Clone, uniffi::Record)]
pub struct Artist {
    pub name: String,
    pub albums: u32,
    pub songs: u32,
    pub artwork: Option<String>,
}

#[derive(Clone, uniffi::Record)]
pub struct Playlist {
    pub id: String,
    pub name: String,
    pub description: String,
    /// A smart playlist follows a rule instead of a list of songs.
    pub smart: bool,
    /// Its own picture, or the first cover among its songs.
    pub artwork: Option<String>,
    pub songs: u32,
    /// Up to four different covers of its songs, for a mosaic when it has no picture of its own.
    pub mosaic: Vec<String>,
}

#[derive(Clone, uniffi::Record)]
pub struct Genre {
    pub name: String,
    pub songs: u32,
    /// A cover from the genre, for its tile.
    pub artwork: Option<String>,
}

#[derive(Clone, uniffi::Record)]
pub struct Home {
    pub recent: Vec<Album>,
    pub added: Vec<Album>,
    pub most_played: Vec<Album>,
}

#[derive(Clone, Copy, uniffi::Enum)]
pub enum RepeatMode {
    Off,
    All,
    One,
}

#[derive(Clone, uniffi::Record)]
pub struct Playback {
    pub current: Option<Song>,
    pub playing: bool,
    pub position: f64,
    pub volume: f32,
    pub repeat: RepeatMode,
    /// How many songs are up next.
    pub up_next: u32,
    /// Changes whenever the songs up next change.
    pub queue_version: u64,
    pub loading: bool,
    pub error: Option<String>,
}

#[derive(Clone, Default, uniffi::Record)]
pub struct ScanStatus {
    pub running: bool,
    pub scanned: u32,
    pub imported: u32,
    pub current: String,
    pub error: Option<String>,
}

#[derive(uniffi::Object)]
pub struct Needle {
    library: Library,
    player: Player,
    scan: Arc<Mutex<ScanStatus>>,
    scanning: Arc<AtomicBool>,
    /// A radio station is being worked out.
    measuring: Arc<AtomicBool>,
    /// Songs are being measured for radio.
    analysing: Arc<AtomicBool>,
    plugins: needle_core::plugins::PluginHost,
    plugin_state: Arc<plugins::PluginState>,
    /// Sends listens to Last.fm and ListenBrainz when they are on.
    _scrobbler: needle_core::integrations::ScrobbleWorker,
}

#[uniffi::export]
impl Needle {
    /// Opens (or makes) Needle's library in `data_dir`, the app's own files folder.
    #[uniffi::constructor]
    pub fn new(data_dir: String) -> Result<Arc<Self>> {
        // Needle's log and crash reports, as on desktop; crashes also go to Android's log.
        needle_core::logfile::init(std::path::Path::new(&data_dir));
        let earlier = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            log(true, &format!("Needle stopped: {info}"));
            earlier(info);
        }));
        log(false, &format!("starting in {data_dir}"));
        #[cfg(target_os = "android")]
        needle_core::set_android_folder(PathBuf::from(&data_dir));
        let library = Library::open(PathBuf::from(data_dir))?;
        let player = Player::new(library.clone());
        log(false, "player started");
        let plugin_state = Arc::new(plugins::PluginState::default());
        let host = plugins::start(&library, &player, plugin_state.clone());
        // Songs streamed from Needle on a computer (Connect) go by their own name; the rest
        // are the plugins' music servers.
        {
            let host = host.clone();
            needle_core::sources::set_resolver(move |plugin, id| {
                if plugin == connect::PC {
                    connect::stream_link(id)
                } else {
                    host.stream_link(plugin, id)
                }
            });
        }
        plugins::follow_playback(
            player.clone(),
            library.clone(),
            host.clone(),
            Arc::new(AtomicBool::new(false)),
        );
        Ok(Arc::new(Self {
            plugins: host,
            plugin_state,
            _scrobbler: needle_core::integrations::ScrobbleWorker::start(library.clone()),
            library,
            player,
            scan: Arc::default(),
            scanning: Arc::default(),
            measuring: Arc::default(),
            analysing: Arc::default(),
        }))
    }

    /// The music folders Needle reads.
    pub fn folders(&self) -> Result<Vec<String>> {
        Ok(self.library.roots()?)
    }

    /// Adds a music folder and reads it (in the background; see [`Needle::scan_status`]).
    pub fn add_folder(&self, path: String) -> Result<()> {
        self.library.add_root(&path)?;
        self.rescan();
        Ok(())
    }

    /// Reads every music folder again, in the background, unless a scan runs already.
    pub fn rescan(&self) {
        if self.scanning.swap(true, Ordering::SeqCst) {
            return;
        }
        let (library, status, scanning) = (
            self.library.clone(),
            self.scan.clone(),
            self.scanning.clone(),
        );
        *status.lock().unwrap() = ScanStatus {
            running: true,
            ..Default::default()
        };
        std::thread::spawn(move || {
            let mut error = None;
            for root in library.roots().unwrap_or_default() {
                let cancel = Arc::new(AtomicBool::new(false));
                let result = scan::import(&library, &PathBuf::from(&root), cancel, |p| {
                    let mut s = status.lock().unwrap();
                    s.scanned = p.scanned as u32;
                    s.imported = p.imported as u32;
                    s.current = p.current.clone();
                });
                if let Err(e) = result {
                    error = Some(format!("{e:#}"));
                }
            }
            let mut s = status.lock().unwrap();
            s.running = false;
            s.current.clear();
            s.error = error;
            scanning.store(false, Ordering::SeqCst);
        });
    }

    pub fn scan_status(&self) -> ScanStatus {
        self.scan.lock().unwrap().clone()
    }

    pub fn song_count(&self) -> Result<u32> {
        Ok(self.library.count()? as u32)
    }

    pub fn home(&self) -> Result<Home> {
        let home = self.library.home(20)?;
        let albums = |list: Vec<AlbumSummary>| list.into_iter().map(Album::from).collect();
        Ok(Home {
            recent: albums(home.recent),
            added: albums(home.added),
            most_played: albums(home.most_played),
        })
    }

    pub fn albums(&self) -> Result<Vec<Album>> {
        Ok(self
            .library
            .albums("")?
            .into_iter()
            .map(Album::from)
            .collect())
    }

    pub fn album_songs(&self, key: String) -> Result<Vec<Song>> {
        Ok(self
            .library
            .album_tracks(&key)?
            .iter()
            .map(Song::from)
            .collect())
    }

    pub fn artists(&self) -> Result<Vec<Artist>> {
        Ok(self
            .library
            .artists("")?
            .into_iter()
            .map(|a| Artist {
                name: a.name,
                albums: a.albums as u32,
                songs: a.tracks as u32,
                artwork: a.artwork,
            })
            .collect())
    }

    pub fn artist_albums(&self, name: String) -> Result<Vec<Album>> {
        Ok(self
            .library
            .artist_albums(&name)?
            .into_iter()
            .map(Album::from)
            .collect())
    }

    pub fn playlists(&self) -> Result<Vec<Playlist>> {
        let mut out = vec![];
        for playlist in self.library.playlists()? {
            let tracks = self.library.playlist_tracks(&playlist)?;
            let mut mosaic: Vec<String> = vec![];
            if playlist.cover.is_none() {
                for a in tracks.iter().filter_map(|t| t.artwork.clone()) {
                    if !mosaic.contains(&a) {
                        mosaic.push(a);
                    }
                    if mosaic.len() == 4 {
                        break;
                    }
                }
            }
            out.push(Playlist {
                mosaic,
                artwork: playlist
                    .cover
                    .clone()
                    .or_else(|| tracks.iter().find_map(|t| t.artwork.clone())),
                songs: tracks.len() as u32,
                smart: playlist.query.is_some(),
                id: playlist.id,
                name: playlist.name,
                description: playlist.description,
            });
        }
        Ok(out)
    }

    pub fn playlist_songs(&self, id: String) -> Result<Vec<Song>> {
        let Some(playlist) = self.library.playlists()?.into_iter().find(|p| p.id == id) else {
            return Ok(vec![]);
        };
        Ok(self
            .library
            .playlist_tracks(&playlist)?
            .iter()
            .filter(|t| !t.missing)
            .map(Song::from)
            .collect())
    }

    /// The album a song is on.
    pub fn album_of(&self, song_id: String) -> Result<Option<Album>> {
        let Some(track) = self.library.track(&song_id)? else {
            return Ok(None);
        };
        let quoted = format!(
            "\"{}\"",
            track.album.replace('\\', "\\\\").replace('"', "\\\"")
        );
        let albums = self.library.albums(&format!("album = {quoted}"))?;
        // Several artists can have an album of the same name: the one this song is on.
        let found = albums
            .iter()
            .find(|a| {
                self.library
                    .album_tracks(&a.key)
                    .is_ok_and(|tracks| tracks.iter().any(|t| t.id == song_id))
            })
            .or(albums.first())
            .cloned();
        Ok(found.map(Album::from))
    }

    /// The library's genres, by name, with how many songs each has.
    pub fn genres(&self) -> Result<Vec<Genre>> {
        Ok(self
            .library
            .genres()?
            .into_iter()
            .map(|(name, songs)| {
                let quoted = needle_core::query::quote(&name);
                let artwork = self
                    .library
                    .search(&format!("genre = {quoted} order by play_count desc limit 30"))
                    .ok()
                    .and_then(|tracks| tracks.into_iter().find_map(|t| t.artwork));
                Genre {
                    name,
                    songs: songs as u32,
                    artwork,
                }
            })
            .collect())
    }

    pub fn genre_songs(&self, name: String) -> Result<Vec<Song>> {
        // As the desktop quotes a value in a search.
        let quoted = format!("\"{}\"", name.replace('\\', "\\\\").replace('"', "\\\""));
        let mut songs: Vec<Song> = self
            .library
            .search(&format!("genre = {quoted}"))?
            .iter()
            .filter(|t| !t.missing)
            .map(Song::from)
            .collect();
        songs.sort_by_key(|s| s.title.to_lowercase());
        Ok(songs)
    }

    /// Every song, by title.
    pub fn songs(&self) -> Result<Vec<Song>> {
        let mut songs: Vec<Song> = self
            .library
            .search("")?
            .iter()
            .filter(|t| !t.missing)
            .map(Song::from)
            .collect();
        songs.sort_by_key(|s| s.title.to_lowercase());
        Ok(songs)
    }

    /// An artist's most played songs.
    pub fn artist_songs(&self, name: String) -> Result<Vec<Song>> {
        Ok(self
            .library
            .top_tracks(&name, 10)?
            .iter()
            .filter(|t| !t.missing)
            .map(Song::from)
            .collect())
    }

    /// Takes a song out of the songs up next.
    pub fn remove_up_next(&self, index: u32) {
        self.player.send(Command::Remove(index as usize));
    }

    /// Moves a song up next from one place to another.
    pub fn move_up_next(&self, from: u32, to: u32) {
        self.player.send(Command::Move(from as usize, to as usize));
    }

    /// Songs matching a search, as on desktop (words, or rules like "rating is at least 4").
    pub fn search(&self, query: String) -> Result<Vec<Song>> {
        Ok(self
            .library
            .search(&query)?
            .iter()
            .filter(|t| !t.missing)
            .take(300)
            .map(Song::from)
            .collect())
    }

    /// Plays `ids` in order, starting at `start`.
    pub fn play(&self, ids: Vec<String>, start: u32) -> Result<()> {
        let items = self.items(&ids)?;
        log(
            false,
            &format!("play {} of {} songs from {start}", items.len(), ids.len()),
        );
        let start = (start as usize).min(items.len().saturating_sub(1));
        self.player.send(Command::PlayAt(items, start));
        Ok(())
    }

    pub fn play_next(&self, ids: Vec<String>) -> Result<()> {
        let items = self.items(&ids)?;
        self.player.send(Command::PlayNext(items));
        Ok(())
    }

    pub fn enqueue(&self, ids: Vec<String>) -> Result<()> {
        let items = self.items(&ids)?;
        self.player.send(Command::Enqueue(items));
        Ok(())
    }

    pub fn toggle(&self) {
        self.player.send(Command::Toggle);
    }

    pub fn next(&self) {
        self.player.send(Command::Next);
    }

    pub fn previous(&self) {
        self.player.send(Command::Previous);
    }

    pub fn seek(&self, seconds: f64) {
        if seconds.is_finite() && seconds >= 0. {
            self.player.send(Command::Seek(seconds));
        }
    }

    pub fn shuffle(&self) {
        self.player.send(Command::Shuffle);
    }

    pub fn set_repeat(&self, mode: RepeatMode) {
        self.player.send(Command::Repeat(match mode {
            RepeatMode::Off => Repeat::Off,
            RepeatMode::All => Repeat::All,
            RepeatMode::One => Repeat::One,
        }));
    }

    /// Plays the song at `index` of the songs up next.
    pub fn jump(&self, index: u32) {
        self.player.send(Command::Jump(index as usize));
    }

    pub fn playback(&self) -> Playback {
        let state = self.player.state();
        Playback {
            current: state.current.as_ref().map(|q| Song::from(&q.track)),
            playing: state.playing,
            position: state.position,
            volume: state.volume,
            repeat: match state.repeat {
                Repeat::Off => RepeatMode::Off,
                Repeat::All => RepeatMode::All,
                Repeat::One => RepeatMode::One,
            },
            up_next: state.queue.len() as u32,
            queue_version: state.queue_version,
            loading: state.loading,
            error: state.error.clone(),
        }
    }

    /// The songs up next, after the one playing.
    pub fn up_next(&self) -> Vec<Song> {
        self.player
            .state()
            .queue
            .iter()
            .map(|q| Song::from(&q.track))
            .collect()
    }

    /// Saves the session and stops playback, for when the app closes.
    pub fn shutdown(&self) {
        self.player.shutdown();
    }
}

impl Needle {
    fn items(&self, ids: &[String]) -> Result<Vec<QueueItem>> {
        Ok(self
            .library
            .tracks_by_ids(ids)?
            .into_iter()
            .filter(|t| !t.missing)
            .map(|track| QueueItem {
                track,
                reason: "Chosen on the phone".into(),
            })
            .collect())
    }
}
