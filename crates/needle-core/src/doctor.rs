//! Fix my library: duplicate songs, albums without covers, album tags from MusicBrainz, and
//! files renamed and moved into tidy folders (with undo).
use crate::{database::Library, integrations::musicbrainz_get, model::Track, scan::TagEdit};
use anyhow::{Context, Result, bail};
use rusqlite::params;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};

// ---------------------------------------------------------------- duplicates

/// Copies of one song. `keep` is the copy to keep: the best sounding one.
#[derive(Clone, Debug)]
pub struct DuplicateGroup {
    pub tracks: Vec<Track>,
    pub keep: usize,
}

/// "The Song (2011 Remaster)" and "the song" compare equal: lowercase letters and digits only,
/// with bracketed remarks about versions dropped.
fn simple(text: &str) -> String {
    let mut out = String::new();
    let mut depth = 0;
    for c in text.chars() {
        match c {
            '(' | '[' => depth += 1,
            ')' | ']' => depth = (depth - 1).max(0),
            c if depth == 0 && c.is_alphanumeric() => out.extend(c.to_lowercase()),
            _ => {}
        }
    }
    out
}

/// Remarks that name the same recording: "(2011 Remaster)", "[Explicit]", "(Album Version)".
/// Any other remark — "(Off Vocal)", "(Instrumental)", "(Japanese ver.)", "(Live)",
/// "(Remix)" — makes a different song.
fn same_recording(remark: &str) -> bool {
    let remark = remark.to_lowercase();
    let words: Vec<&str> = remark
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect();
    !words.is_empty()
        && words.iter().all(|w| {
            w.chars().all(|c| c.is_ascii_digit())
                || matches!(
                    *w,
                    "remaster"
                        | "remastered"
                        | "remasterd"
                        | "digital"
                        | "digitally"
                        | "explicit"
                        | "clean"
                        | "album"
                        | "lp"
                        | "version"
                        | "ver"
                        | "edition"
                        | "mastered"
                        | "for"
                        | "the"
                        | "stereo"
                        | "hd"
                        | "hi"
                        | "res"
                        | "hires"
                        | "24bit"
                        | "bit"
                        | "khz"
                        | "original"
                )
        })
        && (words
            .iter()
            .any(|w| w.starts_with("remaster") || *w == "explicit" || *w == "clean")
            || remark.contains("album version")
            || remark.contains("lp version"))
}

/// The title as duplicate finding compares it: `simple`, but a remark in brackets that is not
/// about the same recording stays part of it, so "Song (Off Vocal)" is not "Song".
fn title_key(title: &str) -> String {
    let mut out = String::new();
    let mut remark = String::new();
    let mut depth = 0usize;
    for c in title.chars() {
        match c {
            '(' | '[' | '（' | '［' | '【' | '「' | '『' => {
                depth += 1;
                if depth == 1 {
                    remark.clear();
                }
            }
            ')' | ']' | '）' | '］' | '】' | '」' | '』' if depth > 0 => {
                depth -= 1;
                if depth == 0 && !same_recording(&remark) {
                    out.push('|');
                    out.push_str(&simple(&remark));
                }
            }
            c if depth > 0 => remark.push(c),
            c if c.is_alphanumeric() => out.extend(c.to_lowercase()),
            _ => {}
        }
    }
    out
}

/// Higher is better: lossless first, then bit depth, sample rate, bitrate, and size. Between
/// equals: the file named after its song, the most played, the one added first.
pub fn quality_rank(track: &Track) -> (bool, bool, i64, i64, i64, i64, bool, i64, i64) {
    let lossless = matches!(
        track.format.to_uppercase().as_str(),
        "FLAC" | "ALAC" | "WAV" | "AIFF" | "APE" | "WV" | "WAVPACK" | "DSF" | "DSD"
    );
    let stem = Path::new(&track.path)
        .file_stem()
        .map(|s| simple(&s.to_string_lossy()))
        .unwrap_or_default();
    let named = !simple(&track.title).is_empty() && stem.contains(&simple(&track.title));
    (
        !track.missing,
        lossless,
        track.bit_depth,
        track.sample_rate,
        track.bitrate,
        track.file_size,
        named,
        track.play_count,
        -track.added_at,
    )
}

