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
    /// The year's most played songs, most first (up to 50), for playing or saving.
    pub top_track_ids: Vec<String>,
    /// Each top song's plays that year, beside `top_track_ids`.
    pub top_track_plays: Vec<usize>,
    /// Songs first heard this year ("new songs found").
    pub new_songs: usize,
    /// January to December: plays, every listen, and seconds listened.
    pub months: Vec<MonthStat>,
    /// The months the top artist was among the five most played (out of `listening_months`).
    pub top_artist_months: usize,
    /// The months with any listening.
    pub listening_months: usize,
    /// Songs to show: the top songs, and one song from each top album for its cover.
    pub tracks: HashMap<String, Track>,
    /// For each of `stats.top_albums`, a song from it.
    pub album_tracks: Vec<Option<String>>,
}

/// One month of a year's listening.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MonthStat {
    /// 1 (January) to 12.
    pub month: u32,
    /// Qualified plays (half the song or four minutes).
    pub plays: usize,
    pub listens: usize,
    pub seconds: f64,
}

/// The year to open first: last year during January, when this one has barely begun.
pub fn default_year(years: &[i32], today: NaiveDate) -> i32 {
    let this = today.year();
    if today.month() == 1 && years.contains(&(this - 1)) {
        this - 1
    } else {
        this
    }
}

/// The year Home shows a "Your year in music" shelf for: this one, in December.
pub fn home_year(today: NaiveDate) -> Option<i32> {
    (today.month() == 12).then(|| today.year())
}

/// The name a year's top songs are saved under.
pub fn playlist_name(year: i32) -> String {
    format!("Top songs of {year}")
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

        let top: Vec<(String, i64)> = db
            .prepare(
                "SELECT track_id, sum(qualified) FROM listens WHERE started_at >= ? AND started_at < ?
                 GROUP BY track_id ORDER BY sum(qualified) DESC, sum(seconds) DESC, track_id LIMIT 50",
            )?
            .query_map(params![from, to], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?;
        let top_track_plays = top.iter().map(|(_, plays)| *plays as usize).collect();
        let top_track_ids: Vec<String> = top.into_iter().map(|(id, _)| id).collect();

        let new_songs: i64 = db.query_row(
            "SELECT count(*) FROM (SELECT min(started_at) AS first FROM listens GROUP BY track_id)
             WHERE first >= ? AND first < ?",
            params![from, to],
            |r| r.get(0),
        )?;

        // By month, and each month's artists, in local time.
        let mut months: Vec<MonthStat> = (1..=12)
            .map(|month| MonthStat {
                month,
                ..Default::default()
            })
            .collect();
        let mut by_month: HashMap<u32, Vec<(String, i64, f64)>> = HashMap::new();
        let mut stmt = db.prepare(
            "SELECT CAST(strftime('%m', started_at, 'unixepoch', 'localtime') AS INTEGER),
                    lower(artist), sum(qualified), count(*), sum(seconds)
             FROM listens WHERE started_at >= ? AND started_at < ? GROUP BY 1, 2",
        )?;
        let mut rows = stmt.query(params![from, to])?;
        while let Some(row) = rows.next()? {
            let month: u32 = row.get(0)?;
            let (artist, plays, listens, seconds): (String, i64, i64, f64) =
                (row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?);
            if let Some(m) = months.get_mut(month.wrapping_sub(1) as usize) {
                m.plays += plays as usize;
                m.listens += listens as usize;
                m.seconds += seconds;
            }
            if !artist.is_empty() {
                by_month
                    .entry(month)
                    .or_default()
                    .push((artist, plays, seconds));
            }
        }
        let listening_months = months.iter().filter(|m| m.listens > 0).count();
        let top_artist_months = stats.top_artists.first().map_or(0, |top| {
            let top = top.artist.to_lowercase();
            by_month
                .values_mut()
                .map(|artists| {
                    artists.sort_by(|a, b| b.1.cmp(&a.1).then(b.2.total_cmp(&a.2)));
                    artists.iter().take(5).any(|a| a.0 == top)
                })
                .filter(|in_top| *in_top)
                .count()
        });

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
            top_track_plays,
            new_songs: new_songs as usize,
            months,
            top_artist_months,
            listening_months,
            tracks,
            album_tracks,
        })
    }

    /// Save the year's top songs (up to 50) as a playlist, "Top songs of 2026"; saving again
    /// refreshes the same playlist. Returns it.
    pub fn save_year_playlist(&self, year: i32) -> Result<crate::model::Playlist> {
        let wrapped = self.wrapped(year)?;
        let name = playlist_name(year);
        let id = self
            .playlists()?
            .into_iter()
            .find(|p| p.name == name && p.query.is_none())
            .map(|p| p.id)
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        let playlist = crate::model::Playlist {
            id,
            name,
            query: None,
            track_ids: wrapped
                .top_track_ids
                .into_iter()
                .filter(|id| wrapped.tracks.contains_key(id))
                .collect(),
            updated_at: chrono::Utc::now().timestamp(),
            ..Default::default()
        };
        self.save_playlist(&playlist)?;
        Ok(playlist)
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
        // Song b was first heard in 2024; a in 2023.
        assert_eq!(w.new_songs, 1);
        assert_eq!(w.top_track_plays, vec![3, 1]);
        assert_eq!(w.months.len(), 12);
        assert_eq!(w.months[2].listens, 5);
        assert_eq!(w.months[2].plays, 4);
        assert!((w.months[2].seconds - 360.).abs() < 1e-6);
        assert_eq!(w.listening_months, 1);
        assert_eq!(w.top_artist_months, 1);
        let saved = library.save_year_playlist(2024).unwrap();
        assert_eq!(saved.name, "Top songs of 2024");
        assert_eq!(saved.track_ids, vec!["b", "a"]);
        // Again: the same playlist, refreshed.
        let again = library.save_year_playlist(2024).unwrap();
        assert_eq!(again.id, saved.id);
        assert_eq!(library.playlists().unwrap().len(), 1);
    }
}
