//! The library tools, as on desktop: tag editing and its backups, Fix my library (duplicates,
//! missing covers, album tags from MusicBrainz, identify by sound, tidy files), importing from
//! other players, songs from a music server kept on the phone, speaker groups, playlist
//! pictures and export, rule suggestions in search, a backup of the library, and theme files.
use crate::{Needle, NeedleError, Result, Song};
use needle_core::{
    audio::Command,
    cast::{self, Group, Speaker},
    doctor, import,
    model::Track,
    query,
    scan::{self, TagEdit},
    sources,
};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, atomic::AtomicBool},
};

fn failed(text: impl Into<String>) -> NeedleError {
    NeedleError::Failed(text.into())
}

#[derive(Clone, uniffi::Record)]
pub struct SongTags {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub album_artist: String,
    pub genre: String,
    /// 0 when it has none.
    pub year: u32,
    /// 0 when it has none.
    pub track_number: u32,
}

/// What to change; a field left `None` stays as each song has it.
#[derive(Clone, uniffi::Record)]
pub struct TagChange {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub album_artist: Option<String>,
    pub genre: Option<String>,
    pub year: Option<u32>,
    pub track_number: Option<u32>,
}

#[derive(Clone, uniffi::Record)]
pub struct TagReport {
    pub saved: u32,
    /// "file: why", for each song that could not be changed.
    pub failed: Vec<String>,
}

#[derive(Clone, uniffi::Record)]
pub struct TagBackupInfo {
    /// When it was made (Unix seconds); also what picks it for [`Needle::restore_tags`].
    pub created_at: i64,
    pub size: u64,
}

#[derive(Clone, uniffi::Record)]
pub struct DuplicateSet {
    pub songs: Vec<Song>,
    /// The copy Needle would keep (the best quality), by its place in `songs`.
    pub keep: u32,
    /// Each song's file, beside `songs`, so the phone can move the others to its trash.
    pub paths: Vec<String>,
}

#[derive(Clone, uniffi::Record)]
pub struct AlbumProblem {
    pub album: String,
    pub artist: String,
    pub song_ids: Vec<String>,
    pub problems: Vec<String>,
}

#[derive(Clone, uniffi::Record)]
pub struct ReleaseChoice {
    pub id: String,
    pub title: String,
    pub artist: String,
    pub year: u32,
    pub country: String,
    pub format: String,
    pub tracks: u32,
    /// How many of the album's songs it finds a track for.
    pub matched: u32,
}

#[derive(Clone, uniffi::Record)]
pub struct SoundMatch {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub score: u32,
    pub recording_id: String,
}

#[derive(Clone, uniffi::Record)]
pub struct TidyPlan {
    pub moves: u32,
    pub in_place: u32,
    pub skipped: u32,
    /// A few of the moves, "from → to", to show what will happen.
    pub examples: Vec<String>,
}

#[derive(Clone, uniffi::Record)]
pub struct FileFailure {
    pub path: String,
    pub reason: String,
}

#[derive(Clone, uniffi::Record)]
pub struct TidyResult {
    pub moved: u32,
    pub failures: Vec<FileFailure>,
}

#[derive(Clone, uniffi::Record)]
pub struct Suggestion {
    /// The part of the text to replace, in UTF-16 units (Kotlin's string positions).
    pub start: u32,
    pub end: u32,
    pub insert: String,
    pub label: String,
    pub detail: String,
}

#[derive(Clone, uniffi::Record)]
pub struct Speakers {
    pub all: Vec<crate::media::Output>,
    /// The speakers playing now, by id; with `this_phone`, the phone plays too.
    pub chosen: Vec<String>,
    pub this_phone: bool,
    /// Extra delay in ms by speaker id ("" for this phone).
    pub delays: HashMap<String, i32>,
}

/// Releases found for an album, kept until one is chosen.
static RELEASES: Mutex<Vec<(doctor::Release, Vec<Track>)>> = Mutex::new(Vec::new());
/// The last tidy plan, carried out as it was shown.
static PLAN: Mutex<Option<doctor::Plan>> = Mutex::new(None);

fn utf16(text: &str, byte: usize) -> u32 {
    text[..byte.min(text.len())].encode_utf16().count() as u32
}

