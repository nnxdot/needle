//! Music sources: songs a plugin brings from a server (Subsonic, Navidrome, …), streamed
//! instead of read from a file on this computer.
//!
//! A source song is a library track whose path is `source://<plugin>/<id>`. Its id is made
//! from that path, so syncing again keeps its ratings, plays, and playlists. To play it,
//! Needle asks the plugin for a stream link and downloads the song into a cache folder while
//! the decoder reads what has arrived. A finished download stays in the cache (up to
//! `CACHE_LIMIT`), so playing it again needs no network.
use crate::{database::Library, model::Track};
use anyhow::{Context, Result, bail};
use rodio::Source;
use serde_json::Value;
use std::{
    collections::HashMap,
    fs::File,
    io::{self, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::{Arc, Condvar, LazyLock, Mutex, RwLock},
    time::{Duration, Instant, SystemTime},
};

pub const SCHEME: &str = "source://";
/// The most the stream cache keeps; the songs played longest ago go first.
pub const CACHE_LIMIT: u64 = 2 << 30;
/// How long a stream may stall before playback gives up.
const STALL: Duration = Duration::from_secs(30);

/// A network error in plain words, without the link: a source's links carry sign-in tokens,
/// which must not end up on screen or in the log.
pub fn http_error(error: &reqwest::Error) -> String {
    let host = error
        .url()
        .and_then(|u| {
            u.host_str()
                .map(|h| u.port().map_or(h.to_string(), |p| format!("{h}:{p}")))
        })
        .unwrap_or_else(|| "the server".into());
    if error.is_timeout() {
        format!("{host} did not answer in time")
    } else if error.is_connect() {
        format!("Could not connect to {host}")
    } else if let Some(status) = error.status() {
        format!("{host} answered {status}")
    } else {
        format!("The connection to {host} failed")
    }
}

/// A song as a source plugin describes it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Song {
    pub id: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub album_artist: String,
    pub genre: String,
    pub year: i64,
    pub track: i64,
    pub disc: i64,
    pub duration: f64,
    /// File type, such as "flac" or "mp3".
    pub format: String,
    /// Kilobits per second.
    pub bitrate: i64,
    pub sample_rate: i64,
    pub bit_depth: i64,
    pub channels: i64,
    pub size: i64,
    /// A link to the cover, if the server has one.
    pub cover: String,
    /// The server's name for the cover. Songs that share it share one download, even when
    /// their links differ (a link can carry a new sign-in code each time).
    pub cover_id: String,
    pub musicbrainz_id: String,
}

impl Song {
    /// Read a song from a plugin's map. Numbers may come as numbers or as text.
    pub fn from_value(value: &Value) -> Option<Self> {
        let text = |key: &str| match &value[key] {
            Value::String(s) => s.trim().to_string(),
            Value::Number(n) => n.to_string(),
            _ => String::new(),
        };
        let number = |key: &str| match &value[key] {
            Value::Number(n) => n.as_f64().unwrap_or(0.),
            Value::String(s) => s.trim().parse().unwrap_or(0.),
            _ => 0.,
        };
        let song = Self {
            id: text("id"),
            title: text("title"),
            artist: text("artist"),
            album: text("album"),
            album_artist: text("album_artist"),
            genre: text("genre"),
            year: number("year") as i64,
            track: number("track") as i64,
            disc: number("disc") as i64,
            duration: number("duration"),
            format: text("format").to_lowercase(),
            bitrate: number("bitrate") as i64,
            sample_rate: number("sample_rate") as i64,
            bit_depth: number("bit_depth") as i64,
            channels: number("channels") as i64,
            size: number("size") as i64,
            cover: text("cover"),
            cover_id: text("cover_id"),
            musicbrainz_id: text("musicbrainz_id"),
        };
        (!song.id.is_empty() && !song.title.is_empty()).then_some(song)
    }
}

