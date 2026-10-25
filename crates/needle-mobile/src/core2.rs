//! Needle 2.0's core for the phones (and the tablet and TV apps): the library choices kept with
//! the computer's settings, artists in order with their letters, an artist's own pictures and
//! fanart.tv's, the slow picture sweep, the library in numbers, On this day, the mood map, a
//! song's waveform, and the visualizer's spectrum.
use crate::{Album, Artist, Needle, NeedleError, Result, Song};
use needle_core::{
    artists::{ArtistChoice, ArtistOrder},
    audio::Command,
    media::pictures::{self, ArtistPicture, PictureSweep},
};
use std::sync::OnceLock;

/// What the library looks like and how it plays, kept with the computer's own settings (the
/// same names on every system).
#[derive(Clone, uniffi::Record)]
pub struct LibraryChoices {
    /// The title font: "system", "poster", "fraunces", "instrument_serif", "dm_serif",
    /// "space_grotesk", "bricolage", "outfit", or "own:<family>". "bahnschrift" from before
    /// 2.0 means poster.
    pub display_font: String,
    /// The text font: "system", "inter", "atkinson", "lexend", "plex_mono", or "own:<family>".
    pub text_font: String,
    /// The player's buttons in order (see [`Needle::player_buttons`]).
    pub player_buttons: Vec<String>,
    /// "name", "most_played", "most_songs", or "recently_added".
    pub artist_order: String,
    pub album_artists_only: bool,
    /// Find every artist's pictures slowly in the background (with online lookups on).
    pub artist_pictures: bool,
    /// The crossfade ends where the music ends and lasts whole bars.
    pub smart_mix: bool,
}

/// One of fanart.tv's pictures of an artist: the full picture's address (for
/// [`Needle::pick_fanart`]) and a small copy on this device to show.
#[derive(Clone, uniffi::Record)]
pub struct FanartPick {
    pub url: String,
    pub preview: String,
}

#[derive(Clone, uniffi::Record)]
pub struct PictureSweepProgress {
    pub done: u32,
    pub total: u32,
    pub allowed: bool,
    pub hurry: bool,
}

#[derive(Clone, uniffi::Record)]
pub struct Count {
    pub name: String,
    pub count: u32,
}

#[derive(Clone, uniffi::Record)]
pub struct LibraryNumbers {
    pub songs: u32,
    pub albums: u32,
    pub artists: u32,
    pub duration: f64,
    pub size: i64,
    pub server_songs: u32,
    pub lossless: u32,
    pub hires: u32,
    pub formats: Vec<Count>,
    pub decades: Vec<Count>,
    pub genres: Vec<Count>,
    pub artists_by_songs: Vec<Count>,
    pub albums_without_cover: u32,
    pub low_quality_albums: u32,
    pub without_cover: Vec<Album>,
    pub low_quality: Vec<Album>,
}

#[derive(Clone, uniffi::Record)]
pub struct OnThisDay {
    pub years_ago: i32,
    /// The day's start (Unix seconds).
    pub day: i64,
    pub albums: Vec<Album>,
    pub listens: u32,
}

#[derive(Clone, uniffi::Record)]
pub struct MoodPoint {
    pub id: String,
    /// Dark (0) to bright (1).
    pub x: f32,
    /// Calm (0) to lively (1).
    pub y: f32,
    /// "Upbeat", "Intense", "Mellow", or "Melancholy".
    pub mood: String,
}

fn picture_kind(kind: &str) -> Result<ArtistPicture> {
    match kind {
        "photo" => Ok(ArtistPicture::Photo),
        "logo" => Ok(ArtistPicture::Logo),
        "banner" => Ok(ArtistPicture::Banner),
        _ => Err(NeedleError::from(anyhow::anyhow!(
            "A picture is \"photo\", \"logo\", or \"banner\""
        ))),
    }
}

fn counts(rows: Vec<(String, usize)>) -> Vec<Count> {
    rows.into_iter()
        .map(|(name, count)| Count {
            name,
            count: count as u32,
        })
        .collect()
}

/// The sweep, started the first time an app says whether it may look.
static SWEEP: OnceLock<PictureSweep> = OnceLock::new();

