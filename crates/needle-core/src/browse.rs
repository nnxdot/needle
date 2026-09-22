//! Library-wide album, artist, genre, and field-value summaries, aggregated in SQLite.
use crate::{database::Library, model::Track, query};
use anyhow::{Context, Result, bail};
use rusqlite::{Connection, params, types::Value};
use serde::Serialize;
use std::collections::HashSet;

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct AlbumSummary {
    /// Opaque, stable identity for [`Library::album_tracks`].
    pub key: String,
    /// An equivalent rule expression, usable in search or a smart playlist.
    pub query: String,
    pub album: String,
    /// Album artist; otherwise the track artist, or "Various artists" when they differ.
    pub artist: String,
    pub year: i64,
    pub tracks: usize,
    pub duration: f64,
    /// Artwork of the first of the album's first 20 tracks (disc/track order) that has any.
    pub artwork: Option<String>,
    /// Distinct formats, sorted and comma-separated, e.g. "FLAC, MP3".
    pub formats: String,
    pub added_at: i64,
    pub play_count: i64,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct ArtistSummary {
    pub name: String,
    pub albums: usize,
    pub tracks: usize,
    pub duration: f64,
    pub artwork: Option<String>,
    pub play_count: i64,
}

pub(crate) fn migrate(db: &Connection) -> Result<()> {
    db.execute_batch(
        "CREATE INDEX IF NOT EXISTS tracks_genre ON tracks(genre COLLATE NOCASE);
        CREATE INDEX IF NOT EXISTS tracks_album_title ON tracks(album COLLATE NOCASE);
        CREATE INDEX IF NOT EXISTS tracks_format ON tracks(format COLLATE NOCASE);
        CREATE INDEX IF NOT EXISTS tracks_album_summary ON tracks(album_artist COLLATE NOCASE,album COLLATE NOCASE,artist,year,duration,format,added_at,play_count);
        CREATE INDEX IF NOT EXISTS tracks_artist_summary ON tracks(artist COLLATE NOCASE,album,duration,play_count);",
    )?;
    Ok(())
}

/// Fields whose distinct values can be listed by [`Library::field_values`].
pub const VALUE_FIELDS: &[&str] = &["artist", "album", "album_artist", "genre", "format"];

fn filtered(expression: &str) -> Result<(String, Vec<Value>)> {
    let q = query::compile(expression, chrono::Utc::now().timestamp())?;
    let source = if q.limit < 500_000 {
        format!(
            "(SELECT t.rowid AS rowid, t.* FROM tracks t WHERE ({}) ORDER BY {} LIMIT {}) t",
            q.sql, q.order, q.limit
        )
    } else {
        format!("tracks t WHERE ({})", q.sql)
    };
    Ok((source, q.parameters))
}

fn sort_name(name: &str) -> String {
    let lower = name.to_lowercase();
    lower
        .strip_prefix("the ")
        .map(str::to_string)
        .unwrap_or(lower)
}

impl Library {
    /// Distinct non-empty values of a text field that start with `prefix` (case-insensitive),
    /// most common first. Supported fields: [`VALUE_FIELDS`].
    pub fn field_values(
        &self,
        field: &str,
        prefix: &str,
        limit: usize,
    ) -> Result<Vec<(String, usize)>> {
        let field = field.to_lowercase();
        if !VALUE_FIELDS.contains(&field.as_str()) {
            bail!("Values are not listed for {field}")
        }
        let pattern = format!(
            "{}%",
            prefix
                .replace('\\', "\\\\")
                .replace('%', "\\%")
                .replace('_', "\\_")
        );
        let db = self.connection()?;
        let mut stmt = db.prepare(&format!(
            "SELECT min({field}), count(*) c FROM tracks WHERE {field} != '' AND {field} LIKE ? ESCAPE '\\' GROUP BY {field} COLLATE NOCASE ORDER BY c DESC, {field} COLLATE NOCASE LIMIT ?"
        ))?;
        let rows = stmt.query_map(params![pattern, limit.min(10_000) as i64], |r| {
            Ok((r.get(0)?, r.get::<_, i64>(1)? as usize))
        })?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    fn album_rows(
        &self,
        db: &Connection,
        source: &str,
        parameters: &[Value],
    ) -> Result<Vec<AlbumSummary>> {
        let mut stmt = db.prepare_cached(&format!(
            "SELECT min(t.album_artist), min(t.album), max(t.year), count(*), sum(t.duration), group_concat(DISTINCT t.format),
                max(t.added_at), sum(t.play_count), count(DISTINCT t.artist COLLATE NOCASE), max(t.artist),
                (SELECT a FROM (SELECT json_extract(x.data,'$.artwork') a FROM tracks x WHERE x.album_artist=t.album_artist COLLATE NOCASE
                    AND x.album=t.album COLLATE NOCASE ORDER BY x.disc, x.track_number LIMIT 20) WHERE a IS NOT NULL LIMIT 1)
             FROM {source} GROUP BY t.album_artist COLLATE NOCASE, t.album COLLATE NOCASE"
        ))?;
        let rows = stmt.query_map(rusqlite::params_from_iter(parameters), |r| {
            let album_artist: String = r.get(0)?;
            let album: String = r.get(1)?;
            let formats: Option<String> = r.get(5)?;
            let mut formats: Vec<&str> = formats
                .as_deref()
                .unwrap_or("")
                .split(',')
                .filter(|f| !f.is_empty())
                .collect();
            formats.sort_unstable();
            formats.dedup();
            let artist = if !album_artist.is_empty() {
                album_artist.clone()
            } else if r.get::<_, i64>(8)? > 1 {
                "Various artists".into()
            } else {
                r.get(9)?
            };
            Ok(AlbumSummary {
                key: serde_json::to_string(&[&album_artist, &album]).unwrap_or_default(),
                query: format!(
                    "album_artist = {} and album = {}",
                    query::quote(&album_artist),
                    query::quote(&album)
                ),
                album,
                artist,
                year: r.get(2)?,
                tracks: r.get::<_, i64>(3)? as usize,
                duration: r.get(4)?,
                artwork: r.get(10)?,
                formats: formats.join(", "),
                added_at: r.get(6)?,
                play_count: r.get(7)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }
    fn sort_albums(albums: &mut [AlbumSummary]) {
        albums.sort_by_cached_key(|a| {
            (
                sort_name(&a.artist),
                a.year,
                a.album.to_lowercase(),
                a.key.clone(),
            )
        });
    }
    /// One row per (album artist, album) over the whole library, optionally filtered by a
    /// rule or search expression ("" for all). Sorted by artist, year, then album.
    pub fn albums(&self, expression: &str) -> Result<Vec<AlbumSummary>> {
        let (source, parameters) = filtered(expression)?;
        let db = self.connection()?;
        let mut albums = self.album_rows(&db, &source, &parameters)?;
        Self::sort_albums(&mut albums);
        Ok(albums)
    }
    /// Tracks of an album identified by [`AlbumSummary::key`], in disc and track order.
    pub fn album_tracks(&self, key: &str) -> Result<Vec<Track>> {
        let [album_artist, album]: [String; 2] =
            serde_json::from_str(key).context("Invalid album key")?;
        let db = self.connection()?;
        let mut stmt = db.prepare("SELECT data,rating,play_count,last_played,missing FROM tracks WHERE album_artist=? COLLATE NOCASE AND album=? COLLATE NOCASE ORDER BY disc, track_number, title COLLATE NOCASE, id")?;
        let rows = stmt.query_map(params![album_artist, album], Self::row_track)?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }
    /// Albums containing a track by `name`, as artist or album artist, with whole-album totals.
    pub fn artist_albums(&self, name: &str) -> Result<Vec<AlbumSummary>> {
        let db = self.connection()?;
        let mut stmt = db.prepare("SELECT DISTINCT album_artist, album FROM tracks WHERE artist=?1 COLLATE NOCASE OR album_artist=?1 COLLATE NOCASE")?;
        let keys: Vec<(String, String)> = stmt
            .query_map([name], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<std::result::Result<_, _>>()?;
        let mut seen = HashSet::new();
        let mut albums = vec![];
        for (album_artist, album) in keys {
            if seen.insert((album_artist.to_lowercase(), album.to_lowercase())) {
                albums.extend(self.album_rows(
                    &db,
                    "tracks t WHERE t.album_artist=? COLLATE NOCASE AND t.album=? COLLATE NOCASE",
                    &[Value::Text(album_artist), Value::Text(album)],
                )?);
            }
        }
        Self::sort_albums(&mut albums);
        Ok(albums)
    }
    /// One row per track artist, optionally filtered by an expression. Sorted by name,
    /// case-insensitively and ignoring a leading "The ".
    pub fn artists(&self, expression: &str) -> Result<Vec<ArtistSummary>> {
        let (source, parameters) = filtered(expression)?;
        let db = self.connection()?;
        let mut stmt = db.prepare(&format!(
            "SELECT min(t.artist), count(DISTINCT t.album COLLATE NOCASE), count(*), sum(t.duration), sum(t.play_count),
                (SELECT a FROM (SELECT json_extract(x.data,'$.artwork') a FROM tracks x WHERE x.artist=t.artist COLLATE NOCASE LIMIT 20) WHERE a IS NOT NULL LIMIT 1)
             FROM {source} GROUP BY t.artist COLLATE NOCASE"
        ))?;
        let mut artists: Vec<ArtistSummary> = stmt
            .query_map(rusqlite::params_from_iter(parameters), |r| {
                Ok(ArtistSummary {
                    name: r.get(0)?,
                    albums: r.get::<_, i64>(1)? as usize,
                    tracks: r.get::<_, i64>(2)? as usize,
                    duration: r.get(3)?,
                    play_count: r.get(4)?,
                    artwork: r.get(5)?,
                })
            })?
            .collect::<std::result::Result<_, _>>()?;
        artists.sort_by_cached_key(|a| (sort_name(&a.name), a.name.clone()));
        Ok(artists)
    }
    /// An artist's most played tracks, then highest rated.
    pub fn top_tracks(&self, artist: &str, limit: usize) -> Result<Vec<Track>> {
        let db = self.connection()?;
        let mut stmt = db.prepare("SELECT data,rating,play_count,last_played,missing FROM tracks WHERE artist=? COLLATE NOCASE ORDER BY play_count DESC, rating DESC, title COLLATE NOCASE, id LIMIT ?")?;
        let rows = stmt.query_map(params![artist, limit.min(500_000) as i64], Self::row_track)?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }
    /// Non-empty genres with track counts, sorted by name.
    pub fn genres(&self) -> Result<Vec<(String, usize)>> {
        let db = self.connection()?;
        let mut stmt = db.prepare("SELECT min(genre), count(*) FROM tracks WHERE genre != '' GROUP BY genre COLLATE NOCASE ORDER BY genre COLLATE NOCASE")?;
        let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get::<_, i64>(1)? as usize)))?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[allow(clippy::too_many_arguments)]
    fn track(
        id: &str,
        artist: &str,
        album_artist: &str,
        album: &str,
        disc: i64,
        number: i64,
        year: i64,
        format: &str,
    ) -> Track {
        Track {
            id: id.into(),
            path: format!("/music/{id}.{}", format.to_lowercase()),
            title: format!("Song {id}"),
            artist: artist.into(),
            album_artist: album_artist.into(),
            album: album.into(),
            genre: if artist == "Bob" {
                "Jazz".into()
            } else {
                "Rock".into()
            },
            disc,
            track_number: number,
            year,
            format: format.into(),
            duration: 100.0,
            added_at: number,
            ..Default::default()
        }
    }
    fn library() -> (tempfile::TempDir, Library) {
        let dir = tempfile::tempdir().unwrap();
        let library = Library::open(dir.path()).unwrap();
        for mut t in [
            track("a1", "Alice", "Alice", "First", 1, 2, 2001, "FLAC"),
            track("a2", "Alice", "alice", "first", 1, 1, 2001, "MP3"),
            track("a3", "Alice", "Alice", "Second", 1, 1, 1999, "FLAC"),
            track("b1", "Bob", "Bob", "Blue", 2, 1, 2010, "FLAC"),
            track("b2", "Bob", "Bob", "Blue", 1, 5, 2010, "FLAC"),
            track("v1", "Carol", "", "Mix", 1, 1, 2020, "MP3"),
            track("v2", "Dave", "", "Mix", 1, 2, 2020, "MP3"),
            track(
                "t1",
                "The Zebras",
                "The Zebras",
                "Stripes",
                1,
                1,
                2000,
                "WAV",
            ),
            track("c1", "Carol", "Compilers", "Hits", 1, 1, 2005, "FLAC"),
        ] {
            if t.id == "a1" {
                t.artwork = Some("first.jpg".into());
            }
            if t.id == "b2" {
                t.play_count = 7;
            }
            library.upsert(&t).unwrap();
        }
        library.rate("b1", 5).unwrap();
        (dir, library)
    }

    #[test]
    fn albums_group_sort_and_round_trip_keys() {
        let (_dir, library) = library();
        let albums = library.albums("").unwrap();
        let names: Vec<_> = albums
            .iter()
            .map(|a| (a.artist.as_str(), a.album.as_str()))
            .collect();
        assert_eq!(
            names,
            [
                ("Alice", "Second"),
                ("Alice", "First"),
                ("Bob", "Blue"),
                ("Compilers", "Hits"),
                ("Various artists", "Mix"),
                ("The Zebras", "Stripes"),
            ]
        );
        let first = &albums[1];
        assert_eq!(first.tracks, 2);
        assert_eq!(first.formats, "FLAC, MP3");
        assert_eq!(first.duration, 200.0);
        assert_eq!(first.artwork.as_deref(), Some("first.jpg"));
        assert_eq!(first.added_at, 2);
        let tracks = library.album_tracks(&first.key).unwrap();
        assert_eq!(
            tracks.iter().map(|t| t.id.as_str()).collect::<Vec<_>>(),
            ["a2", "a1"]
        );
        assert_eq!(library.search(&first.query).unwrap().len(), 2);
        let blue = library.album_tracks(&albums[2].key).unwrap();
        assert_eq!(
            blue.iter().map(|t| t.id.as_str()).collect::<Vec<_>>(),
            ["b2", "b1"]
        );
        assert_eq!(albums[2].play_count, 7);
        assert_eq!(library.search(&albums[4].query).unwrap().len(), 2);
        assert!(library.album_tracks("not json").is_err());

        let rock = library.albums("genre = Rock and year >= 2001").unwrap();
        assert_eq!(
            rock.iter().map(|a| a.album.as_str()).collect::<Vec<_>>(),
            ["First", "Hits", "Mix"]
        );
        let limited = library
            .albums("format = FLAC order by year limit 1")
            .unwrap();
        assert_eq!(limited.len(), 1);
        assert_eq!(limited[0].album, "Second");
        assert!(library.albums("year >").is_err());
    }

    #[test]
    fn artists_top_tracks_genres_and_values() {
        let (_dir, library) = library();
        let artists = library.artists("").unwrap();
        assert_eq!(
            artists.iter().map(|a| a.name.as_str()).collect::<Vec<_>>(),
            ["Alice", "Bob", "Carol", "Dave", "The Zebras"]
        );
        assert_eq!(artists[0].albums, 2);
        assert_eq!(artists[0].tracks, 3);
        assert_eq!(artists[0].artwork.as_deref(), Some("first.jpg"));
        assert_eq!(artists[1].play_count, 7);
        assert_eq!(library.artists("genre = Jazz").unwrap().len(), 1);

        let carol = library.artist_albums("carol").unwrap();
        assert_eq!(
            carol.iter().map(|a| a.album.as_str()).collect::<Vec<_>>(),
            ["Hits", "Mix"]
        );
        assert_eq!(carol[1].tracks, 2);
        assert_eq!(library.artist_albums("Alice").unwrap().len(), 2);
        assert!(library.artist_albums("Nobody").unwrap().is_empty());

        let top = library.top_tracks("bob", 10).unwrap();
        assert_eq!(
            top.iter().map(|t| t.id.as_str()).collect::<Vec<_>>(),
            ["b2", "b1"]
        );
        assert_eq!(library.top_tracks("Bob", 1).unwrap().len(), 1);

        assert_eq!(
            library.genres().unwrap(),
            [("Jazz".to_string(), 2), ("Rock".to_string(), 7)]
        );

        let values = library.field_values("artist", "", 10).unwrap();
        assert_eq!(values[0], ("Alice".into(), 3));
        assert_eq!(
            library.field_values("ARTIST", "ca", 10).unwrap(),
            [("Carol".to_string(), 2)]
        );
        assert_eq!(
            library.field_values("format", "", 1).unwrap(),
            [("FLAC".to_string(), 5)]
        );
        assert!(
            library
                .field_values("album_artist", "%", 10)
                .unwrap()
                .is_empty()
        );
        assert!(library.field_values("title", "", 10).is_err());
        assert!(
            library
                .field_values("genre; DROP TABLE tracks", "", 10)
                .is_err()
        );
    }
}
