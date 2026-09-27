//! Lyrics (with karaoke timing, and lyrics timed on the phone), covers and artist photos,
//! speakers on the network, the sync file, the demo library, What's new, and crash reports.
use crate::{Needle, NeedleError, Result, log};
use needle_core::{
    audio::Command,
    cast::{self, Speaker},
    media::{self, LyricLine as CoreLine, LyricWord},
    model::Track,
};
use std::{path::PathBuf, time::Duration};

#[derive(Clone, uniffi::Record)]
pub struct Word {
    pub time: f64,
    pub text: String,
}

#[derive(Clone, uniffi::Record)]
pub struct LyricLine {
    pub time: f64,
    pub text: String,
    /// Word timing, for karaoke; empty when only the line is timed.
    pub words: Vec<Word>,
}

#[derive(Clone, uniffi::Record)]
pub struct Lyrics {
    /// Timed lines; empty when only plain text is known.
    pub lines: Vec<LyricLine>,
    pub plain: String,
    pub instrumental: bool,
    /// Where they came from: "file", "online", "plugin", or "phone" (timed here).
    pub source: String,
}

#[derive(Clone, uniffi::Record)]
pub struct Output {
    /// What to pass to [`Needle::set_output`]; empty for this phone.
    pub id: String,
    pub name: String,
    /// "phone", "Chromecast", "DLNA", or "AirPlay".
    pub kind: String,
}

#[derive(Clone, uniffi::Record)]
pub struct SyncReport {
    pub songs_matched: u32,
    pub listens_added: u32,
    pub playlists_added: u32,
    pub songs_not_found: u32,
}

#[derive(Clone, uniffi::Record)]
pub struct Notes {
    pub version: String,
    pub text: String,
}

const NOTES: &str = include_str!("../../needle/whats-new.md");

impl Needle {
    /// Lyrics timed on the phone live in the app's own storage, since Android does not let it
    /// write beside the music files.
    fn own_lyrics(&self, id: &str) -> PathBuf {
        self.library
            .directory
            .join("lyrics")
            .join(format!("{id}.lrc"))
    }

    fn track(&self, id: &str) -> Result<Track> {
        self.library
            .track(id)?
            .ok_or_else(|| NeedleError::Failed("That song is no longer in the library".into()))
    }
}

fn line(l: CoreLine) -> LyricLine {
    LyricLine {
        time: l.time,
        text: l.text,
        words: l
            .words
            .into_iter()
            .map(|w| Word {
                time: w.time,
                text: w.text,
            })
            .collect(),
    }
}

#[uniffi::export]
impl Needle {
    /// A song's lyrics: ones timed on this phone, then its own file or a `.lrc` beside it,
    /// then (when on) LRCLIB and the lyrics plugins.
    pub fn lyrics(&self, id: String) -> Result<Option<Lyrics>> {
        let track = self.track(&id)?;
        if let Ok(text) = std::fs::read_to_string(self.own_lyrics(&id)) {
            let lines = media::parse_lrc(&text);
            if !lines.is_empty() {
                return Ok(Some(Lyrics {
                    lines: lines.into_iter().map(line).collect(),
                    plain: String::new(),
                    instrumental: false,
                    source: "phone".into(),
                }));
            }
        }
        let online = self.library.settings().is_ok_and(|s| s.online_media);
        let found = match media::lyrics(&self.library, &track, online, Some(&self.plugins)) {
            Ok(found) => found,
            Err(error) => {
                log(true, &format!("Lyrics for {}: {error:#}", track.title));
                None
            }
        };
        Ok(found.map(|l| Lyrics {
            source: match l.source {
                media::LyricsSource::Sidecar | media::LyricsSource::Embedded => "file",
                media::LyricsSource::Plugin => "plugin",
                _ => "online",
            }
            .into(),
            lines: l.lines.into_iter().map(line).collect(),
            plain: l.plain,
            instrumental: l.instrumental,
        }))
    }

    /// Saves lyrics timed on the phone (one time per line, in seconds).
    pub fn save_timed_lyrics(&self, id: String, lines: Vec<LyricLine>) -> Result<()> {
        let lines: Vec<CoreLine> = lines
            .into_iter()
            .map(|l| CoreLine {
                time: l.time,
                text: l.text,
                words: l
                    .words
                    .into_iter()
                    .map(|w| LyricWord {
                        time: w.time,
                        text: w.text,
                    })
                    .collect(),
            })
            .collect();
        let path = self.own_lyrics(&id);
        if let Some(folder) = path.parent() {
            std::fs::create_dir_all(folder).map_err(anyhow::Error::from)?;
        }
        std::fs::write(&path, media::to_lrc(&lines)).map_err(anyhow::Error::from)?;
        Ok(())
    }