/// Group the library's songs that are the same recording: the same file contents, or the same
/// artist, title, and album within two seconds of each other's length. (The same song on two
/// albums may be a different recording, so it is left alone, and so are versions such as
/// "(Off Vocal)" or "(Instrumental)": see `title_key`.)
pub fn find_duplicates(tracks: &[Track]) -> Vec<DuplicateGroup> {
    let mut by_name: HashMap<(String, String, String), Vec<&Track>> = HashMap::new();
    // Songs on a music server are not files here, so they are never duplicates to recycle.
    for track in tracks
        .iter()
        .filter(|t| !t.missing && t.cue.is_none() && !t.is_streamed())
    {
        let title = title_key(&track.title);
        if simple(&track.title).is_empty() {
            continue;
        }
        by_name
            .entry((simple(track.display_artist()), title, simple(&track.album)))
            .or_default()
            .push(track);
    }
    let mut groups: Vec<Vec<&Track>> = vec![];
    for mut list in by_name.into_values().filter(|l| l.len() > 1) {
        list.sort_by(|a, b| a.duration.total_cmp(&b.duration));
        let mut current: Vec<&Track> = vec![];
        for track in list {
            match current.last() {
                Some(last) if (track.duration - last.duration).abs() > 2. => {
                    if current.len() > 1 {
                        groups.push(std::mem::take(&mut current));
                    }
                    current = vec![track];
                }
                _ => current.push(track),
            }
        }
        if current.len() > 1 {
            groups.push(current);
        }
    }
    // Identical files under different names.
    let named: HashSet<&str> = groups.iter().flatten().map(|t| t.id.as_str()).collect();
    let mut by_hash: HashMap<&str, Vec<&Track>> = HashMap::new();
    for track in tracks
        .iter()
        .filter(|t| !t.missing && t.cue.is_none() && !t.content_hash.is_empty())
    {
        if !named.contains(track.id.as_str()) {
            by_hash.entry(&track.content_hash).or_default().push(track);
        }
    }
    for list in by_hash.into_values().filter(|l| l.len() > 1) {
        if !list[0].content_hash.starts_with("q1:") {
            groups.push(list);
            continue;
        }
        // A quick hash reads only part of each file; the whole files must match too.
        let mut by_file: HashMap<String, Vec<&Track>> = HashMap::new();
        for track in list {
            if let Ok(hash) = crate::scan::full_hash(Path::new(&track.path)) {
                by_file.entry(hash).or_default().push(track);
            }
        }
        groups.extend(by_file.into_values().filter(|l| l.len() > 1));
    }
    let mut groups: Vec<DuplicateGroup> = groups
        .into_iter()
        .map(|list| {
            let tracks: Vec<Track> = list.into_iter().cloned().collect();
            let keep = (0..tracks.len())
                .max_by_key(|&i| quality_rank(&tracks[i]))
                .unwrap_or(0);
            DuplicateGroup { tracks, keep }
        })
        .collect();
    groups.sort_by(|a, b| {
        let key = |g: &DuplicateGroup| {
            (
                g.tracks[0].display_artist().to_lowercase(),
                g.tracks[0].title.to_lowercase(),
            )
        };
        key(a).cmp(&key(b))
    });
    groups
}

/// Move a file to the Recycle Bin, where it can be restored.
#[cfg(windows)]
pub fn recycle(path: &Path) -> Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::UI::Shell::{
        FO_DELETE, FOF_ALLOWUNDO, FOF_NOCONFIRMATION, FOF_NOERRORUI, FOF_SILENT, SHFILEOPSTRUCTW,
        SHFileOperationW,
    };
    // A double-NUL-terminated list of one path, without the long-path prefix the shell rejects.
    let plain = path
        .to_string_lossy()
        .trim_start_matches("\\\\?\\")
        .to_string();
    let mut from: Vec<u16> = std::ffi::OsStr::new(&plain).encode_wide().collect();
    from.extend([0, 0]);
    let mut operation = SHFILEOPSTRUCTW {
        hwnd: 0,
        wFunc: FO_DELETE,
        pFrom: from.as_ptr(),
        pTo: std::ptr::null(),
        fFlags: (FOF_ALLOWUNDO | FOF_NOCONFIRMATION | FOF_NOERRORUI | FOF_SILENT) as u16,
        fAnyOperationsAborted: 0,
        hNameMappings: std::ptr::null_mut(),
        lpszProgressTitle: std::ptr::null(),
    };
    // SAFETY: `from` is a valid double-NUL-terminated wide string that outlives the call.
    let result = unsafe { SHFileOperationW(&mut operation) };
    if result != 0 || operation.fAnyOperationsAborted != 0 {
        bail!(
            "Windows could not move {} to the Recycle Bin (error {result})",
            path.display()
        );
    }
    Ok(())
}
#[cfg(not(windows))]
pub fn recycle(path: &Path) -> Result<()> {
    bail!(
        "Moving {} to the trash is only supported on Windows",
        path.display()
    )
}

impl Library {
    /// Keep `keep` and fold the other copies into it: their plays, rating, listening history,
    /// and playlist places move to it; their files go to the Recycle Bin (when `recycle`), and
    /// they leave the library.
    pub fn merge_duplicates(
        &self,
        keep: &Track,
        others: &[Track],
        recycle_files: bool,
    ) -> Result<()> {
        let others: Vec<&Track> = others.iter().filter(|t| t.id != keep.id).collect();
        if others.is_empty() {
            return Ok(());
        }
        if recycle_files {
            for other in &others {
                if Path::new(&other.path).exists() {
                    recycle(Path::new(&other.path))?;
                }
            }
        }
        let mut db = self.connection()?;
        let tx = db.transaction()?;
        let plays: i64 = others.iter().map(|t| t.play_count).sum::<i64>() + keep.play_count;
        let rating = others
            .iter()
            .map(|t| t.rating)
            .chain([keep.rating])
            .max()
            .unwrap_or(0);
        let last = others
            .iter()
            .filter_map(|t| t.last_played)
            .chain(keep.last_played)
            .max();
        tx.execute(
            "UPDATE tracks SET play_count=?, rating=?, last_played=? WHERE id=?",
            params![plays, rating, last, keep.id],
        )?;
        for other in &others {
            tx.execute(
                "UPDATE listens SET track_id=?1, data=json_set(data,'$.track_id',?1) WHERE track_id=?2",
                params![keep.id, other.id],
            )?;
            tx.execute("DELETE FROM tracks WHERE id=?", [&other.id])?;
        }
        tx.commit()?;
        let gone: HashSet<&str> = others.iter().map(|t| t.id.as_str()).collect();
        for mut playlist in self.playlists()? {
            if playlist
                .track_ids
                .iter()
                .any(|id| gone.contains(id.as_str()))
            {
                let mut seen = HashSet::new();
                playlist.track_ids = playlist
                    .track_ids
                    .iter()
                    .map(|id| {
                        if gone.contains(id.as_str()) {
                            keep.id.clone()
                        } else {
                            id.clone()
                        }
                    })
                    .filter(|id| seen.insert(id.clone()))
                    .collect();
                self.save_playlist(&playlist)?;
            }
        }
        Ok(())
    }