#[uniffi::export]
impl Needle {
    pub fn library_choices(&self) -> Result<LibraryChoices> {
        let s = self.library.settings()?;
        Ok(LibraryChoices {
            display_font: if s.display_font == "bahnschrift" {
                "poster".into()
            } else {
                s.display_font.clone()
            },
            text_font: s.text_font.clone(),
            player_buttons: s.player_buttons().into_iter().map(String::from).collect(),
            artist_order: s.artist_order.clone(),
            album_artists_only: s.album_artists_only,
            artist_pictures: s.artist_pictures,
            smart_mix: s.smart_mix,
        })
    }

    /// Keeps the library choices; the song playing goes on as it was.
    pub fn set_library_choices(&self, value: LibraryChoices) -> Result<()> {
        let s = self.library.update_settings(|s| {
            s.display_font = value.display_font.clone();
            s.text_font = value.text_font.clone();
            s.player_buttons = value.player_buttons.clone();
            s.artist_order = ArtistOrder::from_name(&value.artist_order).name().into();
            s.album_artists_only = value.album_artists_only;
            s.artist_pictures = value.artist_pictures;
            s.smart_mix = value.smart_mix;
        })?;
        self.player.send(Command::Configure(Box::new(s)));
        Ok(())
    }

    /// Every button the player can show, in Needle's own order.
    pub fn player_buttons(&self) -> Vec<String> {
        needle_core::model::PLAYER_BUTTONS
            .iter()
            .map(|b| b.to_string())
            .collect()
    }

    /// Artists in the order chosen: guests after "feat." and both halves of "A & B" (when
    /// both have songs of their own) included; `letter` is each one's place in an A–Z bar.
    pub fn artists_by(
        &self,
        query: String,
        order: String,
        album_artists_only: bool,
    ) -> Result<Vec<Artist>> {
        let choice = ArtistChoice {
            order: ArtistOrder::from_name(&order),
            album_artists_only,
        };
        Ok(self
            .library
            .artists_by(&query, choice)?
            .into_iter()
            .map(|a| Artist {
                name: a.name,
                albums: a.albums as u32,
                songs: a.tracks as u32,
                artwork: a.artwork,
                letter: a.letter,
                plays: a.play_count,
            })
            .collect())
    }

    /// The artist's own chosen picture ("photo", "logo", or "banner"), if one was chosen.
    pub fn own_artist_picture(&self, name: String, kind: String) -> Result<Option<String>> {
        Ok(
            pictures::own_picture(&self.library, &name, picture_kind(&kind)?)
                .map(|p| p.to_string_lossy().into()),
        )
    }

    /// Use this picture file as the artist's photo, logo, or wide picture; kept, never
    /// replaced by a lookup.
    pub fn set_artist_picture(&self, name: String, kind: String, path: String) -> Result<String> {
        Ok(pictures::set_own_picture(
            &self.library,
            &name,
            picture_kind(&kind)?,
            std::path::Path::new(&path),
        )?
        .to_string_lossy()
        .into())
    }

    /// Forget the chosen picture: the found one shows again.
    pub fn reset_artist_picture(&self, name: String, kind: String) -> Result<()> {
        Ok(pictures::reset_picture(
            &self.library,
            &name,
            picture_kind(&kind)?,
        )?)
    }

    /// fanart.tv's pictures of this kind for the artist, best first, with small copies (at most
    /// 24). Slow: call it off the main thread. Empty without online lookups or a key.
    pub fn fanart_choices(&self, name: String, kind: String) -> Result<Vec<FanartPick>> {
        let online = self.library.settings()?.online_media;
        let found = pictures::fanart_choices(&self.library, &name, picture_kind(&kind)?, online)?;
        Ok(found
            .into_iter()
            .take(24)
            .filter_map(|c| {
                let preview = pictures::fanart_preview(&self.library, &c.preview)
                    .or_else(|_| pictures::fanart_preview(&self.library, &c.url))
                    .ok()?;
                Some(FanartPick {
                    url: c.url,
                    preview: preview.to_string_lossy().into(),
                })
            })
            .collect())
    }

    /// Use one of fanart.tv's pictures (from [`Needle::fanart_choices`]) as the artist's.
    pub fn pick_fanart(&self, name: String, kind: String, url: String) -> Result<String> {
        Ok(
            pictures::pick_fanart(&self.library, &name, picture_kind(&kind)?, &url)?
                .to_string_lossy()
                .into(),
        )
    }