    /// Forgets lyrics timed on the phone for a song.
    pub fn delete_timed_lyrics(&self, id: String) {
        let _ = std::fs::remove_file(self.own_lyrics(&id));
    }

    /// An artist's photo from the library's folder, or (when online lookups are on) from
    /// MusicBrainz and Wikimedia Commons.
    pub fn artist_photo(&self, name: String) -> Option<String> {
        let online = self.library.settings().is_ok_and(|s| s.online_media);
        match media::artist_image(&self.library, &name, online) {
            Ok(path) => path.map(|p| p.to_string_lossy().to_string()),
            Err(e) => {
                log(false, &format!("Artist photo for {name}: {e:#}"));
                None
            }
        }
    }

    /// Looks up a missing cover online (Cover Art Archive) for a song's album. Returns how
    /// many songs got it.
    pub fn fetch_cover(&self, id: String) -> Result<u32> {
        let track = self.track(&id)?;
        Ok(media::fetch_album_art(&self.library, &track)? as u32)
    }

    /// This phone, then speakers found on the network (Chromecast, DLNA, AirPlay). Takes a
    /// few seconds; the app must hold Wi-Fi multicast while it looks.
    pub fn outputs(&self) -> Vec<Output> {
        let mut outputs = vec![Output {
            id: String::new(),
            name: "This phone".into(),
            kind: "phone".into(),
        }];
        outputs.extend(
            cast::discover::discover(Duration::from_secs(3))
                .into_iter()
                .map(|s: Speaker| Output {
                    id: s.device_name(),
                    name: s.name.clone(),
                    kind: s.kind.label().into(),
                }),
        );
        outputs
    }

    /// Where the music plays now: an [`Output`] id, or empty for this phone.
    pub fn current_output(&self) -> String {
        self.library
            .settings()
            .ok()
            .and_then(|s| s.output_device)
            .filter(|d| cast::is_network(d))
            .unwrap_or_default()
    }

    /// Plays on a speaker (its [`Output`] id), or on this phone (empty).
    pub fn set_output(&self, id: String) -> Result<()> {
        let mut settings = self.library.settings()?;
        settings.output_device = (!id.is_empty()).then_some(id);
        self.player.send(Command::Configure(Box::new(settings)));
        Ok(())
    }

    /// Writes history, ratings, and playlists to a file locked with `passphrase`.
    pub fn export_sync(&self, path: String, passphrase: String) -> Result<()> {
        Ok(needle_core::sync::export(
            &self.library,
            std::path::Path::new(&path),
            &passphrase,
        )?)
    }

    /// Reads a sync file from Needle on another device into this library.
    pub fn import_sync(&self, path: String, passphrase: String) -> Result<SyncReport> {
        let r = needle_core::sync::import(&self.library, std::path::Path::new(&path), &passphrase)?;
        Ok(SyncReport {
            songs_matched: r.matched_tracks as u32,
            listens_added: r.imported_listens as u32,
            playlists_added: r.imported_playlists as u32,
            songs_not_found: r.unmatched_tracks as u32,
        })
    }

    /// Makes a few short songs in `folder` and adds it as a music folder, to try Needle with.
    pub fn add_demo_library(&self, folder: String) -> Result<()> {
        needle_core::demo::create(std::path::Path::new(&folder))?;
        self.add_folder(folder)
    }

    /// What's new in each version of Needle, newest first.
    pub fn whats_new(&self) -> Vec<Notes> {
        let mut notes = vec![];
        for section in NOTES.split("\n# ").map(|s| s.trim_start_matches("# ")) {
            let (version, text) = section.split_once('\n').unwrap_or((section, ""));
            if !version.trim().is_empty() {
                notes.push(Notes {
                    version: version.trim().to_string(),
                    text: text.trim().to_string(),
                });
            }
        }
        notes
    }

    /// Whether crash reports are sent (without file paths or names).
    pub fn crash_reports(&self) -> bool {
        self.library.settings().is_ok_and(|s| s.crash_reports)
    }

    pub fn set_crash_reports(&self, on: bool) -> Result<()> {
        let mut settings = self.library.settings()?;
        settings.crash_reports = on;
        self.player.send(Command::Configure(Box::new(settings)));
        Ok(())
    }

    /// Sends crash reports waiting from earlier runs, when that is on; returns how many.
    pub fn send_crash_reports(&self) -> u32 {
        let on = self.crash_reports();
        needle_core::logfile::send_pending(&self.library.directory, on) as u32
    }
}
