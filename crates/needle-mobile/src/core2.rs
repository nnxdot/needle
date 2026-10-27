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
    /// The big player's look: "cover", "record", "lyrics", or "visualizer".
    #[uniffi(default = "cover")]
    pub big_look: String,
}

/// The match that heads a search's results: `kind` is "song", "album", or "artist", and the
/// one of `song`, `album`, `artist` it names is set.
#[derive(Clone, uniffi::Record)]
pub struct SearchBest {
    pub kind: String,
    pub song: Option<Song>,
    pub album: Option<Album>,
    pub artist: Option<Artist>,
}

/// A song whose lyrics hold the search, with the line they are in.
#[derive(Clone, uniffi::Record)]
pub struct LyricHit {
    pub song: Song,
    pub line: String,
}

/// What a search finds, for the Search page: the best match, then a few of each.
#[derive(Clone, uniffi::Record)]
pub struct SearchFound {
    pub best: Option<SearchBest>,
    pub songs: Vec<Song>,
    pub albums: Vec<Album>,
    pub artists: Vec<Artist>,
    pub lyrics: Vec<LyricHit>,
}

fn artist_of(a: needle_core::browse::ArtistSummary) -> Artist {
    Artist {
        name: a.name,
        albums: a.albums as u32,
        songs: a.tracks as u32,
        artwork: a.artwork,
        letter: a.letter,
        plays: a.play_count,
    }
}

