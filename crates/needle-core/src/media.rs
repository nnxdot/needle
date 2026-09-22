//! Lyrics, artist photos, and album art: local sources first, then optional online lookups.
//!
//! Online sources: LRCLIB for lyrics, MusicBrainz + Wikidata + Wikimedia Commons for artist
//! photos, MusicBrainz + Cover Art Archive for album covers. Results, including "nothing found",
//! are cached so a library is not re-queried on every play.
use crate::{
    database::Library,
    integrations::{client, musicbrainz_get},
    model::Track,
};
use anyhow::{Result, bail};
use lofty::{file::TaggedFileExt, tag::ItemKey};
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    io::Read,
    path::{Path, PathBuf},
};

const WEEK: i64 = 7 * 86400;
const IMAGE_LIMIT: u64 = 12 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LyricLine {
    /// Seconds from the start of the track.
    pub time: f64,
    pub text: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum LyricsSource {
    /// An `.lrc` or `.txt` file next to the audio file.
    Sidecar,
    /// The file's own lyrics tag.
    Embedded,
    Lrclib,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Lyrics {
    /// Timed lines, sorted. Empty when only plain text is known.
    pub lines: Vec<LyricLine>,
    pub plain: String,
    pub instrumental: bool,
    pub source: LyricsSource,
}

impl Lyrics {
    fn from_text(text: &str, source: LyricsSource) -> Option<Self> {
        let text = text.trim();
        if text.is_empty() {
            return None;
        }
        let lines = parse_lrc(text);
        let plain = if lines.is_empty() {
            text.to_string()
        } else {
            lines
                .iter()
                .map(|l| l.text.as_str())
                .collect::<Vec<_>>()
                .join("\n")
        };
        Some(Self {
            lines,
            plain,
            instrumental: false,
            source,
        })
    }
    /// Index of the line being sung at `position` seconds.
    pub fn line_at(&self, position: f64) -> Option<usize> {
        let next = self.lines.partition_point(|l| l.time <= position);
        next.checked_sub(1)
    }
}

/// Parse LRC: `[mm:ss.xx]text`, several stamps per line, and an optional `[offset:±ms]` tag.
pub fn parse_lrc(text: &str) -> Vec<LyricLine> {
    let mut offset = 0.;
    let mut lines = vec![];
    for raw in text.lines() {
        let mut rest = raw.trim();
        let mut times = vec![];
        while let Some(stripped) = rest.strip_prefix('[') {
            let Some(end) = stripped.find(']') else { break };
            let tag = &stripped[..end];
            rest = stripped[end + 1..].trim_start();
            if let Some(ms) = tag.strip_prefix("offset:") {
                offset = ms.trim().parse::<f64>().unwrap_or(0.) / 1000.;
            } else if let Some((m, s)) = tag.split_once(':')
                && let (Ok(m), Ok(s)) = (m.trim().parse::<f64>(), s.trim().parse::<f64>())
            {
                times.push(m * 60. + s);
            }
        }
        for time in times {
            lines.push(LyricLine {
                time,
                text: rest.to_string(),
            });
        }
    }
    // A positive offset makes lyrics appear sooner.
    for line in &mut lines {
        line.time = (line.time - offset).max(0.);
    }
    lines.sort_by(|a, b| a.time.total_cmp(&b.time));
    lines
}

/// Lyrics from next to the file or inside it. Timed lyrics win over plain ones.
pub fn local_lyrics(track: &Track) -> Option<Lyrics> {
    let path = Path::new(&track.path);
    let mut found = vec![];
    for extension in ["lrc", "txt"] {
        if let Ok(text) = std::fs::read_to_string(path.with_extension(extension)) {
            found.extend(Lyrics::from_text(&text, LyricsSource::Sidecar));
        }
    }
    if let Ok(file) = lofty::read_from_path(path) {
        for tag in file.tags() {
            if let Some(text) = tag.get_string(ItemKey::Lyrics) {
                found.extend(Lyrics::from_text(text, LyricsSource::Embedded));
            }
        }
    }
    found.sort_by_key(|l| l.lines.is_empty());
    found.into_iter().next()
}

fn cached<T: for<'de> Deserialize<'de>>(library: &Library, key: &str) -> Result<Option<T>> {
    let data: Option<String> = library
        .connection()?
        .query_row(
            "SELECT data FROM cache WHERE key=? AND expires>?",
            params![key, chrono::Utc::now().timestamp()],
            |r| r.get(0),
        )
        .optional()?;
    Ok(data.and_then(|d| serde_json::from_str(&d).ok()))
}
fn remember<T: Serialize>(library: &Library, key: &str, value: &T, seconds: i64) -> Result<()> {
    library.connection()?.execute(
        "INSERT INTO cache VALUES (?,?,?) ON CONFLICT(key) DO UPDATE SET expires=excluded.expires,data=excluded.data",
        params![key, chrono::Utc::now().timestamp() + seconds, serde_json::to_string(value)?],
    )?;
    Ok(())
}

fn lrclib_parse(value: &Value) -> Option<Lyrics> {
    if value["instrumental"].as_bool() == Some(true) {
        return Some(Lyrics {
            lines: vec![],
            plain: String::new(),
            instrumental: true,
            source: LyricsSource::Lrclib,
        });
    }
    let synced = value["syncedLyrics"].as_str().unwrap_or_default();
    let plain = value["plainLyrics"].as_str().unwrap_or_default();
    Lyrics::from_text(
        if synced.trim().is_empty() {
            plain
        } else {
            synced
        },
        LyricsSource::Lrclib,
    )
}

/// Local lyrics, or (when `online`) LRCLIB's. Sends artist, title, album and length only.
pub fn lyrics(library: &Library, track: &Track, online: bool) -> Result<Option<Lyrics>> {
    if let Some(local) = local_lyrics(track) {
        return Ok(Some(local));
    }
    if !online || track.title.is_empty() || track.artist.is_empty() {
        return Ok(None);
    }
    let key = format!(
        "lrclib:{}:{}:{}:{:.0}",
        track.artist, track.title, track.album, track.duration
    );
    if let Some(hit) = cached::<Option<Lyrics>>(library, &key)? {
        return Ok(hit);
    }
    let duration = format!("{:.0}", track.duration);
    let response = client()?
        .get("https://lrclib.net/api/get")
        .query(&[
            ("artist_name", track.artist.as_str()),
            ("track_name", track.title.as_str()),
            ("album_name", track.album.as_str()),
            ("duration", duration.as_str()),
        ])
        .send()?;
    let found = if response.status() == reqwest::StatusCode::NOT_FOUND {
        None
    } else {
        lrclib_parse(&response.error_for_status()?.json::<Value>()?)
    };
    remember(
        library,
        &key,
        &found,
        if found.is_some() { 30 * 86400 } else { WEEK },
    )?;
    Ok(found)
}

fn download_image(url: &str, destination: &Path) -> Result<()> {
    let response = client()?.get(url).send()?.error_for_status()?;
    if !response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|h| h.to_str().ok())
        .is_some_and(|s| s.starts_with("image/"))
    {
        bail!("The image service did not return an image");
    }
    let mut bytes = vec![];
    response.take(IMAGE_LIMIT + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > IMAGE_LIMIT {
        bail!("Image exceeds 12 MB");
    }
    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let partial = destination.with_extension("part");
    std::fs::write(&partial, bytes)?;
    std::fs::rename(partial, destination)?;
    Ok(())
}

/// A photo of the artist, cached under `artwork/artists`. Online lookups use MusicBrainz
/// (artist search), then the artist's Wikidata image on Wikimedia Commons.
pub fn artist_image(library: &Library, name: &str, online: bool) -> Result<Option<PathBuf>> {
    if name.trim().is_empty() {
        return Ok(None);
    }
    let file = library
        .directory
        .join("artwork")
        .join("artists")
        .join(format!(
            "{}.img",
            blake3::hash(name.to_lowercase().as_bytes()).to_hex()
        ));
    if file.is_file() {
        return Ok(Some(file));
    }
    if !online {
        return Ok(None);
    }
    let key = format!("artist-image:{}", name.to_lowercase());
    if cached::<bool>(library, &key)?.is_some() {
        return Ok(None);
    }
    let url = find_artist_image(name)?;
    match url {
        Some(url) => {
            download_image(&url, &file)?;
            Ok(Some(file))
        }
        None => {
            remember(library, &key, &false, WEEK * 4)?;
            Ok(None)
        }
    }
}

fn find_artist_image(name: &str) -> Result<Option<String>> {
    let query = format!(
        "artist:\"{}\"",
        name.replace('\\', "\\\\").replace('"', "\\\"")
    );
    let search = musicbrainz_get("artist/", &[("query", query.as_str()), ("limit", "3")])?;
    let Some(artist) = search["artists"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|a| {
            a["score"].as_i64().unwrap_or(0) >= 90
                && a["name"]
                    .as_str()
                    .is_some_and(|n| n.eq_ignore_ascii_case(name))
        })
    else {
        return Ok(None);
    };
    let id = artist["id"].as_str().unwrap_or_default();
    let detail = musicbrainz_get(&format!("artist/{id}"), &[("inc", "url-rels")])?;
    let relations = detail["relations"].as_array().cloned().unwrap_or_default();
    let url = |kind: &str| {
        relations
            .iter()
            .find(|r| r["type"].as_str() == Some(kind))
            .and_then(|r| r["url"]["resource"].as_str())
            .map(String::from)
    };
    let commons_file =
        if let Some(image) = url("image").filter(|u| u.contains("commons.wikimedia.org")) {
            image.rsplit("File:").next().map(String::from)
        } else if let Some(wikidata) = url("wikidata") {
            let entity = wikidata.rsplit('/').next().unwrap_or_default().to_string();
            let data: Value = client()?
                .get("https://www.wikidata.org/w/api.php")
                .query(&[
                    ("action", "wbgetclaims"),
                    ("entity", entity.as_str()),
                    ("property", "P18"),
                    ("format", "json"),
                ])
                .send()?
                .error_for_status()?
                .json()?;
            data["claims"]["P18"][0]["mainsnak"]["datavalue"]["value"]
                .as_str()
                .map(String::from)
        } else {
            None
        };
    Ok(commons_file.map(|file| {
        format!(
            "https://commons.wikimedia.org/w/index.php?title=Special:FilePath/{}&width=600",
            file.replace(' ', "_")
        )
    }))
}

/// Find a front cover on the Cover Art Archive for tracks that have none, and apply it to every
/// track of the same album. Returns the number of tracks updated.
pub fn fetch_album_art(library: &Library, track: &Track) -> Result<usize> {
    if track.album.trim().is_empty() {
        return Ok(0);
    }
    let artist = if track.album_artist.is_empty() {
        &track.artist
    } else {
        &track.album_artist
    };
    let key = format!(
        "album-art:{}:{}",
        artist.to_lowercase(),
        track.album.to_lowercase()
    );
    if cached::<bool>(library, &key)?.is_some() {
        return Ok(0);
    }
    let escape = |s: &str| s.replace('\\', "\\\\").replace('"', "\\\"");
    let query = format!(
        "release:\"{}\" AND artist:\"{}\"",
        escape(&track.album),
        escape(artist)
    );
    let search = musicbrainz_get("release/", &[("query", query.as_str()), ("limit", "5")])?;
    let mut releases: Vec<&Value> = search["releases"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|r| r["score"].as_i64().unwrap_or(0) >= 90)
        .collect();
    // Releases that say they have front art first.
    releases.sort_by_key(|r| r["cover-art-archive"]["front"].as_bool() != Some(true));
    for release in releases {
        let id = release["id"].as_str().unwrap_or_default();
        let Ok(uuid) = uuid::Uuid::parse_str(id) else {
            continue;
        };
        let file = library
            .directory
            .join("artwork")
            .join(format!("caa-{uuid}.jpg"));
        if !file.is_file()
            && download_image(
                &format!("https://coverartarchive.org/release/{uuid}/front-500"),
                &file,
            )
            .is_err()
        {
            continue;
        }
        let path = file.to_string_lossy().to_string();
        let mut updated = 0;
        for mut sibling in library.search(&format!(
            "album_artist = {} and album = {}",
            crate::query::quote(artist),
            crate::query::quote(&track.album)
        ))? {
            if sibling.artwork.is_none() {
                sibling.artwork = Some(path.clone());
                library.upsert(&sibling)?;
                updated += 1;
            }
        }
        return Ok(updated.max(usize::from(track.artwork.is_none())));
    }
    remember(library, &key, &false, WEEK * 4)?;
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_lrc_with_repeats_offset_and_metadata() {
        let lines = parse_lrc(
            "[ar:Someone]\n[offset:500]\n[00:12.50][01:02.00]Chorus\n[00:05.00] First line\nnot timed\n",
        );
        assert_eq!(
            lines,
            vec![
                LyricLine {
                    time: 4.5,
                    text: "First line".into()
                },
                LyricLine {
                    time: 12.0,
                    text: "Chorus".into()
                },
                LyricLine {
                    time: 61.5,
                    text: "Chorus".into()
                },
            ]
        );
    }

    #[test]
    fn finds_the_current_line() {
        let lyrics = Lyrics::from_text(
            "[00:01.00]a\n[00:03.00]b\n[00:07.00]c",
            LyricsSource::Sidecar,
        )
        .unwrap();
        assert_eq!(lyrics.line_at(0.5), None);
        assert_eq!(lyrics.line_at(1.0), Some(0));
        assert_eq!(lyrics.line_at(5.0), Some(1));
        assert_eq!(lyrics.line_at(90.0), Some(2));
    }

    #[test]
    fn plain_text_is_kept_when_untimed() {
        let lyrics = Lyrics::from_text("Line one\nLine two\n", LyricsSource::Embedded).unwrap();
        assert!(lyrics.lines.is_empty());
        assert_eq!(lyrics.plain, "Line one\nLine two");
        assert!(Lyrics::from_text("  \n", LyricsSource::Embedded).is_none());
    }

    #[test]
    fn reads_lrclib_responses() {
        let synced: Value = serde_json::from_str(r#"{"instrumental":false,"plainLyrics":"a\nb","syncedLyrics":"[00:01.00]a\n[00:02.00]b"}"#).unwrap();
        let lyrics = lrclib_parse(&synced).unwrap();
        assert_eq!(lyrics.lines.len(), 2);
        assert_eq!(lyrics.source, LyricsSource::Lrclib);
        let plain: Value = serde_json::from_str(
            r#"{"instrumental":false,"plainLyrics":"just text","syncedLyrics":null}"#,
        )
        .unwrap();
        assert_eq!(lrclib_parse(&plain).unwrap().plain, "just text");
        let instrumental: Value =
            serde_json::from_str(r#"{"instrumental":true,"plainLyrics":null,"syncedLyrics":null}"#)
                .unwrap();
        assert!(lrclib_parse(&instrumental).unwrap().instrumental);
    }

    #[test]
    fn sidecar_lrc_beats_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let audio = dir.path().join("song.flac");
        std::fs::write(&audio, b"not audio").unwrap();
        std::fs::write(dir.path().join("song.lrc"), "[00:01.00]hello").unwrap();
        let track = Track {
            path: audio.to_string_lossy().into(),
            ..Default::default()
        };
        let lyrics = local_lyrics(&track).unwrap();
        assert_eq!(lyrics.source, LyricsSource::Sidecar);
        assert_eq!(lyrics.lines[0].text, "hello");
    }
}