/// The path of a source song.
pub fn path_for(plugin: &str, id: &str) -> String {
    let mut encoded = String::new();
    for c in id.chars() {
        match c {
            '%' | '/' | '#' | '\\' | '?' => encoded += &format!("%{:02X}", c as u32),
            c => encoded.push(c),
        }
    }
    format!("{SCHEME}{plugin}/{encoded}")
}

/// The plugin and server id of a source song's path.
pub fn parse_path(path: &str) -> Option<(&str, String)> {
    let rest = path.strip_prefix(SCHEME)?;
    let (plugin, encoded) = rest.split_once('/')?;
    let mut id = String::new();
    let mut chars = encoded.chars();
    while let Some(c) = chars.next() {
        if c == '%' {
            let hex: String = chars.by_ref().take(2).collect();
            id.push(
                u32::from_str_radix(&hex, 16)
                    .ok()
                    .and_then(char::from_u32)?,
            );
        } else {
            id.push(c);
        }
    }
    Some((plugin, id))
}

/// The library id of a source song: the same every time it is synced.
pub fn track_id(plugin: &str, id: &str) -> String {
    let hash = blake3::hash(path_for(plugin, id).as_bytes()).to_hex();
    format!("src-{}", &hash[..32])
}

/// The rule that lists one source's songs.
pub fn rule(plugin: &str) -> String {
    format!("path starts with \"{SCHEME}{plugin}/\"")
}

/// A library track for `song`.
pub fn to_track(plugin: &str, song: &Song, now: i64) -> Track {
    Track {
        id: track_id(plugin, &song.id),
        path: path_for(plugin, &song.id),
        title: song.title.clone(),
        artist: song.artist.clone(),
        album: song.album.clone(),
        album_artist: song.album_artist.clone(),
        genre: song.genre.clone(),
        year: song.year,
        track_number: song.track,
        disc: song.disc,
        duration: song.duration,
        sample_rate: song.sample_rate,
        bit_depth: song.bit_depth,
        channels: song.channels,
        format: song.format.to_uppercase(),
        bitrate: song.bitrate,
        added_at: now,
        modified_at: now,
        file_size: song.size,
        musicbrainz_id: Some(song.musicbrainz_id.clone()).filter(|m| !m.is_empty()),
        ..Default::default()
    }
}

/// What a sync changed.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Synced {
    pub added: usize,
    pub updated: usize,
    /// Songs the server no longer has; they stay in the library, marked missing.
    pub gone: usize,
    /// Tracks without a cover yet: the track, the link to fetch it from, and which cover it is
    /// (tracks with the same one share a download).
    pub covers: Vec<(String, String, String)>,
}

/// Bring the library in step with the full list of a source's songs.
pub fn apply(library: &Library, plugin: &str, songs: &[Song]) -> Result<Synced> {
    let now = chrono::Utc::now().timestamp();
    let mut db = library.connection()?;
    let tx = db.transaction()?;
    let mut existing: HashMap<String, Track> = HashMap::new();
    {
        let mut statement = tx.prepare(
            "SELECT data,rating,play_count,last_played,missing FROM tracks WHERE path >= ?1 AND path < ?2",
        )?;
        let prefix = format!("{SCHEME}{plugin}/");
        let end = format!("{SCHEME}{plugin}0");
        for track in statement.query_map([&prefix, &end], Library::row_track)? {
            let track = track?;
            existing.insert(track.id.clone(), track);
        }
    }
    let mut synced = Synced::default();
    let mut seen = std::collections::HashSet::new();
    for song in songs {
        let mut track = to_track(plugin, song, now);
        if !seen.insert(track.id.clone()) {
            continue;
        }
        match existing.get(&track.id) {
            Some(old) => {
                track.added_at = old.added_at;
                track.rating = old.rating;
                track.play_count = old.play_count;
                track.last_played = old.last_played;
                track.artwork = old.artwork.clone();
                track.replay_gain = old.replay_gain;
                track.replay_peak = old.replay_peak;
                track.bpm = old.bpm;
                if track != *old {
                    synced.updated += 1;
                    Library::upsert_on(&tx, &track)?;
                }
            }
            None => {
                synced.added += 1;
                Library::upsert_on(&tx, &track)?;
            }
        }
        if track.artwork.is_none() && !song.cover.is_empty() {
            let key = if song.cover_id.is_empty() {
                song.cover.clone()
            } else {
                song.cover_id.clone()
            };
            synced
                .covers
                .push((track.id.clone(), song.cover.clone(), key));
        }
    }
    for (id, old) in &existing {
        if !seen.contains(id) && !old.missing {
            tx.execute("UPDATE tracks SET missing=1 WHERE id=?", [id])?;
            synced.gone += 1;
        }
    }
    tx.commit()?;
    library.set_json(&format!("source-synced:{plugin}"), &now)?;
    Ok(synced)
}

