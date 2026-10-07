//! More of Needle for the phone: favorites, folders, history and its numbers, Wrapped,
//! playlists to make and change, radio, and the A-B loop.
use crate::{Needle, NeedleError, Result, Song, log};
use needle_core::{
    audio::{Command, QueueItem},
    model::Playlist,
    query,
};
use std::sync::atomic::Ordering;

#[derive(Clone, uniffi::Record)]
pub struct Folder {
    pub path: String,
    pub name: String,
    /// Songs in it and in its own folders.
    pub songs: u32,
    pub artwork: Option<String>,
}

#[derive(Clone, uniffi::Record)]
pub struct HistoryItem {
    /// The song, while it is still in the library.
    pub song: Option<Song>,
    pub title: String,
    pub artist: String,
    pub album: String,
    /// Unix seconds.
    pub started_at: i64,
    pub listened: f64,
    /// Played past half (or four minutes): it counts as a play.
    pub counted: bool,
}

#[derive(Clone, uniffi::Record)]
pub struct Ranked {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub plays: u32,
    pub seconds: f64,
    pub song_id: Option<String>,
    pub artwork: Option<String>,
}

#[derive(Clone, uniffi::Record)]
pub struct Day {
    /// `YYYY-MM-DD`, local.
    pub date: String,
    pub seconds: f64,
}

#[derive(Clone, uniffi::Record)]
pub struct Stats {
    pub listens: u32,
    pub plays: u32,
    pub seconds: f64,
    pub songs: u32,
    pub artists: u32,
    pub top_artists: Vec<Ranked>,
    pub top_albums: Vec<Ranked>,
    pub top_songs: Vec<Ranked>,
    pub days: Vec<Day>,
    /// Seconds listened in each hour of the day, 0 to 23.
    pub hours: Vec<f64>,
}

#[derive(Clone, uniffi::Record)]
pub struct Wrapped {
    pub year: i32,
    pub stats: Stats,
    /// Genres by listening time, most first.
    pub genres: Vec<String>,
    pub new_artists: Vec<String>,
    pub busiest_day: Option<Day>,
    pub streak: u32,
    pub peak_hour: Option<u8>,
    /// The year's most played songs, most first.
    pub top_songs: Vec<Song>,
}

#[derive(Clone, uniffi::Record)]
pub struct PlaylistDetail {
    pub id: String,
    pub name: String,
    pub description: String,
    /// The rule of a smart playlist, as typed in search.
    pub rule: Option<String>,
    pub songs: Vec<Song>,
    /// Stored positions of visible occurrences, including repeated songs.
    pub positions: Vec<u32>,
}

fn stats(s: needle_core::history::HistoryStats, library: &needle_core::database::Library) -> Stats {
    let artwork = |id: &Option<String>| {
        id.as_ref()
            .and_then(|id| library.track(id).ok().flatten())
            .and_then(|t| t.artwork)
    };
    let ranked = |list: Vec<needle_core::history::HistoryTop>| {
        list.into_iter()
            .map(|t| Ranked {
                artwork: artwork(&t.track_id),
                title: t.title,
                artist: t.artist,
                album: t.album,
                plays: t.plays as u32,
                seconds: t.seconds,
                song_id: t.track_id,
            })
            .collect()
    };
    Stats {
        listens: s.listens as u32,
        plays: s.plays as u32,
        seconds: s.seconds,
        songs: s.distinct_tracks as u32,
        artists: s.distinct_artists as u32,
        top_artists: ranked(s.top_artists),
        top_albums: ranked(s.top_albums),
        top_songs: ranked(s.top_tracks),
        days: s
            .days
            .into_iter()
            .map(|d| Day {
                date: d.date,
                seconds: d.seconds,
            })
            .collect(),
        hours: s.hours.into_iter().map(|h| h.seconds).collect(),
    }
}

impl Needle {
    fn songs_matching(&self, rule: &str) -> Result<Vec<Song>> {
        Ok(self
            .library
            .search(rule)?
            .iter()
            .filter(|t| !t.missing)
            .map(Song::from)
            .collect())
    }