    /// Albums where no song has a cover: one song from each, to look up by.
    pub fn albums_without_covers(&self) -> Result<Vec<Track>> {
        let ids: Vec<String> = self
            .connection()?
            .prepare(
                "SELECT max(id) FROM tracks WHERE album != '' AND missing = 0 AND path NOT LIKE 'source://%'
                 GROUP BY lower(album), lower(CASE WHEN album_artist != '' THEN album_artist ELSE artist END)
                 HAVING max(json_extract(data,'$.artwork')) IS NULL
                 ORDER BY lower(CASE WHEN album_artist != '' THEN album_artist ELSE artist END), lower(album)",
            )?
            .query_map([], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        let mut tracks = self.tracks_by_ids(&ids)?;
        tracks.sort_by(|a, b| {
            let key = |t: &Track| {
                (
                    t.display_album_artist().to_lowercase(),
                    t.album.to_lowercase(),
                )
            };
            key(a).cmp(&key(b))
        });
        Ok(tracks)
    }
}

// ---------------------------------------------------------------- MusicBrainz album tags

/// A release on MusicBrainz that may be this album.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Release {
    pub id: String,
    pub title: String,
    pub artist: String,
    pub year: Option<u32>,
    pub country: String,
    pub format: String,
    pub score: i64,
    pub tracks: Vec<ReleaseTrack>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ReleaseTrack {
    pub disc: u32,
    pub number: u32,
    pub title: String,
    pub artist: String,
    pub seconds: Option<f64>,
    pub recording_id: String,
}

fn credit(value: &Value) -> String {
    value
        .as_array()
        .into_iter()
        .flatten()
        .map(|c| {
            format!(
                "{}{}",
                c["name"].as_str().unwrap_or_default(),
                c["joinphrase"].as_str().unwrap_or_default()
            )
        })
        .collect()
}

fn parse_release(value: &Value) -> Release {
    let mut tracks = vec![];
    let mut formats = vec![];
    for (d, medium) in value["media"].as_array().into_iter().flatten().enumerate() {
        if let Some(format) = medium["format"].as_str() {
            formats.push(format.to_string());
        }
        let disc = medium["position"].as_u64().unwrap_or(d as u64 + 1) as u32;
        for (i, t) in medium["tracks"]
            .as_array()
            .into_iter()
            .flatten()
            .enumerate()
        {
            let recording = &t["recording"];
            tracks.push(ReleaseTrack {
                disc,
                number: t["position"].as_u64().unwrap_or(i as u64 + 1) as u32,
                title: t["title"]
                    .as_str()
                    .or(recording["title"].as_str())
                    .unwrap_or_default()
                    .to_string(),
                artist: credit(if t["artist-credit"].is_array() {
                    &t["artist-credit"]
                } else {
                    &recording["artist-credit"]
                }),
                seconds: t["length"]
                    .as_f64()
                    .or(recording["length"].as_f64())
                    .map(|ms| ms / 1000.),
                recording_id: recording["id"].as_str().unwrap_or_default().to_string(),
            });
        }
    }
    formats.dedup();
    Release {
        id: value["id"].as_str().unwrap_or_default().to_string(),
        title: value["title"].as_str().unwrap_or_default().to_string(),
        artist: credit(&value["artist-credit"]),
        year: value["date"]
            .as_str()
            .and_then(|d| d.get(..4))
            .and_then(|y| y.parse().ok()),
        country: value["country"].as_str().unwrap_or_default().to_string(),
        format: formats.join(" + "),
        score: value["score"].as_i64().unwrap_or(100),
        tracks,
    }
}

/// Search MusicBrainz for releases of this album; the closest in track count first, each with
/// its track list. Sends the album title and artist only.
pub fn find_releases(album: &str, artist: &str, track_count: usize) -> Result<Vec<Release>> {
    if album.trim().is_empty() {
        bail!("This album has no title to search for.");
    }
    let escape = |s: &str| s.replace('\\', "\\\\").replace('"', "\\\"");
    let mut query = format!("release:\"{}\"", escape(album));
    if !artist.trim().is_empty() {
        query.push_str(&format!(" AND artist:\"{}\"", escape(artist)));
    }
    let search = musicbrainz_get("release/", &[("query", query.as_str()), ("limit", "10")])?;
    let mut found: Vec<Release> = search["releases"]
        .as_array()
        .into_iter()
        .flatten()
        .map(parse_release)
        .filter(|r| r.score >= 60)
        .collect();
    let count = |r: &Release| {
        search["releases"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|v| v["id"].as_str() == Some(&r.id))
            .and_then(|v| v["track-count"].as_u64())
            .unwrap_or(0) as usize
    };
    found.sort_by_key(|r| (count(r).abs_diff(track_count), -r.score));
    found.truncate(4);
    // The search gives no track lists; fetch them for the best few.
    let mut releases = vec![];
    for release in found {
        let full = musicbrainz_get(
            &format!("release/{}", release.id),
            &[("inc", "recordings+artist-credits")],
        )?;
        let mut full = parse_release(&full);
        full.score = release.score;
        releases.push(full);
    }
    Ok(releases)
}

/// For each song, the release track it is, if any: by disc and track number when they agree
/// in length, else by title, else by length alone.
pub fn match_tracks<'a>(tracks: &[Track], release: &'a Release) -> Vec<Option<&'a ReleaseTrack>> {
    let mut used = HashSet::new();
    let close =
        |t: &Track, r: &ReleaseTrack| r.seconds.is_none_or(|s| (s - t.duration).abs() <= 5.);
    let mut picks: Vec<Option<usize>> = tracks
        .iter()
        .map(|t| {
            let disc = t.disc.max(1) as u32;
            release.tracks.iter().position(|r| {
                t.track_number > 0
                    && r.number == t.track_number as u32
                    && r.disc == disc
                    && close(t, r)
            })
        })
        .collect();
    for pick in picks.iter().flatten() {
        used.insert(*pick);
    }
    for (i, track) in tracks.iter().enumerate() {
        if picks[i].is_some() {
            continue;
        }
        let title = simple(&track.title);
        picks[i] = release
            .tracks
            .iter()
            .enumerate()
            .filter(|(j, _)| !used.contains(j))
            .find(|(_, r)| !title.is_empty() && simple(&r.title) == title)
            .or_else(|| {
                release
                    .tracks
                    .iter()
                    .enumerate()
                    .filter(|(j, r)| {
                        !used.contains(j)
                            && r.seconds.is_some_and(|s| (s - track.duration).abs() <= 1.5)
                    })
                    .min_by(|a, b| {
                        let d = |r: &ReleaseTrack| (r.seconds.unwrap_or(0.) - track.duration).abs();
                        d(a.1).total_cmp(&d(b.1))
                    })
            })
            .map(|(j, _)| j);
        if let Some(j) = picks[i] {
            used.insert(j);
        }
    }
    picks
        .into_iter()
        .map(|p| p.map(|j| &release.tracks[j]))
        .collect()
}

