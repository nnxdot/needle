//! Needle 2.0 on the phone: what the new screens ask for that the older calls do not give.
//! Only additions; the older calls stay as they are for the iPhone.
use crate::{Album, Artist, Needle, NeedleError, Result, Song};
use needle_core::{
    artists::{ArtistChoice, ArtistOrder},
    audio::Command,
    browse::AlbumSummary,
    media::pictures::{self, ArtistPicture, PictureSweep},
};
use std::sync::{Mutex, OnceLock};

/// Home's shelves, as the Mac's Home has them.
#[derive(Clone, uniffi::Record)]
pub struct HomeShelves {
    /// Albums played most recently, newest first.
    pub recent: Vec<Album>,
    /// Albums most recently added.
    pub added: Vec<Album>,
    /// Albums with the most favorite songs.
    pub favorites: Vec<Album>,
    /// Albums with the most plays.
    pub most_played: Vec<Album>,
    /// Artists with the most plays.
    pub artists: Vec<Artist>,
}

/// Needle 2.0's settings that live in the core (shared with the computer's Needle).
#[derive(Clone, uniffi::Record)]
pub struct MoreSettings {
    /// The player's buttons, in order: "shuffle", "previous", "play", "next", "repeat",
    /// "favorite", "lyrics", "queue", "speakers". Play is always among them.
    pub player_buttons: Vec<String>,
    /// "name", "most_played", "most_songs", or "recently_added".
    pub artist_order: String,
    /// The Artists list shows only album artists.
    pub album_artists_only: bool,
    /// Find every artist's pictures slowly in the background (with online lookups on).
    pub artist_pictures: bool,
    /// Smart mixes: the crossfade ends where the music ends, in whole bars.
    pub smart_mix: bool,
}

/// An artist in the Artists list, with the letter it is filed under for the A–Z bar.
#[derive(Clone, uniffi::Record)]
pub struct ArtistEntry {
    pub name: String,
    pub albums: u32,
    pub songs: u32,
    pub artwork: Option<String>,
    pub plays: i64,
    /// "A"–"Z", "#", or another script's letter.
    pub letter: String,
}

/// One count of the library in numbers ("FLAC", 1200).
#[derive(Clone, uniffi::Record)]
pub struct NamedCount {
    pub name: String,
    pub count: u32,
}

/// What the library is made of.
#[derive(Clone, uniffi::Record)]
pub struct LibraryNumbers {
    pub songs: u32,
    pub albums: u32,
    pub artists: u32,
    /// Seconds of music.
    pub duration: f64,
    /// Bytes on this phone.
    pub size: i64,
    pub server_songs: u32,
    pub formats: Vec<NamedCount>,
    pub lossless: u32,
    pub hires: u32,
    pub decades: Vec<NamedCount>,
    pub genres: Vec<NamedCount>,
    pub top_artists: Vec<NamedCount>,
    pub albums_without_cover: u32,
    pub low_quality_albums: u32,
    pub without_cover: Vec<Album>,
    pub low_quality: Vec<Album>,
}

/// What played on this day some years ago.
#[derive(Clone, uniffi::Record)]
pub struct DayAgo {
    pub years_ago: i32,
    /// The day's start, Unix seconds.
    pub day: i64,
    pub albums: Vec<Album>,
    pub listens: u32,
}

/// A song on the mood map: `x` dark (0) to bright (1), `y` calm (0) to lively (1).
#[derive(Clone, uniffi::Record)]
pub struct MoodDot {
    pub id: String,
    pub x: f32,
    pub y: f32,
    pub mood: String,
}

/// One of fanart.tv's pictures of an artist, to choose from.
#[derive(Clone, uniffi::Record)]
pub struct FanartPick {
    pub url: String,
    pub preview: String,
    pub likes: i64,
    pub language: String,
}

/// How far the background search for artists' pictures is.
#[derive(Clone, uniffi::Record)]
pub struct PictureProgress {
    pub done: u32,
    pub total: u32,
    /// It may look now (online lookups and the setting on, on Wi-Fi).
    pub allowed: bool,
    /// Check all now was pressed.
    pub hurry: bool,
}

/// "photo", "logo", or "banner".
fn picture_kind(kind: &str) -> ArtistPicture {
    match kind {
        "logo" => ArtistPicture::Logo,
        "banner" => ArtistPicture::Banner,
        _ => ArtistPicture::Photo,
    }
}

fn counts(list: Vec<(String, usize)>) -> Vec<NamedCount> {
    list.into_iter()
        .map(|(name, count)| NamedCount {
            name,
            count: count as u32,
        })
        .collect()
}