    pub(crate) fn playlist(&self, id: &str) -> Result<Playlist> {
        self.library
            .playlist_by_id(id)?
            .ok_or_else(|| NeedleError::Failed("That playlist is gone".into()))
    }
}

#[uniffi::export]
impl Needle {
    // ---------- Favorites and ratings

    /// Sets a song's stars, 0 to 5 (4 and 5 make a favorite).
    pub fn rate(&self, id: String, stars: u8) -> Result<()> {
        let stars = stars.min(5) as i64;
        self.library.rate(&id, stars)?;
        self.plugins.send(needle_core::plugins::PluginEvent::Rated {
            track_id: id,
            stars,
        });
        Ok(())
    }

    /// Makes a song a favorite (five stars), or takes it out of them.
    pub fn toggle_favorite(&self, id: String) -> Result<bool> {
        let rating = self.library.track(&id)?.map_or(0, |t| t.rating);
        let favorite = rating < 4;
        self.rate(id, if favorite { 5 } else { 0 })?;
        Ok(favorite)
    }

    /// A song's stars now (0 to 5); the song a list or the player holds may be older.
    pub fn rating(&self, id: String) -> u8 {
        self.library
            .track(&id)
            .ok()
            .flatten()
            .map_or(0, |t| t.rating.clamp(0, 5) as u8)
    }

    pub fn is_favorite(&self, id: String) -> bool {
        self.library
            .track(&id)
            .ok()
            .flatten()
            .is_some_and(|t| t.rating >= 4)
    }

    pub fn favorites(&self) -> Result<Vec<Song>> {
        self.songs_matching("rating >= 4 order by title")
    }

    /// Songs added in the last 30 days, newest first.
    pub fn recently_added(&self) -> Result<Vec<Song>> {
        self.songs_matching("recent(30d) order by added_at desc")
    }

    // ---------- Folders

    /// The folders inside `path` (a music folder or one of its folders).
    pub fn subfolders(&self, path: String) -> Result<Vec<Folder>> {
        Ok(self
            .library
            .subfolders(&path)?
            .into_iter()
            .map(|f| Folder {
                path: f.path,
                name: f.name,
                songs: f.tracks as u32,
                artwork: f.artwork,
            })
            .collect())
    }

    /// Every song in a folder and its own folders, by path.
    pub fn folder_songs(&self, path: String) -> Result<Vec<Song>> {
        let prefix = format!("{}/", path.trim_end_matches('/'));
        self.songs_matching(&format!(
            "path starts with {} order by path",
            query::quote(&prefix)
        ))
    }

    // ---------- History, numbers, and Wrapped

    pub fn history(&self, offset: u32, limit: u32) -> Result<Vec<HistoryItem>> {
        let listens = self
            .library
            .history_page(offset as usize, limit.min(500) as usize)?;
        let ids: Vec<String> = listens.iter().map(|l| l.track_id.clone()).collect();
        let tracks = self.library.tracks_by_ids(&ids)?;
        Ok(listens
            .into_iter()
            .map(|l| HistoryItem {
                song: tracks.iter().find(|t| t.id == l.track_id).map(Song::from),
                title: l.title,
                artist: l.artist,
                album: l.album,
                started_at: l.started_at,
                listened: l.listened_seconds,
                counted: l.qualified,
            })
            .collect())
    }

    /// Listening numbers over the last `days` days, or all time with none.
    pub fn stats(&self, days: Option<u32>) -> Result<Stats> {
        let since = days.map(|d| chrono::Utc::now().timestamp() - d as i64 * 86_400);
        Ok(stats(self.library.history_stats(since)?, &self.library))
    }

    pub fn wrapped_years(&self) -> Result<Vec<i32>> {
        Ok(self.library.listening_years()?)
    }

