//! Listening-history browsing and aggregate statistics.
//!
//! SQLite aggregates over covering indexes (15-minute time buckets and per-track totals);
//! Rust then folds those into local days/hours (`chrono::Local`) and artist/album rankings.
//! A listen belongs to the local calendar day and hour in which it started.
use crate::{database::Library, model::Listen};
use anyhow::Result;
use chrono::{Local, NaiveDate, TimeZone, Timelike};
use rusqlite::{Connection, params};
use serde::Serialize;
use std::collections::{BTreeMap, HashMap};

/// One ranked artist, album, or track. Fields that do not apply are empty.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct HistoryTop {
    pub track_id: Option<String>,
    pub title: String,
    pub artist: String,
    pub album: String,
    /// Qualified plays (half the track or four minutes).
    pub plays: usize,
    /// Every recorded listen, including skips.
    pub listens: usize,
    pub seconds: f64,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct DayStat {
    /// Local calendar date, `YYYY-MM-DD`.
    pub date: String,
    pub seconds: f64,
    pub listens: usize,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct HourStat {
    /// Local hour of day, 0–23.
    pub hour: u8,
    pub seconds: f64,
    pub listens: usize,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct HistoryStats {
    pub since: Option<i64>,
    pub listens: usize,
    pub plays: usize,
    pub seconds: f64,
    pub distinct_tracks: usize,
    pub distinct_artists: usize,
    /// Top 10 each, ranked by qualified plays, then listening time.
    pub top_artists: Vec<HistoryTop>,
    pub top_albums: Vec<HistoryTop>,
    pub top_tracks: Vec<HistoryTop>,
    /// Every local date from `since` (or the first listen) to today, zero-filled.
    pub days: Vec<DayStat>,
    /// Always 24 entries, hour 0 first.
    pub hours: Vec<HourStat>,
}

pub(crate) fn migrate(db: &Connection) -> Result<()> {
    let has: bool = db.query_row(
        "SELECT EXISTS(SELECT 1 FROM pragma_table_xinfo('listens') WHERE name='seconds')",
        [],
        |r| r.get(0),
    )?;
    if !has {
        db.execute_batch("BEGIN;
            ALTER TABLE listens ADD COLUMN seconds REAL GENERATED ALWAYS AS (coalesce(json_extract(data,'$.listened_seconds'),0)) VIRTUAL;
            ALTER TABLE listens ADD COLUMN artist TEXT GENERATED ALWAYS AS (coalesce(json_extract(data,'$.artist'),'')) VIRTUAL;
            ALTER TABLE listens ADD COLUMN album TEXT GENERATED ALWAYS AS (coalesce(json_extract(data,'$.album'),'')) VIRTUAL;
            ALTER TABLE listens ADD COLUMN title TEXT GENERATED ALWAYS AS (coalesce(json_extract(data,'$.title'),'')) VIRTUAL;
            COMMIT;")?;
    }
    db.execute_batch("CREATE INDEX IF NOT EXISTS listens_range_stats ON listens(started_at,qualified,seconds,track_id,artist,album);
        CREATE INDEX IF NOT EXISTS listens_track_stats ON listens(track_id,started_at,qualified,seconds,artist,album,title);")?;
    Ok(())
}

impl Library {
    /// Newest first.
    pub fn history_page(&self, offset: usize, limit: usize) -> Result<Vec<Listen>> {
        let db = self.connection()?;
        let mut stmt = db.prepare(
            "SELECT data FROM listens ORDER BY started_at DESC, id DESC LIMIT ? OFFSET ?",
        )?;
        let json: Vec<String> = stmt
            .query_map(
                params![
                    limit.min(i64::MAX as usize) as i64,
                    offset.min(i64::MAX as usize) as i64
                ],
                |r| r.get(0),
            )?
            .collect::<std::result::Result<_, _>>()?;
        json.iter().map(|s| Ok(serde_json::from_str(s)?)).collect()
    }
    pub fn history_count(&self) -> Result<usize> {
        Ok(self
            .connection()?
            .query_row("SELECT count(*) FROM listens", [], |r| r.get::<_, i64>(0))?
            as usize)
    }
    /// Aggregates listens that started at or after `since` (Unix seconds), or all listens.
    pub fn history_stats(&self, since: Option<i64>) -> Result<HistoryStats> {
        self.history_stats_between(since, None)
    }
    /// The same for listens that started before `until` too.
    pub fn history_stats_between(
        &self,
        since: Option<i64>,
        until: Option<i64>,
    ) -> Result<HistoryStats> {
        let db = self.connection()?;
        db.execute_batch("PRAGMA temp_store=MEMORY;")?;
        let from = since.unwrap_or(i64::MIN);
        let to = until.unwrap_or(i64::MAX);
        let mut stats = HistoryStats {
            since,
            hours: (0..24)
                .map(|hour| HourStat {
                    hour,
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        };
        // UTC offsets and their transitions are multiples of 15 minutes, so each
        // 15-minute bucket of a time-ordered scan maps to one local day and hour.
        let mut stmt = db.prepare(
            "SELECT started_at, seconds, qualified FROM listens INDEXED BY listens_range_stats WHERE started_at >= ? AND started_at < ? ORDER BY started_at",
        )?;
        let mut days: BTreeMap<NaiveDate, (f64, usize)> = BTreeMap::new();
        let mut clock = LocalClock {
            day: i64::MIN,
            offset: None,
        };
        let mut bucket = None;
        let mut local = None;
        let mut rows = stmt.query([from, to])?;
        while let Some(row) = rows.next()? {
            let (started, seconds, qualified): (i64, f64, bool) =
                (row.get(0)?, row.get(1)?, row.get(2)?);
            stats.listens += 1;
            stats.plays += qualified as usize;
            stats.seconds += seconds;
            if bucket != Some(started.div_euclid(900)) {
                bucket = Some(started.div_euclid(900));
                local = clock.local(started.div_euclid(900) * 900);
            }
            if let Some(time) = local {
                let hour = &mut stats.hours[time.hour() as usize];
                hour.seconds += seconds;
                hour.listens += 1;
                let day = days.entry(time.date()).or_default();
                day.0 += seconds;
                day.1 += 1;
            }
        }
        let start = match since {
            Some(since) => clock.local(since).map(|t| t.date()),
            None => days.keys().next().copied(),
        };
        if let Some(mut date) = start {
            let today = Local::now().date_naive();
            let last = days.keys().next_back().copied();
            // Up to today, or to the end of a range that is already over.
            let end = match until.and_then(|u| clock.local(u - 1)).map(|t| t.date()) {
                Some(limit) => limit.min(today).max(last.unwrap_or(limit)),
                None => last.map_or(today, |last| last.max(today)),
            };
            while date <= end {
                let (seconds, listens) = days.get(&date).copied().unwrap_or_default();
                stats.days.push(DayStat {
                    date: date.format("%Y-%m-%d").to_string(),
                    seconds,
                    listens,
                });
                let Some(next) = date.succ_opt() else { break };
                date = next;
            }
        }
        // A short range is cheaper to sort than scanning every listen in track order.
        let index = if (since.is_some() || until.is_some()) && stats.listens < 200_000 {
            "listens_range_stats"
        } else {
            "listens_track_stats"
        };
        let mut stmt = db.prepare(&format!(
            "SELECT track_id, sum(qualified), count(*), sum(seconds), max(artist), max(album) FROM listens INDEXED BY {index} WHERE started_at >= ? AND started_at < ? GROUP BY track_id"
        ))?;
        type Totals = (usize, usize, f64);
        let add = |into: &mut Totals, from: Totals| {
            into.0 += from.0;
            into.1 += from.1;
            into.2 += from.2;
        };
        let mut tracks: Vec<HistoryTop> = vec![];
        let mut artists: HashMap<String, Totals> = HashMap::new();
        let mut albums: HashMap<String, HashMap<String, Totals>> = HashMap::new();
        let mut rows = stmt.query([from, to])?;
        while let Some(row) = rows.next()? {
            stats.distinct_tracks += 1;
            let totals: Totals = (
                row.get::<_, i64>(1)? as usize,
                row.get::<_, i64>(2)? as usize,
                row.get(3)?,
            );
            let artist = row.get_ref(4)?.as_str().unwrap_or_default();
            let album = row.get_ref(5)?.as_str().unwrap_or_default();
            if !artist.is_empty() {
                match artists.get_mut(artist) {
                    Some(into) => add(into, totals),
                    None => drop(artists.insert(artist.to_string(), totals)),
                }
            }
            if !album.is_empty() {
                let by_artist = match albums.get_mut(album) {
                    Some(by_artist) => by_artist,
                    None => albums.entry(album.to_string()).or_default(),
                };
                match by_artist.get_mut(artist) {
                    Some(into) => add(into, totals),
                    None => drop(by_artist.insert(artist.to_string(), totals)),
                }
            }
            if tracks.len() < 10
                || tracks
                    .iter()
                    .any(|t| (t.plays, t.seconds) < (totals.0, totals.2))
            {
                tracks.push(HistoryTop {
                    track_id: row.get(0)?,
                    artist: artist.into(),
                    album: album.into(),
                    plays: totals.0,
                    listens: totals.1,
                    seconds: totals.2,
                    ..Default::default()
                });
                if tracks.len() > 40 {
                    tracks = rank(tracks);
                }
            }
        }
        let mut folded: HashMap<String, HistoryTop> = HashMap::new();
        for (artist, totals) in artists {
            let top = folded
                .entry(artist.to_lowercase())
                .or_insert_with(|| HistoryTop {
                    artist,
                    ..Default::default()
                });
            top.plays += totals.0;
            top.listens += totals.1;
            top.seconds += totals.2;
        }
        stats.distinct_artists = folded.len();
        stats.top_artists = rank(folded.into_values().collect());
        let mut folded: HashMap<(String, String), HistoryTop> = HashMap::new();
        for (album, by_artist) in albums {
            for (artist, totals) in by_artist {
                let top = folded
                    .entry((album.to_lowercase(), artist.to_lowercase()))
                    .or_insert_with(|| HistoryTop {
                        album: album.clone(),
                        artist,
                        ..Default::default()
                    });
                top.plays += totals.0;
                top.listens += totals.1;
                top.seconds += totals.2;
            }
        }
        stats.top_albums = rank(folded.into_values().collect());
        stats.top_tracks = rank(tracks);
        let mut title = db.prepare(
            "SELECT title FROM listens INDEXED BY listens_track_stats WHERE track_id=? AND started_at >= ? AND started_at < ? ORDER BY started_at DESC LIMIT 1",
        )?;
        for top in &mut stats.top_tracks {
            top.title = title.query_row(params![top.track_id, from, to], |r| r.get(0))?;
        }
        Ok(stats)
    }
}

/// Converts UTC instants to local time, querying the zone once per UTC day unless
/// the offset changes during that day.
struct LocalClock {
    day: i64,
    offset: Option<i32>,
}
impl LocalClock {
    fn offset_at(time: i64) -> Option<i32> {
        Local
            .timestamp_opt(time, 0)
            .single()
            .map(|t| t.offset().local_minus_utc())
    }
    fn local(&mut self, time: i64) -> Option<chrono::NaiveDateTime> {
        let day = time.div_euclid(86400);
        if day != self.day {
            self.day = day;
            let start = Self::offset_at(day * 86400);
            self.offset = start.filter(|_| start == Self::offset_at(day * 86400 + 86399));
        }
        let offset = self.offset.or_else(|| Self::offset_at(time))?;
        chrono::DateTime::from_timestamp(time + offset as i64, 0).map(|t| t.naive_utc())
    }
}

/// Top ten by qualified plays, then listening time, then name.
fn rank(mut list: Vec<HistoryTop>) -> Vec<HistoryTop> {
    let order = |a: &HistoryTop, b: &HistoryTop| {
        b.plays
            .cmp(&a.plays)
            .then(b.seconds.total_cmp(&a.seconds))
            .then_with(|| {
                (&a.artist, &a.album, &a.track_id).cmp(&(&b.artist, &b.album, &b.track_id))
            })
    };
    if list.len() > 10 {
        list.select_nth_unstable_by(9, order);
        list.truncate(10);
    }
    list.sort_by(order);
    list
}

#[cfg(test)]
mod tests {
    use super::*;

    fn listen(id: &str, track: &str, artist: &str, album: &str, at: i64, seconds: f64) -> Listen {
        Listen {
            id: id.into(),
            track_id: track.into(),
            title: format!("{track} title"),
            artist: artist.into(),
            album: album.into(),
            started_at: at,
            listened_seconds: seconds,
            duration: 100.0,
            qualified: seconds >= 50.0,
        }
    }
    fn local(y: i32, m: u32, d: u32, h: u32) -> i64 {
        Local
            .with_ymd_and_hms(y, m, d, h, 30, 0)
            .earliest()
            .unwrap()
            .timestamp()
    }

    #[test]
    fn pages_are_newest_first_and_counted() {
        let dir = tempfile::tempdir().unwrap();
        let library = Library::open(dir.path()).unwrap();
        assert_eq!(library.history_count().unwrap(), 0);
        assert!(library.history_page(0, 10).unwrap().is_empty());
        for i in 0..25 {
            library
                .record_listen(&listen(&format!("l{i:02}"), "t", "A", "B", 1_000 + i, 80.0))
                .unwrap();
        }
        assert_eq!(library.history_count().unwrap(), 25);
        let first = library.history_page(0, 10).unwrap();
        assert_eq!(first.len(), 10);
        assert_eq!(first[0].id, "l24");
        assert_eq!(first[9].id, "l15");
        let last = library.history_page(20, 10).unwrap();
        assert_eq!(
            last.iter().map(|l| l.id.as_str()).collect::<Vec<_>>(),
            ["l04", "l03", "l02", "l01", "l00"]
        );
        assert!(library.history_page(25, 10).unwrap().is_empty());
        assert_eq!(library.history(3).unwrap()[0].id, "l24");
    }

    #[test]
    fn stats_aggregate_tops_days_and_hours() {
        let dir = tempfile::tempdir().unwrap();
        let library = Library::open(dir.path()).unwrap();
        let empty = library.history_stats(None).unwrap();
        assert_eq!(empty.listens, 0);
        assert!(empty.days.is_empty());
        assert_eq!(empty.hours.len(), 24);
        let day1 = local(2024, 3, 1, 9);
        let day3 = local(2024, 3, 3, 21);
        for l in [
            listen("1", "a1", "Alice", "One", day1, 100.0),
            listen("2", "a1", "Alice", "One", day1 + 200, 60.0),
            listen("3", "a2", "alice", "One", day1 + 400, 10.0),
            listen("4", "b1", "Bob", "Two", day3, 90.0),
            listen("5", "b1", "Bob", "Two", day3 + 100, 80.0),
            listen("6", "b1", "Bob", "Two", day3 + 200, 70.0),
            listen("7", "c1", "", "", day3 + 300, 5.0),
        ] {
            library.record_listen(&l).unwrap();
        }
        let stats = library.history_stats(None).unwrap();
        assert_eq!(stats.listens, 7);
        assert_eq!(stats.plays, 5);
        assert_eq!(stats.seconds, 415.0);
        assert_eq!(stats.distinct_tracks, 4);
        assert_eq!(stats.distinct_artists, 2);
        assert_eq!(stats.top_artists.len(), 2);
        assert_eq!(stats.top_artists[0].artist, "Bob");
        assert_eq!(stats.top_artists[0].plays, 3);
        assert_eq!(stats.top_artists[1].listens, 3);
        assert_eq!(stats.top_artists[1].plays, 2);
        assert_eq!(stats.top_artists[1].seconds, 170.0);
        assert_eq!(stats.top_albums[0].album, "Two");
        assert_eq!(stats.top_albums[1].album, "One");
        assert_eq!(stats.top_tracks[0].track_id.as_deref(), Some("b1"));
        assert_eq!(stats.top_tracks[0].title, "b1 title");
        assert_eq!(stats.top_tracks[1].track_id.as_deref(), Some("a1"));
        assert_eq!(stats.top_tracks.len(), 4);
        assert_eq!(stats.days[0].date, "2024-03-01");
        assert_eq!(stats.days[0].listens, 3);
        assert_eq!(stats.days[0].seconds, 170.0);
        assert_eq!(stats.days[1].date, "2024-03-02");
        assert_eq!(stats.days[1].listens, 0);
        assert_eq!(stats.days[2].listens, 4);
        let today = Local::now().date_naive().format("%Y-%m-%d").to_string();
        assert_eq!(stats.days.last().unwrap().date, today);
        assert_eq!(stats.hours[9].listens, 3);
        assert_eq!(stats.hours[21].listens, 4);
        assert_eq!(stats.hours.iter().map(|h| h.listens).sum::<usize>(), 7);

        let recent = library.history_stats(Some(local(2024, 3, 2, 0))).unwrap();
        assert_eq!(recent.listens, 4);
        assert_eq!(recent.days[0].date, "2024-03-02");
        assert_eq!(recent.top_artists[0].artist, "Bob");
        assert_eq!(recent.top_artists.len(), 1);
        assert!(
            serde_json::to_string(&recent)
                .unwrap()
                .contains("\"top_tracks\"")
        );
        let future = library.history_stats(Some(i64::MAX / 2)).unwrap();
        assert_eq!(future.listens, 0);
    }

    #[test]
    fn migration_is_idempotent_and_sync_style_inserts_still_work() {
        let dir = tempfile::tempdir().unwrap();
        let library = Library::open(dir.path()).unwrap();
        drop(Library::open(dir.path()).unwrap());
        library
            .connection()
            .unwrap()
            .execute(
                "INSERT OR IGNORE INTO listens VALUES (?,?,?,?,?)",
                params![
                    "x",
                    "t",
                    5,
                    true,
                    serde_json::to_string(&listen("x", "t", "Z", "Y", 5, 60.0)).unwrap()
                ],
            )
            .unwrap();
        let stats = library.history_stats(None).unwrap();
        assert_eq!(stats.top_artists[0].artist, "Z");
        assert_eq!(stats.seconds, 60.0);
        let db = library.connection().unwrap();
        let mut stmt = db
            .prepare("EXPLAIN QUERY PLAN SELECT sum(seconds), max(title), max(album), count(DISTINCT artist) FROM listens INDEXED BY listens_track_stats WHERE started_at >= 0")
            .unwrap();
        let plan = stmt
            .query_map([], |r| r.get::<_, String>(3))
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap()
            .join("; ");
        assert!(plan.contains("INDEX listens_track_stats"), "{plan}");
    }
}
