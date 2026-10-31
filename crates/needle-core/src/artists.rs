//! Artists as people see them: who a credit names ("A feat. B" is A and B), the key they sort
//! by (a file's sort tag, accents folded, a leading "The" left out), the letter they file under,
//! and the artist list in the order chosen.
use crate::{browse::ArtistSummary, database::Library};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use unicode_normalization::UnicodeNormalization;

/// How the artist list is ordered.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtistOrder {
    /// A to Z by sort name.
    #[default]
    Name,
    MostPlayed,
    MostSongs,
    /// The artist with the newest song first.
    RecentlyAdded,
}

impl ArtistOrder {
    /// Settings value, the name people see.
    pub const ALL: [(ArtistOrder, &'static str, &'static str); 4] = [
        (ArtistOrder::Name, "name", "Name"),
        (ArtistOrder::MostPlayed, "most_played", "Most played"),
        (ArtistOrder::MostSongs, "most_songs", "Most songs"),
        (
            ArtistOrder::RecentlyAdded,
            "recently_added",
            "Recently added",
        ),
    ];

    pub fn from_name(name: &str) -> Self {
        Self::ALL
            .iter()
            .find(|(_, key, _)| *key == name)
            .map_or(Self::Name, |(order, _, _)| *order)
    }

    pub fn name(self) -> &'static str {
        Self::ALL
            .iter()
            .find(|(order, _, _)| *order == self)
            .map_or("name", |(_, key, _)| key)
    }
}

/// What the artist list shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ArtistChoice {
    pub order: ArtistOrder,
    /// Only artists who are an album's artist somewhere, not those only on someone else's song.
    pub album_artists_only: bool,
}

/// Words that bring in a guest: everything after one is someone else's part.
const GUESTS: [&str; 6] = [
    " feat. ",
    " feat ",
    " ft. ",
    " ft ",
    " featuring ",
    " (feat. ",
];
/// Separators that only a list of names uses (tags with several values come joined by these).
const LISTS: [&str; 3] = ["; ", " / ", "\u{0}"];
/// Separators a band's own name may hold ("Simon & Garfunkel", "Earth, Wind & Fire"): a credit
/// is split at these only when every part is an artist with songs of their own.
const JOINS: [&str; 5] = [" & ", ", ", " x ", " X ", " and "];

/// The names a credit holds, the main one first, without splitting at `JOINS` ("Jay & Kay
/// feat. Bo" → ["Jay & Kay", "Bo"]).
pub fn credits(credit: &str) -> Vec<String> {
    let mut main = credit.trim();
    let mut guests = "";
    let lower = main.to_lowercase();
    if let Some(at) = GUESTS.iter().filter_map(|g| lower.find(g)).min() {
        guests = &main[at..];
        main = main[..at].trim();
        // Leave the word itself out ("feat. ", or "(feat. " with its closing bracket).
        let word = GUESTS
            .iter()
            .find(|g| guests.to_lowercase().starts_with(*g))
            .map_or(0, |g| g.len());
        guests = guests[word..].trim().trim_end_matches(')').trim();
    }
    let mut names = vec![];
    for part in [main, guests] {
        let mut pieces = vec![part.to_string()];
        for list in LISTS {
            pieces = pieces
                .iter()
                .flat_map(|p| p.split(list).map(str::to_string).collect::<Vec<_>>())
                .collect();
        }
        // A guest part is a list of guests: "B & C" after "feat." is two people.
        if part == guests {
            for join in [" & ", ", ", " and "] {
                pieces = pieces
                    .iter()
                    .flat_map(|p| p.split(join).map(str::to_string).collect::<Vec<_>>())
                    .collect();
            }
        }
        for piece in pieces {
            let piece = piece.trim();
            if !piece.is_empty() && !names.iter().any(|n: &String| n.eq_ignore_ascii_case(piece)) {
                names.push(piece.to_string());
            }
        }
    }
    names
}

/// The parts of a joined name ("A & B" → ["A", "B"]), when `known` (lowercase names of artists
/// with songs of their own) has every part; otherwise the name alone.
fn split_joined(name: &str, known: &HashSet<String>) -> Vec<String> {
    for join in JOINS {
        if name.contains(join) {
            let parts: Vec<&str> = name.split(join).map(str::trim).collect();
            // Every part, or a "A, B & C" list: split at ", " and the last " & ".
            let parts: Vec<String> = parts
                .iter()
                .flat_map(|p| {
                    JOINS
                        .iter()
                        .find(|j| p.contains(*j))
                        .map_or(vec![p.to_string()], |j| {
                            p.split(j).map(|s| s.trim().to_string()).collect()
                        })
                })
                .collect();
            if parts.len() > 1 && parts.iter().all(|p| known.contains(&p.to_lowercase())) {
                return parts;
            }
        }
    }
    vec![name.to_string()]
}