/// Mark every song of a source missing (after signing out); syncing again brings them back.
pub fn forget(library: &Library, plugin: &str) -> Result<usize> {
    let prefix = format!("{SCHEME}{plugin}/");
    let end = format!("{SCHEME}{plugin}0");
    Ok(library.connection()?.execute(
        "UPDATE tracks SET missing=1 WHERE path >= ?1 AND path < ?2",
        [prefix, end],
    )?)
}

/// How many songs a source has in the library (not counting ones marked missing), and when
/// it was last synced.
pub fn status(library: &Library, plugin: &str) -> (usize, Option<i64>) {
    let prefix = format!("{SCHEME}{plugin}/");
    let end = format!("{SCHEME}{plugin}0");
    let count = library
        .connection()
        .and_then(|db| {
            Ok(db.query_row(
                "SELECT count(*) FROM tracks WHERE path >= ?1 AND path < ?2 AND missing=0",
                [prefix, end],
                |r| r.get::<_, i64>(0),
            )?)
        })
        .unwrap_or(0);
    let synced = library
        .get_json(&format!("source-synced:{plugin}"))
        .ok()
        .flatten();
    (count as usize, synced)
}

/// Download covers, several at once and one per cover, and give them to their tracks.
/// `progress` is called every couple of seconds while covers arrive, so views can show them.
pub fn fetch_covers(
    library: &Library,
    covers: &[(String, String, String)],
    progress: impl Fn() + Sync,
) -> usize {
    let Ok(client) = crate::integrations::client() else {
        return 0;
    };
    let mut by_cover: HashMap<&str, (&str, Vec<&str>)> = HashMap::new();
    for (track, link, key) in covers {
        by_cover
            .entry(key)
            .or_insert_with(|| (link, vec![]))
            .1
            .push(track);
    }
    let jobs = Mutex::new(by_cover.into_values().collect::<Vec<_>>());
    let done = std::sync::atomic::AtomicUsize::new(0);
    let told = Mutex::new(Instant::now());
    std::thread::scope(|scope| {
        for _ in 0..6 {
            scope.spawn(|| {
                loop {
                    let Some((link, tracks)) = jobs.lock().unwrap_or_else(|e| e.into_inner()).pop()
                    else {
                        break;
                    };
                    let saved = (|| -> Result<PathBuf> {
                        let response = client
                            .get(link)
                            .timeout(Duration::from_secs(20))
                            .send()?
                            .error_for_status()?;
                        let mut bytes = vec![];
                        response.take(20 * 1024 * 1024).read_to_end(&mut bytes)?;
                        if bytes.len() < 64 {
                            bail!("not an image");
                        }
                        let path = library
                            .directory
                            .join("artwork")
                            .join(format!("{}.img", blake3::hash(&bytes).to_hex()));
                        if !path.exists() {
                            std::fs::write(&path, &bytes)?;
                        }
                        Ok(path)
                    })();
                    let Ok(path) = saved else {
                        continue;
                    };
                    for id in tracks {
                        if let Ok(Some(mut track)) = library.track(id) {
                            track.artwork = Some(path.to_string_lossy().into());
                            if library.upsert(&track).is_ok() {
                                done.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                            }
                        }
                    }
                    let mut last = told.lock().unwrap_or_else(|e| e.into_inner());
                    if last.elapsed() > Duration::from_secs(2) {
                        *last = Instant::now();
                        drop(last);
                        progress();
                    }
                }
            });
        }
    });
    done.into_inner()
}

