//! Bring ratings, play counts, listening history, and playlists over from other players.
//!
//! Sources: iTunes / Apple Music / MusicBee library XML, a Spotify data export (folder or
//! ZIP), Last.fm and ListenBrainz histories by user name, and a folder of M3U playlists.
//! Songs are matched to the library by file path, then by artist and title (with length as a
//! tie-breaker). Listens for songs that are not in the library are kept in history too, so
//! statistics stay complete. Imported listens are never scrobbled.
use crate::{
    database::Library,
    integrations::client,
    model::{Listen, Playlist},
};
use anyhow::{Context, Result, bail};
use rusqlite::params;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::HashMap,
    io::Read,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SourceKind {
    Itunes,
    Spotify,
    Lastfm,
    ListenBrainz,
    PlaylistFolder,
}

/// A source found on this computer, ready for one-click import.
#[derive(Clone, Debug, Serialize)]
pub struct Detected {
    pub kind: SourceKind,
    pub path: PathBuf,
    pub label: String,
    pub detail: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct ImportReport {
    pub source: String,
    /// Songs from the source found in the library.
    pub matched: usize,
    /// Songs from the source not found in the library.
    pub unmatched: usize,
    pub ratings: usize,
    pub play_counts: usize,
    pub listens: usize,
    pub playlists: usize,
    pub cancelled: bool,
}

impl ImportReport {
    pub fn summary(&self) -> String {
        let mut parts = vec![];
        if self.listens > 0 {
            parts.push(format!("{} listens", self.listens));
        }
        if self.ratings > 0 {
            parts.push(format!("{} ratings", self.ratings));
        }
        if self.play_counts > 0 {
            parts.push(format!("{} play counts", self.play_counts));
        }
        if self.playlists > 0 {
            parts.push(format!("{} playlists", self.playlists));
        }
        let what = if parts.is_empty() {
            "nothing new".to_string()
        } else {
            parts.join(", ")
        };
        format!(
            "{}: imported {what}. {} songs matched your library{}.{}",
            self.source,
            self.matched,
            if self.unmatched > 0 {
                format!(", {} are not in it", self.unmatched)
            } else {
                String::new()
            },
            if self.cancelled {
                " Stopped early."
            } else {
                ""
            }
        )
    }
}

// ---------- Matching ----------

/// Lowercase letters and digits only, without bracketed notes such as "(Remastered 2011)" or
/// a trailing "feat. …".
pub fn normalize(text: &str) -> String {
    let lower = text.to_lowercase();
    let mut out = String::new();
    let mut depth = 0;
    for c in lower.chars() {
        match c {
            '(' | '[' => depth += 1,
            ')' | ']' => depth = (depth - 1).max(0),
            _ if depth == 0 && c.is_alphanumeric() => out.push(c),
            _ => {}
        }
    }
    for marker in ["feat", "ft"] {
        if let Some(i) = lower
            .find(&format!(" {marker}. "))
            .or_else(|| lower.find(&format!(" {marker} ")))
        {
            return normalize(&text[..i.min(text.len())]);
        }
    }
    out
}

fn path_key(path: &str) -> String {
    path.trim_start_matches("\\\\?\\")
        .replace('/', "\\")
        .to_lowercase()
}

/// Every track's path and names, indexed for quick lookups.
pub struct Matcher {
    by_path: HashMap<String, String>,
    by_name: HashMap<String, Vec<(String, f64)>>,
    durations: HashMap<String, f64>,
}

impl Matcher {
    pub fn new(library: &Library) -> Result<Self> {
        let db = library.connection()?;
        let mut statement =
            db.prepare("SELECT id, path, artist, album_artist, title, duration FROM tracks")?;
        let mut matcher = Self {
            by_path: HashMap::new(),
            by_name: HashMap::new(),
            durations: HashMap::new(),
        };
        let rows = statement.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, f64>(5)?,
            ))
        })?;
        for row in rows {
            let (id, path, artist, album_artist, title, duration) = row?;
            matcher.by_path.insert(path_key(&path), id.clone());
            matcher.durations.insert(id.clone(), duration);
            let title = normalize(&title);
            for name in [artist, album_artist] {
                for credit in credits(&name) {
                    let key = format!("{credit}\u{1}{title}");
                    let entry = matcher.by_name.entry(key).or_default();
                    if !entry.iter().any(|(i, _)| *i == id) {
                        entry.push((id.clone(), duration));
                    }
                }
            }
        }
        Ok(matcher)
    }
    pub fn path(&self, path: &str) -> Option<String> {
        self.by_path.get(&path_key(path)).cloned()
    }
    /// The best local match for a song, preferring the closest length when several exist.
    pub fn song(&self, artist: &str, title: &str, seconds: Option<f64>) -> Option<String> {
        let title = normalize(title);
        if title.is_empty() {
            return None;
        }
        let mut candidates: Vec<&(String, f64)> = credits(artist)
            .iter()
            .filter_map(|a| self.by_name.get(&format!("{a}\u{1}{title}")))
            .flatten()
            .collect();
        candidates.dedup_by(|a, b| a.0 == b.0);
        match seconds {
            Some(s) if s > 0. => candidates
                .into_iter()
                .min_by(|a, b| (a.1 - s).abs().total_cmp(&(b.1 - s).abs()))
                .map(|c| c.0.clone()),
            _ => candidates.first().map(|c| c.0.clone()),
        }
    }
    pub fn duration(&self, id: &str) -> Option<f64> {
        self.durations.get(id).copied()
    }
}