fn byte_at(text: &str, units: u32) -> usize {
    let mut count = 0u32;
    for (i, c) in text.char_indices() {
        if count >= units {
            return i;
        }
        count += c.len_utf16() as u32;
    }
    text.len()
}

impl Needle {
    fn song_track(&self, id: &str) -> Result<Track> {
        self.library
            .track(id)?
            .ok_or_else(|| failed("That song is no longer in the library"))
    }

    fn tracks(&self, ids: &[String]) -> Result<Vec<Track>> {
        Ok(self.library.tracks_by_ids(ids)?)
    }

    fn all_tracks(&self) -> Vec<Track> {
        self.library.search("").unwrap_or_default()
    }
}

#[uniffi::export]
impl Needle {
    /// App-owned copies need no shared-storage grant.
    pub fn songs_are_private(&self, ids: Vec<String>) -> Result<bool> {
        let Some(folder) = self.library.directory.parent() else {
            return Ok(false);
        };
        // Android exposes aliases such as /data/user/0 and /data/data. Imported
        // track paths are canonical, so resolve the app folder the same way.
        let folder = folder
            .canonicalize()
            .unwrap_or_else(|_| folder.to_path_buf());
        let tracks = self.library.tracks_by_ids(&ids)?;
        Ok(!ids.is_empty()
            && tracks.len() == ids.len()
            && tracks
                .iter()
                .all(|t| Path::new(t.audio_path()).starts_with(&folder)))
    }
    // ---------- Tags

    pub fn song_tags(&self, id: String) -> Result<SongTags> {
        let t = self.song_track(&id)?;
        Ok(SongTags {
            title: t.title,
            artist: t.artist,
            album: t.album,
            album_artist: t.album_artist,
            genre: t.genre,
            year: t.year.max(0) as u32,
            track_number: t.track_number.max(0) as u32,
        })
    }

    /// Writes tags into the songs' files (music on a server is left alone). Each file is
    /// backed up first and its sound checked after, as on desktop.
    pub fn edit_tags(&self, ids: Vec<String>, change: TagChange) -> Result<TagReport> {
        let edit = TagEdit {
            title: change.title,
            artist: change.artist,
            album: change.album,
            genre: change.genre,
            year: change.year,
            musicbrainz_id: None,
            album_artist: change.album_artist,
            track_number: change.track_number,
        };
        let report = scan::write_tags_batch(
            &self.library,
            &ids,
            &edit,
            Arc::new(AtomicBool::new(false)),
            |_| {},
        )?;
        Ok(TagReport {
            saved: report.saved.len() as u32,
            failed: report
                .failed
                .into_iter()
                .map(|f| format!("{:?}", f))
                .collect(),
        })
    }

    /// Earlier versions of a song's file, newest first.
    pub fn tag_backups(&self, id: String) -> Result<Vec<TagBackupInfo>> {
        Ok(scan::tag_backups(&self.library, &id)?
            .into_iter()
            .map(|b| TagBackupInfo {
                created_at: b.created_at,
                size: b.size,
            })
            .collect())
    }

    /// Puts back the tags of the backup made at `created_at` (the file now is backed up too).
    pub fn restore_tags(&self, id: String, created_at: i64) -> Result<()> {
        let backup = scan::tag_backups(&self.library, &id)?
            .into_iter()
            .find(|b| b.created_at == created_at)
            .ok_or_else(|| failed("That backup is gone"))?;
        Ok(scan::restore_tag_backup(&self.library, &id, &backup)?)
    }

    // ---------- Fix my library

    pub fn duplicates(&self) -> Vec<DuplicateSet> {
        doctor::find_duplicates(&self.all_tracks())
            .into_iter()
            .map(|g| DuplicateSet {
                songs: g.tracks.iter().map(Song::from).collect(),
                paths: g.tracks.iter().map(|t| t.path.clone()).collect(),
                keep: g.keep as u32,
            })
            .collect()
    }

    /// Keeps `keep`: the others' plays, stars, and playlist places move to it, and they leave
    /// the library. Their files stay; the app offers to move them to the phone's trash.
    pub fn merge_duplicates(&self, keep: String, others: Vec<String>) -> Result<()> {
        let keep = self.song_track(&keep)?;
        let others = self.tracks(&others)?;
        Ok(self.library.merge_duplicates(&keep, &others, false)?)
    }

