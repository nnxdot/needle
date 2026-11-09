//! The genre map's other two pictures, laid out here once for every app:
//!
//! - **Cloud**: every genre a word, as big as its songs (or plays), coloured by its family, set
//!   around the middle in a spiral, the biggest first.
//! - **Bubbles**: every genre a circle, as big as its songs (or plays); the genres of one family
//!   (rock, pop, hip hop, …) packed together in one colour, and the families packed together.
//!
//! Genres written differently are one ("Hip-Hop", "hip hop", "HipHop"; "R&B", "RnB"). Places
//! and sizes are in a box 1 wide and `height` high: an app scales them to its own width (or to
//! its height, when the picture is the taller), and draws each word at its `size` (the font's
//! size) or each circle at its `size` (the radius), centred on (`x`, `y`).

use crate::database::Library;
use anyhow::Result;
use serde::Serialize;
use std::collections::HashMap;

/// Which picture.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArtKind {
    Cloud,
    Bubbles,
}

/// One genre in a picture.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ArtItem {
    /// As its songs most often write it.
    pub genre: String,
    /// Every way the library writes it, to play all its songs.
    pub spellings: Vec<String>,
    pub songs: usize,
    pub plays: i64,
    /// "rock", "pop", "hip hop", … or "other".
    pub family: &'static str,
    /// The family's colour, as a hue from 0 to 360.
    pub hue: f32,
    pub x: f32,
    pub y: f32,
    /// A word's font size, or a circle's radius.
    pub size: f32,
    /// For a circle's name: the largest font size that fits its longest word across 80 % of the
    /// circle is `fit` × the radius (see `bubble_label`).
    pub fit: f32,
}

/// A laid-out picture: its items, in a box 1 wide and `height` high.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct GenreArt {
    pub height: f32,
    pub items: Vec<ArtItem>,
}

/// The families, in the order a genre is tested against them, with the words that put a genre
/// in one and the family's hue.
const FAMILIES: &[(&str, &[&str], f32)] = &[
    (
        "metal",
        // Not "core" alone: "Score" ends in it.
        &[
            "metal",
            "metalcore",
            "deathcore",
            "hardcore",
            "grindcore",
            "mathcore",
            "djent",
            "grind",
            "thrash",
            "doom",
        ],
        355.,
    ),
    (
        "hip hop",
        &["hip", "rap", "trap", "drill", "grime", "boombap", "phonk"],
        210.,
    ),
    (
        "r&b and soul",
        &[
            "rnb",
            "soul",
            "funk",
            "gospel",
            "motown",
            "disco",
            "quietstorm",
        ],
        130.,
    ),
    (
        "k-pop and j-pop",
        &[
            "kpop", "jpop", "cpop", "mandopop", "cantopop", "anime", "idol", "jrock", "krock",
        ],
        305.,
    ),
    (
        "latin",
        &[
            "latin",
            "reggaeton",
            "salsa",
            "bachata",
            "cumbia",
            "samba",
            "bossa",
            "mpb",
            "sertanejo",
            "mexicano",
            "tango",
            "forro",
            "axe",
            "merengue",
            "flamenco",
            "urbano",
        ],
        100.,
    ),
    (
        "reggae",
        &["reggae", "ska", "dub", "dancehall", "rocksteady"],
        75.,
    ),
    (
        "electronic",
        &[
            "electro",
            "house",
            "techno",
            "trance",
            "edm",
            "dubstep",
            "drumandbass",
            "dnb",
            "ambient",
            "synth",
            "idm",
            "ukgarage",
            "dance",
            "breakbeat",
            "jungle",
            "downtempo",
            "lofi",
            "chill",
            "hyperpop",
            "club",
            "bass",
            "electronica",
        ],
        255.,
    ),
    (
        "jazz",
        &["jazz", "swing", "bebop", "bigband", "fusion", "lounge"],
        330.,
    ),
    (
        "classical",
        &[
            "classical",
            "orchestra",
            "opera",
            "baroque",
            "symphon",
            "soundtrack",
            "score",
            "cinematic",
            "choral",
            "game",
            "videogame",
            "vgm",
            "chiptune",
        ],
        280.,
    ),
    (
        "country and folk",
        &[
            "country",
            "folk",
            "bluegrass",
            "americana",
            "singersongwriter",
            "acoustic",
            "celtic",
        ],
        50.,
    ),
    ("blues", &["blues"], 230.),
    ("pop", &["pop"], 185.),
    (
        "rock",
        &[
            "rock",
            "punk",
            "grunge",
            "emo",
            "indie",
            "alternative",
            "shoegaze",
            "post",
            "newwave",
            "britpop",
            "garage",
        ],
        25.,
    ),
];