/// The whole credit (without "feat." guests) and each named artist in it.
fn credits(artist: &str) -> Vec<String> {
    let mut out = vec![normalize(artist)];
    let lower = artist.to_lowercase();
    let mut parts = vec![lower.as_str()];
    for separator in [
        " feat. ",
        " ft. ",
        " featuring ",
        " & ",
        ", ",
        " and ",
        " x ",
        " / ",
        "; ",
    ] {
        parts = parts.into_iter().flat_map(|p| p.split(separator)).collect();
    }
    for part in parts {
        let n = normalize(part);
        if !n.is_empty() && !out.contains(&n) {
            out.push(n);
        }
    }
    out.retain(|n| !n.is_empty());
    out
}

// ---------- Writing ----------

struct ImportedListen {
    started_at: i64,
    artist: String,
    title: String,
    album: String,
    seconds: f64,
    /// Whether the source says this counts as a play; `None` applies Needle's rule.
    qualified: Option<bool>,
}

/// Store listens, skipping ones already imported. Returns how many were new.
fn store_listens(
    library: &Library,
    matcher: &Matcher,
    source: &str,
    listens: Vec<ImportedListen>,
    report: &mut ImportReport,
) -> Result<usize> {
    let mut db = library.connection()?;
    let tx = db.transaction()?;
    let mut inserted = 0;
    for l in listens {
        let track_id = matcher.song(&l.artist, &l.title, None);
        match &track_id {
            Some(_) => report.matched += 1,
            None => report.unmatched += 1,
        }
        let duration = track_id
            .as_deref()
            .and_then(|id| matcher.duration(id))
            .unwrap_or(0.);
        let qualified = l.qualified.unwrap_or_else(|| {
            l.seconds >= 30. && (duration <= 0. || l.seconds >= (duration / 2.).min(240.))
        });
        let id = format!(
            "{source}-{}",
            &blake3::hash(
                format!(
                    "{}\u{1}{}\u{1}{}",
                    l.started_at,
                    normalize(&l.artist),
                    normalize(&l.title)
                )
                .as_bytes()
            )
            .to_hex()[..24]
        );
        let listen = Listen {
            id: id.clone(),
            track_id: track_id.clone().unwrap_or_default(),
            title: l.title,
            artist: l.artist,
            album: l.album,
            started_at: l.started_at,
            listened_seconds: l.seconds,
            duration,
            qualified,
        };
        let changed = tx.execute(
            "INSERT OR IGNORE INTO listens VALUES (?,?,?,?,?)",
            params![
                listen.id,
                listen.track_id,
                listen.started_at,
                listen.qualified,
                serde_json::to_string(&listen)?
            ],
        )?;
        if changed > 0 {
            inserted += 1;
            if qualified && let Some(track) = &track_id {
                tx.execute(
                    "UPDATE tracks SET play_count=play_count+1,last_played=max(coalesce(last_played,0),?) WHERE id=?",
                    params![listen.started_at, track],
                )?;
            }
        }
    }
    tx.commit()?;
    report.listens += inserted;
    Ok(inserted)
}

fn save_playlist(
    library: &Library,
    id: String,
    name: &str,
    track_ids: Vec<String>,
) -> Result<bool> {
    if track_ids.is_empty() || name.trim().is_empty() {
        return Ok(false);
    }
    library.save_playlist(&Playlist {
        id,
        name: name.trim().to_string(),
        query: None,
        track_ids,
        updated_at: chrono::Utc::now().timestamp(),
    })?;
    Ok(true)
}