/// The key an artist sorts by: the file's sort tag when it has one, else the name without a
/// leading "The "; accents folded ("Ólafur" sorts with the O's), lowercase. Names in other
/// scripts keep their letters and sort after the Latin ones.
pub fn sort_key(name: &str, sort_tag: Option<&str>) -> String {
    let base = sort_tag
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(name);
    let folded: String = base
        .nfkd()
        .filter(|c| !unicode_normalization::char::is_combining_mark(*c))
        .collect::<String>()
        .to_lowercase();
    let folded = folded.trim();
    let folded = if sort_tag.is_some_and(|s| !s.trim().is_empty()) {
        folded
    } else {
        folded.strip_prefix("the ").unwrap_or(folded)
    };
    // Leading quotes and brackets do not count ("'Til Tuesday", "(Hed) P.E.").
    folded
        .trim_start_matches(|c: char| !c.is_alphanumeric())
        .to_string()
}

/// The letter an artist is filed under in an A–Z bar: "A"–"Z", "#" for digits and signs, or the
/// first letter of another script.
pub fn index_letter(sort: &str) -> String {
    match sort.chars().next() {
        None => "#".into(),
        Some(c) if c.is_ascii_alphabetic() => c.to_ascii_uppercase().to_string(),
        Some(c) if c.is_alphabetic() => c.to_uppercase().collect(),
        Some(_) => "#".into(),
    }
}

/// The sort key for comparing: digits and signs first, then Latin letters, then other scripts.
fn order_key(sort: &str) -> (u8, String) {
    let class = match sort.chars().next() {
        Some(c) if c.is_ascii_alphabetic() => 1,
        Some(c) if c.is_alphabetic() => 2,
        _ => 0,
    };
    (class, sort.to_string())
}

#[derive(Default)]
struct Tally {
    name: String,
    sort_tag: Option<String>,
    albums: HashSet<String>,
    tracks: usize,
    duration: f64,
    play_count: i64,
    added_at: i64,
    artwork: Option<String>,
    album_artist: bool,
}