// ---- Streaming ----------------------------------------------------------------------------

pub type Resolver = dyn Fn(&str, &str) -> Result<String> + Send + Sync;
static RESOLVER: RwLock<Option<Arc<Resolver>>> = RwLock::new(None);
static CACHE: RwLock<Option<PathBuf>> = RwLock::new(None);
/// Downloads under way, by cache file.
static ACTIVE: LazyLock<Mutex<HashMap<PathBuf, Arc<Download>>>> = LazyLock::new(Default::default);

/// How to turn a source song into a stream link (the plugin host sets this).
pub fn set_resolver(resolver: impl Fn(&str, &str) -> Result<String> + Send + Sync + 'static) {
    *RESOLVER.write().unwrap_or_else(|e| e.into_inner()) = Some(Arc::new(resolver));
}

/// Where streamed songs are kept.
pub fn set_cache_dir(dir: PathBuf) {
    *CACHE.write().unwrap_or_else(|e| e.into_inner()) = Some(dir);
}

fn cache_dir() -> Result<PathBuf> {
    CACHE
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
        .context("Streaming is not set up")
}

/// The cache file for a source song.
pub fn cache_path(track: &Track) -> Result<PathBuf> {
    cache_path_in(&cache_dir()?, track)
}

fn cache_path_in(cache: &Path, track: &Track) -> Result<PathBuf> {
    let (plugin, id) = track.source().context("Not a streamed song")?;
    let extension = Some(track.format.to_lowercase())
        .filter(|f| !f.is_empty() && f.chars().all(|c| c.is_ascii_alphanumeric()))
        .unwrap_or_else(|| "audio".into());
    Ok(cache.join(plugin).join(format!(
        "{}.{extension}",
        &blake3::hash(id.as_bytes()).to_hex()[..32]
    )))
}

/// Whether a source song is fully in the cache (so it plays without the network).
pub fn is_cached(track: &Track) -> bool {
    cache_path(track).is_ok_and(|p| p.is_file())
}

#[derive(Default)]
struct Progress {
    written: u64,
    total: Option<u64>,
    done: bool,
    error: Option<String>,
    /// Readers still using the download; with none left, an unfinished download stops.
    readers: usize,
}

struct Download {
    part: PathBuf,
    progress: Mutex<Progress>,
    changed: Condvar,
}

impl Download {
    fn add_reader(&self) {
        self.progress
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .readers += 1;
    }
    /// One reader is done; with none left, an unfinished download stops.
    fn release(&self) {
        let mut progress = self.progress.lock().unwrap_or_else(|e| e.into_inner());
        progress.readers = progress.readers.saturating_sub(1);
    }
    /// Wait until `until` holds or the download ends or stalls. Returns the progress then.
    fn wait(&self, until: impl Fn(&Progress) -> bool, limit: Duration) -> io::Result<(u64, bool)> {
        let started = Instant::now();
        let mut progress = self.progress.lock().unwrap_or_else(|e| e.into_inner());
        let mut last = progress.written;
        let mut quiet = Instant::now();
        loop {
            if let Some(error) = &progress.error {
                return Err(io::Error::other(error.clone()));
            }
            if until(&progress) || progress.done {
                return Ok((progress.written, progress.done));
            }
            if progress.written != last {
                last = progress.written;
                quiet = Instant::now();
            }
            if quiet.elapsed() > STALL || started.elapsed() > limit {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "The server stopped sending the song",
                ));
            }
            progress = self
                .changed
                .wait_timeout(progress, Duration::from_millis(500))
                .unwrap_or_else(|e| e.into_inner())
                .0;
        }
    }
}