    pub fn wrapped(&self, year: i32) -> Result<Wrapped> {
        let w = self.library.wrapped(year)?;
        let top_songs = w
            .top_track_ids
            .iter()
            .filter_map(|id| w.tracks.get(id))
            .map(Song::from)
            .collect();
        Ok(Wrapped {
            year: w.year,
            stats: stats(w.stats, &self.library),
            genres: w.genres.into_iter().map(|(g, _)| g).collect(),
            new_artists: w.new_artists.into_iter().map(|a| a.artist).collect(),
            busiest_day: w.busiest_day.map(|d| Day {
                date: d.date,
                seconds: d.seconds,
            }),
            streak: w.streak as u32,
            peak_hour: w.peak_hour,
            top_songs,
        })
    }

    // ---------- Playlists

    pub fn playlist_detail(&self, id: String) -> Result<PlaylistDetail> {
        let playlist = self.playlist(&id)?;
        let (songs, positions) = if playlist.query.is_some() || playlist.rules.is_some() {
            let songs: Vec<_> = self
                .library
                .playlist_tracks(&playlist)?
                .iter()
                .filter(|t| !t.missing)
                .map(Song::from)
                .collect();
            let positions = (0..songs.len() as u32).collect();
            (songs, positions)
        } else {
            let tracks: std::collections::HashMap<_, _> = self
                .library
                .tracks_by_ids(&playlist.track_ids)?
                .into_iter()
                .map(|t| (t.id.clone(), t))
                .collect();
            playlist
                .track_ids
                .iter()
                .enumerate()
                .filter_map(|(i, id)| {
                    tracks
                        .get(id)
                        .filter(|t| !t.missing)
                        .map(|t| (Song::from(t), i as u32))
                })
                .unzip()
        };
        Ok(PlaylistDetail {
            id: playlist.id,
            name: playlist.name,
            description: playlist.description,
            rule: playlist.query,
            songs,
            positions,
        })
    }

    /// Makes a playlist: of `songs`, or with `rule` a smart one. Returns its id.
    pub fn create_playlist(
        &self,
        name: String,
        description: String,
        songs: Vec<String>,
        rule: Option<String>,
    ) -> Result<String> {
        let playlist = Playlist {
            id: uuid::Uuid::new_v4().to_string(),
            name: name.trim().to_string(),
            description: description.trim().to_string(),
            query: rule.map(|r| r.trim().to_string()).filter(|r| !r.is_empty()),
            track_ids: songs,
            updated_at: chrono::Utc::now().timestamp(),
            ..Default::default()
        };
        self.library.save_playlist(&playlist)?;
        Ok(playlist.id)
    }

    /// Renames a playlist, changes its words, or (for a smart one) its rule.
    pub fn edit_playlist(
        &self,
        id: String,
        name: String,
        description: String,
        rule: Option<String>,
    ) -> Result<()> {
        let mut playlist = self.playlist(&id)?;
        playlist.name = name.trim().to_string();
        playlist.description = description.trim().to_string();
        if playlist.query.is_some() {
            playlist.query = rule.map(|r| r.trim().to_string()).filter(|r| !r.is_empty());
            playlist.rules = None;
        }
        playlist.updated_at = chrono::Utc::now().timestamp();
        Ok(self.library.save_playlist(&playlist)?)
    }

    pub fn add_to_playlist(&self, id: String, songs: Vec<String>) -> Result<()> {
        let mut playlist = self.playlist(&id)?;
        if playlist.query.is_some() {
            return Err(NeedleError::Failed(
                "A smart playlist picks its own songs".into(),
            ));
        }
        playlist.track_ids.extend(songs);
        playlist.updated_at = chrono::Utc::now().timestamp();
        Ok(self.library.save_playlist(&playlist)?)
    }

    pub fn remove_from_playlist(&self, id: String, index: u32) -> Result<()> {
        let mut playlist = self.playlist(&id)?;
        if (index as usize) < playlist.track_ids.len() {
            playlist.track_ids.remove(index as usize);
            playlist.updated_at = chrono::Utc::now().timestamp();
            self.library.save_playlist(&playlist)?;
        }
        Ok(())
    }