/// The hue of genres in no family.
const OTHER_HUE: f32 = 160.;

/// A genre's family and its hue.
pub fn family(genre: &str) -> (&'static str, f32) {
    let compact = key(genre);
    let lower = genre.to_lowercase().replace('&', "n");
    let words: Vec<&str> = lower
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect();
    // A long term anywhere ("drumandbass" in "Drum & Bass"); a short one only as a whole word,
    // the start of one, or (for the shortest) its end: "Synthpop" is pop, "Skate Punk" not ska,
    // "Post-Rock" not a soundtrack.
    let has = |term: &str| {
        (term.len() >= 4 && compact.contains(term))
            || words.iter().any(|w| {
                *w == term
                    || (term.len() >= 4 && w.starts_with(term))
                    || (term.len() <= 4 && w.ends_with(term))
            })
    };
    for (name, terms, hue) in FAMILIES {
        if terms.iter().any(|t| has(t)) {
            return (name, *hue);
        }
    }
    ("other", OTHER_HUE)
}

/// One key for a genre however it is written: lower case, "&" and "and" as "n" between two
/// letters (R&B, R and B, RnB), and nothing but letters and digits ("Hip-Hop", "hip hop").
pub fn key(genre: &str) -> String {
    let lower = genre.trim().to_lowercase();
    let lower = lower
        .replace(" and ", "n")
        .replace(" & ", "n")
        .replace('&', "n");
    let compact: String = lower.chars().filter(|c| c.is_alphanumeric()).collect();
    // "Rhythm and Blues" is R&B too.
    if compact == "rhythmnblues" {
        "rnb".into()
    } else {
        compact
    }
}

/// The genres in one genre tag: several when a tagger wrote them with `;` or a NUL between
/// (ID3v2.4). A space is part of a genre's name ("Classic Rock"), never a split.
fn genres_in(field: &str) -> impl Iterator<Item = &str> {
    field
        .split([';', '\0'])
        .map(str::trim)
        .filter(|g| !g.is_empty())
}

/// One genre after merging: its name, spellings, songs, and plays.
#[derive(Clone, Debug, Default)]
struct Merged {
    names: HashMap<String, usize>,
    songs: usize,
    plays: i64,
}

impl Library {
    /// The genre map's cloud or bubbles: the `most` genres with the most songs (or, with
    /// `by_plays`, the most plays), laid out in a box 1 wide whose height is `1 / aspect`
    /// (an aspect of 2 is twice as wide as high).
    pub fn genre_art(
        &self,
        kind: ArtKind,
        by_plays: bool,
        aspect: f32,
        most: usize,
    ) -> Result<GenreArt> {
        let db = self.connection()?;
        let mut stmt =
            db.prepare("SELECT genre, play_count FROM tracks WHERE genre != '' AND missing=0")?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?;
        let mut by: HashMap<String, Merged> = HashMap::new();
        for row in rows {
            let (field, plays) = row?;
            // Several genres in one tag, as some taggers write them.
            for genre in genres_in(&field) {
                let k = key(genre);
                if k.is_empty() {
                    continue;
                }
                let m = by.entry(k).or_default();
                *m.names.entry(genre.to_string()).or_default() += 1;
                m.songs += 1;
                m.plays += plays;
            }
        }
        let mut genres: Vec<(String, Vec<String>, usize, i64)> = by
            .into_values()
            .map(|m| {
                let mut spellings: Vec<(String, usize)> = m.names.into_iter().collect();
                spellings.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
                let name = spellings[0].0.clone();
                (
                    name,
                    spellings.into_iter().map(|s| s.0).collect(),
                    m.songs,
                    m.plays,
                )
            })
            .collect();
        let weight = |g: &(String, Vec<String>, usize, i64)| {
            if by_plays {
                g.3.max(0) as f64
            } else {
                g.2 as f64
            }
        };
        genres.sort_by(|a, b| weight(b).total_cmp(&weight(a)).then(a.0.cmp(&b.0)));
        genres.retain(|g| weight(g) > 0.);
        genres.truncate(most.max(1));
        let weights: Vec<f64> = genres.iter().map(weight).collect();
        let items: Vec<ArtItem> = genres
            .into_iter()
            .map(|(genre, spellings, songs, plays)| {
                let (family, hue) = family(&genre);
                let fit = 1.6 / longest_word(&genre);
                ArtItem {
                    fit,
                    genre,
                    spellings,
                    songs,
                    plays,
                    family,
                    hue,
                    x: 0.,
                    y: 0.,
                    size: 0.,
                }
            })
            .collect();
        // A size not known yet (0 / 0) is no number: the spirals would never find a place.
        let aspect = if aspect.is_finite() {
            aspect.clamp(0.4, 4.)
        } else {
            1.
        };
        Ok(match kind {
            ArtKind::Cloud => cloud(items, &weights, aspect),
            ArtKind::Bubbles => bubbles(items, &weights, aspect),
        })
    }