    /// Whether the picture sweep may look now: online lookups and Artist pictures on, on
    /// Wi-Fi, and no scan running. Starts the sweep the first time.
    pub fn allow_picture_sweep(&self, allowed: bool) {
        SWEEP
            .get_or_init(|| PictureSweep::start(self.library.clone()))
            .set_allowed(allowed);
    }

    /// Look for every artist's pictures now, at the services' own pace.
    pub fn check_all_artist_pictures(&self) {
        SWEEP
            .get_or_init(|| PictureSweep::start(self.library.clone()))
            .check_all_now();
    }

    pub fn picture_sweep_progress(&self) -> PictureSweepProgress {
        let p = SWEEP.get().map(|s| s.progress()).unwrap_or_default();
        PictureSweepProgress {
            done: p.done as u32,
            total: p.total as u32,
            allowed: p.allowed,
            hurry: p.hurry,
        }
    }

    /// What the library is made of.
    pub fn library_numbers(&self) -> Result<LibraryNumbers> {
        let n = self.library.numbers()?;
        Ok(LibraryNumbers {
            songs: n.songs as u32,
            albums: n.albums as u32,
            artists: n.artists as u32,
            duration: n.duration,
            size: n.size,
            server_songs: n.server_songs as u32,
            lossless: n.lossless as u32,
            hires: n.hires as u32,
            formats: counts(n.formats),
            decades: counts(n.decades),
            genres: counts(n.genres),
            artists_by_songs: counts(n.artists_by_songs),
            albums_without_cover: n.albums_without_cover as u32,
            low_quality_albums: n.low_quality_albums as u32,
            without_cover: n.without_cover.into_iter().map(Album::from).collect(),
            low_quality: n.low_quality.into_iter().map(Album::from).collect(),
        })
    }

    /// What played on this day one and five years ago (`utc_offset` in seconds east of UTC),
    /// at most `limit` albums a day; days with nothing played are left out.
    pub fn on_this_day(&self, utc_offset: i32, limit: u32) -> Result<Vec<OnThisDay>> {
        let now = chrono::Utc::now().timestamp();
        Ok(self
            .library
            .on_this_day(now, utc_offset, &[1, 5], limit as usize)?
            .into_iter()
            .map(|d| OnThisDay {
                years_ago: d.years_ago,
                day: d.day,
                albums: d.albums.into_iter().map(Album::from).collect(),
                listens: d.listens as u32,
            })
            .collect())
    }

    /// Every measured song on the mood map.
    pub fn mood_map(&self) -> Result<Vec<MoodPoint>> {
        Ok(self
            .library
            .mood_map()?
            .into_iter()
            .map(|p| MoodPoint {
                id: p.id,
                x: p.x,
                y: p.y,
                mood: p.mood.into(),
            })
            .collect())
    }

    /// The songs inside a circle on the mood map, nearest the centre first (to play).
    pub fn mood_area(&self, x: f32, y: f32, radius: f32, limit: u32) -> Result<Vec<Song>> {
        Ok(self
            .library
            .mood_area(x, y, radius, limit as usize)?
            .iter()
            .map(Song::from)
            .collect())
    }

    /// A song's shape for the waveform seek bar: `count` heights, 0 to 1. Measures the song
    /// the first time (about a second): call it off the main thread. Empty when it cannot be
    /// measured (a server song not kept yet).
    pub fn waveform(&self, id: String, count: u32) -> Result<Vec<f32>> {
        let Some(track) = self.library.track(&id)? else {
            return Ok(vec![]);
        };
        Ok(self.library.waveform_now(&track)?.heights(count as usize))
    }

    /// The visualizer: the spectrum of what plays now, in `bands` bands, 0 to 1. Call it about
    /// 30 times a second while the visualizer shows; it costs nothing otherwise.
    pub fn spectrum(&self, bands: u32) -> Vec<f32> {
        needle_core::scope::spectrum(bands as usize)
    }

    /// How loud what plays now is, 0 to 1, for the visualizer's glow.
    pub fn sound_level(&self) -> f32 {
        needle_core::scope::level()
    }
}
