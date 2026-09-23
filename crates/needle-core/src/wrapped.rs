//! A year of listening, told back: the totals, the favourites, the new finds, and the rhythm.
use crate::{
    database::Library,
    history::{DayStat, HistoryStats, HistoryTop},
    model::Track,
};
use anyhow::Result;
use chrono::{Datelike, Local, NaiveDate, TimeZone};
use rusqlite::params;
use std::collections::HashMap;

#[derive(Clone, Debug, Default)]
pub struct Wrapped {
    pub year: i32,
    pub stats: HistoryStats,
    /// Genres by listening time, most first (at most six).
    pub genres: Vec<(String, f64)>,
    /// Artists first heard this year, most played first (at most five). Empty when there is
    /// no listening from before the year to compare with.
    pub new_artists: Vec<HistoryTop>,
    pub busiest_day: Option<DayStat>,
    /// The most days in a row with some listening.
    pub streak: usize,
    /// The hour of day with the most listening, 0–23.
    pub peak_hour: Option<u8>,
    /// The year's most played songs, most first (up to 25), for playing or saving.
    pub top_track_ids: Vec<String>,
    /// Songs to show: the top songs, and one song from each top album for its cover.
    pub tracks: HashMap<String, Track>,
    /// For each of `stats.top_albums`, a song from it.
    pub album_tracks: Vec<Option<String>>,
}

/// Unix seconds at the start of 1 January of `year`, local time.
fn new_year(year: i32) -> i64 {
    NaiveDate::from_ymd_opt(year, 1, 1)
        .and_then(|d| d.and_hms_opt(0, 0, 0))
        .and_then(|t| Local.from_local_datetime(&t).earliest())
        .map_or(0, |t| t.timestamp())
}

/// The longest run of consecutive days with any listening.
fn longest_streak(days: &[DayStat]) -> usize {
    let (mut best, mut run) = (0, 0);
    for day in days {
        run = if day.listens > 0 { run + 1 } else { 0 };
        best = best.max(run);
    }
    best
}

/// "Pop; K-Pop / Dance" → ["Pop", "K-Pop", "Dance"].
fn split_genres(genre: &str) -> impl Iterator<Item = &str> {
    genre
        .split([';', '/', ',', '\0'])
        .map(str::trim)
        .filter(|g| !g.is_empty())
}