/// Start (or join) the download of a source song into `target`. The caller counts as a
/// reader until it calls `release`.
fn download(
    track: &Track,
    target: &Path,
    cache: &Path,
    resolver: &Resolver,
) -> Result<Arc<Download>> {
    let mut active = ACTIVE.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(running) = active.get(target) {
        running.add_reader();
        return Ok(running.clone());
    }
    let (plugin, id) = track.source().context("Not a streamed song")?;
    let link = resolver(plugin, &id)?;
    if !(link.starts_with("http://") || link.starts_with("https://")) {
        bail!("The plugin gave no stream link for this song");
    }
    if let Some(folder) = target.parent() {
        std::fs::create_dir_all(folder)?;
    }
    let part = target.with_extension(format!(
        "{}.part",
        target.extension().unwrap_or_default().to_string_lossy()
    ));
    let job = Arc::new(Download {
        part: part.clone(),
        progress: Mutex::new(Progress {
            readers: 1,
            ..Default::default()
        }),
        changed: Condvar::new(),
    });
    active.insert(target.to_path_buf(), job.clone());
    drop(active);
    let (thread_job, target, cache) = (job.clone(), target.to_path_buf(), cache.to_path_buf());
    std::thread::Builder::new()
        .name("needle-stream".into())
        .spawn(move || {
            let job = thread_job;
            let result = (|| -> Result<()> {
                let client = reqwest::blocking::Client::builder()
                    .connect_timeout(Duration::from_secs(15))
                    .build()?;
                let mut response = client
                    .get(&link)
                    .send()
                    .map_err(|e| anyhow::anyhow!(http_error(&e)))?;
                if !response.status().is_success() {
                    bail!("The server answered {}", response.status());
                }
                if response
                    .headers()
                    .get("content-type")
                    .and_then(|v| v.to_str().ok())
                    .is_some_and(|t| {
                        t.starts_with("text/") || t.contains("json") || t.contains("xml")
                    })
                {
                    let mut text = String::new();
                    let _ = response.take(2000).read_to_string(&mut text);
                    bail!(
                        "The server sent a message instead of music: {}",
                        text.trim()
                    );
                }
                {
                    let mut progress = job.progress.lock().unwrap_or_else(|e| e.into_inner());
                    progress.total = response.content_length();
                }
                let mut file = File::create(&job.part)?;
                let mut buffer = vec![0u8; 64 * 1024];
                loop {
                    let n = response
                        .read(&mut buffer)
                        .map_err(|_| anyhow::anyhow!("The server stopped sending the song"))?;
                    if n == 0 {
                        break;
                    }
                    file.write_all(&buffer[..n])?;
                    file.flush()?;
                    let mut progress = job.progress.lock().unwrap_or_else(|e| e.into_inner());
                    progress.written += n as u64;
                    let stop = progress.readers == 0;
                    drop(progress);
                    job.changed.notify_all();
                    if stop {
                        bail!("Nobody is listening any more");
                    }
                }
                drop(file);
                std::fs::rename(&job.part, &target)?;
                Ok(())
            })();
            let mut progress = job.progress.lock().unwrap_or_else(|e| e.into_inner());
            match result {
                Ok(()) => progress.done = true,
                Err(error) => {
                    progress.error = Some(format!("{error:#}"));
                    let _ = std::fs::remove_file(&job.part);
                }
            }
            drop(progress);
            job.changed.notify_all();
            ACTIVE
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .remove(&target);
            prune(&cache, CACHE_LIMIT);
        })?;
    Ok(job)
}