/// The tag changes that would make each song match the release. Only fields that differ.
pub fn release_edits(tracks: &[Track], release: &Release) -> Vec<(String, TagEdit)> {
    let album_artist = if release.artist.is_empty() {
        None
    } else {
        Some(release.artist.clone())
    };
    tracks
        .iter()
        .zip(match_tracks(tracks, release))
        .filter_map(|(track, found)| {
            let found = found?;
            let differs =
                |now: &str, new: &str| (!new.is_empty() && now != new).then(|| new.to_string());
            let edit = TagEdit {
                title: differs(&track.title, &found.title),
                artist: differs(&track.artist, &found.artist),
                album: differs(&track.album, &release.title),
                album_artist: album_artist
                    .as_ref()
                    .and_then(|a| differs(&track.album_artist, a)),
                year: release.year.filter(|y| *y as i64 != track.year),
                track_number: (found.number as i64 != track.track_number).then_some(found.number),
                musicbrainz_id: (!found.recording_id.is_empty()
                    && track.musicbrainz_id.as_deref() != Some(found.recording_id.as_str()))
                .then(|| found.recording_id.clone()),
                genre: None,
            };
            (!edit.is_empty()).then(|| (track.id.clone(), edit))
        })
        .collect()
}

/// An album whose tags have gaps.
#[derive(Clone, Debug, PartialEq)]
pub struct AlbumIssue {
    pub album: String,
    pub artist: String,
    pub track_ids: Vec<String>,
    /// What is wrong, in words.
    pub problems: Vec<&'static str>,
}

/// Albums missing a year or track numbers, or whose songs have different artists and no album
/// artist to hold them together.
pub fn album_issues(tracks: &[Track]) -> Vec<AlbumIssue> {
    let mut albums: HashMap<(String, String), Vec<&Track>> = HashMap::new();
    for track in tracks
        .iter()
        .filter(|t| !t.missing && t.cue.is_none() && !t.is_streamed() && !t.album.trim().is_empty())
    {
        // Without an album artist, an album is its songs in one folder (a compilation's songs
        // have different artists).
        let owner = if track.album_artist.is_empty() {
            crate::browse::folder_of(track.file_path()).to_lowercase()
        } else {
            track.album_artist.to_lowercase()
        };
        albums
            .entry((track.album.to_lowercase(), owner))
            .or_default()
            .push(track);
    }
    let mut issues: Vec<AlbumIssue> = albums
        .into_values()
        .filter_map(|list| {
            let mut problems = vec![];
            if list.iter().all(|t| t.year <= 0) {
                problems.push("no year");
            }
            if list.iter().any(|t| t.track_number <= 0) {
                problems.push("track numbers missing");
            }
            let artists: HashSet<String> = list.iter().map(|t| t.artist.to_lowercase()).collect();
            if artists.len() > 1 && list.iter().all(|t| t.album_artist.is_empty()) {
                problems.push("no album artist");
            }
            (!problems.is_empty()).then(|| AlbumIssue {
                album: list[0].album.clone(),
                artist: if artists.len() > 1 && list[0].album_artist.is_empty() {
                    "Various artists".to_string()
                } else {
                    list[0].display_album_artist().to_string()
                },
                track_ids: list.iter().map(|t| t.id.clone()).collect(),
                problems,
            })
        })
        .collect();
    issues.sort_by(|a, b| {
        (a.artist.to_lowercase(), a.album.to_lowercase())
            .cmp(&(b.artist.to_lowercase(), b.album.to_lowercase()))
    });
    issues
}

// ---------------------------------------------------------------- organize

pub const PATTERNS: [&str; 3] = [
    "{album_artist}/{album}/{disc-}{track} {title}",
    "{album_artist}/{year} - {album}/{track} {title}",
    "{artist}/{title}",
];

/// One file to move.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Move {
    pub id: String,
    pub from: String,
    pub to: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Plan {
    pub moves: Vec<Move>,
    /// Already where the pattern puts them.
    pub in_place: usize,
    /// Left alone, and why.
    pub skipped: Vec<(String, String)>,
}

