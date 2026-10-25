//! The library looked at as a whole: what it is made of ("Your library in numbers"), and what
//! was playing on this day in earlier years ("On this day").
use crate::{browse::AlbumSummary, database::Library};
use anyhow::Result;
use serde::Serialize;
use std::collections::HashMap;

/// What a library is made of.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct LibraryNumbers {
    pub songs: usize,
    pub albums: usize,
    pub artists: usize,
    /// Seconds of music.
    pub duration: f64,
    /// Bytes on disk (songs on this device only).
    pub size: i64,
    /// Songs from a music server.
    pub server_songs: usize,
    /// Songs per format ("FLAC", "MP3", …), most first.
    pub formats: Vec<(String, usize)>,
    /// Lossless songs (FLAC, ALAC, WAV, AIFF, APE, WavPack, DSD).
    pub lossless: usize,
    /// Hi-res songs: more than 16 bits, or more than 48 kHz.
    pub hires: usize,
    /// Songs per decade ("1990s"), oldest first; songs without a year are left out.
    pub decades: Vec<(String, usize)>,
    /// The ten genres with the most songs.
    pub genres: Vec<(String, usize)>,
    /// The ten artists with the most songs.
    pub artists_by_songs: Vec<(String, usize)>,
    /// Albums with no cover at all.
    pub albums_without_cover: usize,
    /// Albums whose songs are all lossy and under 192 kbit/s.
    pub low_quality_albums: usize,
    /// Up to 50 of each, for a list to fix.
    pub without_cover: Vec<AlbumSummary>,
    pub low_quality: Vec<AlbumSummary>,
}

const LOSSLESS: [&str; 10] = [
    "FLAC", "ALAC", "WAV", "AIFF", "AIF", "APE", "WV", "DSF", "DFF", "TTA",
];

/// A time on this day `years` years before `now` (local time `offset` seconds east of UTC):
/// that day's start and end, in Unix seconds.
pub fn day_years_ago(now: i64, offset: i32, years: i32) -> Option<(i64, i64)> {
    use chrono::{Datelike, TimeZone};
    let zone = chrono::FixedOffset::east_opt(offset)?;
    let today = zone.timestamp_opt(now, 0).single()?.date_naive();
    // 29 February falls on the 28th in other years.
    let then = today.with_year(today.year() - years).or_else(|| {
        today
            .with_day(28)
            .and_then(|d| d.with_year(today.year() - years))
    })?;
    let start = zone
        .from_local_datetime(&then.and_hms_opt(0, 0, 0)?)
        .single()?
        .timestamp();
    Some((start, start + 86_400))
}

/// What was playing on this day some years ago.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct OnThisDay {
    pub years_ago: i32,
    /// The day's start (Unix seconds), to name it ("25 October 2025").
    pub day: i64,
    /// Albums played that day, most played first.
    pub albums: Vec<AlbumSummary>,
    /// Songs listened to that day.
    pub listens: usize,
}