/// Keep the cache under `limit` bytes, removing the files used longest ago.
pub fn prune(dir: &Path, limit: u64) {
    let mut files: Vec<(SystemTime, u64, PathBuf)> = walkdir::WalkDir::new(dir)
        .into_iter()
        .flatten()
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().extension().is_none_or(|x| x != "part"))
        .filter_map(|e| {
            let meta = e.metadata().ok()?;
            Some((
                meta.modified().unwrap_or(SystemTime::UNIX_EPOCH),
                meta.len(),
                e.into_path(),
            ))
        })
        .collect();
    let mut total: u64 = files.iter().map(|f| f.1).sum();
    files.sort();
    for (_, size, path) in files {
        if total <= limit {
            break;
        }
        if std::fs::remove_file(&path).is_ok() {
            total -= size;
        }
    }
}

/// Reads a download while it is still arriving: reads past what has come wait for it.
struct Growing {
    job: Arc<Download>,
    file: Option<File>,
    position: u64,
}

impl Growing {
    fn file(&mut self) -> io::Result<&mut File> {
        if self.file.is_none() {
            self.job.wait(|p| p.written > 0, STALL)?;
            self.file = Some(File::open(&self.job.part)?);
        }
        Ok(self.file.as_mut().unwrap())
    }
}

impl Read for Growing {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if buffer.is_empty() {
            return Ok(0);
        }
        let position = self.position;
        let (written, _) = self
            .job
            .wait(|p| p.written > position, Duration::from_secs(600))?;
        if written <= position {
            return Ok(0);
        }
        let size = buffer.len().min((written - position) as usize);
        let file = self.file()?;
        file.seek(SeekFrom::Start(position))?;
        let n = file.read(&mut buffer[..size])?;
        self.position += n as u64;
        Ok(n)
    }
}

impl Seek for Growing {
    fn seek(&mut self, to: SeekFrom) -> io::Result<u64> {
        self.position = match to {
            SeekFrom::Start(at) => at,
            SeekFrom::Current(by) => self
                .position
                .checked_add_signed(by)
                .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Seek before start"))?,
            SeekFrom::End(by) => {
                let total = {
                    let progress = self.job.progress.lock().unwrap_or_else(|e| e.into_inner());
                    progress.total.or(progress.done.then_some(progress.written))
                };
                let total = match total {
                    Some(total) => total,
                    None => self.job.wait(|_| false, Duration::from_secs(600))?.0,
                };
                total.checked_add_signed(by).ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidInput, "Seek before start")
                })?
            }
        };
        Ok(self.position)
    }
}

impl Drop for Growing {
    fn drop(&mut self) {
        self.job.release();
    }
}

/// Formats the stream decoder reads while they arrive; others wait for the whole song.
fn streams_as_it_comes(format: &str) -> bool {
    matches!(
        format,
        "mp3" | "flac" | "wav" | "aac" | "adts" | "ogg" | "oga" | "m4a" | "mp4" | "m4b" | "alac"
    )
}

/// Play a source song: from the cache when it is there, else streamed from the server.
pub fn open(track: &Track) -> Result<Box<dyn Source + Send>> {
    let resolver = RESOLVER
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
        .context("Music sources are not running")?;
    open_with(track, &cache_dir()?, &*resolver)
}