// ---------- iTunes, Apple Music, MusicBee ----------

fn file_url_path(url: &str) -> String {
    let rest = url
        .strip_prefix("file://localhost/")
        .or_else(|| url.strip_prefix("file:///"))
        .or_else(|| url.strip_prefix("file://"))
        .unwrap_or(url);
    let bytes = rest.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let Ok(v) = u8::from_str_radix(&rest[i + 1..i + 3], 16)
        {
            out.push(v);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).replace('/', "\\")
}

/// Ratings (unless you already rated the song in Needle), play counts, last-played and
/// date-added times, and ordinary playlists from a library XML exported by iTunes, Apple
/// Music, or MusicBee.
pub fn import_itunes(
    library: &Library,
    path: &Path,
    progress: &mut dyn FnMut(&str),
) -> Result<ImportReport> {
    progress("Reading the library file…");
    let root: plist::Value = plist::Value::from_file(path)
        .with_context(|| format!("{} is not a library XML file", path.display()))?;
    let root = root
        .as_dictionary()
        .context("The library file has no content")?;
    let tracks = root
        .get("Tracks")
        .and_then(|t| t.as_dictionary())
        .context("The library file lists no tracks")?;
    let matcher = Matcher::new(library)?;
    let mut report = ImportReport {
        source: "iTunes library".into(),
        ..Default::default()
    };
    let mut ids: HashMap<String, String> = HashMap::new();
    let db = library.connection()?;
    progress(&format!("Matching {} songs…", tracks.len()));
    let unix = |d: plist::Date| {
        std::time::SystemTime::from(d)
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .ok()
    };
    for (key, value) in tracks {
        let Some(track) = value.as_dictionary() else {
            continue;
        };
        let text = |k: &str| {
            track
                .get(k)
                .and_then(|v| v.as_string())
                .unwrap_or_default()
                .to_string()
        };
        let int = |k: &str| {
            track
                .get(k)
                .and_then(|v| v.as_signed_integer())
                .unwrap_or(0)
        };
        let location = text("Location");
        let found = if location.is_empty() {
            None
        } else {
            matcher.path(&file_url_path(&location))
        }
        .or_else(|| {
            matcher.song(
                &text("Artist"),
                &text("Name"),
                Some(int("Total Time") as f64 / 1000.),
            )
        });
        let Some(id) = found else {
            report.unmatched += 1;
            continue;
        };
        report.matched += 1;
        ids.insert(key.clone(), id.clone());
        let computed = track
            .get("Rating Computed")
            .and_then(|v| v.as_boolean())
            .unwrap_or(false);
        let rating = int("Rating");
        if rating > 0 && !computed {
            let stars = ((rating as f64) / 20.).round().clamp(1., 5.) as i64;
            report.ratings += db.execute(
                "UPDATE tracks SET rating=? WHERE id=? AND rating=0",
                params![stars, id],
            )?;
        }
        let plays = int("Play Count");
        let last = track
            .get("Play Date UTC")
            .and_then(|v| v.as_date())
            .and_then(unix);
        if plays > 0 {
            report.play_counts += db.execute(
                "UPDATE tracks SET play_count=max(play_count,?), last_played=max(coalesce(last_played,0),coalesce(?,0)) WHERE id=? AND play_count<?",
                params![plays, last, id, plays],
            )?;
        }
        if let Some(added) = track
            .get("Date Added")
            .and_then(|v| v.as_date())
            .and_then(unix)
        {
            // The column is used by rules; the JSON copy is what the app reads back.
            db.execute(
                "UPDATE tracks SET added_at=min(added_at,?1), data=json_set(data,'$.added_at',min(added_at,?1)) WHERE id=?2",
                params![added, id],
            )?;
        }
    }
    if let Some(playlists) = root.get("Playlists").and_then(|p| p.as_array()) {
        progress("Importing playlists…");
        for playlist in playlists.iter().filter_map(|p| p.as_dictionary()) {
            let flag = |k: &str| {
                playlist
                    .get(k)
                    .and_then(|v| v.as_boolean())
                    .unwrap_or(false)
            };
            if flag("Master")
                || flag("Folder")
                || playlist.get("Distinguished Kind").is_some()
                || playlist.get("Visible").and_then(|v| v.as_boolean()) == Some(false)
            {
                continue;
            }
            let name = playlist
                .get("Name")
                .and_then(|v| v.as_string())
                .unwrap_or_default();
            let persistent = playlist
                .get("Playlist Persistent ID")
                .and_then(|v| v.as_string())
                .unwrap_or(name);
            let members: Vec<String> = playlist
                .get("Playlist Items")
                .and_then(|v| v.as_array())
                .into_iter()
                .flatten()
                .filter_map(|item| item.as_dictionary()?.get("Track ID")?.as_signed_integer())
                .filter_map(|id| ids.get(&id.to_string()).cloned())
                .collect();
            if save_playlist(library, format!("itunes-{persistent}"), name, members)? {
                report.playlists += 1;
            }
        }
    }
    Ok(report)
}