fn albums(list: Vec<AlbumSummary>) -> Vec<Album> {
    list.into_iter().map(Album::from).collect()
}

fn path(p: std::path::PathBuf) -> String {
    p.to_string_lossy().to_string()
}

/// The one background search for artists' pictures, started on first use.
static SWEEP: OnceLock<Mutex<Option<PictureSweep>>> = OnceLock::new();

#[uniffi::export]
impl Needle {
    /// Home's shelves: albums played lately, new, loved, and most played, and the artists
    /// played most.
    pub fn home_shelves(&self) -> Result<HomeShelves> {
        let home = self.library.home(20)?;
        Ok(HomeShelves {
            recent: albums(home.recent),
            added: albums(home.added),
            favorites: albums(home.favorites),
            most_played: albums(home.most_played),
            artists: home
                .artists
                .into_iter()
                .map(|a| Artist {
                    name: a.name,
                    albums: a.albums as u32,
                    songs: a.tracks as u32,
                    artwork: a.artwork,
                })
                .collect(),
        })
    }

    pub fn more_settings(&self) -> Result<MoreSettings> {
        let s = self.library.settings()?;
        Ok(MoreSettings {
            player_buttons: s.player_buttons().into_iter().map(String::from).collect(),
            artist_order: s.artist_order.clone(),
            album_artists_only: s.album_artists_only,
            artist_pictures: s.artist_pictures,
            smart_mix: s.smart_mix,
        })
    }

    pub fn set_more_settings(&self, value: MoreSettings) -> Result<()> {
        let s = self.library.update_settings(|s| {
            s.player_buttons = value.player_buttons.clone();
            s.artist_order = ArtistOrder::from_name(&value.artist_order).name().to_string();
            s.album_artists_only = value.album_artists_only;
            s.artist_pictures = value.artist_pictures;
            s.smart_mix = value.smart_mix;
        })?;
        self.player.send(Command::Configure(Box::new(s)));
        Ok(())
    }

    /// The Artists list in the order chosen in the settings.
    pub fn artist_list(&self) -> Result<Vec<ArtistEntry>> {
        let s = self.library.settings()?;
        let choice = ArtistChoice {
            order: ArtistOrder::from_name(&s.artist_order),
            album_artists_only: s.album_artists_only,
        };
        Ok(self
            .library
            .artists_by("", choice)?
            .into_iter()
            .map(|a| ArtistEntry {
                name: a.name,
                albums: a.albums as u32,
                songs: a.tracks as u32,
                artwork: a.artwork,
                plays: a.play_count,
                letter: a.letter,
            })
            .collect())
    }

    pub fn library_numbers(&self) -> Result<LibraryNumbers> {
        let n = self.library.numbers()?;
        Ok(LibraryNumbers {
            songs: n.songs as u32,
            albums: n.albums as u32,
            artists: n.artists as u32,
            duration: n.duration,
            size: n.size,
            server_songs: n.server_songs as u32,
            formats: counts(n.formats),
            lossless: n.lossless as u32,
            hires: n.hires as u32,
            decades: counts(n.decades),
            genres: counts(n.genres),
            top_artists: counts(n.artists_by_songs),
            albums_without_cover: n.albums_without_cover as u32,
            low_quality_albums: n.low_quality_albums as u32,
            without_cover: albums(n.without_cover),
            low_quality: albums(n.low_quality),
        })
    }

    /// What played on this day one and five years ago (`offset`: the phone's time zone, in
    /// seconds east of UTC). Days with nothing played are left out.
    pub fn on_this_day(&self, offset: i32) -> Result<Vec<DayAgo>> {
        let now = chrono::Utc::now().timestamp();
        Ok(self
            .library
            .on_this_day(now, offset, &[1, 5], 12)?
            .into_iter()
            .map(|d| DayAgo {
                years_ago: d.years_ago,
                day: d.day,
                albums: albums(d.albums),
                listens: d.listens as u32,
            })
            .collect())
    }

    /// The song's loud and quiet shape for the seek bar, `count` heights 0 to 1; measured
    /// now (about a second) the first time. Empty for a song that cannot be read.
    pub fn waveform(&self, id: String, count: u32) -> Result<Vec<f32>> {
        let Some(track) = self.library.track(&id)? else {
            return Ok(vec![]);
        };
        Ok(self.library.waveform_now(&track)?.heights(count as usize))
    }