/// `open`, with the cache folder and the way to get stream links given.
pub fn open_with(
    track: &Track,
    cache: &Path,
    resolver: &Resolver,
) -> Result<Box<dyn Source + Send>> {
    let target = cache_path_in(cache, track)?;
    if target.is_file() {
        // Played again: move it to the back of the queue for pruning.
        let _ = File::options()
            .append(true)
            .open(&target)
            .and_then(|f| f.set_modified(SystemTime::now()));
        return Ok(Box::new(crate::audio_file::decode(&target)?));
    }
    let job = download(track, &target, cache, resolver)?;
    let format = track.format.to_lowercase();
    let is_ogg_opus = format == "opus";
    if streams_as_it_comes(&format) && !is_ogg_opus {
        let total = job.progress.lock().unwrap_or_else(|e| e.into_inner()).total;
        job.add_reader();
        let reader = Growing {
            job: job.clone(),
            file: None,
            position: 0,
        };
        let mut builder = rodio::Decoder::builder()
            .with_data(reader)
            .with_seekable(true)
            .with_gapless(true)
            .with_hint(&format);
        if let Some(total) = total {
            builder = builder.with_byte_len(total);
        }
        match builder.build() {
            Ok(decoder) => {
                job.release();
                return Ok(Box::new(decoder));
            }
            Err(error) => crate::logfile::warn(format!(
                "Streaming {} as it arrives failed ({error}); waiting for the whole song",
                track.title
            )),
        }
    }
    // The whole song first (formats read from the end, and Dolby through FFmpeg).
    let waited = job.wait(|_| false, Duration::from_secs(600));
    job.release();
    waited.map_err(|e| anyhow::anyhow!("{} could not be streamed: {e}", track.title))?;
    Ok(Box::new(crate::audio_file::decode(&target)?))
}