    /// Albums where no song has a cover.
    pub fn albums_without_covers(&self) -> Vec<Song> {
        self.library
            .albums_without_covers()
            .unwrap_or_default()
            .iter()
            .map(Song::from)
            .collect()
    }

    /// Looks up covers for the albums in `ids` (one song of each) on the Cover Art Archive.
    /// Returns how many albums got one.
    pub fn find_covers(&self, ids: Vec<String>) -> u32 {
        let mut found = 0;
        for track in self.tracks(&ids).unwrap_or_default() {
            if needle_core::media::fetch_album_art(&self.library, &track).unwrap_or(0) > 0 {
                found += 1;
            }
        }
        found
    }

    /// Albums missing a year or track numbers, or with mixed artists and no album artist.
    pub fn album_problems(&self) -> Vec<AlbumProblem> {
        doctor::album_issues(&self.all_tracks())
            .into_iter()
            .map(|a| AlbumProblem {
                album: a.album,
                artist: a.artist,
                song_ids: a.track_ids,
                problems: a.problems.into_iter().map(String::from).collect(),
            })
            .collect()
    }

    /// Releases on MusicBrainz that may be this album (its songs `ids`).
    pub fn find_releases(&self, ids: Vec<String>) -> Result<Vec<ReleaseChoice>> {
        let tracks = self.tracks(&ids)?;
        let first = tracks.first().ok_or_else(|| failed("No songs"))?;
        let releases =
            doctor::find_releases(&first.album, first.display_album_artist(), tracks.len())?;
        let choices = releases
            .iter()
            .map(|r| ReleaseChoice {
                id: r.id.clone(),
                title: r.title.clone(),
                artist: r.artist.clone(),
                year: r.year.unwrap_or(0),
                country: r.country.clone(),
                format: r.format.clone(),
                tracks: r.tracks.len() as u32,
                matched: doctor::match_tracks(&tracks, r)
                    .iter()
                    .filter(|m| m.is_some())
                    .count() as u32,
            })
            .collect();
        *RELEASES.lock().unwrap() = releases.into_iter().map(|r| (r, tracks.clone())).collect();
        Ok(choices)
    }

    /// Writes the chosen release's names, numbers, and year into the album's songs.
    pub fn apply_release(&self, release_id: String) -> Result<TagReport> {
        let (release, tracks) = RELEASES
            .lock()
            .unwrap()
            .iter()
            .find(|(r, _)| r.id == release_id)
            .cloned()
            .ok_or_else(|| failed("Look the album up again"))?;
        let mut report = TagReport {
            saved: 0,
            failed: vec![],
        };
        for (id, edit) in doctor::release_edits(&tracks, &release) {
            match scan::write_tags(&self.library, &id, &edit) {
                Ok(()) => report.saved += 1,
                Err(e) => report.failed.push(format!("{e:#}")),
            }
        }
        Ok(report)
    }

    /// Whether an AcoustID key is set, for identifying songs by their sound.
    pub fn has_acoustid_key(&self) -> bool {
        needle_core::integrations::acoustid_key().is_some()
    }

    pub fn set_acoustid_key(&self, key: String) -> Result<()> {
        Ok(needle_core::integrations::save_secret(
            needle_core::integrations::SecretKind::AcoustidKey,
            key.trim(),
        )?)
    }

    /// What a song is, found from its sound (AcoustID, then MusicBrainz).
    pub fn identify(&self, id: String) -> Result<Vec<SoundMatch>> {
        let track = self.song_track(&id)?;
        let key = needle_core::integrations::acoustid_key()
            .ok_or_else(|| failed("Add an AcoustID key first"))?;
        Ok(
            needle_core::integrations::acoustid_lookup(
                &self.library,
                Path::new(&track.path),
                &key,
            )?
            .into_iter()
            .map(|m| SoundMatch {
                title: m.title,
                artist: m.artist,
                album: m.album,
                score: m.score.clamp(0, 100) as u32,
                recording_id: m.id,
            })
            .collect(),
        )
    }