/// Safe as one part of a Windows path: no reserved characters or names, no trailing dots.
fn clean(part: &str) -> String {
    let mut out: String = part
        .chars()
        .map(|c| {
            if "<>:\"/\\|?*".contains(c) || c.is_control() {
                '_'
            } else {
                c
            }
        })
        .collect();
    out = out.trim().trim_end_matches(['.', ' ']).to_string();
    let reserved = [
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "LPT1", "LPT2", "LPT3",
    ];
    if reserved.contains(&out.to_uppercase().as_str()) {
        out.push('_');
    }
    if out.chars().count() > 120 {
        out = out
            .chars()
            .take(120)
            .collect::<String>()
            .trim_end()
            .to_string();
    }
    out
}

/// The path, below the music folder, that `pattern` gives this song (without the extension).
pub fn pattern_path(pattern: &str, track: &Track) -> String {
    let or = |value: &str, fallback: &str| {
        if value.trim().is_empty() {
            fallback.to_string()
        } else {
            value.to_string()
        }
    };
    let artist = or(&track.artist, "Unknown artist");
    let values: [(&str, String); 8] = [
        (
            "{album_artist}",
            or(track.display_album_artist(), "Unknown artist"),
        ),
        ("{artist}", artist),
        ("{album}", or(&track.album, "Unknown album")),
        ("{title}", or(&track.title, "Untitled")),
        (
            "{year}",
            if track.year > 0 {
                track.year.to_string()
            } else {
                "0000".into()
            },
        ),
        (
            "{track}",
            if track.track_number > 0 {
                format!("{:02}", track.track_number)
            } else {
                "00".into()
            },
        ),
        ("{disc}", track.disc.max(1).to_string()),
        // "2-" before the track number on multi-disc albums only.
        (
            "{disc-}",
            if track.disc > 1 {
                format!("{}-", track.disc)
            } else {
                String::new()
            },
        ),
    ];
    pattern
        .split(['/', '\\'])
        .map(|part| {
            let mut part = part.to_string();
            for (key, value) in &values {
                part = part.replace(key, value);
            }
            clean(&part)
        })
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        // Folders: `\` on Windows, `/` on Linux (where `\` is a letter a name can hold).
        .join(std::path::MAIN_SEPARATOR_STR)
}

/// Where each song would go under `pattern`, inside the music folder it is already in. Songs
/// that would end up with the same name as another stay where they are.
pub fn plan_organize(tracks: &[Track], roots: &[String], pattern: &str) -> Plan {
    let mut plan = Plan::default();
    let mut wanted: Vec<(&Track, String)> = vec![];
    // Songs on a music server have no file here to move.
    for track in tracks.iter().filter(|t| !t.is_streamed()) {
        let skip =
            |plan: &mut Plan, why: &str| plan.skipped.push((track.path.clone(), why.to_string()));
        if track.cue.is_some() {
            skip(&mut plan, "part of a CUE sheet");
            continue;
        }
        if track.missing {
            skip(&mut plan, "file is missing");
            continue;
        }
        let lower = track.path.to_lowercase();
        let Some(root) = roots
            .iter()
            .filter(|r| lower.starts_with(&r.trim_end_matches(['\\', '/']).to_lowercase()))
            .max_by_key(|r| r.len())
        else {
            skip(&mut plan, "not inside a music folder");
            continue;
        };
        let extension = Path::new(&track.path)
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        let relative = pattern_path(pattern, track);
        if relative.is_empty() {
            skip(&mut plan, "the pattern gives no name");
            continue;
        }
        wanted.push((
            track,
            format!(
                "{}{}{relative}.{extension}",
                root.trim_end_matches(['\\', '/']),
                std::path::MAIN_SEPARATOR
            ),
        ));
    }
    let mut count: HashMap<String, usize> = HashMap::new();
    for (_, to) in &wanted {
        *count.entry(to.to_lowercase()).or_default() += 1;
    }
    for (track, to) in wanted {
        if count[&to.to_lowercase()] > 1 {
            plan.skipped.push((
                track.path.clone(),
                "another song would get the same name".into(),
            ));
        } else if to.eq_ignore_ascii_case(&track.path) {
            plan.in_place += 1;
        } else if Path::new(&to).exists() {
            plan.skipped.push((
                track.path.clone(),
                "a file with that name is already there".into(),
            ));
        } else {
            plan.moves.push(Move {
                id: track.id.clone(),
                from: track.path.clone(),
                to,
            });
        }
    }
    plan
}

/// Files that travel with a song: its lyrics.
fn companions(path: &Path) -> Vec<PathBuf> {
    ["lrc", "txt"]
        .iter()
        .map(|e| path.with_extension(e))
        .filter(|p| p.is_file())
        .collect()
}

/// Remove `folder` and its parents while they are empty, stopping at `stop`.
fn prune(folder: &Path, stop: &Path) {
    let mut folder = folder.to_path_buf();
    while folder.starts_with(stop) && folder != stop && std::fs::remove_dir(&folder).is_ok() {
        let Some(parent) = folder.parent() else { break };
        folder = parent.to_path_buf();
    }
}

/// Files that could not be handled, each with the reason.
pub type Failures = Vec<(String, String)>;

/// Pictures in a folder: album covers and the like.
fn pictures(folder: &Path) -> Vec<PathBuf> {
    std::fs::read_dir(folder)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.is_file()
                && p.extension().is_some_and(|e| {
                    matches!(
                        e.to_string_lossy().to_lowercase().as_str(),
                        "jpg" | "jpeg" | "png" | "webp" | "gif" | "bmp"
                    )
                })
        })
        .collect()
}

/// Whether nothing but pictures is left in `folder`.
fn only_pictures(folder: &Path) -> bool {
    std::fs::read_dir(folder)
        .into_iter()
        .flatten()
        .flatten()
        .all(|e| pictures(folder).contains(&e.path()))
}

