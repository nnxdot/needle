//! CUE sheets: one audio file holding a whole album, with a `.cue` file that marks where each
//! track starts. Each track becomes a song of its own, pointing at a stretch of the audio file.
use crate::{database::Library, model::Track};
use anyhow::{Context, Result};
use lofty::{
    file::{AudioFile, TaggedFileExt},
    prelude::{Accessor, ItemKey},
    probe::Probe,
};
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Sheet {
    pub title: String,
    pub performer: String,
    pub genre: String,
    pub date: String,
    pub tracks: Vec<CueTrack>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CueTrack {
    pub number: i64,
    pub file: String,
    pub title: String,
    pub performer: String,
    /// Seconds into `file` where the track starts (INDEX 01).
    pub start: f64,
}

/// Read a cue sheet's text, whatever its encoding: UTF-8 (with or without BOM), else Latin-1.
fn text(bytes: &[u8]) -> String {
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    match std::str::from_utf8(bytes) {
        Ok(s) => s.to_string(),
        Err(_) => bytes.iter().map(|&b| b as char).collect(),
    }
}

/// The value after a keyword: a quoted string, or the rest of the line.
fn value(rest: &str) -> String {
    let rest = rest.trim();
    if let Some(stripped) = rest.strip_prefix('"') {
        stripped.split('"').next().unwrap_or_default().to_string()
    } else {
        rest.to_string()
    }
}

/// `mm:ss:ff` with 75 frames a second.
fn time(value: &str) -> Option<f64> {
    let mut parts = value.trim().split(':').map(|p| p.parse::<f64>().ok());
    let (m, s, f) = (parts.next()??, parts.next()??, parts.next()??);
    Some(m * 60. + s + f / 75.)
}

pub fn parse(bytes: &[u8]) -> Sheet {
    let mut sheet = Sheet::default();
    let mut file = String::new();
    let mut current: Option<CueTrack> = None;
    for line in text(bytes).lines() {
        let line = line.trim();
        let (keyword, rest) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
        match keyword.to_ascii_uppercase().as_str() {
            "FILE" => {
                // The file name is quoted, or runs up to the type word at the end.
                let rest = rest.trim();
                file = if rest.starts_with('"') {
                    value(rest)
                } else {
                    rest.rsplit_once(' ')
                        .map(|(name, _)| name)
                        .unwrap_or(rest)
                        .to_string()
                };
            }
            "TRACK" => {
                if let Some(track) = current.take().filter(|t| t.start >= 0.) {
                    sheet.tracks.push(track);
                }
                let number = rest
                    .split_whitespace()
                    .next()
                    .and_then(|n| n.parse().ok())
                    .unwrap_or(0);
                current = Some(CueTrack {
                    number,
                    file: file.clone(),
                    start: -1.,
                    ..Default::default()
                });
            }
            "TITLE" => match current.as_mut() {
                Some(track) => track.title = value(rest),
                None => sheet.title = value(rest),
            },
            "PERFORMER" => match current.as_mut() {
                Some(track) => track.performer = value(rest),
                None => sheet.performer = value(rest),
            },
            "INDEX" => {
                let mut parts = rest.split_whitespace();
                if parts.next() == Some("01")
                    && let (Some(track), Some(start)) =
                        (current.as_mut(), parts.next().and_then(time))
                {
                    track.start = start;
                }
            }
            "REM" => {
                let (key, rest) = rest
                    .trim()
                    .split_once(char::is_whitespace)
                    .unwrap_or((rest, ""));
                match key.to_ascii_uppercase().as_str() {
                    "GENRE" => sheet.genre = value(rest),
                    "DATE" => sheet.date = value(rest),
                    _ => {}
                }
            }
            _ => {}
        }
    }
    if let Some(track) = current.filter(|t| t.start >= 0.) {
        sheet.tracks.push(track);
    }
    sheet
}

/// The audio file a cue sheet names, looked up beside the sheet. Also tries other extensions,
/// because sheets often still name the `.wav` a CD was ripped to before it was compressed.
fn locate(folder: &Path, name: &str) -> Option<PathBuf> {
    let direct = folder.join(name);
    if direct.is_file() {
        return Some(direct);
    }
    let stem = Path::new(name)
        .file_stem()?
        .to_string_lossy()
        .to_lowercase();
    fs::read_dir(folder)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .find(|p| {
            p.file_stem()
                .is_some_and(|s| s.to_string_lossy().to_lowercase() == stem)
                && p.extension().is_some_and(|e| {
                    crate::scan::EXTENSIONS.contains(&e.to_string_lossy().to_lowercase().as_str())
                })
        })
}

/// Audio files the cue sheets under `root` split into tracks; the scanner skips them as
/// whole songs.
pub fn covered_files(root: &Path) -> HashSet<PathBuf> {
    let mut covered = HashSet::new();
    for entry in walkdir::WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .flatten()
    {
        let path = entry.path();
        if !path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("cue"))
        {
            continue;
        }
        let Ok(bytes) = fs::read(path) else { continue };
        let folder = path.parent().unwrap_or(Path::new("."));
        for track in parse(&bytes).tracks {
            if let Some(file) = locate(folder, &track.file).and_then(|p| p.canonicalize().ok()) {
                covered.insert(file);
            }
        }
    }
    covered
}