impl Library {
    /// Local calendar years with any listening, newest first.
    pub fn listening_years(&self) -> Result<Vec<i32>> {
        let (first, last): (Option<i64>, Option<i64>) = self.connection()?.query_row(
            "SELECT min(started_at), max(started_at) FROM listens",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        let year = |t: Option<i64>| {
            t.and_then(|t| Local.timestamp_opt(t, 0).single())
                .map(|t| t.year())
        };
        Ok(match (year(first), year(last)) {
            (Some(first), Some(last)) => (first..=last).rev().collect(),
            _ => vec![],
        })
    }

    pub fn wrapped(&self, year: i32) -> Result<Wrapped> {
        let (from, to) = (new_year(year), new_year(year + 1));
        let stats = self.history_stats_between(Some(from), Some(to))?;
        let db = self.connection()?;

        let mut genres: HashMap<String, (String, f64)> = HashMap::new();
        let mut stmt = db.prepare(
            "SELECT t.genre, sum(l.seconds) FROM listens l JOIN tracks t ON t.id = l.track_id
             WHERE l.started_at >= ? AND l.started_at < ? GROUP BY t.genre",
        )?;
        let mut rows = stmt.query(params![from, to])?;
        while let Some(row) = rows.next()? {
            let (genre, seconds): (String, f64) = (row.get(0)?, row.get(1)?);
            for name in split_genres(&genre) {
                genres
                    .entry(name.to_lowercase())
                    .or_insert_with(|| (name.to_string(), 0.))
                    .1 += seconds;
            }
        }
        let mut genres: Vec<(String, f64)> = genres.into_values().collect();
        genres.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        genres.truncate(6);

        let heard_before: bool = db.query_row(
            "SELECT EXISTS(SELECT 1 FROM listens WHERE started_at < ?)",
            [from],
            |r| r.get(0),
        )?;
        let new_artists = if heard_before {
            let mut stmt = db.prepare(
                "SELECT max(artist), sum(CASE WHEN started_at < ? THEN qualified ELSE 0 END),
                        sum(CASE WHEN started_at < ? THEN 1 ELSE 0 END),
                        sum(CASE WHEN started_at < ? THEN seconds ELSE 0 END)
                 FROM listens WHERE artist != '' AND started_at < ?
                 GROUP BY lower(artist) HAVING min(started_at) >= ?
                 ORDER BY 2 DESC, 4 DESC LIMIT 5",
            )?;
            stmt.query_map(params![to, to, to, to, from], |r| {
                Ok(HistoryTop {
                    artist: r.get(0)?,
                    plays: r.get::<_, i64>(1)? as usize,
                    listens: r.get::<_, i64>(2)? as usize,
                    seconds: r.get(3)?,
                    ..Default::default()
                })
            })?
            .collect::<rusqlite::Result<_>>()?
        } else {
            vec![]
        };

        let top_track_ids: Vec<String> = db
            .prepare(
                "SELECT track_id FROM listens WHERE started_at >= ? AND started_at < ?
                 GROUP BY track_id ORDER BY sum(qualified) DESC, sum(seconds) DESC, track_id LIMIT 25",
            )?
            .query_map(params![from, to], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?;

        let mut album_song = db.prepare(
            "SELECT id FROM tracks WHERE album = ? AND (album_artist = ? OR artist = ?) ORDER BY disc, track_number LIMIT 1",
        )?;
        let album_tracks: Vec<Option<String>> = stats
            .top_albums
            .iter()
            .map(|a| {
                album_song
                    .query_row(params![a.album, a.artist, a.artist], |r| r.get(0))
                    .ok()
            })
            .collect();
        let mut wanted: Vec<String> = top_track_ids.clone();
        wanted.extend(album_tracks.iter().flatten().cloned());
        let tracks = self
            .tracks_by_ids(&wanted)?
            .into_iter()
            .map(|t| (t.id.clone(), t))
            .collect();

        let busiest_day = stats
            .days
            .iter()
            .filter(|d| d.listens > 0)
            .max_by(|a, b| a.seconds.total_cmp(&b.seconds))
            .cloned();
        let peak_hour = stats
            .hours
            .iter()
            .filter(|h| h.listens > 0)
            .max_by(|a, b| a.seconds.total_cmp(&b.seconds))
            .map(|h| h.hour);
        Ok(Wrapped {
            year,
            streak: longest_streak(&stats.days),
            stats,
            genres,
            new_artists,
            busiest_day,
            peak_hour,
            top_track_ids,
            tracks,
            album_tracks,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Listen;

    fn listen(id: &str, track: &str, artist: &str, at: i64, seconds: f64) -> Listen {
        Listen {
            id: id.into(),
            track_id: track.into(),
            title: track.into(),
            artist: artist.into(),
            album: format!("{artist} album"),
            started_at: at,
            listened_seconds: seconds,
            duration: 100.,
            qualified: seconds >= 50.,
        }
    }
    fn at(y: i32, m: u32, d: u32, h: u32) -> i64 {
        Local
            .with_ymd_and_hms(y, m, d, h, 0, 0)
            .earliest()
            .unwrap()
            .timestamp()
    }

    #[test]
    fn a_year_told_back() {
        let dir = tempfile::tempdir().unwrap();
        let library = Library::open(dir.path()).unwrap();
        assert!(library.listening_years().unwrap().is_empty());
        for (id, genre) in [("a", "Pop; Dance"), ("b", "Jazz")] {
            let path = format!("{id}.flac");
            library
                .upsert(&Track {
                    id: id.into(),
                    path,
                    genre: genre.into(),
                    ..Default::default()
                })
                .unwrap();
        }
        for l in [
            // Last year: Old Band only.
            listen("0", "a", "Old Band", at(2023, 12, 30, 20), 80.),
            // This year: three days in a row, then a gap.
            listen("1", "a", "Old Band", at(2024, 3, 1, 21), 90.),
            listen("2", "b", "New Face", at(2024, 3, 2, 21), 100.),
            listen("3", "b", "New Face", at(2024, 3, 2, 22), 100.),
            listen("4", "b", "New Face", at(2024, 3, 3, 9), 60.),
            listen("5", "a", "Old Band", at(2024, 3, 9, 21), 10.),
            // Next year: not counted.
            listen("6", "a", "Later", at(2025, 1, 1, 1), 100.),
        ] {
            library.record_listen(&l).unwrap();
        }
        assert_eq!(library.listening_years().unwrap(), vec![2025, 2024, 2023]);
        let w = library.wrapped(2024).unwrap();
        assert_eq!(w.stats.listens, 5);
        assert_eq!(w.stats.top_artists[0].artist, "New Face");
        assert_eq!(w.top_track_ids, vec!["b", "a"]);
        assert!(w.tracks.contains_key("b"));
        assert_eq!(w.streak, 3);
        assert_eq!(w.busiest_day.as_ref().unwrap().date, "2024-03-02");
        assert_eq!(w.peak_hour, Some(21));
        assert_eq!(w.stats.days.last().unwrap().date, "2024-12-31");
        let genres: Vec<&str> = w.genres.iter().map(|g| g.0.as_str()).collect();
        assert_eq!(genres, vec!["Jazz", "Dance", "Pop"]);
        assert_eq!(w.new_artists.len(), 1);
        assert_eq!(w.new_artists[0].artist, "New Face");
        assert_eq!(w.new_artists[0].plays, 3);
        // The first year has nothing to compare with, so no one is "new".
        assert!(library.wrapped(2023).unwrap().new_artists.is_empty());
    }
}