impl Library {
    /// Carry out `moves`, updating the library as each file moves. Returns the moves made and
    /// the failures. What was moved is remembered so it can be undone.
    pub fn organize(&self, moves: &[Move], roots: &[String]) -> Result<(Vec<Move>, Failures)> {
        let mut done = vec![];
        let mut failed = vec![];
        for m in moves {
            match self.move_track(m, roots) {
                Ok(()) => done.push(m.clone()),
                Err(e) => failed.push((m.from.clone(), format!("{e:#}"))),
            }
        }
        // A folder whose songs all went to one new folder takes its pictures along.
        let mut destinations: HashMap<PathBuf, HashSet<PathBuf>> = HashMap::new();
        for m in &done {
            if let (Some(old), Some(new)) = (Path::new(&m.from).parent(), Path::new(&m.to).parent())
            {
                destinations
                    .entry(old.to_path_buf())
                    .or_default()
                    .insert(new.to_path_buf());
            }
        }
        let mut carried = vec![];
        for (old, new) in destinations {
            let new: Vec<PathBuf> = new.into_iter().collect();
            let [new] = &new[..] else { continue };
            if !old.is_dir() || !only_pictures(&old) {
                continue;
            }
            for picture in pictures(&old) {
                let Some(name) = picture.file_name() else {
                    continue;
                };
                let m = Move {
                    id: String::new(),
                    from: picture.to_string_lossy().into(),
                    to: new.join(name).to_string_lossy().into(),
                };
                if self.move_track(&m, roots).is_ok() {
                    carried.push(m);
                }
            }
        }
        let count = done.len();
        done.extend(carried);
        if !done.is_empty() {
            self.set_json("organize_undo", &done)?;
        }
        done.truncate(count);
        Ok((done, failed))
    }