/// Add or update the songs of one cue sheet. Returns how many were new or changed.
pub fn import_sheet(library: &Library, cue: &Path) -> Result<usize> {
    let cue = cue.canonicalize()?;
    let folder = cue.parent().context("A cue sheet needs a folder")?;
    let sheet = parse(&fs::read(&cue)?);
    let cue_modified = fs::metadata(&cue)?
        .modified()?
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as i64;
    let mut changed = 0;
    let mut seen = std::collections::HashSet::new();
    let mut recordings = std::collections::HashMap::new();
    let now = chrono::Utc::now().timestamp();
    for (i, entry) in sheet.tracks.iter().enumerate() {
        let Some(audio) = locate(folder, &entry.file).and_then(|p| p.canonicalize().ok()) else {
            continue;
        };
        let audio_meta = fs::metadata(&audio)?;
        let modified = cue_modified.max(
            audio_meta
                .modified()?
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos() as i64,
        );
        let path = format!("{}#{}", cue.to_string_lossy(), entry.number);
        seen.insert(path.clone());
        let previous = library.track_by_path(&path)?;
        if previous.as_ref().is_some_and(|t| {
            t.modified_at == modified && !t.missing && t.content_hash.starts_with("cue2:")
        }) {
            continue;
        }
        // Whole-file facts come from the audio file itself.
        let (properties, tags) = match Probe::open(&audio).and_then(|p| p.read()) {
            Ok(tagged) => {
                let properties = tagged.properties().clone();
                (
                    Some(properties),
                    tagged.primary_tag().or_else(|| tagged.first_tag()).cloned(),
                )
            }
            Err(_) => (None, None),
        };
        let file_duration = properties
            .as_ref()
            .map(|p| p.duration().as_secs_f64())
            .unwrap_or(0.);
        // A track ends where the next one in the same file starts, or at the end of the file.
        let end = sheet
            .tracks
            .get(i + 1)
            .filter(|next| next.file == entry.file)
            .map(|next| next.start);
        let duration = end.unwrap_or(file_duration) - entry.start;
        let artwork = tags
            .as_ref()
            .and_then(|tag| {
                let pictures = tag.pictures();
                pictures
                    .iter()
                    .find(|p| p.pic_type() == lofty::picture::PictureType::CoverFront)
                    .or_else(|| pictures.first())
                    .cloned()
            })
            .filter(|p| p.data().len() <= 20 * 1024 * 1024)
            .and_then(|picture| {
                let destination = library
                    .directory
                    .join("artwork")
                    .join(format!("{}.img", blake3::hash(picture.data()).to_hex()));
                (destination.exists() || fs::write(&destination, picture.data()).is_ok())
                    .then(|| destination.to_string_lossy().to_string())
            })
            .or_else(|| crate::scan::folder_cover(folder));
        let album = if sheet.title.is_empty() {
            tags.as_ref()
                .and_then(|t| t.album().map(|a| a.to_string()))
                .unwrap_or_default()
        } else {
            sheet.title.clone()
        };
        let album_artist = if sheet.performer.is_empty() {
            tags.as_ref()
                .and_then(|t| t.get_string(ItemKey::AlbumArtist).map(str::to_string))
                .unwrap_or_default()
        } else {
            sheet.performer.clone()
        };
        let artist = if entry.performer.is_empty() {
            album_artist.clone()
        } else {
            entry.performer.clone()
        };
        let recording = match recordings.entry(audio.clone()) {
            std::collections::hash_map::Entry::Occupied(e) => e.into_mut(),
            std::collections::hash_map::Entry::Vacant(e) => {
                e.insert(crate::scan::full_hash(&audio)?)
            }
        };
        let identity = format!(
            "cue2:{}",
            blake3::hash(
                format!(
                    "{recording}:{}:{:?}",
                    entry.start.to_bits(),
                    end.map(f64::to_bits)
                )
                .as_bytes()
            )
            .to_hex()
        );
        let mut track = Track {
            id: previous
                .as_ref()
                .map(|t| t.id.clone())
                .unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
            path,
            title: if entry.title.is_empty() {
                format!("Track {}", entry.number)
            } else {
                entry.title.clone()
            },
            artist,
            album,
            album_artist,
            genre: sheet.genre.clone(),
            year: sheet
                .date
                .get(..4)
                .and_then(|y| y.parse().ok())
                .unwrap_or(0),
            track_number: entry.number,
            disc: 1,
            duration: duration.max(0.),
            sample_rate: properties
                .as_ref()
                .and_then(|p| p.sample_rate())
                .unwrap_or(0) as i64,
            bit_depth: properties.as_ref().and_then(|p| p.bit_depth()).unwrap_or(0) as i64,
            channels: properties.as_ref().and_then(|p| p.channels()).unwrap_or(0) as i64,
            bitrate: properties
                .as_ref()
                .and_then(|p| p.audio_bitrate())
                .unwrap_or(0) as i64,
            format: audio
                .extension()
                .unwrap_or_default()
                .to_string_lossy()
                .to_uppercase(),
            added_at: previous.as_ref().map(|t| t.added_at).unwrap_or(now),
            modified_at: modified,
            file_size: audio_meta.len() as i64,
            // Hash each recording once per sheet, and bind the identity to the played span.
            content_hash: identity,
            rating: previous.as_ref().map(|t| t.rating).unwrap_or(0),
            play_count: previous.as_ref().map(|t| t.play_count).unwrap_or(0),
            last_played: previous.as_ref().and_then(|t| t.last_played),
            artwork,
            cue: Some(crate::model::CueSpan {
                audio: audio.to_string_lossy().to_string(),
                start: entry.start,
                end,
            }),
            metadata_version: 2,
            ..Default::default()
        };
        if let Some(previous) = &previous
            && previous.cue == track.cue
            && previous
                .analysis_audio_hash
                .as_ref()
                .is_some_and(|identity| {
                    crate::scan::audio_identity(&audio).ok().as_ref() == Some(identity)
                })
        {
            crate::scan::preserve_loudness(previous, &mut track);
        }
        library.upsert(&track)?;
        changed += 1;
    }
    // Keep retired entries (and their history) but make them unavailable.
    let db = library.connection()?;
    let mut statement =
        db.prepare("SELECT id,path FROM tracks WHERE json_extract(data,'$.cue') IS NOT NULL")?;
    let old = statement
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let prefix = format!("{}#", cue.to_string_lossy());
    for (id, path) in old {
        if path
            .strip_prefix(&prefix)
            .is_some_and(|n| n.parse::<u32>().is_ok())
            && !seen.contains(&path)
        {
            changed += db.execute("UPDATE tracks SET missing=1 WHERE id=? AND missing=0", [id])?;
        }
    }
    // A song imported earlier as the whole album file is now its tracks.
    for entry in &sheet.tracks {
        if let Some(audio) = locate(folder, &entry.file).and_then(|p| p.canonicalize().ok())
            && let Some(whole) = library.track_by_path(&audio.to_string_lossy())?
        {
            library
                .connection()?
                .execute("DELETE FROM tracks WHERE id=?", [whole.id])?;
        }
    }
    Ok(changed)
}