    /// The song's shape only if it was measured already; never measures.
    pub fn waveform_kept(&self, id: String, count: u32) -> Vec<f32> {
        self.library
            .waveform(&id)
            .ok()
            .flatten()
            .map(|w| w.heights(count as usize))
            .unwrap_or_default()
    }

    pub fn mood_map(&self) -> Result<Vec<MoodDot>> {
        Ok(self
            .library
            .mood_map()?
            .into_iter()
            .map(|p| MoodDot {
                id: p.id,
                x: p.x,
                y: p.y,
                mood: p.mood.to_string(),
            })
            .collect())
    }

    /// The songs inside a circle on the mood map, nearest its centre first.
    pub fn mood_area(&self, x: f32, y: f32, radius: f32, limit: u32) -> Result<Vec<Song>> {
        Ok(self
            .library
            .mood_area(x, y, radius, limit as usize)?
            .iter()
            .map(Song::from)
            .collect())
    }

    /// The spectrum of what plays now, in `bands` bands spaced as the ear hears, 0 to 1.
    pub fn spectrum(&self, bands: u32) -> Vec<f32> {
        needle_core::scope::spectrum(bands as usize)
    }

    /// How loud what plays now is, 0 to 1.
    pub fn sound_level(&self) -> f32 {
        needle_core::scope::level()
    }

    /// The picture someone chose for this artist ("photo", "logo", or "banner"), if any.
    pub fn own_artist_picture(&self, name: String, kind: String) -> Option<String> {
        pictures::own_picture(&self.library, &name, picture_kind(&kind)).map(path)
    }

    /// Use a picture file as the artist's photo, logo, or wide picture; kept, never replaced.
    pub fn set_artist_picture(&self, name: String, kind: String, file: String) -> Result<String> {
        Ok(path(pictures::set_own_picture(
            &self.library,
            &name,
            picture_kind(&kind),
            std::path::Path::new(&file),
        )?))
    }

    /// Forget the chosen picture: the automatic one shows again.
    pub fn reset_artist_picture(&self, name: String, kind: String) -> Result<()> {
        Ok(pictures::reset_picture(&self.library, &name, picture_kind(&kind))?)
    }

    /// Every picture fanart.tv has of this kind for the artist, best first.
    pub fn artist_fanart_choices(&self, name: String, kind: String) -> Result<Vec<FanartPick>> {
        let online = self.library.settings()?.online_media;
        Ok(pictures::fanart_choices(&self.library, &name, picture_kind(&kind), online)?
            .into_iter()
            .map(|c| FanartPick {
                url: c.url,
                preview: c.preview,
                likes: c.likes,
                language: c.language,
            })
            .collect())
    }

    /// Use one of fanart.tv's pictures as the artist's.
    pub fn pick_artist_fanart(&self, name: String, kind: String, url: String) -> Result<String> {
        Ok(path(pictures::pick_fanart(
            &self.library,
            &name,
            picture_kind(&kind),
            &url,
        )?))
    }

    /// Whether the background search for artists' pictures may look now: the app says so
    /// from the settings and the network (Wi-Fi only on a phone). Starts it the first time.
    pub fn set_artist_pictures_allowed(&self, allowed: bool) {
        let sweep = SWEEP.get_or_init(|| Mutex::new(None));
        let mut sweep = sweep.lock().unwrap_or_else(|e| e.into_inner());
        if sweep.is_none() && allowed {
            *sweep = Some(PictureSweep::start(self.library.clone()));
        }
        if let Some(sweep) = sweep.as_ref() {
            sweep.set_allowed(allowed);
        }
    }

    /// Look for every artist's pictures now, at the services' own pace.
    pub fn check_all_artist_pictures(&self) -> Result<()> {
        let sweep = SWEEP.get_or_init(|| Mutex::new(None));
        let mut sweep = sweep.lock().unwrap_or_else(|e| e.into_inner());
        if sweep.is_none() {
            *sweep = Some(PictureSweep::start(self.library.clone()));
        }
        match sweep.as_ref() {
            Some(s) => {
                s.check_all_now();
                Ok(())
            }
            None => Err(NeedleError::from(anyhow::anyhow!("The search could not start"))),
        }
    }

    pub fn artist_pictures_progress(&self) -> PictureProgress {
        let sweep = SWEEP.get_or_init(|| Mutex::new(None));
        let sweep = sweep.lock().unwrap_or_else(|e| e.into_inner());
        let p = sweep.as_ref().map(|s| s.progress()).unwrap_or_default();
        PictureProgress {
            done: p.done as u32,
            total: p.total as u32,
            allowed: p.allowed,
            hurry: p.hurry,
        }
    }
}
