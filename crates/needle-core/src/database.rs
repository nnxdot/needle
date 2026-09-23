use crate::{
    model::{Listen, Playlist, Settings, Track},
    query,
};
use anyhow::{Context, Result, bail};
use rusqlite::{Connection, OptionalExtension, params};
use std::{
    path::{Path, PathBuf},
    time::Duration,
};

#[derive(Clone, Debug)]
pub struct Library {
    pub directory: PathBuf,
}
pub struct SearchPage {
    pub tracks: Vec<Track>,
    pub total: usize,
}

impl Library {
    pub fn default_directory() -> PathBuf {
        directories::ProjectDirs::from("studio", "nnx", "Needle")
            .map(|d| d.data_local_dir().to_path_buf())
            .unwrap_or_else(|| PathBuf::from(".needle"))
    }
    pub fn open(directory: impl Into<PathBuf>) -> Result<Self> {
        let library = Self {
            directory: directory.into(),
        };
        std::fs::create_dir_all(library.directory.join("artwork"))?;
        std::fs::create_dir_all(library.directory.join("backups"))?;
        let db = library.connection()?;
        db.execute_batch("
            PRAGMA journal_mode=WAL;
            CREATE TABLE IF NOT EXISTS tracks (
                id TEXT PRIMARY KEY, path TEXT NOT NULL UNIQUE, title TEXT NOT NULL,
                artist TEXT NOT NULL, album TEXT NOT NULL, album_artist TEXT NOT NULL,
                genre TEXT NOT NULL, year INTEGER NOT NULL, track_number INTEGER NOT NULL,
                disc INTEGER NOT NULL, duration REAL NOT NULL, sample_rate INTEGER NOT NULL,
                bit_depth INTEGER NOT NULL, format TEXT NOT NULL, bpm REAL,
                rating INTEGER NOT NULL DEFAULT 0, added_at INTEGER NOT NULL,
                last_played INTEGER, play_count INTEGER NOT NULL DEFAULT 0,
                missing INTEGER NOT NULL DEFAULT 0, content_hash TEXT NOT NULL, data TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS tracks_artist ON tracks(artist COLLATE NOCASE);
            CREATE INDEX IF NOT EXISTS tracks_album ON tracks(album_artist COLLATE NOCASE,album COLLATE NOCASE,disc,track_number);
            CREATE INDEX IF NOT EXISTS tracks_rating ON tracks(rating);
            CREATE INDEX IF NOT EXISTS tracks_rating_order ON tracks(rating DESC,id);
            CREATE INDEX IF NOT EXISTS tracks_added ON tracks(added_at);
            CREATE INDEX IF NOT EXISTS tracks_hash ON tracks(content_hash);
            CREATE INDEX IF NOT EXISTS tracks_year ON tracks(year);
            CREATE INDEX IF NOT EXISTS tracks_bpm ON tracks(bpm);
            CREATE INDEX IF NOT EXISTS tracks_search_order ON tracks(artist COLLATE NOCASE,album COLLATE NOCASE,disc,track_number,id);
            CREATE TABLE IF NOT EXISTS roots(path TEXT PRIMARY KEY, last_scan INTEGER NOT NULL);
            CREATE TABLE IF NOT EXISTS playlists(id TEXT PRIMARY KEY, name TEXT NOT NULL, data TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS listens(
                id TEXT PRIMARY KEY, track_id TEXT NOT NULL, started_at INTEGER NOT NULL,
                qualified INTEGER NOT NULL, data TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS listens_track_time ON listens(track_id, started_at);
            CREATE INDEX IF NOT EXISTS listens_time ON listens(started_at);
            CREATE TABLE IF NOT EXISTS scrobbles(
                listen_id TEXT NOT NULL, service TEXT NOT NULL, status TEXT NOT NULL DEFAULT 'pending',
                attempts INTEGER NOT NULL DEFAULT 0, next_attempt INTEGER NOT NULL DEFAULT 0,
                error TEXT NOT NULL DEFAULT '', PRIMARY KEY(listen_id,service)
            );
            CREATE TABLE IF NOT EXISTS settings(key TEXT PRIMARY KEY, value TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS cache(key TEXT PRIMARY KEY, expires INTEGER NOT NULL, data TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS features(track_id TEXT PRIMARY KEY, version INTEGER NOT NULL, data TEXT NOT NULL);
            PRAGMA user_version=1;
        ")?;
        let has_fts: bool = db.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name='tracks_fts')",
            [],
            |r| r.get(0),
        )?;
        if !has_fts {
            db.execute_batch("BEGIN;
            CREATE VIRTUAL TABLE tracks_fts USING fts5(search,tokenize='trigram');
            INSERT INTO tracks_fts(rowid,search) SELECT rowid,title || char(10) || artist || char(10) || album || char(10) || genre FROM tracks;
            CREATE TRIGGER tracks_fts_insert AFTER INSERT ON tracks BEGIN
                INSERT INTO tracks_fts(rowid,search) VALUES(new.rowid,new.title || char(10) || new.artist || char(10) || new.album || char(10) || new.genre); END;
            CREATE TRIGGER tracks_fts_delete AFTER DELETE ON tracks BEGIN DELETE FROM tracks_fts WHERE rowid=old.rowid; END;
            CREATE TRIGGER tracks_fts_update AFTER UPDATE OF title,artist,album,genre ON tracks BEGIN
                DELETE FROM tracks_fts WHERE rowid=old.rowid;
                INSERT INTO tracks_fts(rowid,search) VALUES(new.rowid,new.title || char(10) || new.artist || char(10) || new.album || char(10) || new.genre); END;
            COMMIT;")?;
        }
        crate::history::migrate(&db)?;
        crate::browse::migrate(&db)?;
        Ok(library)
    }
    pub fn connection(&self) -> Result<Connection> {
        let db = Connection::open(self.directory.join("library.db"))?;
        db.busy_timeout(Duration::from_secs(10))?;
        // `field matches "pattern"` in rules: case-insensitive regular expressions, compiled
        // once per statement.
        db.create_scalar_function(
            "needle_regexp",
            2,
            rusqlite::functions::FunctionFlags::SQLITE_UTF8
                | rusqlite::functions::FunctionFlags::SQLITE_DETERMINISTIC,
            |ctx| {
                let pattern: std::sync::Arc<regex::Regex> = ctx.get_or_create_aux(0, |value| {
                    regex::RegexBuilder::new(value.as_str()?)
                        .case_insensitive(true)
                        .size_limit(1 << 20)
                        .build()
                        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
                })?;
                Ok(match ctx.get_raw(1) {
                    rusqlite::types::ValueRef::Text(text) => {
                        pattern.is_match(&String::from_utf8_lossy(text))
                    }
                    rusqlite::types::ValueRef::Null => false,
                    other => pattern.is_match(&format!("{other:?}")),
                })
            },
        )?;
        // `folder = "…"` in rules: the folder a track's file (or CUE sheet) is in.
        db.create_scalar_function(
            "needle_folder",
            1,
            rusqlite::functions::FunctionFlags::SQLITE_UTF8
                | rusqlite::functions::FunctionFlags::SQLITE_DETERMINISTIC,
            |ctx| {
                Ok(crate::browse::folder_of(&ctx.get::<String>(0).unwrap_or_default()).to_string())
            },
        )?;
        db.execute_batch("PRAGMA foreign_keys=ON; PRAGMA synchronous=NORMAL;")?;
        Ok(db)
    }
    pub fn upsert_on(db: &Connection, track: &Track) -> Result<()> {
        db.execute("INSERT INTO tracks(id,path,title,artist,album,album_artist,genre,year,track_number,disc,duration,sample_rate,bit_depth,format,bpm,rating,added_at,last_played,play_count,missing,content_hash,data)
            VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21,?22)
            ON CONFLICT(id) DO UPDATE SET path=excluded.path,title=excluded.title,artist=excluded.artist,
            album=excluded.album,album_artist=excluded.album_artist,genre=excluded.genre,year=excluded.year,
            track_number=excluded.track_number,disc=excluded.disc,duration=excluded.duration,sample_rate=excluded.sample_rate,
            bit_depth=excluded.bit_depth,format=excluded.format,bpm=excluded.bpm,missing=excluded.missing,
            content_hash=excluded.content_hash,data=excluded.data",
            params![track.id, track.path, track.title, track.artist, track.album, track.album_artist, track.genre,
                track.year, track.track_number, track.disc, track.duration, track.sample_rate, track.bit_depth,
                track.format, track.bpm, track.rating, track.added_at, track.last_played, track.play_count,
                track.missing, track.content_hash, serde_json::to_string(track)?])?;
        Ok(())
    }
    pub fn upsert(&self, track: &Track) -> Result<()> {
        Self::upsert_on(&self.connection()?, track)
    }
    pub(crate) fn row_track(row: &rusqlite::Row<'_>) -> rusqlite::Result<Track> {
        let data: String = row.get(0)?;
        let mut track: Track = serde_json::from_str(&data).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })?;
        track.rating = row.get(1)?;
        track.play_count = row.get(2)?;
        track.last_played = row.get(3)?;
        track.missing = row.get(4)?;
        Ok(track)
    }
    pub fn search(&self, expression: &str) -> Result<Vec<Track>> {
        let query = query::compile(expression, chrono::Utc::now().timestamp())?;
        let db = self.connection()?;
        let sql = format!(
            "SELECT t.data,t.rating,t.play_count,t.last_played,t.missing FROM {} WHERE ({}) ORDER BY {} LIMIT {}",
            query.from(),
            query.filter(),
            query.order,
            query.limit
        );
        let mut statement = db.prepare(&sql)?;
        let rows = statement.query_map(
            rusqlite::params_from_iter(query.parameters),
            Self::row_track,
        )?;
        let tracks: Vec<Track> = rows.collect::<std::result::Result<_, _>>()?;
        Ok(match query.spread {
            Some(field) => query::spread(tracks, field),
            None => tracks,
        })
    }
    pub fn search_page(
        &self,
        expression: &str,
        offset: usize,
        page_size: usize,
    ) -> Result<SearchPage> {
        let query = query::compile(expression, chrono::Utc::now().timestamp())?;
        let db = self.connection()?;
        let count: i64 = db.query_row(
            &format!(
                "SELECT count(*) FROM (SELECT 1 FROM {} WHERE ({}) LIMIT {})",
                query.from(),
                query.filter(),
                query.limit
            ),
            rusqlite::params_from_iter(query.parameters.iter()),
            |r| r.get(0),
        )?;
        let total = (count as usize).min(query.limit);
        let size = page_size.min(total.saturating_sub(offset));
        let ordered_index = if query.per.is_none()
            && total == query.limit
            && query.limit <= 1000
            && query.order
                == "t.artist COLLATE NOCASE, t.album COLLATE NOCASE, t.disc, t.track_number, t.id"
        {
            " INDEXED BY tracks_search_order"
        } else {
            ""
        };
        let sql = format!(
            "SELECT t.data,t.rating,t.play_count,t.last_played,t.missing FROM {}{ordered_index} WHERE ({}) ORDER BY {} LIMIT {} OFFSET {}",
            query.from(),
            query.filter(),
            query.order,
            size,
            offset
        );
        let mut stmt = db.prepare(&sql)?;
        let tracks: Vec<Track> = stmt
            .query_map(
                rusqlite::params_from_iter(query.parameters.iter()),
                Self::row_track,
            )?
            .collect::<std::result::Result<_, _>>()?;
        let tracks = match query.spread {
            Some(field) => query::spread(tracks, field),
            None => tracks,
        };
        Ok(SearchPage { tracks, total })
    }
    pub fn track(&self, id: &str) -> Result<Option<Track>> {
        Ok(self
            .connection()?
            .query_row(
                "SELECT data,rating,play_count,last_played,missing FROM tracks WHERE id=?",
                [id],
                Self::row_track,
            )
            .optional()?)
    }
    pub fn track_by_path(&self, path: &str) -> Result<Option<Track>> {
        Self::track_by_path_on(&self.connection()?, path)
    }
    pub fn track_by_path_on(db: &Connection, path: &str) -> Result<Option<Track>> {
        Ok(db
            .query_row(
                "SELECT data,rating,play_count,last_played,missing FROM tracks WHERE path=?",
                [path],
                Self::row_track,
            )
            .optional()?)
    }
    pub fn moved_track(&self, hash: &str) -> Result<Option<Track>> {
        Self::moved_track_on(&self.connection()?, hash)
    }
    pub fn moved_track_on(db: &Connection, hash: &str) -> Result<Option<Track>> {
        let mut stmt = db.prepare(
            "SELECT data,rating,play_count,last_played,missing FROM tracks WHERE content_hash=?",
        )?;
        for track in stmt.query_map([hash], Self::row_track)? {
            let track = track?;
            if std::fs::metadata(track.file_path())
                .is_err_and(|e| e.kind() == std::io::ErrorKind::NotFound)
            {
                return Ok(Some(track));
            }
        }
        Ok(None)
    }
    pub fn count(&self) -> Result<usize> {
        Ok(self
            .connection()?
            .query_row("SELECT count(*) FROM tracks", [], |r| r.get::<_, i64>(0))?
            as usize)
    }
    pub fn rate(&self, id: &str, rating: i64) -> Result<()> {
        if !(0..=5).contains(&rating) {
            bail!("Rating must be between 0 and 5")
        }
        self.connection()?
            .execute("UPDATE tracks SET rating=? WHERE id=?", params![rating, id])?;
        Ok(())
    }
    pub fn roots(&self) -> Result<Vec<String>> {
        let db = self.connection()?;
        let mut statement = db.prepare("SELECT path FROM roots ORDER BY path")?;
        Ok(statement
            .query_map([], |r| r.get(0))?
            .collect::<std::result::Result<_, _>>()?)
    }
    pub fn add_root(&self, path: &str) -> Result<()> {
        self.connection()?.execute("INSERT INTO roots VALUES (?,?) ON CONFLICT(path) DO UPDATE SET last_scan=excluded.last_scan", params![path,chrono::Utc::now().timestamp()])?;
        Ok(())
    }
    pub fn settings(&self) -> Result<Settings> {
        Ok(self.get_json("settings")?.unwrap_or_default())
    }
    pub fn save_settings(&self, settings: &Settings) -> Result<()> {
        self.set_json("settings", settings)
    }
    pub fn get_json<T: serde::de::DeserializeOwned>(&self, key: &str) -> Result<Option<T>> {
        let value: Option<String> = self
            .connection()?
            .query_row("SELECT value FROM settings WHERE key=?", [key], |r| {
                r.get(0)
            })
            .optional()?;
        value
            .map(|s| serde_json::from_str(&s).context("Invalid saved setting"))
            .transpose()
    }
    pub fn set_json<T: serde::Serialize>(&self, key: &str, value: &T) -> Result<()> {
        self.connection()?.execute(
            "INSERT INTO settings VALUES (?,?) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            params![key, serde_json::to_string(value)?],
        )?;
        Ok(())
    }
    pub fn playlists(&self) -> Result<Vec<Playlist>> {
        let db = self.connection()?;
        let mut stmt = db.prepare("SELECT data FROM playlists ORDER BY name COLLATE NOCASE")?;
        let json: Vec<String> = stmt
            .query_map([], |r| r.get(0))?
            .collect::<std::result::Result<_, _>>()?;
        json.iter().map(|s| Ok(serde_json::from_str(s)?)).collect()
    }
    pub fn save_playlist(&self, playlist: &Playlist) -> Result<()> {
        if playlist.name.trim().is_empty() {
            bail!("Give the playlist a name")
        }
        if let Some(q) = &playlist.query {
            query::compile(q, chrono::Utc::now().timestamp())?;
        }
        self.connection()?.execute("INSERT INTO playlists VALUES (?,?,?) ON CONFLICT(id) DO UPDATE SET name=excluded.name,data=excluded.data", params![playlist.id,playlist.name,serde_json::to_string(playlist)?])?;
        Ok(())
    }
    pub fn delete_playlist(&self, id: &str) -> Result<()> {
        self.connection()?
            .execute("DELETE FROM playlists WHERE id=?", [id])?;
        Ok(())
    }
    pub fn playlist_tracks(&self, playlist: &Playlist) -> Result<Vec<Track>> {
        if let Some(q) = &playlist.query {
            self.search(q)
        } else {
            self.tracks_by_ids(&playlist.track_ids)
        }
    }
    pub fn tracks_by_ids(&self, ids: &[String]) -> Result<Vec<Track>> {
        let db = self.connection()?;
        let mut statement = db.prepare_cached(
            "SELECT data,rating,play_count,last_played,missing FROM tracks WHERE id=?",
        )?;
        ids.iter()
            .filter_map(|id| {
                statement
                    .query_row([id], Self::row_track)
                    .optional()
                    .transpose()
            })
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }
    pub fn record_listen(&self, listen: &Listen) -> Result<()> {
        let mut db = self.connection()?;
        let tx = db.transaction()?;
        let changed = tx.execute(
            "INSERT OR IGNORE INTO listens VALUES (?,?,?,?,?)",
            params![
                listen.id,
                listen.track_id,
                listen.started_at,
                listen.qualified,
                serde_json::to_string(listen)?
            ],
        )?;
        if changed > 0 && listen.qualified {
            tx.execute("UPDATE tracks SET play_count=play_count+1,last_played=max(coalesce(last_played,0),?) WHERE id=?", params![listen.started_at,listen.track_id])?;
            let settings = self.settings()?;
            for (service, enabled) in [
                ("lastfm", settings.lastfm_enabled),
                ("listenbrainz", settings.listenbrainz_enabled),
            ] {
                if enabled && (service != "lastfm" || listen.duration > 30.0) {
                    tx.execute(
                        "INSERT OR IGNORE INTO scrobbles(listen_id,service) VALUES (?,?)",
                        params![listen.id, service],
                    )?;
                }
            }
        }
        tx.commit()?;
        Ok(())
    }
    pub fn history(&self, limit: usize) -> Result<Vec<Listen>> {
        let db = self.connection()?;
        let mut stmt = db.prepare("SELECT data FROM listens ORDER BY started_at DESC LIMIT ?")?;
        let json: Vec<String> = stmt
            .query_map([limit as i64], |r| r.get(0))?
            .collect::<std::result::Result<_, _>>()?;
        json.iter().map(|s| Ok(serde_json::from_str(s)?)).collect()
    }
    pub fn export_playlist(&self, playlist: &Playlist, destination: &Path) -> Result<()> {
        let mut text = String::from("#EXTM3U\n");
        for track in self.playlist_tracks(playlist)? {
            text.push_str(&format!(
                "#EXTINF:{},{} - {}\n{}\n",
                track.duration as i64,
                track.artist.replace(['\r', '\n'], " "),
                track.title.replace(['\r', '\n'], " "),
                track.path.replace(['\r', '\n'], "")
            ));
        }
        std::fs::write(destination, text)?;
        Ok(())
    }
    pub fn import_playlist(&self, source: &Path) -> Result<Playlist> {
        let text = std::fs::read_to_string(source)?;
        let mut ids = vec![];
        for line in text
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
        {
            let path = source.parent().unwrap_or(Path::new(".")).join(line);
            if let Ok(path) = path.canonicalize()
                && let Some(track) = self.track_by_path(&path.to_string_lossy())?
            {
                ids.push(track.id);
            }
        }
        if ids.is_empty() {
            bail!("No playlist entries match your imported library")
        }
        let playlist = Playlist {
            id: uuid::Uuid::new_v4().to_string(),
            name: source
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .into(),
            query: None,
            track_ids: ids,
            updated_at: chrono::Utc::now().timestamp(),
        };
        self.save_playlist(&playlist)?;
        Ok(playlist)
    }
    pub fn backup(&self, destination: &Path) -> Result<()> {
        self.connection()?.backup("main", destination, None)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn persistence_query_and_history_are_consistent() {
        let dir = tempfile::tempdir().unwrap();
        let db = Library::open(dir.path()).unwrap();
        let track = Track {
            id: "one".into(),
            path: "/one.flac".into(),
            title: "First".into(),
            artist: "Alice".into(),
            bpm: Some(125.0),
            duration: 100.0,
            ..Default::default()
        };
        db.upsert(&track).unwrap();
        db.rate("one", 5).unwrap();
        db.upsert(&track).unwrap();
        assert_eq!(db.search("rating = 5 and bpm > 120").unwrap().len(), 1);
        assert_eq!(db.search("Alice").unwrap().len(), 1);
        assert!(db.search("title = 'x\" OR 1=1 --'").unwrap().is_empty());
        let listen = Listen {
            id: "event".into(),
            track_id: "one".into(),
            title: "First".into(),
            artist: "Alice".into(),
            album: String::new(),
            started_at: chrono::Utc::now().timestamp(),
            listened_seconds: 70.0,
            duration: 100.0,
            qualified: true,
        };
        db.record_listen(&listen).unwrap();
        db.record_listen(&listen).unwrap();
        assert_eq!(db.track("one").unwrap().unwrap().play_count, 1);
        assert_eq!(db.search("played(7d)").unwrap().len(), 1);
        assert!(db.search("not played(7d)").unwrap().is_empty());
        drop(db);
        assert_eq!(Library::open(dir.path()).unwrap().count().unwrap(), 1);
    }
    fn wav(path: &Path, frames: usize) {
        let mut writer = hound::WavWriter::create(
            path,
            hound::WavSpec {
                channels: 1,
                sample_rate: 8000,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            },
        )
        .unwrap();
        for i in 0..frames {
            writer.write_sample((i % 100) as i16).unwrap();
        }
        writer.finalize().unwrap();
    }
    fn playlist(id: &str, name: &str, query: Option<&str>, ids: &[&str]) -> Playlist {
        Playlist {
            id: id.into(),
            name: name.into(),
            query: query.map(String::from),
            track_ids: ids.iter().map(|s| s.to_string()).collect(),
            updated_at: 1,
        }
    }

    #[test]
    fn playlists_save_rename_delete_and_resolve_tracks() {
        let dir = tempfile::tempdir().unwrap();
        let db = Library::open(dir.path()).unwrap();
        for (id, rating) in [("a", 5), ("b", 2), ("c", 4)] {
            db.upsert(&Track {
                id: id.into(),
                path: format!("/{id}.flac"),
                title: id.into(),
                ..Default::default()
            })
            .unwrap();
            db.rate(id, rating).unwrap();
        }
        assert!(db.save_playlist(&playlist("x", "  ", None, &[])).is_err());
        assert!(
            db.save_playlist(&playlist("x", "Broken", Some("rating >"), &[]))
                .is_err()
        );
        assert!(db.playlists().unwrap().is_empty());
        db.save_playlist(&playlist("s", "zeta", None, &["c", "gone", "a"]))
            .unwrap();
        db.save_playlist(&playlist(
            "q",
            "Alpha",
            Some("rating >= 4 order by rating desc"),
            &[],
        ))
        .unwrap();
        let names: Vec<_> = db
            .playlists()
            .unwrap()
            .into_iter()
            .map(|p| p.name)
            .collect();
        assert_eq!(names, ["Alpha", "zeta"]);
        let ids = |p: &Playlist| {
            db.playlist_tracks(p)
                .unwrap()
                .into_iter()
                .map(|t| t.id)
                .collect::<Vec<_>>()
        };
        let lists = db.playlists().unwrap();
        assert_eq!(ids(&lists[0]), ["a", "c"]);
        assert_eq!(ids(&lists[1]), ["c", "a"]);
        let mut renamed = lists[1].clone();
        renamed.name = "Beta".into();
        db.save_playlist(&renamed).unwrap();
        let lists = db.playlists().unwrap();
        assert_eq!(lists.len(), 2);
        assert_eq!(lists[1].name, "Beta");
        assert_eq!(lists[1].track_ids, ["c", "gone", "a"]);
        db.delete_playlist("q").unwrap();
        db.delete_playlist("unknown").unwrap();
        assert_eq!(db.playlists().unwrap().len(), 1);
        assert_eq!(db.count().unwrap(), 3);
    }

    #[test]
    fn ratings_are_bounded_and_survive_rescans() {
        let dir = tempfile::tempdir().unwrap();
        let db = Library::open(dir.path()).unwrap();
        let track = Track {
            id: "t".into(),
            path: "/t.flac".into(),
            ..Default::default()
        };
        db.upsert(&track).unwrap();
        for rating in [0, 3, 5] {
            db.rate("t", rating).unwrap();
            assert_eq!(db.track("t").unwrap().unwrap().rating, rating);
        }
        assert!(db.rate("t", 6).is_err());
        assert!(db.rate("t", -1).is_err());
        db.upsert(&track).unwrap();
        assert_eq!(db.track("t").unwrap().unwrap().rating, 5);
        assert_eq!(db.search("rating = 5").unwrap().len(), 1);
    }

    #[test]
    fn m3u_export_and_import_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let music = dir.path().join("music");
        std::fs::create_dir_all(music.join("sub")).unwrap();
        let db = Library::open(dir.path().join("db")).unwrap();
        let mut ids = vec![];
        for name in ["one.wav", "sub/two.wav", "three.wav"] {
            let path = music.join(name);
            wav(&path, 800 + ids.len());
            crate::scan::import_one(&db, &path).unwrap();
            let track = db
                .track_by_path(&path.canonicalize().unwrap().to_string_lossy())
                .unwrap()
                .unwrap();
            ids.push(track.id);
        }
        let mut odd = db.track(&ids[1]).unwrap().unwrap();
        odd.title = "Line\nbreak".into();
        db.upsert(&odd).unwrap();
        let order = [ids[2].clone(), ids[0].clone(), ids[1].clone()];
        let list = Playlist {
            id: "p".into(),
            name: "Mix".into(),
            query: None,
            track_ids: order.to_vec(),
            updated_at: 1,
        };
        let exported = music.join("Road trip.m3u8");
        db.export_playlist(&list, &exported).unwrap();
        let text = std::fs::read_to_string(&exported).unwrap();
        assert!(text.starts_with("#EXTM3U\n"));
        assert_eq!(text.lines().count(), 7);
        assert!(text.contains("#EXTINF:0, - Line break\n"));
        let imported = db.import_playlist(&exported).unwrap();
        assert_eq!(imported.name, "Road trip");
        assert_eq!(imported.track_ids, order);
        assert_ne!(imported.id, "p");
        assert_eq!(db.playlists().unwrap().len(), 1);

        let relative = music.join("relative.m3u");
        std::fs::write(
            &relative,
            "#EXTM3U\r\n\r\nsub/two.wav\r\n  one.wav  \r\nnot-there.wav\r\n",
        )
        .unwrap();
        assert_eq!(
            db.import_playlist(&relative).unwrap().track_ids,
            [ids[1].clone(), ids[0].clone()]
        );
        let nothing = music.join("nothing.m3u");
        std::fs::write(&nothing, "#EXTM3U\nmissing.wav\n").unwrap();
        assert!(db.import_playlist(&nothing).is_err());
        assert_eq!(db.playlists().unwrap().len(), 2);
    }

    #[test]
    fn missing_files_are_flagged_and_recovered() {
        let dir = tempfile::tempdir().unwrap();
        let music = dir.path().join("music");
        std::fs::create_dir(&music).unwrap();
        let db = Library::open(dir.path().join("db")).unwrap();
        let (keep, lose) = (music.join("keep.wav"), music.join("lose.wav"));
        wav(&keep, 800);
        wav(&lose, 900);
        let scan = |db: &Library| {
            crate::scan::import(db, &music, std::sync::Arc::new(Default::default()), |_| {})
                .unwrap()
        };
        scan(&db);
        assert!(db.search("missing").unwrap().is_empty());
        let saved = std::fs::read(&lose).unwrap();
        std::fs::remove_file(&lose).unwrap();
        scan(&db);
        let missing = db.search("missing").unwrap();
        assert_eq!(missing.len(), 1);
        assert!(missing[0].missing);
        assert!(missing[0].path.ends_with("lose.wav"));
        assert_eq!(db.count().unwrap(), 2);
        assert_eq!(db.search("not missing and rating = 0").unwrap().len(), 1);
        std::fs::write(&lose, saved).unwrap();
        scan(&db);
        assert!(db.search("missing").unwrap().is_empty());
        assert_eq!(
            db.track(&missing[0].id).unwrap().unwrap().path,
            missing[0].path
        );
    }
}