    /// The songs of a genre written any of these ways, shuffled, at most `limit` (to play a word
    /// or a bubble): matched as the pictures count them, each genre of a tag apart ("Rock; Pop"
    /// is in both) and written any way ("Hip-Hop", "hip hop"), each song once.
    pub fn songs_of_genres(
        &self,
        spellings: &[String],
        limit: usize,
    ) -> Result<Vec<crate::model::Track>> {
        let keys: std::collections::HashSet<String> = spellings.iter().map(|g| key(g)).collect();
        let db = self.connection()?;
        let mut stmt =
            db.prepare("SELECT id, genre FROM tracks WHERE genre != '' AND missing=0")?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
        let mut ids = vec![];
        for row in rows {
            let (id, field) = row?;
            if genres_in(&field).any(|g| keys.contains(&key(g))) {
                ids.push(id);
            }
        }
        drop(stmt);
        use rand::seq::SliceRandom;
        ids.shuffle(&mut rand::thread_rng());
        ids.truncate(limit.max(1));
        self.tracks_by_ids(&ids)
    }
}

/// The font size for a genre's name inside a circle of radius `r`: as big as fits, its longest
/// word on one line across 80 % of the circle (never broken mid-word); `None` when that is under
/// 7 (the circle shows no name; it has a tooltip).
pub fn bubble_label(genre: &str, r: f32) -> Option<f32> {
    let size = (r * 0.32).clamp(8., 20.).min(1.6 * r / longest_word(genre));
    (size >= 7.).then_some(size)
}

/// The width of a name's longest word at font size 1.
fn longest_word(genre: &str) -> f32 {
    genre
        .split(|c: char| c.is_whitespace() || c == '-' || c == '/')
        .map(word_width)
        .fold(0f32, f32::max)
        .max(0.6)
}

/// A rough width of a word at font size 1 (bold, a little wide on purpose so words never touch).
pub fn word_width(word: &str) -> f32 {
    word.chars()
        .map(|c| {
            if c.is_ascii() {
                if c == ' ' {
                    0.32
                } else if "MWmw".contains(c) {
                    0.9
                } else {
                    0.62
                }
            } else {
                1.05
            }
        })
        .sum::<f32>()
        .max(0.6)
}

/// Words around the middle in a spiral, the biggest first, in a box `aspect` times as wide as
/// high, then scaled to 1 wide.
fn cloud(mut items: Vec<ArtItem>, weights: &[f64], aspect: f32) -> GenreArt {
    let top = weights.first().copied().unwrap_or(1.).max(1.);
    // Rectangles placed so far: left, top, right, bottom.
    let mut placed: Vec<[f32; 4]> = vec![];
    let gap = 0.04;
    for (item, &w) in items.iter_mut().zip(weights) {
        // Font sizes from 1 to 6 by the square root, so small genres stay readable.
        let size = 1. + 5. * (w / top).sqrt() as f32;
        let (half_w, half_h) = (word_width(&item.genre) * size / 2. + gap, size * 0.62 + gap);
        let mut t = 0f32;
        loop {
            // An oval spiral, as wide as the box.
            let r = 0.35 * t;
            let (x, y) = (r * t.cos() * aspect.sqrt(), r * t.sin() / aspect.sqrt());
            let rect = [x - half_w, y - half_h, x + half_w, y + half_h];
            if !placed
                .iter()
                .any(|p| rect[0] < p[2] && p[0] < rect[2] && rect[1] < p[3] && p[1] < rect[3])
            {
                placed.push(rect);
                item.x = x;
                item.y = y;
                item.size = size;
                break;
            }
            t += 0.08;
        }
    }
    fit(items, &placed, aspect, |item, s| item.size *= s)
}