    pub fn move_in_playlist(&self, id: String, from: u32, to: u32) -> Result<()> {
        let mut playlist = self.playlist(&id)?;
        let (from, to) = (from as usize, to as usize);
        if from < playlist.track_ids.len() && to < playlist.track_ids.len() {
            let song = playlist.track_ids.remove(from);
            playlist.track_ids.insert(to, song);
            playlist.updated_at = chrono::Utc::now().timestamp();
            self.library.save_playlist(&playlist)?;
        }
        Ok(())
    }

    pub fn delete_playlist(&self, id: String) -> Result<()> {
        Ok(self.library.delete_playlist(&id)?)
    }

    /// Whether a smart playlist's rule is one Needle understands (`None`), or what is wrong.
    pub fn check_rule(&self, rule: String) -> Option<String> {
        query::compile(&rule, chrono::Utc::now().timestamp())
            .err()
            .map(|e| format!("{e:#}"))
    }

    // ---------- Radio and the loop

    /// Plays a station that sounds like this song: it first, then songs that flow from it.
    pub fn start_radio(&self, id: String) -> Result<()> {
        let Some(track) = self.library.track(&id)? else {
            return Ok(());
        };
        let (library, player) = (self.library.clone(), self.player.clone());
        self.measuring.store(true, Ordering::SeqCst);
        let measuring = self.measuring.clone();
        std::thread::spawn(move || {
            let station = library.radio(std::slice::from_ref(&track), 49);
            measuring.store(false, Ordering::SeqCst);
            match station {
                Ok(songs) => {
                    let items = std::iter::once(track)
                        .chain(songs)
                        .map(|track| QueueItem {
                            track,
                            reason: "Radio".into(),
                        })
                        .collect();
                    player.send(Command::Play(items));
                }
                Err(e) => log(true, &format!("Radio: {e:#}")),
            }
        });
        self.measure_in_background();
        Ok(())
    }

    /// A station from an artist's songs, with others that sound like them.
    pub fn start_artist_radio(&self, name: String) {
        let (library, player) = (self.library.clone(), self.player.clone());
        std::thread::spawn(move || match library.artist_radio(&name, 50) {
            Ok(songs) => {
                let items = songs
                    .into_iter()
                    .map(|track| QueueItem {
                        track,
                        reason: "Radio".into(),
                    })
                    .collect();
                player.send(Command::Play(items));
            }
            Err(e) => log(true, &format!("Artist radio: {e:#}")),
        });
        self.measure_in_background();
    }

    /// How many songs are measured for radio, of how many.
    pub fn measured(&self) -> Vec<u32> {
        let (done, all) = self.library.measured_count().unwrap_or((0, 0));
        vec![done as u32, all as u32]
    }

    /// Measures how songs sound, in the background, a few at a time, for radio.
    pub fn measure_in_background(&self) {
        if self.analysing.swap(true, Ordering::SeqCst) {
            return;
        }
        let (library, analysing) = (self.library.clone(), self.analysing.clone());
        std::thread::spawn(move || {
            loop {
                let batch = library.unmeasured(20).unwrap_or_default();
                if batch.is_empty() {
                    break;
                }
                for track in batch {
                    // One that cannot be measured is stored as such, so it is not tried again.
                    let features = needle_core::radio::analyze(&track).unwrap_or(
                        needle_core::radio::Features {
                            version: needle_core::radio::VERSION,
                            ..Default::default()
                        },
                    );
                    let _ = library.save_features(&track.id, &features);
                }
            }
            analysing.store(false, Ordering::SeqCst);
        });
    }

    /// Repeats the stretch from `start` to `end` seconds of the song playing; `None` stops it.
    pub fn set_loop(&self, start: Option<f64>, end: Option<f64>) {
        let range = start.zip(end).filter(|(a, b)| b > a);
        self.player.send(Command::Loop(range));
    }

    pub fn loop_range(&self) -> Vec<f64> {
        self.player
            .state()
            .loop_range
            .map_or(vec![], |(a, b)| vec![a, b])
    }
}