    /// Songs whose cover is the picture at `from` now find it at `to`.
    fn retarget_artwork(&self, from: &str, to: &str) -> Result<()> {
        let ids: Vec<String> = self
            .connection()?
            .prepare("SELECT id FROM tracks WHERE json_extract(data,'$.artwork') = ?")?
            .query_map([from], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        for mut track in self.tracks_by_ids(&ids)? {
            track.artwork = Some(to.to_string());
            self.upsert(&track)?;
        }
        Ok(())
    }

    /// Move one file: a song (updating the library), or with no id, a picture.
    fn move_track(&self, m: &Move, roots: &[String]) -> Result<()> {
        let (from, to) = (Path::new(&m.from), Path::new(&m.to));
        let root = roots
            .iter()
            .map(Path::new)
            .filter(|r| from.starts_with(r))
            .max_by_key(|r| r.as_os_str().len());
        if m.id.is_empty() {
            if to.exists() {
                bail!("{} already exists", to.display());
            }
            if let Some(parent) = to.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::rename(from, to)
                .with_context(|| format!("Could not move {}", from.display()))?;
            self.retarget_artwork(&m.from, &m.to)?;
            if let (Some(folder), Some(root)) = (from.parent(), root) {
                prune(folder, root);
            }
            return Ok(());
        }
        let mut track = self
            .track(&m.id)?
            .context("The song is no longer in the library")?;
        if track.path != m.from {
            bail!("The song moved since the plan was made");
        }
        if to.exists() {
            bail!("{} already exists", to.display());
        }
        if let Some(parent) = to.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::rename(from, to).with_context(|| format!("Could not move {}", from.display()))?;
        for companion in companions(from) {
            if let Some(extension) = companion.extension() {
                let _ = std::fs::rename(&companion, to.with_extension(extension));
            }
        }
        track.path = m.to.clone();
        self.upsert(&track)?;
        if let (Some(folder), Some(root)) = (from.parent(), root) {
            prune(folder, root);
        }
        Ok(())
    }

    /// Whether the last organize can be undone.
    pub fn can_undo_organize(&self) -> bool {
        self.get_json::<Vec<Move>>("organize_undo")
            .ok()
            .flatten()
            .is_some_and(|m| !m.is_empty())
    }

    /// Put the files of the last organize back where they were. Returns how many moved back.
    pub fn undo_organize(&self) -> Result<usize> {
        let moves: Vec<Move> = self.get_json("organize_undo")?.unwrap_or_default();
        let roots = self.roots()?;
        let mut back = 0;
        for m in moves.iter().rev() {
            let reverse = Move {
                id: m.id.clone(),
                from: m.to.clone(),
                to: m.from.clone(),
            };
            if self.move_track(&reverse, &roots).is_ok() && !m.id.is_empty() {
                back += 1;
            }
        }
        self.set_json("organize_undo", &Vec::<Move>::new())?;
        Ok(back)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(id: &str, artist: &str, title: &str, duration: f64, format: &str) -> Track {
        Track {
            id: id.into(),
            path: format!("C:\\Music\\{id}.{}", format.to_lowercase()),
            artist: artist.into(),
            title: title.into(),
            duration,
            format: format.into(),
            content_hash: id.into(),
            ..Default::default()
        }
    }

    #[test]
    fn quick_hash_matches_are_confirmed_with_the_whole_file() {
        let dir = tempfile::tempdir().unwrap();
        let mut bytes = vec![7u8; 3 << 20];
        let write = |name: &str, bytes: &[u8]| {
            let path = dir.path().join(name);
            std::fs::write(&path, bytes).unwrap();
            path.to_string_lossy().to_string()
        };
        let (a, b) = (write("a.flac", &bytes), write("b.flac", &bytes));
        bytes[3 << 19] = 8; // the middle, which a quick hash does not read
        let c = write("c.flac", &bytes);
        let quick = crate::scan::quick_hash(Path::new(&a), bytes.len() as u64).unwrap();
        assert_eq!(
            quick,
            crate::scan::quick_hash(Path::new(&c), bytes.len() as u64).unwrap()
        );
        let with = |id: &str, path: &str| Track {
            path: path.into(),
            content_hash: quick.clone(),
            ..track(id, id, id, 100., "FLAC")
        };
        let groups = find_duplicates(&[with("a", &a), with("b", &b), with("c", &c)]);
        assert_eq!(groups.len(), 1);
        let mut ids: Vec<&str> = groups[0].tracks.iter().map(|t| t.id.as_str()).collect();
        ids.sort();
        assert_eq!(ids, ["a", "b"]);
    }

    #[test]
    fn versions_of_a_song_are_not_duplicates() {
        let on_album = |id: &str, title: &str| Track {
            album: "Aozora Jumping Heart".into(),
            ..track(id, "Aqours", title, 262., "MP3")
        };
        let tracks = vec![
            on_album("a", "Aozora Jumping Heart"),
            on_album("b", "Aozora Jumping Heart (Off Vocal)"),
            on_album("c", "Aozora Jumping Heart (Instrumental)"),
            on_album("d", "Aozora Jumping Heart (Japanese ver.)"),
            on_album("e", "Aozora Jumping Heart [Live]"),
            on_album("f", "Aozora Jumping Heart（Off Vocal）"),
            on_album("g", "Aozora Jumping Heart - Instrumental"),
        ];
        let groups = find_duplicates(&tracks);
        // Only "(Off Vocal)" and "（Off Vocal）" (full-width brackets) are the same song.
        assert_eq!(groups.len(), 1, "{groups:?}");
        let mut ids: Vec<&str> = groups[0].tracks.iter().map(|t| t.id.as_str()).collect();
        ids.sort();
        assert_eq!(ids, ["b", "f"]);
        // Remasters and explicit tags still count as the same recording.
        for remark in [
            "(2011 Remaster)",
            "[Explicit]",
            "(Remastered 2009)",
            "(Album Version)",
        ] {
            let pair = vec![
                on_album("x", "Song"),
                on_album("y", &format!("Song {remark}")),
            ];
            assert_eq!(find_duplicates(&pair).len(), 1, "{remark}");
        }
        assert!(!same_recording("Remix"));
        assert!(!same_recording("Taylor's Version"));
    }

    #[test]
    fn finds_duplicates_and_keeps_the_best() {
        let mut same_file = track("e", "X", "Other", 50., "MP3");
        same_file.content_hash = "shared".into();
        let mut same_file2 = track("f", "Y", "Else", 90., "MP3");
        same_file2.content_hash = "shared".into();
        let tracks = vec![
            track("a", "Band", "Song", 200., "MP3"),
            track("b", "band", "Song (2011 Remaster)", 201., "FLAC"),
            track("c", "Band", "Song", 260., "FLAC"),
            track("d", "Band", "Another", 200., "MP3"),
            Track {
                album: "Live".into(),
                ..track("g", "Band", "Song", 200.5, "FLAC")
            },
            same_file,
            same_file2,
        ];
        let groups = find_duplicates(&tracks);
        assert_eq!(groups.len(), 2);
        let song = groups
            .iter()
            .find(|g| g.tracks.len() == 2 && g.tracks.iter().any(|t| t.id == "a"))
            .unwrap();
        assert_eq!(song.tracks[song.keep].id, "b");
        assert!(
            groups
                .iter()
                .any(|g| g.tracks.iter().all(|t| t.content_hash == "shared"))
        );
    }

    #[test]
    fn merging_moves_plays_history_and_playlists() {
        let dir = tempfile::tempdir().unwrap();
        let library = Library::open(dir.path()).unwrap();
        let mut keep = track("k", "A", "S", 100., "FLAC");
        keep.play_count = 2;
        let mut other = track("o", "A", "S", 100., "MP3");
        other.play_count = 3;
        other.rating = 5;
        library.upsert(&keep).unwrap();
        library.upsert(&other).unwrap();
        let db = library.connection().unwrap();
        db.execute(
            "UPDATE tracks SET play_count=?, rating=? WHERE id=?",
            params![3, 5, "o"],
        )
        .unwrap();
        db.execute("UPDATE tracks SET play_count=? WHERE id=?", params![2, "k"])
            .unwrap();
        library
            .record_listen(&crate::model::Listen {
                id: "l".into(),
                track_id: "o".into(),
                ..Default::default()
            })
            .unwrap();
        library
            .save_playlist(&crate::model::Playlist {
                id: "p".into(),
                name: "P".into(),
                query: None,
                track_ids: vec!["o".into(), "k".into()],
                updated_at: 0,
                ..Default::default()
            })
            .unwrap();
        let (keep, other) = (
            library.track("k").unwrap().unwrap(),
            library.track("o").unwrap().unwrap(),
        );
        library.merge_duplicates(&keep, &[other], false).unwrap();
        assert!(library.track("o").unwrap().is_none());
        let kept = library.track("k").unwrap().unwrap();
        assert_eq!((kept.play_count, kept.rating), (5, 5));
        assert_eq!(library.history(5).unwrap()[0].track_id, "k");
        assert_eq!(library.playlists().unwrap()[0].track_ids, vec!["k"]);
    }

    #[test]
    fn matches_release_tracks_and_plans_tags() {
        let release = parse_release(&serde_json::json!({
            "id": "r1", "title": "Real Album", "date": "1999-05-01", "country": "GB",
            "artist-credit": [{"name": "The Band", "joinphrase": ""}],
            "media": [{"position": 1, "format": "CD", "tracks": [
                {"position": 1, "title": "First", "length": 200000, "recording": {"id": "11111111-1111-1111-1111-111111111111", "artist-credit": [{"name": "The Band"}]}},
                {"position": 2, "title": "Second", "length": 180000, "recording": {"id": "22222222-2222-2222-2222-222222222222", "artist-credit": [{"name": "The Band"}]}}
            ]}]
        }));
        assert_eq!(release.year, Some(1999));
        assert_eq!(release.tracks[1].seconds, Some(180.));
        let mut a = track("a", "The Band", "first", 200.5, "FLAC");
        a.album = "Real Album".into();
        let mut b = track("b", "The Band", "track 2", 180.4, "FLAC");
        b.album = "real album".into();
        let edits = release_edits(&[b.clone(), a.clone()], &release);
        let for_b = &edits.iter().find(|e| e.0 == "b").unwrap().1;
        assert_eq!(for_b.title.as_deref(), Some("Second"));
        assert_eq!(for_b.track_number, Some(2));
        assert_eq!(for_b.year, Some(1999));
        assert_eq!(for_b.album.as_deref(), Some("Real Album"));
        let for_a = &edits.iter().find(|e| e.0 == "a").unwrap().1;
        assert_eq!(for_a.title.as_deref(), Some("First"));
        assert_eq!(for_a.album, None);
    }

    #[test]
    fn clashing_names_stay_put() {
        let mut a = track("a", "A", "Same", 1., "FLAC");
        a.album = "X".into();
        let mut b = a.clone();
        b.id = "b".into();
        b.path = "C:\\Music\\other.flac".into();
        let plan = plan_organize(&[a, b], &["C:\\Music".to_string()], PATTERNS[0]);
        assert!(plan.moves.is_empty());
        assert_eq!(plan.skipped.len(), 2);
    }

    #[test]
    fn finds_albums_with_gaps() {
        let mut a = track("a", "One", "x", 1., "FLAC");
        a.album = "Mix".into();
        a.year = 2001;
        a.track_number = 1;
        let mut b = track("b", "Two", "y", 1., "FLAC");
        b.album = "Mix".into();
        b.year = 2001;
        b.track_number = 2;
        let mut c = track("c", "Solo", "z", 1., "FLAC");
        c.album = "Fine".into();
        c.year = 1990;
        c.track_number = 1;
        let issues = album_issues(&[a, b, c]);
        // "Mix" has two artists in one folder and no album artist; "Fine" is fine.
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].problems, vec!["no album artist"]);
        assert_eq!(issues[0].artist, "Various artists");
        let mut d = track("d", "Band", "w", 1., "FLAC");
        d.album = "Bare".into();
        let issues = album_issues(&[d]);
        assert_eq!(issues[0].problems, vec!["no year", "track numbers missing"]);
    }