// ---------- Spotify ----------

/// Reads the JSON files of a Spotify data export: a folder, a ZIP, or one JSON file.
fn spotify_files(path: &Path) -> Result<Vec<(String, String)>> {
    let wanted = |name: &str| {
        let file = name
            .rsplit(['/', '\\'])
            .next()
            .unwrap_or(name)
            .to_lowercase();
        file.ends_with(".json")
            && (file.starts_with("streaming_history")
                || file.starts_with("streaminghistory")
                || file.starts_with("playlist"))
    };
    let mut files = vec![];
    if path.is_dir() {
        for entry in walkdir::WalkDir::new(path)
            .max_depth(4)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let name = entry.path().to_string_lossy().to_string();
            if entry.file_type().is_file() && wanted(&name) {
                files.push((name, std::fs::read_to_string(entry.path())?));
            }
        }
    } else if path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("zip"))
    {
        let mut archive = zip::ZipArchive::new(std::fs::File::open(path)?)?;
        for i in 0..archive.len() {
            let mut file = archive.by_index(i)?;
            let name = file.name().to_string();
            if wanted(&name) && file.size() < 512 * 1024 * 1024 {
                let mut text = String::new();
                file.read_to_string(&mut text)?;
                files.push((name, text));
            }
        }
    } else {
        files.push((
            path.to_string_lossy().into(),
            std::fs::read_to_string(path)?,
        ));
    }
    if files.is_empty() {
        bail!(
            "No Spotify listening history or playlists were found there. Choose the folder or ZIP from your Spotify data download."
        );
    }
    Ok(files)
}

fn spotify_listen(entry: &Value) -> Option<ImportedListen> {
    // Extended history: "ts" is when playback ended, in UTC.
    if let Some(title) = entry["master_metadata_track_name"].as_str() {
        let ms = entry["ms_played"].as_f64().unwrap_or(0.);
        let ended = chrono::DateTime::parse_from_rfc3339(entry["ts"].as_str()?)
            .ok()?
            .timestamp();
        return Some(ImportedListen {
            started_at: ended - (ms / 1000.) as i64,
            artist: entry["master_metadata_album_artist_name"]
                .as_str()
                .unwrap_or_default()
                .into(),
            title: title.into(),
            album: entry["master_metadata_album_album_name"]
                .as_str()
                .unwrap_or_default()
                .into(),
            seconds: ms / 1000.,
            qualified: None,
        });
    }
    // Account-data history: "endTime" as "YYYY-MM-DD HH:MM" in UTC.
    let title = entry["trackName"].as_str()?;
    let ms = entry["msPlayed"].as_f64().unwrap_or(0.);
    let ended = chrono::NaiveDateTime::parse_from_str(entry["endTime"].as_str()?, "%Y-%m-%d %H:%M")
        .ok()?
        .and_utc()
        .timestamp();
    Some(ImportedListen {
        started_at: ended - (ms / 1000.) as i64,
        artist: entry["artistName"].as_str().unwrap_or_default().into(),
        title: title.into(),
        album: String::new(),
        seconds: ms / 1000.,
        qualified: None,
    })
}