    /// Writes an identified match's names into the song.
    pub fn apply_match(&self, id: String, found: SoundMatch) -> Result<()> {
        let edit = TagEdit {
            title: Some(found.title),
            artist: Some(found.artist),
            album: (!found.album.is_empty()).then_some(found.album),
            musicbrainz_id: uuid::Uuid::parse_str(&found.recording_id)
                .ok()
                .map(|_| found.recording_id),
            ..Default::default()
        };
        Ok(scan::write_tags(&self.library, &id, &edit)?)
    }

    /// The ways files can be laid out, for [`Needle::plan_tidy`].
    pub fn tidy_patterns(&self) -> Vec<String> {
        doctor::PATTERNS.iter().map(|p| p.to_string()).collect()
    }

    /// Where the files would go under `pattern`, inside their music folders.
    pub fn plan_tidy(&self, pattern: String) -> TidyPlan {
        let roots = self.library.roots().unwrap_or_default();
        let plan = doctor::plan_organize(&self.all_tracks(), &roots, &pattern);
        let name = |p: &str| {
            let root = roots
                .iter()
                .find(|r| p.starts_with(r.as_str()))
                .map_or(0, |r| r.len());
            p[root..].trim_start_matches('/').to_string()
        };
        let shown = TidyPlan {
            moves: plan.moves.len() as u32,
            in_place: plan.in_place as u32,
            skipped: plan.skipped.len() as u32,
            examples: plan
                .moves
                .iter()
                .take(5)
                .map(|m| format!("{} → {}", name(&m.from), name(&m.to)))
                .collect(),
        };
        *PLAN.lock().unwrap() = Some(plan);
        shown
    }

    /// Moves the files as the last plan showed. Returns how many moved.
    pub fn tidy(&self) -> Result<TidyResult> {
        let plan = PLAN
            .lock()
            .unwrap()
            .take()
            .ok_or_else(|| failed("Plan it again"))?;
        let roots = self.library.roots()?;
        let (done, failures) = self.library.organize(&plan.moves, &roots)?;
        Ok(TidyResult {
            moved: done.len() as u32,
            failures: failures
                .into_iter()
                .map(|(path, reason)| FileFailure { path, reason })
                .collect(),
        })
    }

    pub fn can_undo_tidy(&self) -> bool {
        self.library.can_undo_organize()
    }

    pub fn undo_tidy(&self) -> Result<u32> {
        Ok(self.library.undo_organize()? as u32)
    }

    // ---------- Import

    /// Reads another player's library into this one: "itunes" (an iTunes, Apple Music, or
    /// MusicBee XML file), "spotify" (a Spotify data export, zip or folder), or "playlist"
    /// (an .m3u file). Returns what was brought in, in words.
    pub fn import_file(&self, kind: String, path: String) -> Result<String> {
        let path = Path::new(&path);
        let mut quiet = |_: &str| {};
        let report = match kind.as_str() {
            "itunes" => import::import_itunes(&self.library, path, &mut quiet)?,
            "spotify" => import::import_spotify(&self.library, path, &mut quiet)?,
            "playlist" => {
                let playlist = self.library.import_playlist(path)?;
                return Ok(format!(
                    "Added {} ({} songs)",
                    playlist.name,
                    playlist.track_ids.len()
                ));
            }
            _ => return Err(failed("Unknown kind of file")),
        };
        Ok(report.summary())
    }

    /// Brings in a listening history: "lastfm" (needs an API key) or "listenbrainz".
    pub fn import_history(&self, service: String, user: String, api_key: String) -> Result<String> {
        let cancel = Arc::new(AtomicBool::new(false));
        let mut quiet = |_: &str| {};
        let report = match service.as_str() {
            "lastfm" => import::import_lastfm(&self.library, &user, &api_key, cancel, &mut quiet)?,
            _ => import::import_listenbrainz(&self.library, &user, cancel, &mut quiet)?,
        };
        Ok(report.summary())
    }

    // ---------- Music server songs kept on the phone

    pub fn is_kept(&self, id: String) -> bool {
        self.library
            .track(&id)
            .ok()
            .flatten()
            .is_some_and(|t| sources::is_kept(&t))
    }

    /// Downloads songs from a music server to play without a connection, or lets them go.
    /// Returns how many changed.
    pub fn keep_offline(&self, ids: Vec<String>, keep: bool) -> Result<u32> {
        let mut changed = 0;
        let mut last = None;
        for track in self.tracks(&ids)? {
            if sources::parse_path(&track.path).is_none() {
                continue;
            }
            let result = if keep {
                sources::keep(&track)
            } else {
                sources::unkeep(&track)
            };
            match result {
                Ok(()) => changed += 1,
                Err(e) => last = Some(format!("{e:#}")),
            }
        }
        match (changed, last) {
            (0, Some(e)) => Err(failed(e)),
            _ => Ok(changed),
        }
    }