impl Library {
    /// The artist list: one row per person or band a song credits (guests after "feat." too,
    /// and both halves of "A & B" when both have songs of their own), filtered by an
    /// expression, in the order chosen.
    pub fn artists_by(&self, expression: &str, choice: ArtistChoice) -> Result<Vec<ArtistSummary>> {
        let (source, parameters) = crate::browse::filtered_source(expression)?;
        let db = self.connection()?;
        let mut stmt = db.prepare(&format!(
            "SELECT t.artist, t.album_artist, t.album, t.duration, t.play_count, t.added_at,
                json_extract(t.data,'$.artwork'), json_extract(t.data,'$.artist_sort'),
                json_extract(t.data,'$.album_artist_sort')
             FROM {source}"
        ))?;
        struct Row {
            artist: String,
            album_artist: String,
            album: String,
            duration: f64,
            play_count: i64,
            added_at: i64,
            artwork: Option<String>,
            artist_sort: Option<String>,
            album_artist_sort: Option<String>,
        }
        let rows: Vec<Row> = stmt
            .query_map(rusqlite::params_from_iter(parameters), |r| {
                Ok(Row {
                    artist: r.get(0)?,
                    album_artist: r.get(1)?,
                    album: r.get(2)?,
                    duration: r.get(3)?,
                    play_count: r.get(4)?,
                    added_at: r.get(5)?,
                    artwork: r.get(6)?,
                    artist_sort: r.get(7)?,
                    album_artist_sort: r.get(8)?,
                })
            })?
            .collect::<std::result::Result<_, _>>()?;
        // Names with songs of their own, as the main credit: "A & B" splits only into these.
        let known: HashSet<String> = rows
            .iter()
            .filter_map(|r| credits(&r.artist).into_iter().next())
            .map(|n| n.to_lowercase())
            .collect();
        let mut tallies: HashMap<String, Tally> = HashMap::new();
        for row in &rows {
            let mut names = vec![];
            for (i, credit) in credits(&row.artist).into_iter().enumerate() {
                for name in split_joined(&credit, &known) {
                    names.push((name, i == 0 && !credit.contains(" & ")));
                }
            }
            let album_artists: HashSet<String> = credits(&row.album_artist)
                .into_iter()
                .flat_map(|c| split_joined(&c, &known))
                .map(|n| n.to_lowercase())
                .collect();
            for (n, (name, main)) in names.into_iter().enumerate() {
                let key = name.to_lowercase();
                let tally = tallies.entry(key.clone()).or_insert_with(|| Tally {
                    name: name.clone(),
                    ..Default::default()
                });
                // The sort tag belongs to the song's one main artist.
                if main && n == 0 && tally.sort_tag.is_none() {
                    tally.sort_tag = row.artist_sort.clone().filter(|s| !s.trim().is_empty());
                }
                if album_artists.contains(&key) {
                    tally.album_artist = true;
                    if tally.sort_tag.is_none() && row.album_artist.eq_ignore_ascii_case(&name) {
                        tally.sort_tag = row
                            .album_artist_sort
                            .clone()
                            .filter(|s| !s.trim().is_empty());
                    }
                }
                tally.albums.insert(row.album.to_lowercase());
                tally.tracks += 1;
                tally.duration += row.duration;
                tally.play_count += row.play_count;
                tally.added_at = tally.added_at.max(row.added_at);
                if tally.artwork.is_none() {
                    tally.artwork = row.artwork.clone();
                }
            }
        }
        let mut artists: Vec<(String, i64, ArtistSummary)> = tallies
            .into_values()
            .filter(|t| !choice.album_artists_only || t.album_artist)
            .map(|t| {
                let sort = sort_key(&t.name, t.sort_tag.as_deref());
                let letter = index_letter(&sort);
                (
                    sort,
                    t.added_at,
                    ArtistSummary {
                        name: t.name,
                        albums: t.albums.len(),
                        tracks: t.tracks,
                        duration: t.duration,
                        artwork: t.artwork,
                        play_count: t.play_count,
                        letter,
                    },
                )
            })
            .collect();
        artists.sort_by(|(sa, aa, a), (sb, ab, b)| {
            let by_name = order_key(sa)
                .cmp(&order_key(sb))
                .then_with(|| a.name.cmp(&b.name));
            match choice.order {
                ArtistOrder::Name => by_name,
                ArtistOrder::MostPlayed => b.play_count.cmp(&a.play_count).then(by_name),
                ArtistOrder::MostSongs => b.tracks.cmp(&a.tracks).then(by_name),
                ArtistOrder::RecentlyAdded => ab.cmp(aa).then(by_name),
            }
        });
        Ok(artists.into_iter().map(|(_, _, a)| a).collect())
    }

    /// The songs that credit `name`: as the artist, a guest, one half of "A & B", or the album
    /// artist.
    pub(crate) fn credited(&self, name: &str) -> Result<Vec<Credited>> {
        let db = self.connection()?;
        let wanted = name.trim().to_lowercase();
        let pattern = format!(
            "%{}%",
            name.trim()
                .replace('\\', "\\\\")
                .replace('%', "\\%")
                .replace('_', "\\_")
        );
        let mut stmt = db.prepare(
            "SELECT id, artist, album_artist, album FROM tracks
             WHERE artist LIKE ?1 ESCAPE '\\' OR album_artist LIKE ?1 ESCAPE '\\'",
        )?;
        let rows: Vec<Credited> = stmt
            .query_map([pattern], |r| {
                Ok(Credited {
                    id: r.get(0)?,
                    artist: r.get(1)?,
                    album_artist: r.get(2)?,
                    album: r.get(3)?,
                })
            })?
            .collect::<std::result::Result<_, _>>()?;
        let named = |credit: &str| {
            credits(credit).iter().any(|c| {
                c.to_lowercase() == wanted
                    || JOINS
                        .iter()
                        .any(|j| c.split(j).any(|p| p.trim().to_lowercase() == wanted))
            })
        };
        Ok(rows
            .into_iter()
            .filter(|c| named(&c.artist) || named(&c.album_artist))
            .collect())
    }
}

/// A song that credits an artist, with what places it.
pub(crate) struct Credited {
    pub id: String,
    pub artist: String,
    pub album_artist: String,
    pub album: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credits_split_guests_and_lists() {
        assert_eq!(
            credits("Daft Punk feat. Pharrell Williams"),
            ["Daft Punk", "Pharrell Williams"]
        );
        assert_eq!(credits("A ft. B & C"), ["A", "B", "C"]);
        assert_eq!(credits("Song Maker (feat. Guest)"), ["Song Maker", "Guest"]);
        assert_eq!(credits("Simon & Garfunkel"), ["Simon & Garfunkel"]);
        assert_eq!(credits("A; B"), ["A", "B"]);
        assert_eq!(credits("AC/DC"), ["AC/DC"]);
        assert_eq!(credits("  "), Vec::<String>::new());
    }

