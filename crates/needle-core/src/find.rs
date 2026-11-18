//! Search across the whole library at once, for the Search page: the best match, songs,
//! albums, artists, and lines of lyrics. And the lyrics index it reads: the words of every
//! song's own lyrics (the file beside it and its tags), read slowly in the background, plus
//! any lyrics shown since (from LRCLIB or a server). Nothing is fetched to fill it. And what
//! was looked for before: the words typed, and the results opened from Search.
use crate::{
    browse::{AlbumSummary, ArtistSummary},
    database::Library,
    media::Lyrics,
    model::Track,
};
use anyhow::Result;
use rusqlite::{Connection, params};
use std::{
    collections::HashSet,
    path::PathBuf,
    sync::{Mutex, OnceLock},
    time::Duration,
};

/// The Search page's "Try a rule" chips: a name, and the rule it puts in the search field.
pub const RULE_EXAMPLES: [(&str, &str); 5] = [
    ("Favorites", "rating >= 4"),
    ("Played this month", "played(30d)"),
    ("Never played", "play_count = 0"),
    ("Hi-res", "bit_depth >= 24"),
    ("Before 2000", "year < 2000"),
];

/// Raised when what is kept for a song changes, so every song is read again.
const VERSION: i64 = 1;

pub(crate) fn migrate(db: &Connection) -> Result<()> {
    db.execute_batch(
        "CREATE TABLE IF NOT EXISTS lyrics_text(track_id TEXT PRIMARY KEY, hash TEXT NOT NULL, version INTEGER NOT NULL, text TEXT NOT NULL);
         CREATE VIRTUAL TABLE IF NOT EXISTS lyrics_fts USING fts5(text, track_id UNINDEXED, tokenize='trigram');
         CREATE TABLE IF NOT EXISTS recent_searches(kind TEXT NOT NULL, key TEXT NOT NULL, UNIQUE(kind,key));",
    )?;
    Ok(())
}

/// The match that heads the results.
#[derive(Clone, Debug, PartialEq)]
pub enum Best {
    Song(Box<Track>),
    Album(AlbumSummary),
    Artist(ArtistSummary),
}

/// A song whose lyrics hold the words, with the line they are in.
#[derive(Clone, Debug, PartialEq)]
pub struct LyricMatch {
    pub track: Track,
    pub line: String,
}

/// Everything a search finds, a few of each, and how many of each there are in all.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Found {
    pub best: Option<Best>,
    pub songs: Vec<Track>,
    pub albums: Vec<AlbumSummary>,
    pub artists: Vec<ArtistSummary>,
    pub lyrics: Vec<LyricMatch>,
    /// All the songs the search matches, not only the ones in `songs`.
    pub song_count: usize,
    pub album_count: usize,
    pub artist_count: usize,
    /// All the songs whose lyrics hold the words.
    pub lyric_count: usize,
}

/// Something looked for before, for Search's Recent list.
#[derive(Clone, Debug, PartialEq)]
pub enum Recent {
    /// Words typed and then searched for with the keyboard's Search or Enter key.
    Words(String),
    /// A result opened or played from Search.
    Song(Box<Track>),
    Album(AlbumSummary),
    Artist(ArtistSummary),
}

/// The kinds [`Library::remember_search`] takes: what its key is for each is said there.
pub const RECENT_KINDS: [&str; 4] = ["words", "song", "album", "artist"];
/// How many recent searches are kept; an older one is dropped.
pub const RECENT_KEPT: usize = 20;

/// How well `name` answers `query` (both lowercase): the same, starting with it, or holding it.
fn closeness(name: &str, query: &str) -> u32 {
    let name = name.trim().to_lowercase();
    if name == query {
        3
    } else if name.starts_with(query) {
        2
    } else if name.contains(query) {
        1
    } else {
        0
    }
}

/// The lyrics as plain text, one line each.
fn words_of(lyrics: &Lyrics) -> String {
    if lyrics.lines.is_empty() {
        lyrics.plain.trim().to_string()
    } else {
        lyrics
            .lines
            .iter()
            .map(|l| l.text.trim())
            .filter(|l| !l.is_empty())
            .collect::<Vec<_>>()
            .join("\n")
    }
}