/// Upgrade legacy size-based identities before a caller trusts them for association.
pub(crate) fn refresh_identities(library: &Library) -> Result<()> {
    let db = library.connection()?;
    let mut statement = db.prepare("SELECT path FROM tracks WHERE json_extract(data,'$.cue') IS NOT NULL AND content_hash NOT LIKE 'cue2:%'")?;
    let paths = statement
        .query_map([], |r| r.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let sheets: std::collections::HashSet<_> = paths
        .iter()
        .filter_map(|p| p.rsplit_once('#').map(|(p, _)| p.to_owned()))
        .collect();
    for sheet in sheets {
        // Unavailable sheets remain in history, with identities excluded by callers.
        if Path::new(&sheet).is_file() {
            import_sheet(library, Path::new(&sheet))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHEET: &str = "\u{FEFF}REM GENRE \"Jazz\"
REM DATE 1959
PERFORMER \"Miles Quintet\"
TITLE \"Kind of Test\"
FILE \"album.wav\" WAVE
  TRACK 01 AUDIO
    TITLE \"So What\"
    INDEX 01 00:00:00
  TRACK 02 AUDIO
    TITLE \"Freddie\"
    PERFORMER \"Guest Player\"
    INDEX 00 00:09:50
    INDEX 01 00:10:00
  TRACK 03 AUDIO
    TITLE \"Blue\"
    INDEX 01 00:20:37
";

    #[test]
    fn parses_tracks_times_and_album_details() {
        let sheet = parse(SHEET.as_bytes());
        assert_eq!(
            (sheet.title.as_str(), sheet.performer.as_str()),
            ("Kind of Test", "Miles Quintet")
        );
        assert_eq!(
            (sheet.genre.as_str(), sheet.date.as_str()),
            ("Jazz", "1959")
        );
        assert_eq!(sheet.tracks.len(), 3);
        assert_eq!(sheet.tracks[1].title, "Freddie");
        assert_eq!(sheet.tracks[1].performer, "Guest Player");
        assert_eq!(sheet.tracks[1].start, 10.);
        assert!((sheet.tracks[2].start - (20. + 37. / 75.)).abs() < 1e-9);
        assert!(sheet.tracks.iter().all(|t| t.file == "album.wav"));
    }

    #[test]
    fn imports_each_track_and_plays_only_its_part() {
        let dir = tempfile::tempdir().unwrap();
        let library = Library::open(dir.path().join("library")).unwrap();
        let music = dir.path().join("music");
        fs::create_dir(&music).unwrap();
        // 30 s of silence at 8 kHz; each track's start is marked by one loud sample.
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 8000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::create(music.join("album.wav"), spec).unwrap();
        for i in 0..8000 * 30 {
            writer
                .write_sample(if i == 80_000 { 20000i16 } else { 0 })
                .unwrap();
        }
        writer.finalize().unwrap();
        // The sheet still names the ripped ".flac": it should find the .wav anyway.
        fs::write(
            music.join("album.cue"),
            SHEET.replace("album.wav", "album.flac"),
        )
        .unwrap();
        assert!(covered_files(&music).contains(&music.join("album.wav").canonicalize().unwrap()));
        assert_eq!(import_sheet(&library, &music.join("album.cue")).unwrap(), 3);
        let tracks = library.search("order by track_number").unwrap();
        assert_eq!(
            tracks.iter().map(|t| t.title.as_str()).collect::<Vec<_>>(),
            ["So What", "Freddie", "Blue"]
        );
        assert_eq!(tracks[1].artist, "Guest Player");
        assert_eq!(tracks[0].album_artist, "Miles Quintet");
        assert_eq!(tracks[0].year, 1959);
        assert!((tracks[0].duration - 10.).abs() < 0.01);
        assert!((tracks[2].duration - (30. - 20. - 37. / 75.)).abs() < 0.05);
        // Track 2 starts exactly on the marker sample and lasts about ten seconds.
        let samples: Vec<f32> = crate::audio_file::decode_track(&tracks[1])
            .unwrap()
            .collect();
        assert!(samples[0] > 0.5, "starts on the marker");
        assert!((samples.len() as f64 / 8000. - (20. + 37. / 75. - 10.)).abs() < 0.02);
        // Scanning again changes nothing.
        assert_eq!(import_sheet(&library, &music.join("album.cue")).unwrap(), 0);
    }
}