    #[test]
    fn joined_names_split_only_into_known_artists() {
        let known: HashSet<String> = ["jay", "kay"].iter().map(|s| s.to_string()).collect();
        assert_eq!(split_joined("Jay & Kay", &known), ["Jay", "Kay"]);
        assert_eq!(
            split_joined("Simon & Garfunkel", &known),
            ["Simon & Garfunkel"]
        );
        assert_eq!(
            split_joined("Earth, Wind & Fire", &known),
            ["Earth, Wind & Fire"]
        );
    }

    #[test]
    fn sort_keys_fold_accents_and_use_tags() {
        assert_eq!(sort_key("Ólafur Arnalds", None), "olafur arnalds");
        assert_eq!(sort_key("The Beatles", None), "beatles");
        assert_eq!(
            sort_key("David Bowie", Some("Bowie, David")),
            "bowie, david"
        );
        assert_eq!(sort_key("The The", Some("The The")), "the the");
        assert_eq!(sort_key("'Til Tuesday", None), "til tuesday");
        assert_eq!(index_letter(&sort_key("Ólafur Arnalds", None)), "O");
        assert_eq!(index_letter(&sort_key("10cc", None)), "#");
        assert_eq!(index_letter(&sort_key("宇多田ヒカル", None)), "宇");
    }

    fn song(
        id: &str,
        artist: &str,
        album_artist: &str,
        album: &str,
        plays: i64,
    ) -> crate::model::Track {
        crate::model::Track {
            id: id.into(),
            path: format!("/music/{id}.flac"),
            title: format!("Song {id}"),
            artist: artist.into(),
            album_artist: album_artist.into(),
            album: album.into(),
            duration: 100.,
            play_count: plays,
            added_at: plays,
            ..Default::default()
        }
    }

    #[test]
    fn the_list_credits_guests_and_joined_names() {
        let dir = tempfile::tempdir().unwrap();
        let library = Library::open(dir.path()).unwrap();
        let mut bowie = song("d1", "David Bowie", "David Bowie", "Low", 1);
        bowie.artist_sort = Some("Bowie, David".into());
        for t in [
            bowie,
            song("j1", "Jay", "Jay", "Solo", 5),
            song("k1", "Kay", "Kay", "Alone", 2),
            song("jk", "Jay & Kay", "Jay & Kay", "Together", 3),
            song("sg", "Simon & Garfunkel", "Simon & Garfunkel", "Bridge", 4),
            song(
                "f1",
                "Ólafur Arnalds feat. Guest Singer",
                "Ólafur Arnalds",
                "Re:member",
                9,
            ),
        ] {
            library.upsert(&t).unwrap();
        }
        let names = |choice| {
            library
                .artists_by("", choice)
                .unwrap()
                .into_iter()
                .map(|a| a.name)
                .collect::<Vec<_>>()
        };
        assert_eq!(
            names(ArtistChoice::default()),
            [
                "David Bowie",
                "Guest Singer",
                "Jay",
                "Kay",
                "Ólafur Arnalds",
                "Simon & Garfunkel"
            ]
        );
        assert_eq!(
            names(ArtistChoice {
                album_artists_only: true,
                ..Default::default()
            }),
            [
                "David Bowie",
                "Jay",
                "Kay",
                "Ólafur Arnalds",
                "Simon & Garfunkel"
            ]
        );
        assert_eq!(
            names(ArtistChoice {
                order: ArtistOrder::MostPlayed,
                ..Default::default()
            })[..2],
            ["Guest Singer", "Ólafur Arnalds"]
        );
        let jay = library
            .artists("")
            .unwrap()
            .into_iter()
            .find(|a| a.name == "Jay")
            .unwrap();
        assert_eq!((jay.tracks, jay.albums, jay.letter.as_str()), (2, 2, "J"));
        assert_eq!(library.artist_albums("Kay").unwrap().len(), 2);
        assert_eq!(library.top_tracks("Guest Singer", 10).unwrap().len(), 1);
    }

    #[test]
    fn orders_put_digits_then_latin_then_other_scripts() {
        let mut names = vec!["宇多田", "beatles", "10cc", "abba"];
        names.sort_by_key(|n| order_key(n));
        assert_eq!(names, ["10cc", "abba", "beatles", "宇多田"]);
    }
}