/// A genre's place inside its family: its index, and its middle from the family's.
type Placed = (usize, f32, f32);

/// Each family's circles packed together, the families then packed together.
fn bubbles(mut items: Vec<ArtItem>, weights: &[f64], aspect: f32) -> GenreArt {
    let top = weights.first().copied().unwrap_or(1.).max(1.);
    for (item, &w) in items.iter_mut().zip(weights) {
        // The area follows the weight; the smallest still holds a short word.
        item.size = (0.18 + (w / top).sqrt() as f32).max(0.18);
    }
    // Families, the biggest first; inside each, circles packed around its middle.
    let mut families: Vec<(&'static str, Vec<usize>)> = vec![];
    for (i, item) in items.iter().enumerate() {
        match families.iter_mut().find(|f| f.0 == item.family) {
            Some(f) => f.1.push(i),
            None => families.push((item.family, vec![i])),
        }
    }
    let gap = 0.03;
    // Each family as one circle around its packed members: (radius, members' places).
    let mut groups: Vec<(f32, Vec<Placed>)> = vec![];
    for (_, members) in &families {
        let circles: Vec<f32> = members.iter().map(|&i| items[i].size).collect();
        let spots = pack(&circles, gap, 1.);
        let reach = spots
            .iter()
            .zip(&circles)
            .map(|((x, y), r)| (x * x + y * y).sqrt() + r)
            .fold(0f32, f32::max);
        groups.push((
            reach,
            members
                .iter()
                .zip(spots)
                .map(|(&i, (x, y))| (i, x, y))
                .collect(),
        ));
    }
    let radii: Vec<f32> = groups.iter().map(|g| g.0).collect();
    let middles = pack(&radii, gap * 4., aspect);
    let mut extents: Vec<[f32; 4]> = vec![];
    for ((_, members), (cx, cy)) in groups.iter().zip(middles) {
        for &(i, x, y) in members {
            let item = &mut items[i];
            item.x = cx + x;
            item.y = cy + y;
            extents.push([
                item.x - item.size,
                item.y - item.size,
                item.x + item.size,
                item.y + item.size,
            ]);
        }
    }
    fit(items, &extents, aspect, |item, s| item.size *= s)
}

/// Circles of these radii (the biggest first, as given) set one by one as near the middle as
/// they go without touching, on an oval spiral `aspect` times as wide as high: their middles.
fn pack(radii: &[f32], gap: f32, aspect: f32) -> Vec<(f32, f32)> {
    let mut spots: Vec<(f32, f32)> = vec![];
    for (i, &r) in radii.iter().enumerate() {
        if i == 0 {
            spots.push((0., 0.));
            continue;
        }
        let step = (radii[0] * 0.02).max(0.002);
        let mut t = 0f32;
        loop {
            let d = step * t;
            let (x, y) = (d * t.cos() * aspect.sqrt(), d * t.sin() / aspect.sqrt());
            if spots.iter().zip(radii).all(|((sx, sy), sr)| {
                let (dx, dy) = (x - sx, y - sy);
                (dx * dx + dy * dy).sqrt() >= r + sr + gap
            }) {
                spots.push((x, y));
                break;
            }
            t += 0.05;
        }
    }
    spots
}

/// Moves and scales the items so their extents fit a box 1 wide and `1 / aspect` high (or, if
/// they are the taller shape, as high as they need), centred.
fn fit(
    mut items: Vec<ArtItem>,
    extents: &[[f32; 4]],
    aspect: f32,
    scale_size: impl Fn(&mut ArtItem, f32),
) -> GenreArt {
    if extents.is_empty() {
        return GenreArt {
            height: 1. / aspect,
            items,
        };
    }
    let (l, t, r, b) = extents
        .iter()
        .fold((f32::MAX, f32::MAX, f32::MIN, f32::MIN), |a, e| {
            (a.0.min(e[0]), a.1.min(e[1]), a.2.max(e[2]), a.3.max(e[3]))
        });
    let (w, h) = ((r - l).max(1e-3), (b - t).max(1e-3));
    let height = (h / w).max(1. / aspect);
    let s = 1. / w;
    let top = (height - h * s) / 2.;
    for item in &mut items {
        item.x = (item.x - l) * s;
        item.y = (item.y - t) * s + top;
        scale_size(item, s);
    }
    GenreArt { height, items }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Track;

    #[test]
    fn genres_written_differently_are_one() {
        assert_eq!(key("Hip-Hop"), key("hip hop"));
        assert_eq!(key("HipHop"), key("Hip Hop"));
        assert_eq!(key("R&B"), key("RnB"));
        assert_eq!(key("R and B"), key("r&b"));
        assert_eq!(key("Rhythm and Blues"), "rnb");
        assert_ne!(key("Rock"), key("Pop"));
    }

    #[test]
    fn genres_fall_into_families() {
        assert_eq!(family("Death Metal").0, "metal");
        assert_eq!(family("Hip Hop/Rap").0, "hip hop");
        assert_eq!(family("Neo Soul").0, "r&b and soul");
        assert_eq!(family("Deep House").0, "electronic");
        assert_eq!(family("K-Pop").0, "k-pop and j-pop");
        assert_eq!(family("Indie Pop").0, "pop");
        assert_eq!(family("Shoegaze").0, "rock");
        assert_eq!(family("Polka").0, "other");
        assert_eq!(family("Post-Rock").0, "rock");
        assert_eq!(family("Skate Punk").0, "rock");
        assert_eq!(family("Synthpop").0, "electronic");
        assert_eq!(family("Dancehall").0, "reggae");
        assert_eq!(family("Dubstep").0, "electronic");
        assert_eq!(family("Drum & Bass").0, "electronic");
        assert_eq!(family("Trap").0, "hip hop");
        assert_eq!(family("Video game music").0, "classical");
        assert_eq!(family("Game").0, "classical");
        assert_eq!(family("Score").0, "classical");
        assert_eq!(family("Post-Hardcore").0, "metal");
    }

    fn library(genres: &[(&str, usize)]) -> (tempfile::TempDir, Library) {
        let dir = tempfile::tempdir().unwrap();
        let library = Library::open(dir.path()).unwrap();
        let mut n = 0;
        for (genre, count) in genres {
            for _ in 0..*count {
                n += 1;
                library
                    .upsert(&Track {
                        id: format!("t{n}"),
                        path: format!("/music/{n}.flac"),
                        title: format!("Song {n}"),
                        genre: genre.to_string(),
                        ..Default::default()
                    })
                    .unwrap();
            }
        }
        (dir, library)
    }

    #[test]
    fn the_cloud_merges_spellings_and_never_overlaps() {
        let (_dir, library) = library(&[
            ("Hip-Hop", 9),
            ("hip hop", 3),
            ("Rock", 8),
            ("Pop", 6),
            ("Jazz", 2),
            ("Death Metal", 2),
            ("Deep House", 4),
            ("Neo Soul", 1),
            ("K-Pop", 5),
            ("Polka", 1),
            ("Shoegaze", 2),
        ]);
        let art = library.genre_art(ArtKind::Cloud, false, 2., 50).unwrap();
        assert_eq!(art.items.len(), 10, "Hip-Hop and hip hop are one");
        let hip = &art.items[0];
        assert_eq!((hip.genre.as_str(), hip.songs), ("Hip-Hop", 12));
        assert_eq!(hip.spellings, ["Hip-Hop", "hip hop"]);
        // Every word inside the box, none on another.
        let rects: Vec<[f32; 4]> = art
            .items
            .iter()
            .map(|i| {
                let (hw, hh) = (word_width(&i.genre) * i.size / 2., i.size * 0.62);
                [i.x - hw, i.y - hh, i.x + hw, i.y + hh]
            })
            .collect();
        for (a, ra) in rects.iter().enumerate() {
            assert!(
                ra[0] >= -1e-4
                    && ra[2] <= 1. + 1e-4
                    && ra[1] >= -1e-4
                    && ra[3] <= art.height + 1e-4,
                "{a} outside"
            );
            for rb in &rects[a + 1..] {
                assert!(
                    !(ra[0] < rb[2] && rb[0] < ra[2] && ra[1] < rb[3] && rb[1] < ra[3]),
                    "words overlap"
                );
            }
        }
        // Bigger genres, bigger words.
        assert!(art.items[0].size > art.items.last().unwrap().size);
    }

    #[test]
    fn a_genre_plays_every_song_once_however_its_tag_is_written() {
        let (_dir, library) = library(&[("Rock; Pop", 2), ("rock", 3), ("Rock", 1), ("Jazz", 1)]);
        let art = library.genre_art(ArtKind::Bubbles, false, 1.5, 50).unwrap();
        let rock = art.items.iter().find(|i| key(&i.genre) == "rock").unwrap();
        assert_eq!(rock.songs, 6);
        let songs = library.songs_of_genres(&rock.spellings, 500).unwrap();
        let ids: std::collections::HashSet<String> = songs.iter().map(|t| t.id.clone()).collect();
        assert_eq!(
            (songs.len(), ids.len()),
            (6, 6),
            "each Rock song once, the Rock; Pop ones too"
        );
        let pop = art.items.iter().find(|i| i.genre == "Pop").unwrap();
        assert_eq!(
            library.songs_of_genres(&pop.spellings, 500).unwrap().len(),
            2
        );
    }

    #[test]
    fn a_two_word_genre_plays_and_one_word_matches_only_itself() {
        let (_dir, library) = library(&[
            ("Classic Rock", 2),
            ("Hip Hop", 3),
            ("Rock", 1),
            ("Hard Rock", 4),
            ("Indie Rock; Pop", 5),
        ]);
        let art = library.genre_art(ArtKind::Bubbles, false, 1.5, 50).unwrap();
        let songs = |name: &str| {
            let item = art.items.iter().find(|i| i.genre == name).unwrap();
            assert_eq!(
                library.songs_of_genres(&item.spellings, 500).unwrap().len(),
                item.songs,
                "{name} plays as many songs as its picture counts"
            );
            item.songs
        };
        assert_eq!(songs("Classic Rock"), 2);
        assert_eq!(songs("Hip Hop"), 3);
        assert_eq!(songs("Rock"), 1, "Hard Rock and Indie Rock are not Rock");
        assert_eq!(songs("Pop"), 5);
    }

    #[test]
    fn a_size_that_is_no_number_still_lays_out() {
        let (_dir, library) = library(&[("Rock", 3), ("Jazz", 2)]);
        for kind in [ArtKind::Bubbles, ArtKind::Cloud] {
            for aspect in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
                let art = library.genre_art(kind, false, aspect, 50).unwrap();
                assert_eq!(art.items.len(), 2);
                assert!(art.height.is_finite());
            }
        }
    }

    #[test]
    fn bubbles_never_overlap_and_families_sit_together() {
        let (_dir, library) = library(&[
            ("Rock", 8),
            ("Indie Rock", 4),
            ("Punk", 3),
            ("Pop", 6),
            ("Synthpop", 2),
            ("Jazz", 2),
            ("Deep House", 4),
            ("Techno", 3),
            ("Hip Hop", 7),
            ("Trap", 2),
        ]);
        let art = library.genre_art(ArtKind::Bubbles, false, 1.5, 50).unwrap();
        assert_eq!(art.items.len(), 10);
        for (a, x) in art.items.iter().enumerate() {
            assert!(
                x.x - x.size >= -1e-4 && x.x + x.size <= 1. + 1e-4,
                "{} outside",
                x.genre
            );
            assert!(
                x.y - x.size >= -1e-4 && x.y + x.size <= art.height + 1e-4,
                "{} outside",
                x.genre
            );
            for y in &art.items[a + 1..] {
                let d = ((x.x - y.x).powi(2) + (x.y - y.y).powi(2)).sqrt();
                assert!(
                    d >= x.size + y.size - 1e-4,
                    "{} and {} overlap",
                    x.genre,
                    y.genre
                );
            }
        }
        // A family's own genres are nearer each other than to another family's.
        let at = |g: &str| art.items.iter().find(|i| i.genre == g).unwrap();
        let near =
            |a: &str, b: &str| ((at(a).x - at(b).x).powi(2) + (at(a).y - at(b).y).powi(2)).sqrt();
        assert!(near("Deep House", "Techno") < near("Deep House", "Jazz"));
        // Playing by plays: no plays yet, nothing to show.
        assert!(
            library
                .genre_art(ArtKind::Bubbles, true, 1.5, 50)
                .unwrap()
                .items
                .is_empty()
        );
    }
}
