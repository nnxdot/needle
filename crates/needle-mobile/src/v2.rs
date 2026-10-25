//! Needle 2.0 for the phones (and the Mac where it is the same): artists in the order chosen
//! with their A–Z letters, artists' pictures (the slow search, and the ones you choose), the
//! library in numbers, on this day, the mood map, the waveform seek bar, the visualizer, and
//! the settings that came with them.
use crate::{Album, Needle, Result};
use needle_core::{
    artists::{ArtistChoice, ArtistOrder},
    audio::{Command, QueueItem},
    media::pictures::{self, ArtistPicture, PictureSweep},
};
use std::{
    path::Path,
    sync::{
        OnceLock,
        atomic::{AtomicBool, Ordering},
    },
};

/// The slow search for every artist's pictures, started the first time it may look.
static SWEEP: OnceLock<PictureSweep> = OnceLock::new();
/// The app lets it look now (no library scan runs; on a phone, on Wi-Fi).
static SWEEP_MAY: AtomicBool = AtomicBool::new(false);

fn picture_kind(kind: &str) -> ArtistPicture {
    match kind {
        "logo" => ArtistPicture::Logo,
        "banner" => ArtistPicture::Banner,
        _ => ArtistPicture::Photo,
    }
}

/// The 2.0 settings, kept with the library's (the computer's names).
#[derive(Clone, uniffi::Record)]
pub struct MoreSettings {
    /// The player's buttons, in order: "shuffle", "previous", "play", "next", "repeat",
    /// "favorite", "lyrics", "queue", "speakers".
    pub player_buttons: Vec<String>,
    /// "name", "most_played", "most_songs", or "recently_added".
    pub artist_order: String,
    /// The Artists list shows only album artists.
    pub album_artists_only: bool,
    /// Find every artist's photo, logo, and wide picture slowly in the background.
    pub artist_pictures: bool,
    /// The crossfade starts where a song's music ends and lasts whole bars of its tempo.
    pub smart_mix: bool,
}

/// An artist in the list, with the letter they file under in the A–Z bar.
#[derive(Clone, uniffi::Record)]
pub struct ListedArtist {
    pub name: String,
    pub albums: u32,
    pub songs: u32,
    pub plays: i64,
    pub artwork: Option<String>,
    /// "A"–"Z", "#", or another script's letter.
    pub letter: String,
}

/// How far the slow search for artists' pictures is.
#[derive(Clone, uniffi::Record)]
pub struct PictureProgress {
    pub done: u32,
    pub total: u32,
    pub allowed: bool,
    /// Check all now was pressed.
    pub hurry: bool,
}

/// One of fanart.tv's pictures of an artist.
#[derive(Clone, uniffi::Record)]
pub struct FanartPick {
    pub url: String,
    /// A small copy, for the list to choose from.
    pub preview: String,
    pub likes: i64,
    pub language: String,
}

/// A name and how many.
#[derive(Clone, uniffi::Record)]
pub struct Counted {
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
    /// Bytes on this device.
    pub size: i64,
    pub server_songs: u32,
    pub formats: Vec<Counted>,
    pub lossless: u32,
    pub hires: u32,
    pub decades: Vec<Counted>,
    pub genres: Vec<Counted>,
    pub artists_by_songs: Vec<Counted>,
    pub albums_without_cover: u32,
    pub low_quality_albums: u32,
    pub without_cover: Vec<Album>,
    pub low_quality: Vec<Album>,
}

/// What played on this day some years ago.
#[derive(Clone, uniffi::Record)]
pub struct DayPlayed {
    pub years_ago: i32,
    /// The day's start (Unix seconds).
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

fn counted(list: Vec<(String, usize)>) -> Vec<Counted> {
    list.into_iter()
        .map(|(name, count)| Counted {
            name,
            count: count as u32,
        })
        .collect()
}

impl Needle {
    /// The search looks while the app lets it and online lookups and Artist pictures are on.
    pub(crate) fn refresh_sweep(&self) {
        let settings = self.library.settings().unwrap_or_default();
        let allowed =
            SWEEP_MAY.load(Ordering::SeqCst) && settings.online_media && settings.artist_pictures;
        if allowed || SWEEP.get().is_some() {
            SWEEP
                .get_or_init(|| PictureSweep::start(self.library.clone()))
                .set_allowed(allowed);
        }
    }