    /// Saves songs from a music server as files in `folder` (a music folder), where they
    /// join the library as the phone's own. Returns how many were saved.
    pub fn save_to_music(&self, ids: Vec<String>, folder: String) -> Result<u32> {
        let mut saved = 0;
        for track in self.tracks(&ids)? {
            if sources::parse_path(&track.path).is_some() {
                sources::save_to(&track, Path::new(&folder))?;
                saved += 1;
            }
        }
        if saved > 0 {
            self.rescan();
        }
        Ok(saved)
    }

    /// Songs kept on the phone for a server, and the space they take in bytes.
    pub fn kept_usage(&self, plugin: String) -> Vec<u64> {
        let (count, bytes) = sources::kept_usage(&plugin);
        vec![count as u64, bytes]
    }

    // ---------- Speakers

    /// The speakers on the network and which play now, with their delays.
    pub fn speakers(&self) -> Speakers {
        let all = self
            .outputs()
            .into_iter()
            .filter(|o| !o.id.is_empty())
            .collect();
        let current = self
            .library
            .settings()
            .ok()
            .and_then(|s| s.output_device)
            .unwrap_or_default();
        let (chosen, this_phone, delays) = if let Some(group) = Group::from_device_name(&current) {
            let delays = group
                .delays
                .iter()
                .map(|(address, ms)| {
                    let id = group
                        .speakers
                        .iter()
                        .find(|s| &s.address == address)
                        .map_or(String::new(), |s| s.device_name());
                    (id, *ms)
                })
                .collect();
            (
                group.speakers.iter().map(|s| s.device_name()).collect(),
                group.this_computer,
                delays,
            )
        } else if Speaker::from_device_name(&current).is_some() {
            (vec![current], false, HashMap::new())
        } else {
            (vec![], true, HashMap::new())
        };
        Speakers {
            all,
            chosen,
            this_phone,
            delays,
        }
    }

    /// Plays on several speakers at once (their ids), and on this phone too with
    /// `this_phone`, each held back by its delay in ms ("" is this phone).
    pub fn set_speakers(
        &self,
        ids: Vec<String>,
        this_phone: bool,
        delays: HashMap<String, i32>,
    ) -> Result<()> {
        let speakers: Vec<Speaker> = ids
            .iter()
            .filter_map(|id| Speaker::from_device_name(id))
            .collect();
        let device = match (speakers.len(), this_phone) {
            (0, _) => None,
            (1, false) if delays.is_empty() => Some(speakers[0].device_name()),
            _ => {
                let delays = delays
                    .into_iter()
                    .map(|(id, ms)| {
                        let address =
                            Speaker::from_device_name(&id).map_or(String::new(), |s| s.address);
                        (address, ms.clamp(0, 2000))
                    })
                    .collect();
                Some(
                    Group {
                        speakers,
                        this_computer: this_phone,
                        delays,
                    }
                    .device_name(),
                )
            }
        };
        let mut settings = self.library.settings()?;
        settings.output_device = device.filter(|d| cast::is_network(d));
        self.player.send(Command::Configure(Box::new(settings)));
        Ok(())
    }

    // ---------- Playlists

    /// A picture of the playlist's own (a copy is kept in Needle's files), or back to its
    /// songs' covers with `None`.
    pub fn set_playlist_picture(&self, id: String, path: Option<String>) -> Result<()> {
        let mut playlist = self
            .library
            .playlists()?
            .into_iter()
            .find(|p| p.id == id)
            .ok_or_else(|| failed("That playlist is gone"))?;
        playlist.cover = match path {
            Some(path) => {
                let bytes = std::fs::read(&path).map_err(anyhow::Error::from)?;
                if bytes.len() > 20 * 1024 * 1024 {
                    return Err(failed("That picture is too large"));
                }
                let folder = self.library.directory.join("artwork");
                std::fs::create_dir_all(&folder).map_err(anyhow::Error::from)?;
                let file = folder.join(format!("{}.img", uuid::Uuid::new_v4()));
                std::fs::write(&file, &bytes).map_err(anyhow::Error::from)?;
                Some(file.to_string_lossy().into())
            }
            None => None,
        };
        Ok(self.library.save_playlist(&playlist)?)
    }