/// Listening history and playlists from a Spotify data download.
pub fn import_spotify(
    library: &Library,
    path: &Path,
    progress: &mut dyn FnMut(&str),
) -> Result<ImportReport> {
    progress("Reading the Spotify export…");
    let files = spotify_files(path)?;
    let matcher = Matcher::new(library)?;
    let mut report = ImportReport {
        source: "Spotify".into(),
        ..Default::default()
    };
    let mut listens = vec![];
    for (name, text) in &files {
        let value: Value =
            serde_json::from_str(text).with_context(|| format!("{name} is not valid JSON"))?;
        if let Some(entries) = value.as_array() {
            listens.extend(entries.iter().filter_map(spotify_listen));
        } else if let Some(playlists) = value["playlists"].as_array() {
            for playlist in playlists {
                let name = playlist["name"].as_str().unwrap_or_default();
                let ids: Vec<String> = playlist["items"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|item| {
                        let track = &item["track"];
                        matcher.song(
                            track["artistName"].as_str()?,
                            track["trackName"].as_str()?,
                            None,
                        )
                    })
                    .collect();
                let id = format!("spotify-{}", &blake3::hash(name.as_bytes()).to_hex()[..16]);
                if save_playlist(library, id, name, ids)? {
                    report.playlists += 1;
                }
            }
        }
    }
    progress(&format!("Saving {} listens…", listens.len()));
    store_listens(library, &matcher, "spotify", listens, &mut report)?;
    Ok(report)
}

// ---------- Last.fm and ListenBrainz ----------

fn resume_key(service: &str, user: &str) -> String {
    format!("import:{service}:{}", user.to_lowercase())
}

/// Every scrobble for a Last.fm user, newest first, continuing from the last import.
pub fn import_lastfm(
    library: &Library,
    user: &str,
    api_key: &str,
    cancel: Arc<AtomicBool>,
    progress: &mut dyn FnMut(&str),
) -> Result<ImportReport> {
    if user.trim().is_empty() || api_key.trim().is_empty() {
        bail!(
            "Enter a Last.fm user name. Importing also needs a Last.fm API key (Settings › Listening services)."
        );
    }
    let since: i64 = library.get_json(&resume_key("lastfm", user))?.unwrap_or(0);
    let matcher = Matcher::new(library)?;
    let mut report = ImportReport {
        source: "Last.fm".into(),
        ..Default::default()
    };
    let mut newest = since;
    let mut page = 1;
    loop {
        if cancel.load(Ordering::Relaxed) {
            report.cancelled = true;
            break;
        }
        let from = (since + 1).to_string();
        let page_text = page.to_string();
        let response: Value = client()?
            .get("https://ws.audioscrobbler.com/2.0/")
            .query(&[
                ("method", "user.getrecenttracks"),
                ("user", user.trim()),
                ("api_key", api_key.trim()),
                ("format", "json"),
                ("limit", "200"),
                ("page", &page_text),
                ("from", &from),
            ])
            .send()?
            .json()?;
        if let Some(message) = response["message"].as_str() {
            bail!("Last.fm: {message}");
        }
        let total_pages = response["recenttracks"]["@attr"]["totalPages"]
            .as_str()
            .and_then(|p| p.parse::<u64>().ok())
            .unwrap_or(0);
        let tracks = response["recenttracks"]["track"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        let batch: Vec<ImportedListen> = tracks
            .iter()
            .filter(|t| t["@attr"]["nowplaying"].as_str() != Some("true"))
            .filter_map(|t| {
                Some(ImportedListen {
                    started_at: t["date"]["uts"].as_str()?.parse().ok()?,
                    artist: t["artist"]["#text"].as_str()?.into(),
                    title: t["name"].as_str()?.into(),
                    album: t["album"]["#text"].as_str().unwrap_or_default().into(),
                    seconds: 0.,
                    qualified: Some(true),
                })
            })
            .collect();
        newest = batch.iter().map(|l| l.started_at).fold(newest, i64::max);
        progress(&format!("Last.fm: page {page} of {}", total_pages.max(1)));
        store_listens(library, &matcher, "lastfm", batch, &mut report)?;
        if page as u64 >= total_pages || tracks.is_empty() {
            break;
        }
        page += 1;
        std::thread::sleep(std::time::Duration::from_millis(250));
    }
    if !report.cancelled {
        library.set_json(&resume_key("lastfm", user), &newest)?;
    }
    Ok(report)
}

/// Every public listen of a ListenBrainz user, continuing from the last import. No token needed.
pub fn import_listenbrainz(
    library: &Library,
    user: &str,
    cancel: Arc<AtomicBool>,
    progress: &mut dyn FnMut(&str),
) -> Result<ImportReport> {
    if user.trim().is_empty() {
        bail!("Enter a ListenBrainz user name.");
    }
    let since: i64 = library
        .get_json(&resume_key("listenbrainz", user))?
        .unwrap_or(0);
    let matcher = Matcher::new(library)?;
    let mut report = ImportReport {
        source: "ListenBrainz".into(),
        ..Default::default()
    };
    let mut newest = since;
    let mut before: Option<i64> = None;
    loop {
        if cancel.load(Ordering::Relaxed) {
            report.cancelled = true;
            break;
        }
        let mut query = vec![("count", "1000".to_string())];
        match before {
            Some(b) => query.push(("max_ts", b.to_string())),
            None if since > 0 => query.push(("min_ts", since.to_string())),
            None => {}
        }
        let response: Value = client()?
            .get(format!(
                "https://api.listenbrainz.org/1/user/{}/listens",
                user.trim()
            ))
            .query(&query)
            .send()?
            .error_for_status()
            .context("ListenBrainz did not return listens for that user name")?
            .json()?;
        let listens = response["payload"]["listens"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        let batch: Vec<ImportedListen> = listens
            .iter()
            .filter_map(|l| {
                let meta = &l["track_metadata"];
                Some(ImportedListen {
                    started_at: l["listened_at"].as_i64()?,
                    artist: meta["artist_name"].as_str()?.into(),
                    title: meta["track_name"].as_str()?.into(),
                    album: meta["release_name"].as_str().unwrap_or_default().into(),
                    seconds: meta["additional_info"]["duration_ms"]
                        .as_f64()
                        .map(|ms| ms / 1000.)
                        .unwrap_or(0.),
                    qualified: Some(true),
                })
            })
            .filter(|l| l.started_at > since)
            .collect();
        let oldest = batch.iter().map(|l| l.started_at).min();
        newest = batch.iter().map(|l| l.started_at).fold(newest, i64::max);
        progress(&format!(
            "ListenBrainz: {} listens so far",
            report.listens + batch.len()
        ));
        store_listens(library, &matcher, "listenbrainz", batch, &mut report)?;
        match oldest {
            Some(oldest) if listens.len() >= 1000 && oldest > since => before = Some(oldest),
            _ => break,
        }
    }
    if !report.cancelled {
        library.set_json(&resume_key("listenbrainz", user), &newest)?;
    }
    Ok(report)
}

// ---------- Playlist folders ----------

/// Every M3U/M3U8 playlist in a folder (for example, exported from foobar2000 or MusicBee).
pub fn import_playlist_folder(
    library: &Library,
    folder: &Path,
    progress: &mut dyn FnMut(&str),
) -> Result<ImportReport> {
    let mut report = ImportReport {
        source: "Playlist folder".into(),
        ..Default::default()
    };
    let files: Vec<PathBuf> = std::fs::read_dir(folder)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e.eq_ignore_ascii_case("m3u") || e.eq_ignore_ascii_case("m3u8"))
        })
        .collect();
    if files.is_empty() {
        bail!("That folder has no .m3u or .m3u8 playlists.");
    }
    for file in files {
        progress(&format!(
            "Reading {}",
            file.file_name().unwrap_or_default().to_string_lossy()
        ));
        match library.import_playlist(&file) {
            Ok(playlist) => {
                report.playlists += 1;
                report.matched += playlist.track_ids.len();
            }
            Err(_) => report.unmatched += 1,
        }
    }
    Ok(report)
}