    #[test]
    fn organizes_by_pattern_and_undoes() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("Music");
        std::fs::create_dir_all(root.join("messy")).unwrap();
        let file = root.join("messy").join("x.flac");
        std::fs::write(&file, b"audio").unwrap();
        std::fs::write(root.join("messy").join("x.lrc"), b"[00:01.00]hi").unwrap();
        let picture = root.join("messy").join("cover.jpg");
        std::fs::write(&picture, b"jpeg").unwrap();
        let library = Library::open(dir.path().join("data")).unwrap();
        let root_text = root.to_string_lossy().to_string();
        library.add_root(&root_text).unwrap();
        let mut song = track("s", "Artist: Name", "Why?", 100., "FLAC");
        song.path = file.to_string_lossy().into();
        song.album = "CON".into();
        song.track_number = 3;
        song.artwork = Some(picture.to_string_lossy().into());
        library.upsert(&song).unwrap();
        assert_eq!(
            pattern_path(PATTERNS[0], &song),
            ["Artist_ Name", "CON_", "03 Why_"].join(std::path::MAIN_SEPARATOR_STR)
        );
        let plan = plan_organize(
            &[song.clone()],
            std::slice::from_ref(&root_text),
            PATTERNS[0],
        );
        assert_eq!(plan.moves.len(), 1);
        let (done, failed) = library
            .organize(&plan.moves, std::slice::from_ref(&root_text))
            .unwrap();
        assert!(failed.is_empty(), "{failed:?}");
        let moved = Path::new(&done[0].to);
        assert!(moved.is_file());
        assert!(moved.with_extension("lrc").is_file());
        // The cover went along, and the song knows where.
        let cover = moved.with_file_name("cover.jpg");
        assert!(cover.is_file());
        assert_eq!(
            library.track("s").unwrap().unwrap().artwork.as_deref(),
            Some(&*cover.to_string_lossy())
        );
        assert!(!root.join("messy").exists(), "the empty folder is removed");
        assert_eq!(library.track("s").unwrap().unwrap().path, done[0].to);
        // Planning again finds it in place.
        let moved_song = library.track("s").unwrap().unwrap();
        assert_eq!(
            plan_organize(&[moved_song], std::slice::from_ref(&root_text), PATTERNS[0]).in_place,
            1
        );
        assert!(library.can_undo_organize());
        assert_eq!(library.undo_organize().unwrap(), 1);
        assert!(file.is_file());
        assert!(root.join("messy").join("x.lrc").is_file());
        assert!(picture.is_file());
        assert_eq!(
            library.track("s").unwrap().unwrap().artwork.as_deref(),
            Some(&*picture.to_string_lossy())
        );
        assert!(
            !cover.parent().unwrap().exists(),
            "the new folder is gone again"
        );
        assert!(!library.can_undo_organize());
    }
}