/// Home's shelves, as on the computers.
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
            big_look: s.big_look.clone(),
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
            s.big_look = if needle_core::model::BIG_LOOKS.contains(&value.big_look.as_str()) {
                value.big_look.clone()
            } else {
                "cover".into()
            };
        })?;
        self.player.send(Command::Configure(Box::new(s)));
        Ok(())
    }

    /// Home's shelves, `limit` covers each (On this day is [`Needle::on_this_day`]).
    pub fn home_shelves(&self, limit: u32) -> Result<HomeShelves> {
        let home = self.library.home(limit.max(1) as usize)?;
        let albums = |list: Vec<needle_core::browse::AlbumSummary>| -> Vec<Album> {
            list.into_iter().map(Album::from).collect()
        };
        Ok(HomeShelves {
            recent: albums(home.recent),
            added: albums(home.added),
            favorites: albums(home.favorites),
            most_played: albums(home.most_played),
            artists: home.artists.into_iter().map(artist_of).collect(),
        })
    }

    /// The Search page's "Try a rule" chips, as name and rule in turn.
    pub fn rule_examples(&self) -> Vec<String> {
        needle_core::find::RULE_EXAMPLES
            .iter()
            .flat_map(|(name, rule)| [name.to_string(), rule.to_string()])
            .collect()
    }

    /// What `query` finds (it may be a rule): the best match, then up to `limit` songs,
    /// albums, artists, and lines of lyrics that hold it. The lyrics are the songs' own (read
    /// in the background) and any shown since.
    pub fn find(&self, query: String, limit: u32) -> Result<SearchFound> {
        use needle_core::find::Best;
        let found = self.library.find(&query, limit.max(1) as usize)?;
        Ok(SearchFound {
            best: found.best.map(|best| match best {
                Best::Song(t) => SearchBest {
                    kind: "song".into(),
                    song: Some(Song::from(&*t)),
                    album: None,
                    artist: None,
                },
                Best::Album(a) => SearchBest {
                    kind: "album".into(),
                    song: None,
                    album: Some(Album::from(a)),
                    artist: None,
                },
                Best::Artist(a) => SearchBest {
                    kind: "artist".into(),
                    song: None,
                    album: None,
                    artist: Some(artist_of(a)),
                },
            }),
            songs: found.songs.iter().map(Song::from).collect(),
            albums: found.albums.into_iter().map(Album::from).collect(),
            artists: found.artists.into_iter().map(artist_of).collect(),
            lyrics: found
                .lyrics
                .into_iter()
                .map(|m| LyricHit {
                    song: Song::from(&m.track),
                    line: m.line,
                })
                .collect(),
        })
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

/// The sleep timer while it runs.
#[derive(Clone, uniffi::Record)]
pub struct SleepTimerState {
    /// "minutes", "end_of_song", or "end_of_album".
    pub kind: String,
    /// For "minutes": the minutes chosen, with any added since.
    pub minutes: u32,
    /// The music fades out over the last minute.
    pub fade: bool,
    /// Seconds left (for the end of a song or an album: what is left of it, at its speed).
    pub remaining: f64,
    /// When the music stops (Unix seconds).
    pub ends_at: i64,
}

/// How songs play: the speed (0.5 to 2), the pitch in semitones (−12 to 12), whether a new
/// speed keeps the pitch, and whether this is for the song playing only. `available` is false
/// on bit-perfect USB output, where nothing may change the sound.
#[derive(Clone, uniffi::Record)]
pub struct SpeedPitch {
    pub speed: f32,
    pub semitones: f32,
    pub keep_pitch: bool,
    pub only_this_song: bool,
    pub available: bool,
}

/// One month of a year's listening.
#[derive(Clone, uniffi::Record)]
pub struct MonthPlays {
    /// 1 (January) to 12.
    pub month: u32,
    /// Plays (half the song or four minutes).
    pub plays: u32,
    /// Every listen, skips too.
    pub listens: u32,
    pub seconds: f64,
}

#[derive(Clone, uniffi::Record)]
pub struct YearTopArtist {
    pub name: String,
    pub seconds: f64,
    pub plays: u32,
    /// The months the artist was among the five most played.
    pub months_in_top_five: u32,
}

#[derive(Clone, uniffi::Record)]
pub struct TopSong {
    pub song: Song,
    pub plays: u32,
}

/// "Your 2026 in music".
#[derive(Clone, uniffi::Record)]
pub struct YearInReview {
    pub year: i32,
    /// Seconds listened (hours: / 3600; whole days: / 86400).
    pub seconds: f64,
    pub plays: u32,
    pub listens: u32,
    /// Songs first heard that year.
    pub new_songs: u32,
    /// January to December, always 12.
    pub months: Vec<MonthPlays>,
    /// The months with any listening.
    pub listening_months: u32,
    pub top_artist: Option<YearTopArtist>,
    /// The most played songs, most first, up to 50 (Play top 50 plays them).
    pub top_songs: Vec<TopSong>,
}

/// One line being timed in [`LyricTimer`].
#[derive(Clone, uniffi::Record)]
pub struct TimingLine {
    pub text: String,
    /// Seconds, once tapped.
    pub time: Option<f64>,
    /// The words with the spaces after them, and their times (in word mode).
    pub words: Vec<TimingWord>,
}

#[derive(Clone, uniffi::Record)]
pub struct TimingWord {
    pub text: String,
    pub time: Option<f64>,
}

/// "Time the lyrics": tap as each line (or word) starts; −0.2 s and +0.2 s move the line last
/// tapped; Redo line taps it again. The same steps as on the computers (needle-core's
/// `timing`). Save with [`Needle::save_lyric_timer`].
#[derive(uniffi::Object)]
pub struct LyricTimer {
    timing: std::sync::Mutex<needle_core::timing::LyricTiming>,
}

impl LyricTimer {
    fn with<T>(&self, f: impl FnOnce(&mut needle_core::timing::LyricTiming) -> T) -> T {
        f(&mut self.timing.lock().unwrap_or_else(|e| e.into_inner()))
    }
}

#[uniffi::export]
impl LyricTimer {
    /// From the song's lyrics (timed ones keep their times, to correct), or, without any, from
    /// `text` typed or pasted (one line per sung line).
    #[uniffi::constructor]
    pub fn new(lyrics: Option<crate::media::Lyrics>, text: String) -> std::sync::Arc<Self> {
        use needle_core::{media, timing::LyricTiming};
        let timing = match lyrics {
            Some(l) if !l.lines.is_empty() || !l.plain.trim().is_empty() => {
                LyricTiming::from_lyrics(&media::Lyrics {
                    lines: l
                        .lines
                        .into_iter()
                        .map(|line| media::LyricLine {
                            time: line.time,
                            text: line.text,
                            words: line
                                .words
                                .into_iter()
                                .map(|w| media::LyricWord {
                                    time: w.time,
                                    text: w.text,
                                })
                                .collect(),
                        })
                        .collect(),
                    plain: l.plain,
                    instrumental: l.instrumental,
                    source: media::LyricsSource::Sidecar,
                    provider: String::new(),
                })
            }
            _ => LyricTiming::from_text(&text),
        };
        std::sync::Arc::new(Self {
            timing: std::sync::Mutex::new(timing),
        })
    }

    pub fn lines(&self) -> Vec<TimingLine> {
        self.with(|t| {
            t.lines
                .iter()
                .map(|l| TimingLine {
                    text: l.text.clone(),
                    time: l.time,
                    words: l
                        .words
                        .iter()
                        .map(|(text, time)| TimingWord {
                            text: text.clone(),
                            time: *time,
                        })
                        .collect(),
                })
                .collect()
        })
    }
    /// The line the next tap stamps (the number of lines once all are tapped).
    pub fn next_line(&self) -> u32 {
        self.with(|t| t.next.0 as u32)
    }
    /// In word mode, the word of `next_line` the next tap stamps.
    pub fn next_word(&self) -> u32 {
        self.with(|t| t.next.1 as u32)
    }
    /// The line −0.2 s and +0.2 s move (the last tapped or chosen).
    pub fn selected(&self) -> Option<u32> {
        self.with(|t| t.selected.map(|i| i as u32))
    }
    pub fn word_mode(&self) -> bool {
        self.with(|t| t.words)
    }
    /// Tap words instead of lines (or lines again), from the chosen line.
    pub fn set_word_mode(&self, on: bool) {
        self.with(|t| t.set_words(on))
    }
    /// Stamp the next line (or word) at `position` seconds of the song.
    pub fn tap(&self, position: f64) {
        self.with(|t| t.tap(position))
    }
    /// Take back the last tap; false when there is none.
    pub fn undo(&self) -> bool {
        self.with(|t| t.undo())
    }
    /// −0.2 s.
    pub fn earlier(&self) {
        self.with(|t| t.nudge(-needle_core::timing::NUDGE))
    }
    /// +0.2 s.
    pub fn later(&self) {
        self.with(|t| t.nudge(needle_core::timing::NUDGE))
    }
    /// Redo line: the next tap stamps the last tapped line again.
    pub fn redo_line(&self) {
        self.with(|t| t.redo_line())
    }
    /// Choose a line: the next tap stamps it.
    pub fn select(&self, index: u32) {
        self.with(|t| t.select(index as usize))
    }
    /// Every line is tapped.
    pub fn done(&self) -> bool {
        self.with(|t| t.done())
    }
    /// Something changed since it opened or was saved (Save is worth it).
    pub fn changed(&self) -> bool {
        self.with(|t| t.changed)
    }
    /// The lines to save, every gap filled in; empty while nothing is tapped.
    pub fn finished(&self) -> Vec<crate::media::LyricLine> {
        self.with(|t| t.finished()).map_or_else(Vec::new, |lines| {
            lines
                .into_iter()
                .map(|l| crate::media::LyricLine {
                    time: l.time,
                    text: l.text,
                    words: l
                        .words
                        .into_iter()
                        .map(|w| crate::media::Word {
                            time: w.time,
                            text: w.text,
                        })
                        .collect(),
                })
                .collect()
        })
    }
    /// The `.lrc` text; empty while nothing is tapped.
    pub fn lrc(&self) -> String {
        self.with(|t| t.lrc())
    }
}

fn sleep_kind_name(kind: needle_core::audio::SleepKind) -> (&'static str, u32) {
    use needle_core::audio::SleepKind;
    match kind {
        SleepKind::After(seconds) => ("minutes", (seconds / 60.).round() as u32),
        SleepKind::EndOfSong => ("end_of_song", 0),
        SleepKind::EndOfAlbum => ("end_of_album", 0),
    }
}

#[uniffi::export]
impl Needle {
    /// The sleep timer's choices in minutes, as chips: 15, 30, 45, 60 (then "End of song" and
    /// "End of album").
    pub fn sleep_minutes(&self) -> Vec<u32> {
        vec![15, 30, 45, 60]
    }

    /// The sleep timer, while it runs (read with the playback, a few times a second).
    pub fn sleep_timer(&self) -> Option<SleepTimerState> {
        let sleep = self.player.state().sleep?;
        let (kind, minutes) = sleep_kind_name(sleep.kind);
        Some(SleepTimerState {
            kind: kind.into(),
            minutes,
            fade: sleep.fade,
            remaining: sleep.remaining,
            ends_at: chrono::Utc::now().timestamp() + sleep.remaining.round() as i64,
        })
    }

    /// Start the sleep timer, again from now: `kind` "minutes" (with `minutes`),
    /// "end_of_song", or "end_of_album". `fade` is kept as the switch's next default. When it
    /// ends the music pauses; for the end of a song or an album, the next song waits at its
    /// start.
    pub fn start_sleep_timer(&self, kind: String, minutes: u32, fade: bool) -> Result<()> {
        use needle_core::audio::{SleepKind, SleepTimer};
        let kind = match kind.as_str() {
            "minutes" if minutes > 0 => SleepKind::After(minutes as f64 * 60.),
            "end_of_song" => SleepKind::EndOfSong,
            "end_of_album" => SleepKind::EndOfAlbum,
            _ => {
                return Err(NeedleError::from(anyhow::anyhow!(
                    "A sleep timer is \"minutes\" (more than 0), \"end_of_song\", or \"end_of_album\""
                )));
            }
        };
        self.player
            .send(Command::Sleep(Some(SleepTimer { kind, fade })));
        Ok(())
    }

    pub fn cancel_sleep_timer(&self) {
        self.player.send(Command::Sleep(None));
    }

    /// More time on the running timer (Android's "Shake to add 10 minutes"). A timer for the
    /// end of a song or an album becomes one for its time left plus these minutes.
    pub fn add_sleep_minutes(&self, minutes: u32) {
        self.player.send(Command::SleepLonger(minutes as f64 * 60.));
    }

    /// The Fade out switch: for the running timer, and kept for the next.
    pub fn set_sleep_fade(&self, fade: bool) {
        self.player.send(Command::SleepFade(fade));
    }

    /// The Fade out switch as last chosen (on at first).
    pub fn sleep_fade(&self) -> bool {
        self.library.settings().map_or(true, |s| s.sleep_fade)
    }

    /// How many times the sleep timer has stopped the music since Needle started: when it
    /// goes up, the timer has just ended.
    pub fn sleep_timer_ended(&self) -> u64 {
        self.player.state().sleep_ended
    }

    /// The speed and pitch now (a change from [`Needle::set_speed_and_pitch`] shows here
    /// within a few hundredths of a second).
    pub fn speed_and_pitch(&self) -> SpeedPitch {
        let state = self.player.state();
        SpeedPitch {
            speed: state.tempo.speed,
            semitones: state.tempo.semitones,
            keep_pitch: state.tempo.keep_pitch,
            only_this_song: state.tempo_song_only,
            available: state.tempo_available,
        }
    }

    /// Change the speed and pitch at once, without a break in the music; kept for every song
    /// unless `only_this_song` (`available` is ignored). Speeds snap to hundredths, pitch to
    /// tenths of a semitone.
    pub fn set_speed_and_pitch(&self, value: SpeedPitch) {
        self.player.send(Command::Tempo(
            needle_core::tempo::Tempo {
                speed: value.speed,
                semitones: value.semitones,
                keep_pitch: value.keep_pitch,
            },
            value.only_this_song,
        ));
    }

    /// The speed chips: 0.75, 1, 1.25, 1.5, 2.
    pub fn speed_presets(&self) -> Vec<f32> {
        needle_core::tempo::SPEED_PRESETS.to_vec()
    }

    /// The years with any listening, newest first.
    pub fn listening_years(&self) -> Result<Vec<i32>> {
        Ok(self.library.listening_years()?)
    }

    /// The year to open first: last year during January, else this one.
    pub fn review_year(&self) -> Result<i32> {
        let years = self.library.listening_years()?;
        Ok(needle_core::wrapped::default_year(
            &years,
            chrono::Local::now().date_naive(),
        ))
    }

    /// The year for Home's "Your year in music" shelf: this one, in December, when it has
    /// any listening.
    pub fn home_review_year(&self) -> Result<Option<i32>> {
        let years = self.library.listening_years()?;
        Ok(
            needle_core::wrapped::home_year(chrono::Local::now().date_naive())
                .filter(|year| years.contains(year)),
        )
    }

    /// "Your 2026 in music", from the listening history (call it off the main thread).
    pub fn year_in_review(&self, year: i32) -> Result<YearInReview> {
        let w = self.library.wrapped(year)?;
        let top_artist = w.stats.top_artists.first().map(|a| YearTopArtist {
            name: a.artist.clone(),
            seconds: a.seconds,
            plays: a.plays as u32,
            months_in_top_five: w.top_artist_months as u32,
        });
        let top_songs = w
            .top_track_ids
            .iter()
            .zip(&w.top_track_plays)
            .filter_map(|(id, plays)| {
                Some(TopSong {
                    song: Song::from(w.tracks.get(id)?),
                    plays: *plays as u32,
                })
            })
            .collect();
        Ok(YearInReview {
            year: w.year,
            seconds: w.stats.seconds,
            plays: w.stats.plays as u32,
            listens: w.stats.listens as u32,
            new_songs: w.new_songs as u32,
            months: w
                .months
                .iter()
                .map(|m| MonthPlays {
                    month: m.month,
                    plays: m.plays as u32,
                    listens: m.listens as u32,
                    seconds: m.seconds,
                })
                .collect(),
            listening_months: w.listening_months as u32,
            top_artist,
            top_songs,
        })
    }

    /// Make a playlist: the year's top songs (up to 50) as "Top songs of 2026"; again, the
    /// same playlist refreshed. Returns its id.
    pub fn save_year_playlist(&self, year: i32) -> Result<String> {
        Ok(self.library.save_year_playlist(year)?.id)
    }

    /// Save the timed lyrics for the song `id` (on the phone, in Needle's own storage, as
    /// [`Needle::save_timed_lyrics`]). False when nothing was tapped.
    pub fn save_lyric_timer(&self, id: String, timer: std::sync::Arc<LyricTimer>) -> Result<bool> {
        let lines = timer.finished();
        if lines.is_empty() {
            return Ok(false);
        }
        self.save_timed_lyrics(id, lines)?;
        timer.with(|t| t.changed = false);
        Ok(true)
    }
}

#[cfg(test)]
mod timer_tests {
    use super::LyricTimer;

    #[test]
    fn the_lyric_timer_taps_moves_redoes_and_gives_lrc() {
        let timer = LyricTimer::new(None, "One\n\nTwo".into());
        assert_eq!(timer.lines().len(), 2);
        assert!(timer.finished().is_empty());
        timer.tap(1.0);
        timer.later();
        timer.tap(3.0);
        assert!(timer.done());
        timer.redo_line();
        assert_eq!(timer.next_line(), 1);
        timer.tap(3.5);
        timer.earlier();
        assert_eq!(timer.lrc(), "[00:01.20]One\n[00:03.30]Two\n");
        assert_eq!(timer.finished().len(), 2);
        assert!(timer.undo());
        assert_eq!(timer.lines()[1].time, Some(3.0));
    }
}