impl Library {
    /// Keeps a song's lyrics for searching (`None`: it has none). Replaces what was kept.
    pub fn keep_lyrics(&self, track: &Track, lyrics: Option<&Lyrics>) -> Result<()> {
        let text = lyrics.map(words_of).unwrap_or_default();
        let mut db = self.connection()?;
        let tx = db.transaction()?;
        tx.execute("DELETE FROM lyrics_fts WHERE track_id=?", [&track.id])?;
        tx.execute(
            "INSERT INTO lyrics_text(track_id,hash,version,text) VALUES(?,?,?,?) ON CONFLICT(track_id) DO UPDATE SET hash=excluded.hash,version=excluded.version,text=excluded.text",
            params![track.id, track.content_hash, VERSION, text],
        )?;
        if !text.is_empty() {
            tx.execute(
                "INSERT INTO lyrics_fts(text,track_id) VALUES(?,?)",
                params![text, track.id],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Songs on this device whose own lyrics are not read yet (or changed since), at most
    /// `limit`.
    pub fn lyrics_unread(&self, limit: usize) -> Result<Vec<Track>> {
        let ids: Vec<String> = self
            .connection()?
            .prepare(
                "SELECT t.id FROM tracks t LEFT JOIN lyrics_text l ON l.track_id=t.id
                 WHERE t.missing=0 AND t.path NOT LIKE 'source://%' AND t.path NOT LIKE 'http%'
                 AND (l.track_id IS NULL OR l.version<>? OR l.hash<>t.content_hash) LIMIT ?",
            )?
            .query_map(params![VERSION, limit as i64], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        self.tracks_by_ids(&ids)
    }

    /// Reads the own lyrics of up to `limit` songs not read yet. Returns how many it read.
    pub fn read_lyrics(&self, limit: usize) -> Result<usize> {
        let tracks = self.lyrics_unread(limit)?;
        for track in &tracks {
            self.keep_lyrics(track, crate::media::local_lyrics(track).as_ref())?;
        }
        Ok(tracks.len())
    }

    /// Songs whose kept lyrics hold `query` (three letters or more), with the line it is in.
    pub fn lyric_matches(&self, query: &str, limit: usize) -> Result<Vec<LyricMatch>> {
        let query = query.trim();
        if query.chars().count() < 3 {
            return Ok(vec![]);
        }
        let phrase = format!("\"{}\"", query.replace('"', "\"\""));
        let rows: Vec<(String, String)> = self
            .connection()?
            .prepare("SELECT track_id,text FROM lyrics_fts WHERE lyrics_fts MATCH ? LIMIT ?")?
            .query_map(params![phrase, limit as i64 * 4], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })?
            .collect::<rusqlite::Result<_>>()?;
        let wanted = query.to_lowercase();
        let ids: Vec<String> = rows.iter().map(|(id, _)| id.clone()).collect();
        let mut tracks = self.tracks_by_ids(&ids)?;
        tracks.retain(|t| !t.missing);
        let mut found = vec![];
        for (id, text) in rows {
            let Some(track) = tracks.iter().find(|t| t.id == id) else {
                continue;
            };
            let Some(line) = text.lines().find(|l| l.to_lowercase().contains(&wanted)) else {
                continue;
            };
            found.push(LyricMatch {
                track: track.clone(),
                line: line.trim().to_string(),
            });
            if found.len() == limit {
                break;
            }
        }
        Ok(found)
    }

    /// How many songs' kept lyrics hold `query` (three letters or more).
    pub fn lyric_count(&self, query: &str) -> Result<usize> {
        let query = query.trim();
        if query.chars().count() < 3 {
            return Ok(0);
        }
        let phrase = format!("\"{}\"", query.replace('"', "\"\""));
        let count: i64 = self.connection()?.query_row(
            "SELECT count(*) FROM lyrics_fts f JOIN tracks t ON t.id=f.track_id WHERE lyrics_fts MATCH ? AND t.missing=0",
            [phrase],
            |r| r.get(0),
        )?;
        Ok(count.max(0) as usize)
    }

    /// Keeps something looked for, as the newest: `kind` "words" (`key`: the words typed),
    /// "song" (a song's id), "album" ([`AlbumSummary::key`]), or "artist" (the name). The one
    /// kept before under the same kind and key moves to the front; only the newest
    /// [`RECENT_KEPT`] stay.
    pub fn remember_search(&self, kind: &str, key: &str) -> Result<()> {
        let key = key.trim();
        if !RECENT_KINDS.contains(&kind) {
            anyhow::bail!("A recent search is \"words\", \"song\", \"album\", or \"artist\"");
        }
        if key.is_empty() || key.chars().count() > 300 {
            return Ok(());
        }
        let mut db = self.connection()?;
        let tx = db.transaction()?;
        // Words are one search whatever their case.
        if kind == "words" {
            tx.execute(
                "DELETE FROM recent_searches WHERE kind='words' AND key=? COLLATE NOCASE",
                [key],
            )?;
        }
        tx.execute(
            "INSERT OR REPLACE INTO recent_searches(kind,key) VALUES(?,?)",
            params![kind, key],
        )?;
        tx.execute(
            "DELETE FROM recent_searches WHERE rowid NOT IN (SELECT rowid FROM recent_searches ORDER BY rowid DESC LIMIT ?)",
            [RECENT_KEPT as i64],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Drops one recent search (the Recent list's ×).
    pub fn forget_search(&self, kind: &str, key: &str) -> Result<()> {
        self.connection()?.execute(
            "DELETE FROM recent_searches WHERE kind=? AND key=? COLLATE NOCASE",
            params![kind, key.trim()],
        )?;
        Ok(())
    }

    /// Drops every recent search (the Recent list's Clear).
    pub fn clear_searches(&self) -> Result<()> {
        self.connection()?
            .execute("DELETE FROM recent_searches", [])?;
        Ok(())
    }

    /// What was looked for, the newest first, at most `limit`. A song, album, or artist that
    /// is no longer in the library is left out (it stays kept, and shows again if it returns).
    pub fn recent_searches(&self, limit: usize) -> Result<Vec<Recent>> {
        let rows: Vec<(String, String)> = self
            .connection()?
            .prepare("SELECT kind,key FROM recent_searches ORDER BY rowid DESC")?
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?;
        let mut recent = vec![];
        for (kind, key) in rows {
            if recent.len() >= limit {
                break;
            }
            let found = match kind.as_str() {
                "words" => Some(Recent::Words(key)),
                "song" => self
                    .tracks_by_ids(&[key])?
                    .into_iter()
                    .find(|t| !t.missing)
                    .map(|t| Recent::Song(Box::new(t))),
                "album" => self.album_by_key(&key).ok().flatten().map(Recent::Album),
                "artist" => {
                    let name = crate::query::quote(&key);
                    let wanted = key.to_lowercase();
                    self.artists(&format!("artist = {name} or album_artist = {name}"))
                        .unwrap_or_default()
                        .into_iter()
                        .find(|a| a.name.to_lowercase() == wanted)
                        .map(Recent::Artist)
                }
                _ => None,
            };
            recent.extend(found);
        }
        Ok(recent)
    }

    /// What `query` finds: songs, albums, and artists it matches (it may be a rule), lines of
    /// lyrics that hold it, and the best of them. `limit` of each, and how many there are.
    pub fn find(&self, query: &str, limit: usize) -> Result<Found> {
        let query = query.trim();
        if query.is_empty() {
            return Ok(Found::default());
        }
        let mut songs = self.search(query)?;
        songs.retain(|t| !t.missing);
        // The most played first, then the order the rule gives.
        songs.sort_by_key(|t| std::cmp::Reverse(t.play_count));
        let mut albums = self.albums(query)?;
        albums.sort_by_key(|a| std::cmp::Reverse(a.play_count));
        let mut artists = self.artists(query)?;
        artists.sort_by_key(|a| std::cmp::Reverse(a.play_count));
        let lyrics = self.lyric_matches(query, limit)?;
        let lyric_count = self.lyric_count(query)?.max(lyrics.len());
        let (song_count, album_count, artist_count) = (songs.len(), albums.len(), artists.len());

        // The best: a name that answers the search most closely; albums, then artists, then
        // songs when two answer as closely, and the most played of each.
        let wanted = query.to_lowercase();
        let album = albums
            .iter()
            .map(|a| (closeness(&a.album, &wanted), a))
            .filter(|(c, _)| *c > 0)
            .max_by_key(|(c, a)| (*c, a.play_count));
        let artist = artists
            .iter()
            .map(|a| (closeness(&a.name, &wanted), a))
            .filter(|(c, _)| *c > 0)
            .max_by_key(|(c, a)| (*c, a.play_count));
        let song = songs
            .iter()
            .map(|s| (closeness(&s.title, &wanted), s))
            .filter(|(c, _)| *c > 0)
            .max_by_key(|(c, s)| (*c, s.play_count));
        let mut best: Option<(u32, u32, Best)> = None;
        let mut offer = |score: u32, rank: u32, found: Best| {
            if best
                .as_ref()
                .is_none_or(|(s, r, _)| (score, rank) > (*s, *r))
            {
                best = Some((score, rank, found));
            }
        };
        if let Some((c, a)) = album {
            offer(c, 3, Best::Album(a.clone()));
        }
        if let Some((c, a)) = artist {
            offer(c, 2, Best::Artist(a.clone()));
        }
        if let Some((c, s)) = song {
            offer(c, 1, Best::Song(Box::new(s.clone())));
        }
        songs.truncate(limit);
        albums.truncate(limit);
        artists.truncate(limit);
        Ok(Found {
            best: best.map(|(_, _, b)| b),
            songs,
            albums,
            artists,
            lyrics,
            song_count,
            album_count,
            artist_count,
            lyric_count,
        })
    }
}

/// Reads every song's own lyrics into the index, slowly, on a thread of its own: started
/// once for each library folder however often it is asked.
pub fn start_reading_lyrics(library: Library) {
    static STARTED: OnceLock<Mutex<HashSet<PathBuf>>> = OnceLock::new();
    let started = STARTED.get_or_init(Default::default);
    if !started
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(library.directory.clone())
    {
        return;
    }
    let _ = std::thread::Builder::new()
        .name("lyrics-index".into())
        .spawn(move || {
            // Not while Needle starts and its first scan runs.
            std::thread::sleep(Duration::from_secs(20));
            loop {
                let pause = match library.read_lyrics(40) {
                    Ok(0) => Duration::from_secs(60),
                    Ok(_) => Duration::from_millis(250),
                    Err(problem) => {
                        crate::logfile::warn(format!("Reading lyrics for search: {problem:#}"));
                        Duration::from_secs(300)
                    }
                };
                std::thread::sleep(pause);
            }
        });
}

/// Whether the song's lyrics were read for the index (for tests).
#[cfg(test)]
fn kept(library: &Library, id: &str) -> Option<String> {
    use rusqlite::OptionalExtension;
    library
        .connection()
        .ok()?
        .query_row("SELECT text FROM lyrics_text WHERE track_id=?", [id], |r| {
            r.get(0)
        })
        .optional()
        .ok()
        .flatten()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn song(
        dir: &std::path::Path,
        id: &str,
        title: &str,
        artist: &str,
        album: &str,
        plays: i64,
    ) -> Track {
        let path = dir.join(format!("{id}.flac"));
        std::fs::write(&path, b"").unwrap();
        Track {
            id: id.into(),
            path: path.to_string_lossy().into(),
            title: title.into(),
            artist: artist.into(),
            album_artist: artist.into(),
            album: album.into(),
            content_hash: format!("h-{id}"),
            play_count: plays,
            ..Default::default()
        }
    }

    #[test]
    fn the_rule_chips_are_rules() {
        for (_, rule) in RULE_EXAMPLES {
            crate::query::compile(rule, 0).unwrap();
        }
    }

    #[test]
    fn finds_names_lyrics_and_the_best() {
        let dir = tempfile::tempdir().unwrap();
        let library = Library::open(dir.path().join("data")).unwrap();
        let music = dir.path().join("music");
        std::fs::create_dir_all(&music).unwrap();
        let tracks = [
            song(&music, "a", "Opening", "Mara Quinn", "Harbor Lights", 9),
            song(
                &music,
                "b",
                "Harbor Lights",
                "Mara Quinn",
                "Harbor Lights",
                3,
            ),
            song(&music, "c", "Hard Light", "Sol Arden", "Low Desert", 1),
        ];
        for t in &tracks {
            library.upsert(t).unwrap();
        }
        // The song's own lyrics, in a file beside it, are read by the index.
        std::fs::write(
            music.join("a.lrc"),
            "[00:01.00]Harbor lights are coming on\n[00:05.00]The tide is turning slow\n",
        )
        .unwrap();
        assert_eq!(library.read_lyrics(10).unwrap(), 3);
        assert_eq!(library.read_lyrics(10).unwrap(), 0, "read once");
        assert!(kept(&library, "a").unwrap().contains("tide is turning"));

        let found = library.find("harbor", 5).unwrap();
        // The album named exactly so wins over the song of the same name.
        assert!(matches!(found.best, Some(Best::Album(ref a)) if a.album == "Harbor Lights"));
        assert_eq!(found.lyrics.len(), 1);
        assert_eq!(found.lyrics[0].track.id, "a");
        assert_eq!(found.lyrics[0].line, "Harbor lights are coming on");

        // Only the lyrics hold "tide".
        let found = library.find("tide", 5).unwrap();
        assert!(found.songs.is_empty());
        assert_eq!(found.lyrics[0].line, "The tide is turning slow");
        assert!(found.best.is_none());

        // An artist's name.
        let found = library.find("sol arden", 5).unwrap();
        assert!(matches!(found.best, Some(Best::Artist(ref a)) if a.name == "Sol Arden"));

        // Shown lyrics replace the file's; a changed file is read again.
        let other = Lyrics {
            lines: vec![],
            plain: "Sand and slow heat".into(),
            instrumental: false,
            source: crate::media::LyricsSource::Lrclib,
            provider: String::new(),
        };
        library.keep_lyrics(&tracks[2], Some(&other)).unwrap();
        assert_eq!(
            library.lyric_matches("slow heat", 5).unwrap()[0].track.id,
            "c"
        );
        let mut changed = tracks[2].clone();
        changed.content_hash = "h-c2".into();
        library.upsert(&changed).unwrap();
        assert_eq!(library.lyrics_unread(10).unwrap().len(), 1);
        // Two letters are too few to look for.
        assert!(library.lyric_matches("ha", 5).unwrap().is_empty());
    }

    #[test]
    fn counts_all_that_match_not_only_what_it_gives() {
        let dir = tempfile::tempdir().unwrap();
        let library = Library::open(dir.path().join("data")).unwrap();
        let music = dir.path().join("music");
        std::fs::create_dir_all(&music).unwrap();
        for i in 0..7 {
            let t = song(
                &music,
                &format!("s{i}"),
                &format!("Moon {i}"),
                &format!("Artist {i}"),
                &format!("Moon Album {i}"),
                i,
            );
            library.upsert(&t).unwrap();
            std::fs::write(
                music.join(format!("s{i}.lrc")),
                "[00:01.00]Under the moon\n",
            )
            .unwrap();
        }
        library.read_lyrics(20).unwrap();
        let found = library.find("moon", 3).unwrap();
        assert_eq!(
            (found.songs.len(), found.albums.len(), found.lyrics.len()),
            (3, 3, 3)
        );
        assert_eq!(
            (found.song_count, found.album_count, found.lyric_count),
            (7, 7, 7)
        );
        // Artists are the ones the matching songs credit: more than `limit` here too.
        assert_eq!((found.artists.len(), found.artist_count), (3, 7));
        assert_eq!(library.find("", 3).unwrap().song_count, 0);
    }

    #[test]
    fn keeps_recent_searches_newest_first() {
        let dir = tempfile::tempdir().unwrap();
        let library = Library::open(dir.path().join("data")).unwrap();
        let music = dir.path().join("music");
        std::fs::create_dir_all(&music).unwrap();
        let track = song(&music, "a", "Opening", "Mara Quinn", "Harbor Lights", 2);
        library.upsert(&track).unwrap();
        let album = library.albums("").unwrap().remove(0);

        library.remember_search("words", "harbor").unwrap();
        library.remember_search("song", "a").unwrap();
        library.remember_search("album", &album.key).unwrap();
        library.remember_search("artist", "Mara Quinn").unwrap();
        // Gone from the library: kept, but not shown.
        library.remember_search("song", "nothing").unwrap();
        // The same words again, in another case: one search, now the newest.
        library.remember_search("words", "Harbor").unwrap();
        assert!(library.remember_search("genre", "x").is_err());
        library.remember_search("words", "  ").unwrap();

        let recent = library.recent_searches(10).unwrap();
        assert_eq!(recent.len(), 4);
        assert_eq!(recent[0], Recent::Words("Harbor".into()));
        assert!(matches!(&recent[1], Recent::Artist(a) if a.name == "Mara Quinn"));
        assert!(matches!(&recent[2], Recent::Album(a) if a.key == album.key));
        assert!(matches!(&recent[3], Recent::Song(t) if t.id == "a"));
        assert_eq!(library.recent_searches(2).unwrap().len(), 2);

        library.forget_search("artist", "Mara Quinn").unwrap();
        assert_eq!(library.recent_searches(10).unwrap().len(), 3);

        // Only the newest are kept.
        for i in 0..RECENT_KEPT + 5 {
            library
                .remember_search("words", &format!("search {i}"))
                .unwrap();
        }
        let recent = library.recent_searches(100).unwrap();
        assert_eq!(recent.len(), RECENT_KEPT);
        assert_eq!(
            recent[0],
            Recent::Words(format!("search {}", RECENT_KEPT + 4))
        );

        library.clear_searches().unwrap();
        assert!(library.recent_searches(10).unwrap().is_empty());
    }
}