    /// Writes the playlist as an .m3u8 file, for other players.
    pub fn export_playlist(&self, id: String, path: String) -> Result<()> {
        let playlist = self
            .library
            .playlists()?
            .into_iter()
            .find(|p| p.id == id)
            .ok_or_else(|| failed("That playlist is gone"))?;
        Ok(self.library.export_playlist(&playlist, Path::new(&path))?)
    }

    // ---------- Search

    /// Completions for a smart rule being typed (`rating >= 4`, `genre = "K-pop"`), with the
    /// library's own names. Plain words give none.
    pub fn suggest(&self, input: String, cursor: u32) -> Vec<Suggestion> {
        let at = byte_at(&input, cursor);
        query::suggest_with_library(&self.library, &input, at, 8)
            .into_iter()
            .map(|s| Suggestion {
                start: utf16(&input, s.replace.start),
                end: utf16(&input, s.replace.end),
                insert: s.insert,
                label: s.label,
                detail: s.detail,
            })
            .collect()
    }

    /// Whether the text is a rule (and will search by it) rather than words.
    pub fn is_rule(&self, input: String) -> bool {
        query::looks_like_rule(&input)
    }

    // ---------- Lyrics

    /// Adds and turns on the NetEase lyrics plugin, and forgets that a song had no lyrics, so
    /// the next look asks NetEase too.
    pub fn add_netease(&self, song: String) -> Result<()> {
        needle_core::plugins::install_example(&self.library, "netease-lyrics")?;
        self.plugins.send(needle_core::plugins::PluginEvent::Reload);
        self.plugins.send(needle_core::plugins::PluginEvent::Enable(
            "netease-lyrics".into(),
            true,
        ));
        if let Ok(track) = self.song_track(&song) {
            let key = format!(
                "lrclib:{}:{}:{}:{:.0}",
                track.artist, track.title, track.album, track.duration
            );
            if let Ok(connection) = self.library.connection() {
                let _ = connection.execute("DELETE FROM cache WHERE key = ?", [key]);
            }
        }
        Ok(())
    }

    // ---------- Backup

    /// A copy of the whole library (songs, plays, stars, playlists, settings) in one file.
    pub fn backup_library(&self, path: String) -> Result<()> {
        Ok(self.library.backup(&PathBuf::from(path))?)
    }

    // ---------- Themes

    /// Saves a theme made in the app (as desktop's theme editor does): `base` is "dark",
    /// "midnight", or "light", `colors` by slot as `#rrggbb`. Returns its id.
    pub fn save_theme(
        &self,
        id: Option<String>,
        name: String,
        base: String,
        colors: HashMap<String, String>,
    ) -> Result<String> {
        let name = name.trim();
        if name.is_empty() {
            return Err(failed("Give the theme a name"));
        }
        let mut table = toml::map::Map::new();
        table.insert("name".into(), toml::Value::String(name.into()));
        table.insert("base".into(), toml::Value::String(base));
        let mut slots = toml::map::Map::new();
        for (slot, color) in colors {
            let hex = color.trim_start_matches('#');
            if hex.len() == 6 && hex.chars().all(|c| c.is_ascii_hexdigit()) {
                slots.insert(slot, toml::Value::String(format!("#{hex}")));
            }
        }
        table.insert("colors".into(), toml::Value::Table(slots));
        let text = toml::to_string(&toml::Value::Table(table)).map_err(anyhow::Error::from)?;
        crate::media::save_theme_file(&self.library, id.as_deref(), name, &text)
    }

    /// Removes a theme made in the app or added from a file.
    pub fn delete_theme(&self, id: String) -> Result<()> {
        let slug = crate::media::owned_theme_slug(&id)?;
        let file = self
            .library
            .directory
            .join("themes")
            .join(format!("{slug}.toml"));
        if file.parent() == Some(self.library.directory.join("themes").as_path()) && file.is_file()
        {
            std::fs::remove_file(file).map_err(anyhow::Error::from)?;
        }
        Ok(())
    }
}
