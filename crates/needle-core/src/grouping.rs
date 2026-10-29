//! What makes an album (Settings › Library): which album artist each song is shown under, so
//! that songs meant as one album sit together. Only what Needle shows changes; files never do.
//!
//! An album is its album artist and its name. A song's album artist is its album artist tag;
//! without one, "Various Artists" for a song tagged as part of a compilation (iTunes'
//! compilation flag, with that switch on), else its own artist. With "The folder", songs in one
//! folder with one album name and no album artist tag are one album: under their artist when
//! they share one, else under "Various Artists".
use crate::database::Library;
use crate::model::{Settings, Track};
use anyhow::Result;
use std::collections::HashMap;

/// The album artist of songs by several artists put together.
pub const VARIOUS: &str = "Various Artists";

/// How albums are made: "tags" (the album artist and album tags) or "folder".
pub const ALBUM_GROUPINGS: [&str; 2] = ["tags", "folder"];

/// The rule, from the settings.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AlbumRule {
    pub by_folder: bool,
    pub compilations: bool,
}

impl AlbumRule {
    pub fn of(settings: &Settings) -> Self {
        Self {
            by_folder: settings.album_grouping == "folder",
            compilations: settings.compilations_various,
        }
    }
}

/// The album artist tag a song was read with. Songs read before Needle kept it: their album
/// artist, unless it is just their artist (then they had none).
fn tag_of(track: &Track) -> Option<&str> {
    if track.metadata_version >= 4 {
        track.album_artist_tag.as_deref()
    } else if track.album_artist != track.artist {
        Some(track.album_artist.as_str())
    } else {
        None
    }
    .filter(|t| !t.trim().is_empty())
}

/// Each song's album artist under the rule, for songs on this device (a music server's songs
/// keep the server's).
pub fn album_artists(tracks: &[Track], rule: AlbumRule) -> Vec<String> {
    let mut out: Vec<String> = tracks
        .iter()
        .map(|t| match tag_of(t) {
            Some(tag) => tag.to_string(),
            None if rule.compilations && t.compilation => VARIOUS.to_string(),
            None => t.artist.clone(),
        })
        .collect();
    if rule.by_folder {
        // The untagged songs of each folder and album name, and their artists.
        let mut groups: HashMap<(String, String), Vec<usize>> = HashMap::new();
        for (i, t) in tracks.iter().enumerate() {
            if tag_of(t).is_none() && !t.album.trim().is_empty() {
                let folder = crate::browse::folder_of(&t.path).to_lowercase();
                groups
                    .entry((folder, t.album.trim().to_lowercase()))
                    .or_default()
                    .push(i);
            }
        }
        for members in groups.values() {
            let first = &out[members[0]];
            let alike = members
                .iter()
                .all(|&i| out[i].to_lowercase() == first.to_lowercase());
            let shared = if alike {
                first.clone()
            } else {
                VARIOUS.to_string()
            };
            for &i in members {
                out[i] = shared.clone();
            }
        }
    }
    out
}

impl Library {
    /// Give every song on this device its album artist under Settings' rule; the number of
    /// songs that moved. After each scan, and when the rule changes.
    pub fn apply_album_rule(&self) -> Result<usize> {
        let rule = AlbumRule::of(&self.settings()?);
        let db = self.connection()?;
        let mut tracks: Vec<Track> = {
            let mut stmt = db.prepare("SELECT data FROM tracks WHERE path NOT LIKE ?1")?;
            let rows = stmt.query_map([format!("{}%", crate::sources::SCHEME)], |r| {
                r.get::<_, String>(0)
            })?;
            rows.filter_map(|r| r.ok())
                .filter_map(|data| serde_json::from_str::<Track>(&data).ok())
                .collect()
        };
        let artists = album_artists(&tracks, rule);
        let mut moved = 0;
        db.execute_batch("BEGIN")?;
        for (track, artist) in tracks.iter_mut().zip(artists) {
            if track.album_artist != artist {
                track.album_artist = artist;
                Library::upsert_on(&db, track)?;
                moved += 1;
            }
        }
        db.execute_batch("COMMIT")?;
        Ok(moved)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn song(path: &str, artist: &str, album: &str, tag: Option<&str>, compilation: bool) -> Track {
        Track {
            path: path.into(),
            artist: artist.into(),
            album: album.into(),
            album_artist: tag.unwrap_or(artist).into(),
            album_artist_tag: tag.map(String::from),
            compilation,
            metadata_version: 4,
            ..Default::default()
        }
    }

    #[test]
    fn compilations_and_folders_make_one_album() {
        let songs = [
            song("/m/Hits/1.flac", "A", "Hits", None, true),
            song("/m/Hits/2.flac", "B", "Hits", None, true),
            song("/m/Mix/1.flac", "C", "Mix", None, false),
            song("/m/Mix/2.flac", "D", "Mix", None, false),
            song("/m/Solo/1.flac", "E", "Solo", None, false),
            song("/m/Solo/2.flac", "E", "Solo", None, false),
            song("/m/Tagged/1.flac", "F", "Tagged", Some("The Band"), true),
        ];
        let tags = AlbumRule {
            by_folder: false,
            compilations: true,
        };
        assert_eq!(
            album_artists(&songs, tags),
            [VARIOUS, VARIOUS, "C", "D", "E", "E", "The Band"]
        );
        // Without the compilation switch, each song keeps its own artist.
        let plain = AlbumRule {
            by_folder: false,
            compilations: false,
        };
        assert_eq!(album_artists(&songs, plain)[..2], ["A", "B"]);
        // By folder: a folder of several artists is one album.
        let folder = AlbumRule {
            by_folder: true,
            compilations: false,
        };
        assert_eq!(
            album_artists(&songs, folder),
            [VARIOUS, VARIOUS, VARIOUS, VARIOUS, "E", "E", "The Band"]
        );
    }

    #[test]
    fn the_rule_moves_songs_in_the_library_and_back() {
        let dir = tempfile::tempdir().unwrap();
        let library = Library::open(dir.path().join("data")).unwrap();
        for (id, artist) in [("a", "A"), ("b", "B")] {
            let mut t = song(&format!("/m/Hits/{id}.flac"), artist, "Hits", None, true);
            t.id = id.into();
            library.upsert(&t).unwrap();
        }
        // Out of the box, a compilation is one album by Various Artists.
        assert_eq!(library.apply_album_rule().unwrap(), 2);
        assert_eq!(library.track("a").unwrap().unwrap().album_artist, VARIOUS);
        assert_eq!(library.apply_album_rule().unwrap(), 0);
        // With the switch off, each song is its artist's again.
        let mut settings = library.settings().unwrap();
        settings.compilations_various = false;
        library.save_settings(&settings).unwrap();
        assert_eq!(library.apply_album_rule().unwrap(), 2);
        assert_eq!(library.track("b").unwrap().unwrap().album_artist, "B");
    }

    #[test]
    fn songs_read_before_keep_their_album_artist() {
        let old = Track {
            artist: "A".into(),
            album_artist: "The Band".into(),
            metadata_version: 3,
            ..Default::default()
        };
        assert_eq!(tag_of(&old), Some("The Band"));
        let untagged = Track {
            artist: "A".into(),
            album_artist: "A".into(),
            metadata_version: 3,
            ..Default::default()
        };
        assert_eq!(tag_of(&untagged), None);
    }
}