impl Library {
    /// What the library is made of.
    pub fn numbers(&self) -> Result<LibraryNumbers> {
        let db = self.connection()?;
        let mut numbers = LibraryNumbers::default();
        let mut formats: HashMap<String, usize> = HashMap::new();
        let mut decades: HashMap<i64, usize> = HashMap::new();
        let mut genres: HashMap<String, (String, usize)> = HashMap::new();
        let mut albums: HashMap<(String, String), (bool, bool, String, String)> = HashMap::new();
        let mut stmt = db.prepare(
            "SELECT format, bit_depth, sample_rate, year, genre, duration, album_artist, album,
                json_extract(data,'$.file_size'), json_extract(data,'$.artwork'),
                json_extract(data,'$.bitrate'), path
             FROM tracks WHERE missing=0",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, i64>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, f64>(5)?,
                r.get::<_, String>(6)?,
                r.get::<_, String>(7)?,
                r.get::<_, Option<i64>>(8)?,
                r.get::<_, Option<String>>(9)?,
                r.get::<_, Option<i64>>(10)?,
                r.get::<_, String>(11)?,
            ))
        })?;
        for row in rows {
            let (
                format,
                bits,
                rate,
                year,
                genre,
                duration,
                album_artist,
                album,
                size,
                artwork,
                bitrate,
                path,
            ) = row?;
            let format = format.to_uppercase();
            numbers.songs += 1;
            numbers.duration += duration;
            if path.starts_with(crate::sources::SCHEME) {
                numbers.server_songs += 1;
            } else {
                numbers.size += size.unwrap_or(0);
            }
            let lossless = LOSSLESS.contains(&format.as_str());
            if lossless {
                numbers.lossless += 1;
            }
            if bits > 16 || rate > 48_000 {
                numbers.hires += 1;
            }
            *formats.entry(format).or_default() += 1;
            if year > 0 {
                *decades.entry(year / 10 * 10).or_default() += 1;
            }
            if !genre.trim().is_empty() {
                let entry = genres
                    .entry(genre.trim().to_lowercase())
                    .or_insert_with(|| (genre.trim().to_string(), 0));
                entry.1 += 1;
            }
            let low = !lossless && bitrate.is_some_and(|b| b > 0 && b < 192);
            let entry = albums
                .entry((album_artist.to_lowercase(), album.to_lowercase()))
                .or_insert((false, true, album_artist.clone(), album.clone()));
            entry.0 |= artwork.is_some();
            entry.1 &= low;
        }
        numbers.albums = albums.len();
        let mut formats: Vec<(String, usize)> = formats.into_iter().collect();
        formats.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        numbers.formats = formats;
        let mut decades: Vec<(i64, usize)> = decades.into_iter().collect();
        decades.sort();
        numbers.decades = decades
            .into_iter()
            .map(|(d, n)| (format!("{d}s"), n))
            .collect();
        let mut genres: Vec<(String, usize)> = genres.into_values().collect();
        genres.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        genres.truncate(10);
        numbers.genres = genres;
        let artists = self.artists_by("", crate::artists::ArtistChoice::default())?;
        numbers.artists = artists.len();
        let mut by_songs: Vec<(String, usize)> =
            artists.into_iter().map(|a| (a.name, a.tracks)).collect();
        by_songs.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        by_songs.truncate(10);
        numbers.artists_by_songs = by_songs;
        let mut without: Vec<(String, String)> = vec![];
        let mut low: Vec<(String, String)> = vec![];
        for (has_cover, all_low, album_artist, album) in albums.into_values() {
            if album.is_empty() {
                continue;
            }
            if !has_cover {
                numbers.albums_without_cover += 1;
                without.push((album_artist.clone(), album.clone()));
            }
            if all_low {
                numbers.low_quality_albums += 1;
                low.push((album_artist, album));
            }
        }
        for list in [&mut without, &mut low] {
            list.sort_by_key(|(artist, album)| (artist.to_lowercase(), album.to_lowercase()));
            list.truncate(50);
        }
        numbers.without_cover = self.albums_for_keys(&without)?;
        numbers.low_quality = self.albums_for_keys(&low)?;
        Ok(numbers)
    }

    /// What was playing on this day, `years` years ago each (for example `[1, 5]`), at most
    /// `limit` albums a day. Days with nothing played are left out.
    pub fn on_this_day(
        &self,
        now: i64,
        offset: i32,
        years: &[i32],
        limit: usize,
    ) -> Result<Vec<OnThisDay>> {
        let db = self.connection()?;
        let mut days = vec![];
        for &ago in years {
            let Some((start, end)) = day_years_ago(now, offset, ago) else {
                continue;
            };
            let mut stmt = db.prepare_cached(
                "SELECT t.album_artist, t.album, count(*) FROM listens l JOIN tracks t ON t.id=l.track_id
                 WHERE l.started_at >= ?1 AND l.started_at < ?2 AND t.album != ''
                 GROUP BY t.album_artist COLLATE NOCASE, t.album COLLATE NOCASE
                 ORDER BY count(*) DESC, max(l.started_at) DESC LIMIT ?3",
            )?;
            let keys: Vec<(String, String)> = stmt
                .query_map(rusqlite::params![start, end, limit as i64], |r| {
                    Ok((r.get(0)?, r.get(1)?))
                })?
                .collect::<std::result::Result<_, _>>()?;
            let listens: i64 = db.query_row(
                "SELECT count(*) FROM listens WHERE started_at >= ?1 AND started_at < ?2",
                rusqlite::params![start, end],
                |r| r.get(0),
            )?;
            if listens == 0 {
                continue;
            }
            days.push(OnThisDay {
                years_ago: ago,
                day: start,
                albums: self.albums_for_keys(&keys)?,
                listens: listens as usize,
            });
        }
        Ok(days)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Track;

    fn song(id: &str, album: &str, format: &str, year: i64, bits: i64, bitrate: i64) -> Track {
        Track {
            id: id.into(),
            path: format!("/music/{id}.{}", format.to_lowercase()),
            title: id.into(),
            artist: "Mara Quinn".into(),
            album_artist: "Mara Quinn".into(),
            album: album.into(),
            format: format.into(),
            year,
            bit_depth: bits,
            sample_rate: 44_100,
            bitrate,
            duration: 200.,
            file_size: 1000,
            genre: "Folk".into(),
            artwork: (album == "Tidewater").then(|| "cover.jpg".into()),
            ..Default::default()
        }
    }

    #[test]
    fn numbers_count_formats_decades_and_albums_to_fix() {
        let dir = tempfile::tempdir().unwrap();
        let library = Library::open(dir.path()).unwrap();
        for t in [
            song("a", "Tidewater", "FLAC", 2024, 24, 0),
            song("b", "Tidewater", "FLAC", 2024, 16, 0),
            song("c", "Old Tapes", "MP3", 1994, 0, 128),
        ] {
            library.upsert(&t).unwrap();
        }
        let n = library.numbers().unwrap();
        assert_eq!(
            (n.songs, n.albums, n.artists, n.lossless, n.hires),
            (3, 2, 1, 2, 1)
        );
        assert_eq!(n.formats, [("FLAC".to_string(), 2), ("MP3".to_string(), 1)]);
        assert_eq!(
            n.decades,
            [("1990s".to_string(), 1), ("2020s".to_string(), 2)]
        );
        assert_eq!((n.albums_without_cover, n.low_quality_albums), (1, 1));
        assert_eq!(n.low_quality[0].album, "Old Tapes");
        assert_eq!(n.size, 3000);
    }

    #[test]
    fn on_this_day_finds_a_year_ago() {
        // 25 October 2026, noon UTC.
        let now = 1_792_929_600;
        let (start, end) = day_years_ago(now, 0, 1).unwrap();
        assert_eq!(end - start, 86_400);
        assert_eq!(start, 1_761_350_400); // 25 October 2025, 00:00 UTC.
        // A leap day falls on the 28th.
        let leap = 1_709_208_000; // 29 February 2024, noon UTC.
        let (s, _) = day_years_ago(leap, 0, 1).unwrap();
        assert_eq!(s, 1_677_542_400); // 28 February 2023.
    }
}