// ---------- Detection ----------

/// Import sources already on this computer.
pub fn detect() -> Vec<Detected> {
    let mut found = vec![];
    let Some(home) = std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
    else {
        return found;
    };
    for (relative, label) in [
        ("Music/iTunes/iTunes Music Library.xml", "iTunes library"),
        ("Music/iTunes/iTunes Library.xml", "iTunes library"),
        (
            "Music/MusicBee/iTunes Music Library.xml",
            "MusicBee library",
        ),
        ("Music/MusicBee/MusicBee Library.xml", "MusicBee library"),
        ("Music/Apple Music/Library.xml", "Apple Music library"),
        ("Downloads/Library.xml", "Exported music library"),
    ] {
        let path = home.join(relative);
        if path.is_file() {
            let modified = std::fs::metadata(&path)
                .and_then(|m| m.modified())
                .ok()
                .map(|t| {
                    chrono::DateTime::<chrono::Local>::from(t)
                        .format("%-d %b %Y")
                        .to_string()
                })
                .unwrap_or_default();
            found.push(Detected {
                kind: SourceKind::Itunes,
                path,
                label: label.into(),
                detail: format!("Ratings, play counts, and playlists · saved {modified}"),
            });
        }
    }
    if let Ok(entries) = std::fs::read_dir(home.join("Downloads")) {
        for entry in entries.filter_map(|e| e.ok()) {
            let name = entry.file_name().to_string_lossy().to_lowercase();
            if name.contains("spotify") || name.starts_with("my_spotify_data") {
                found.push(Detected {
                    kind: SourceKind::Spotify,
                    path: entry.path(),
                    label: "Spotify data download".into(),
                    detail: format!(
                        "Listening history and playlists · {}",
                        entry.file_name().to_string_lossy()
                    ),
                });
            }
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Track;

    fn library() -> (tempfile::TempDir, Library) {
        let dir = tempfile::tempdir().unwrap();
        let library = Library::open(dir.path()).unwrap();
        for (id, path, artist, title, duration) in [
            (
                "a",
                "C:\\Music\\Björk\\Post\\01 Army of Me.flac",
                "Björk",
                "Army of Me",
                234.,
            ),
            (
                "b",
                "C:\\Music\\Massive Attack\\Teardrop.flac",
                "Massive Attack",
                "Teardrop",
                330.,
            ),
            (
                "c",
                "C:\\Music\\Daft Punk\\Get Lucky.flac",
                "Daft Punk",
                "Get Lucky (feat. Pharrell Williams)",
                369.,
            ),
        ] {
            library
                .upsert(&Track {
                    id: id.into(),
                    path: path.into(),
                    artist: artist.into(),
                    album_artist: artist.into(),
                    title: title.into(),
                    duration,
                    added_at: 1_700_000_000,
                    ..Default::default()
                })
                .unwrap();
        }
        (dir, library)
    }

    #[test]
    fn normalizes_titles_and_credits() {
        assert_eq!(normalize("Get Lucky (feat. Pharrell Williams)"), "getlucky");
        assert_eq!(normalize("Army of Me [Remastered 2011]"), "armyofme");
        assert_eq!(normalize("Hyperballad feat. Someone"), "hyperballad");
        assert_eq!(
            credits("Daft Punk feat. Pharrell Williams"),
            ["daftpunk", "pharrellwilliams"]
        );
        assert_eq!(
            credits("Simon & Garfunkel"),
            ["simongarfunkel", "simon", "garfunkel"]
        );
        assert_eq!(
            file_url_path("file://localhost/C:/Music/Bj%C3%B6rk/Post/01%20Army%20of%20Me.flac"),
            "C:\\Music\\Björk\\Post\\01 Army of Me.flac"
        );
    }

    #[test]
    fn matches_by_path_then_names() {
        let (_dir, library) = library();
        let m = Matcher::new(&library).unwrap();
        assert_eq!(
            m.path("c:/music/björk/post/01 army of me.flac").as_deref(),
            Some("a")
        );
        assert_eq!(
            m.song("Daft Punk feat. Pharrell Williams", "Get Lucky", None)
                .as_deref(),
            Some("c")
        );
        assert_eq!(
            m.song("MASSIVE ATTACK", "teardrop (2006 remaster)", Some(331.))
                .as_deref(),
            Some("b")
        );
        assert_eq!(m.song("Someone Else", "Teardrop", None), None);
    }

    #[test]
    fn itunes_xml_brings_ratings_counts_and_playlists() {
        let (dir, library) = library();
        let xml = dir.path().join("Library.xml");
        std::fs::write(&xml, r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>Tracks</key><dict>
 <key>100</key><dict><key>Track ID</key><integer>100</integer><key>Name</key><string>Army of Me</string><key>Artist</key><string>Björk</string>
  <key>Location</key><string>file://localhost/C:/Music/Bj%C3%B6rk/Post/01%20Army%20of%20Me.flac</string>
  <key>Rating</key><integer>80</integer><key>Play Count</key><integer>12</integer><key>Play Date UTC</key><date>2024-03-01T10:00:00Z</date>
  <key>Date Added</key><date>2010-01-01T00:00:00Z</date></dict>
 <key>101</key><dict><key>Track ID</key><integer>101</integer><key>Name</key><string>Teardrop</string><key>Artist</key><string>Massive Attack</string>
  <key>Rating</key><integer>100</integer><key>Rating Computed</key><true/></dict>
 <key>102</key><dict><key>Track ID</key><integer>102</integer><key>Name</key><string>Unknown Song</string><key>Artist</key><string>Nobody</string></dict>
</dict>
<key>Playlists</key><array>
 <dict><key>Name</key><string>Library</string><key>Master</key><true/><key>Playlist Items</key><array><dict><key>Track ID</key><integer>100</integer></dict></array></dict>
 <dict><key>Name</key><string>Night drive</string><key>Playlist Persistent ID</key><string>ABC</string><key>Playlist Items</key><array>
  <dict><key>Track ID</key><integer>101</integer></dict><dict><key>Track ID</key><integer>100</integer></dict><dict><key>Track ID</key><integer>102</integer></dict></array></dict>
</array></dict></plist>"#).unwrap();
        let report = import_itunes(&library, &xml, &mut |_| {}).unwrap();
        assert_eq!(
            (
                report.matched,
                report.unmatched,
                report.ratings,
                report.play_counts,
                report.playlists
            ),
            (2, 1, 1, 1, 1)
        );
        let army = library.track("a").unwrap().unwrap();
        assert_eq!(
            (army.rating, army.play_count, army.added_at),
            (4, 12, 1262304000)
        );
        assert_eq!(
            library.track("b").unwrap().unwrap().rating,
            0,
            "computed album ratings are ignored"
        );
        let playlists = library.playlists().unwrap();
        assert_eq!(playlists.len(), 1);
        assert_eq!(playlists[0].track_ids, ["b", "a"]);
        // Importing again changes nothing and does not duplicate the playlist.
        let again = import_itunes(&library, &xml, &mut |_| {}).unwrap();
        assert_eq!((again.ratings, again.play_counts), (0, 0));
        assert_eq!(library.playlists().unwrap().len(), 1);
    }

    #[test]
    fn spotify_history_becomes_listens_once() {
        let (dir, library) = library();
        let export = dir.path().join("Spotify Extended Streaming History");
        std::fs::create_dir(&export).unwrap();
        std::fs::write(export.join("Streaming_History_Audio_2024.json"), r#"[
 {"ts":"2024-05-01T12:05:00Z","ms_played":240000,"master_metadata_track_name":"Army of Me","master_metadata_album_artist_name":"Björk","master_metadata_album_album_name":"Post"},
 {"ts":"2024-05-01T12:06:00Z","ms_played":8000,"master_metadata_track_name":"Teardrop","master_metadata_album_artist_name":"Massive Attack","master_metadata_album_album_name":"Mezzanine"},
 {"ts":"2024-05-01T13:00:00Z","ms_played":200000,"master_metadata_track_name":"Not Here","master_metadata_album_artist_name":"Nobody","master_metadata_album_album_name":"X"},
 {"ts":"2024-05-01T14:00:00Z","ms_played":900000,"master_metadata_track_name":null,"episode_name":"A podcast"}
]"#).unwrap();
        std::fs::write(export.join("Playlist1.json"), r#"{"playlists":[{"name":"Faves","items":[{"track":{"trackName":"Teardrop","artistName":"Massive Attack"}},{"track":{"trackName":"Nope","artistName":"Nobody"}}]}]}"#).unwrap();
        let report = import_spotify(&library, &export, &mut |_| {}).unwrap();
        assert_eq!(
            (
                report.listens,
                report.matched,
                report.unmatched,
                report.playlists
            ),
            (3, 2, 1, 1)
        );
        assert_eq!(library.track("a").unwrap().unwrap().play_count, 1);
        assert_eq!(
            library.track("b").unwrap().unwrap().play_count,
            0,
            "an 8-second skip is not a play"
        );
        assert_eq!(library.history_count().unwrap(), 3);
        let again = import_spotify(&library, &export, &mut |_| {}).unwrap();
        assert_eq!(again.listens, 0);
        assert_eq!(library.track("a").unwrap().unwrap().play_count, 1);
    }

    #[test]
    fn imported_listens_are_not_scrobbled() {
        let (dir, library) = library();
        let mut settings = library.settings().unwrap();
        settings.lastfm_enabled = true;
        library.save_settings(&settings).unwrap();
        let file = dir.path().join("StreamingHistory_music_0.json");
        std::fs::write(&file, r#"[{"endTime":"2024-05-01 12:05","artistName":"Björk","trackName":"Army of Me","msPlayed":240000}]"#).unwrap();
        assert_eq!(
            import_spotify(&library, &file, &mut |_| {})
                .unwrap()
                .listens,
            1
        );
        let queued: i64 = library
            .connection()
            .unwrap()
            .query_row("SELECT count(*) FROM scrobbles", [], |r| r.get(0))
            .unwrap();
        assert_eq!(queued, 0);
    }
}