/// Download a source song completely and copy it into `folder` as
/// `Artist/Album/NN Title.ext`. Returns the new file.
pub fn save_to(track: &Track, folder: &Path) -> Result<PathBuf> {
    let resolver = RESOLVER
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
        .context("Music sources are not running")?;
    let cache = cache_dir()?;
    let target = cache_path_in(&cache, track)?;
    if !target.is_file() {
        let job = download(track, &target, &cache, &*resolver)?;
        let waited = job.wait(|_| false, Duration::from_secs(1800));
        job.release();
        waited?;
    }
    let clean = |s: &str, fallback: &str| {
        let s: String = s
            .chars()
            .map(|c| {
                if "<>:\"/\\|?*".contains(c) || c.is_control() {
                    '_'
                } else {
                    c
                }
            })
            .collect();
        let s = s.trim().trim_end_matches('.').to_string();
        if s.is_empty() {
            fallback.to_string()
        } else {
            s
        }
    };
    let extension = target
        .extension()
        .map(|e| e.to_string_lossy().to_string())
        .unwrap_or_else(|| "audio".into());
    let name = if track.track_number > 0 {
        format!(
            "{:02} {}",
            track.track_number,
            clean(&track.title, "Untitled")
        )
    } else {
        clean(&track.title, "Untitled")
    };
    let dir = folder
        .join(clean(track.display_album_artist(), "Unknown artist"))
        .join(clean(&track.album, "Unknown album"));
    std::fs::create_dir_all(&dir)?;
    let mut destination = dir.join(format!("{name}.{extension}"));
    let mut n = 2;
    while destination.exists() {
        destination = dir.join(format!("{name} ({n}).{extension}"));
        n += 1;
    }
    std::fs::copy(&target, &destination)?;
    Ok(destination)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn paths_and_ids_round_trip() {
        let path = path_for("subsonic", "al/12#3%");
        assert_eq!(path, "source://subsonic/al%2F12%233%25");
        assert_eq!(
            parse_path(&path),
            Some(("subsonic", "al/12#3%".to_string()))
        );
        assert_eq!(parse_path("C:\\Music\\a.flac"), None);
        assert_eq!(track_id("subsonic", "1"), track_id("subsonic", "1"));
        assert_ne!(track_id("subsonic", "1"), track_id("jellyfin", "1"));
        let track = to_track(
            "subsonic",
            &Song {
                id: "7".into(),
                title: "T".into(),
                ..Default::default()
            },
            0,
        );
        assert!(track.is_streamed());
        assert_eq!(track.source(), Some(("subsonic", "7".to_string())));
    }

    #[test]
    fn songs_are_read_leniently() {
        let song = Song::from_value(&json!({
            "id": 42, "title": "Hey Hi", "artist": "KiiiKiii", "year": "2026",
            "duration": 170.6, "format": "FLAC", "bitrate": 900, "cover": "http://x/c"
        }))
        .unwrap();
        assert_eq!(
            (song.id.as_str(), song.year, song.format.as_str()),
            ("42", 2026, "flac")
        );
        assert!(
            Song::from_value(&json!({"id": "1"})).is_none(),
            "a song needs a title"
        );
    }

    #[test]
    fn syncing_keeps_ratings_and_marks_songs_the_server_lost() {
        let dir = tempfile::tempdir().unwrap();
        let library = Library::open(dir.path()).unwrap();
        let song = |id: &str, title: &str| Song {
            id: id.into(),
            title: title.into(),
            artist: "A".into(),
            cover: format!("http://server/cover/{id}"),
            ..Default::default()
        };
        let first = apply(&library, "sub", &[song("1", "One"), song("2", "Two")]).unwrap();
        assert_eq!((first.added, first.updated, first.gone), (2, 0, 0));
        assert_eq!(first.covers.len(), 2);
        let id = track_id("sub", "1");
        library
            .connection()
            .unwrap()
            .execute("UPDATE tracks SET rating=5 WHERE id=?", [&id])
            .unwrap();
        let again = apply(&library, "sub", &[song("1", "One (remaster)")]).unwrap();
        assert_eq!((again.added, again.updated, again.gone), (0, 1, 1));
        let one = library.track(&id).unwrap().unwrap();
        assert_eq!((one.title.as_str(), one.rating), ("One (remaster)", 5));
        assert!(
            library
                .track(&track_id("sub", "2"))
                .unwrap()
                .unwrap()
                .missing
        );
        assert_eq!(status(&library, "sub").0, 1);
        assert!(status(&library, "sub").1.is_some());
        // Another source's songs are left alone.
        apply(&library, "other", &[song("9", "Nine")]).unwrap();
        assert_eq!(forget(&library, "sub").unwrap(), 2);
        assert_eq!(status(&library, "other").0, 1);
        assert_eq!(library.search(&rule("other")).unwrap().len(), 1);
    }

    /// Server songs are not files here: they are never duplicates to recycle, files to move,
    /// albums to tag or give covers, or songs to download just to measure.
    #[test]
    fn file_only_features_leave_server_songs_alone() {
        let dir = tempfile::tempdir().unwrap();
        let library = Library::open(dir.path()).unwrap();
        let song = |id: &str| Song {
            id: id.into(),
            title: "Same".into(),
            artist: "A".into(),
            album: "B".into(),
            ..Default::default()
        };
        apply(&library, "sub", &[song("1"), song("2")]).unwrap();
        let tracks = library.search("").unwrap();
        assert_eq!(tracks.len(), 2);
        assert!(crate::doctor::find_duplicates(&tracks).is_empty());
        assert!(crate::doctor::album_issues(&tracks).is_empty());
        let plan = crate::doctor::plan_organize(&tracks, &["C:\\Music".into()], "{artist}/{title}");
        assert!(plan.moves.is_empty() && plan.skipped.is_empty());
        assert!(library.albums_without_covers().unwrap().is_empty());
        assert!(library.unmeasured(10).unwrap().is_empty());
    }

    #[test]
    fn prune_removes_the_oldest_files_first() {
        let dir = tempfile::tempdir().unwrap();
        for (i, name) in ["a.flac", "b.flac", "c.flac"].iter().enumerate() {
            let path = dir.path().join(name);
            std::fs::write(&path, vec![0u8; 100]).unwrap();
            File::options()
                .append(true)
                .open(&path)
                .unwrap()
                .set_modified(SystemTime::UNIX_EPOCH + Duration::from_secs(1000 + i as u64))
                .unwrap();
        }
        std::fs::write(dir.path().join("d.flac.part"), vec![0u8; 100]).unwrap();
        prune(dir.path(), 200);
        assert!(!dir.path().join("a.flac").exists());
        assert!(dir.path().join("b.flac").exists() && dir.path().join("c.flac").exists());
        assert!(
            dir.path().join("d.flac.part").exists(),
            "downloads under way stay"
        );
    }
}