    pub(crate) fn sweep_progress(&self) -> pictures::SweepProgress {
        SWEEP.get().map(|s| s.progress()).unwrap_or_default()
    }
}

#[uniffi::export]
impl Needle {
    pub fn more_settings(&self) -> Result<MoreSettings> {
        let s = self.library.settings()?;
        Ok(MoreSettings {
            player_buttons: s.player_buttons().into_iter().map(String::from).collect(),
            artist_order: s.artist_order,
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
        self.refresh_sweep();
        Ok(())
    }

    /// The Artists list, in the order chosen, with only album artists when asked.
    pub fn listed_artists(&self) -> Result<Vec<ListedArtist>> {
        let s = self.library.settings()?;
        let choice = ArtistChoice {
            order: ArtistOrder::from_name(&s.artist_order),
            album_artists_only: s.album_artists_only,
        };
        Ok(self
            .library
            .artists_by("", choice)?
            .into_iter()
            .map(|a| ListedArtist {
                name: a.name,
                albums: a.albums as u32,
                songs: a.tracks as u32,
                plays: a.play_count,
                artwork: a.artwork,
                letter: a.letter,
            })
            .collect())
    }

    /// Whether the slow search for artists' pictures may look now (`may`: no library scan
    /// runs; on a phone, on Wi-Fi); it also needs online lookups and Artist pictures on.
    /// Starts it the first time.
    pub fn artist_pictures_allow(&self, may: bool) {
        SWEEP_MAY.store(may, Ordering::SeqCst);
        self.refresh_sweep();
    }

    /// How far the search is ("212 of 640 artists").
    pub fn artist_pictures_progress(&self) -> PictureProgress {
        let p = self.sweep_progress();
        PictureProgress {
            done: p.done as u32,
            total: p.total as u32,
            allowed: p.allowed,
            hurry: p.hurry,
        }
    }

    /// Check all now: every artist, at the services' own pace.
    pub fn artist_pictures_check_all_now(&self) {
        self.refresh_sweep();
        if let Some(sweep) = SWEEP.get() {
            sweep.check_all_now();
        }
    }

    /// Which of the artist's pictures someone chose ("photo", "logo", "banner").
    pub fn artist_own_pictures(&self, name: String) -> Vec<String> {
        ["photo", "logo", "banner"]
            .into_iter()
            .filter(|kind| pictures::own_picture(&self.library, &name, picture_kind(kind)).is_some())
            .map(String::from)
            .collect()
    }

    /// Uses a picture file as the artist's photo, logo, or wide picture (`kind`); kept, and
    /// never replaced by a lookup.
    pub fn set_artist_picture(&self, name: String, kind: String, path: String) -> Result<()> {
        pictures::set_own_picture(&self.library, &name, picture_kind(&kind), Path::new(&path))?;
        Ok(())
    }

    /// Forgets the chosen picture: the automatic one (or the folder's) shows again.
    pub fn reset_artist_picture(&self, name: String, kind: String) -> Result<()> {
        pictures::reset_picture(&self.library, &name, picture_kind(&kind))?;
        Ok(())
    }

    /// Every picture fanart.tv has of this kind for the artist, best first. Empty with online
    /// lookups off.
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

    /// Uses one of fanart.tv's pictures as the artist's, kept and never replaced.
    pub fn pick_artist_fanart(&self, name: String, kind: String, url: String) -> Result<()> {
        pictures::pick_fanart(&self.library, &name, picture_kind(&kind), &url)?;
        Ok(())
    }

    /// Your library in numbers.
    pub fn library_numbers(&self) -> Result<LibraryNumbers> {
        let n = self.library.numbers()?;
        let albums = |list: Vec<needle_core::browse::AlbumSummary>| list.into_iter().map(Album::from).collect();
        Ok(LibraryNumbers {
            songs: n.songs as u32,
            albums: n.albums as u32,
            artists: n.artists as u32,
            duration: n.duration,
            size: n.size,
            server_songs: n.server_songs as u32,
            formats: counted(n.formats),
            lossless: n.lossless as u32,
            hires: n.hires as u32,
            decades: counted(n.decades),
            genres: counted(n.genres),
            artists_by_songs: counted(n.artists_by_songs),
            albums_without_cover: n.albums_without_cover as u32,
            low_quality_albums: n.low_quality_albums as u32,
            without_cover: albums(n.without_cover),
            low_quality: albums(n.low_quality),
        })
    }

    /// What played on this day one and five years ago (days with plays only). `offset`:
    /// seconds east of UTC here.
    pub fn on_this_day(&self, offset: i32) -> Result<Vec<DayPlayed>> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or_default();
        Ok(self
            .library
            .on_this_day(now, offset, &[1, 5], 12)?
            .into_iter()
            .map(|d| DayPlayed {
                years_ago: d.years_ago,
                day: d.day,
                albums: d.albums.into_iter().map(Album::from).collect(),
                listens: d.listens as u32,
            })
            .collect())
    }

    /// Every measured song on the mood map.
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

    /// Plays the songs inside a circle on the mood map (places 0 to 1). Returns how many.
    pub fn play_mood_area(&self, x: f32, y: f32, radius: f32) -> Result<u32> {
        let tracks = self.library.mood_area(x, y, radius, 200)?;
        let reason = "From the mood map".to_string();
        let items: Vec<QueueItem> = tracks
            .into_iter()
            .map(|track| QueueItem {
                track,
                reason: reason.clone(),
            })
            .collect();
        let count = items.len() as u32;
        if count > 0 {
            self.player.send(Command::Play(items));
        }
        Ok(count)
    }

    /// The song's loud and quiet shape for the seek bar, `count` heights 0 to 1, measuring it
    /// now if it was not (about a second: call it away from the main thread). Empty when it
    /// cannot be read.
    pub fn waveform_heights(&self, id: String, count: u32) -> Vec<f32> {
        let Ok(Some(track)) = self.library.track(&id) else {
            return vec![];
        };
        self.library
            .waveform_now(&track)
            .map(|w| w.heights(count as usize))
            .unwrap_or_default()
    }

    /// What plays now, for the visualizer: `bands` bands from 30 Hz to 16 kHz, each 0 to 1.
    pub fn spectrum(&self, bands: u32) -> Vec<f32> {
        needle_core::scope::spectrum(bands as usize)
    }
}
