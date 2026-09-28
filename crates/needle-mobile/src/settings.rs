//! Settings for the phone app: what the core plays with (crossfade, ReplayGain, the
//! equalizer, scrobbling), and the app's own look, kept beside the library's settings.
use crate::{Needle, NeedleError, Result};
use needle_core::{
    audio::Command,
    dsp,
    integrations::{self, Credentials, LastfmPending},
};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;

#[derive(Clone, uniffi::Record)]
pub struct PlaybackSettings {
    /// Seconds songs overlap as one ends and the next begins (0 = off).
    pub crossfade: f32,
    /// Even out loudness with the songs' ReplayGain tags.
    pub replay_gain: bool,
    /// With ReplayGain: keep an album's own quiet and loud songs as they are.
    pub album_gain: bool,
    /// Look up lyrics online (LRCLIB) when a song has none.
    pub online_media: bool,
    /// When nothing is left up next, keep playing songs that match this search (empty: stop).
    pub autoplay_query: String,
    /// Measure each song's sound in the background, for radio.
    pub sound_analysis: bool,
    /// Scrobble with the artist's name as Apple Music writes it (키키 → KiiiKiii).
    pub scrobble_corrections: bool,
    /// Ask needle.nnx.fyi once a day whether a newer Needle is out.
    pub check_updates: bool,
}

#[derive(Clone, uniffi::Record)]
pub struct SoundSettings {
    pub eq: bool,
    pub preamp: f32,
    /// Gain in dB for each of [`Needle::eq_bands`].
    pub bands: Vec<f32>,
    pub preset: String,
    /// −1 fully left, +1 fully right.
    pub balance: f32,
    pub mono: bool,
    pub crossfeed: bool,
    /// "graphic" (the ten sliders) or "parametric" (bands of any kind, frequency, and width).
    pub mode: String,
    pub parametric: Vec<EqBand>,
}

/// One band of the parametric equalizer.
#[derive(Clone, uniffi::Record)]
pub struct EqBand {
    /// "peak", "lowshelf", "highshelf", "lowpass", "highpass", or "notch".
    pub kind: String,
    pub frequency: f32,
    pub gain: f32,
    pub q: f32,
    pub on: bool,
}

#[derive(Clone, uniffi::Record)]
pub struct EqPreset {
    pub name: String,
    pub preamp: f32,
    pub bands: Vec<f32>,
}

/// The app's own look and behaviour.
#[derive(Clone, Serialize, Deserialize, uniffi::Record)]
#[serde(default)]
pub struct AppSettings {
    /// The player's background is the playing cover, blurred.
    pub cover_backdrop: bool,
    /// Colours from the wallpaper (Android 12 and newer) instead of Needle's amber.
    pub wallpaper_colors: bool,
    /// Needle's own switch, besides Android's "Remove animations".
    pub reduce_motion: bool,
    /// Lyrics move with the song, the line playing lit.
    pub live_lyrics: bool,
    /// A little vibration on play, pause, and long presses.
    pub haptics: bool,
    /// The player opens by itself when a song is chosen.
    pub open_player_on_play: bool,
    /// Keep the screen on while the lyrics show.
    pub keep_screen_on_lyrics: bool,
    /// "night", "midnight", "day", or a custom theme's id.
    pub theme: String,
    /// Album, playlist, and artist pages take their cover's colours.
    pub cover_colors: bool,
    /// The whole app takes the colours of the song playing.
    pub ambient_colors: bool,
    /// The blurred cover behind pages and the player turns slowly.
    pub moving_backdrop: bool,
    /// How soft that blurred cover is, 20 to 120.
    pub backdrop_blur: f32,
    /// The seek bar and the mini player's progress: "wave", "line", or "thick".
    pub seek_style: String,
    /// The mini player takes the playing cover's colour, rather than the page's.
    pub mini_player_colored: bool,
    /// The player's cover: "square", "round" (a turning record), or "shape" (Material's shapes).
    pub cover_shape: String,
    /// Font for titles: "flex" (Roboto Flex, narrow and heavy), "fraunces" (a soft serif, as
    /// on the desktop), or "system".
    pub title_font: String,
    /// All text larger or smaller: 0.85 to 1.3.
    pub text_scale: f32,
    /// How close lists sit: "compact", "comfortable", or "spacious".
    pub density: String,
    /// The accent where no cover gives one: a hex colour like "#3e63dd", or empty for amber.
    pub accent_color: String,
    /// Film grain over the whole app, 0 (none) to 1, as on the desktop.
    pub grain: f32,
    /// The player's buttons: "expressive" (Material shapes that change as they are pressed),
    /// "round", or "minimal" (icons only, as in Apple Music).
    pub control_style: String,
    /// The buttons beside play: "skip" (the song before and after) or "jump" (10 seconds).
    pub side_buttons: String,
    /// A volume slider in the player (always there while playing on the computer).
    pub volume_slider: bool,
    /// The welcome guide was seen (or skipped).
    pub welcomed: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            cover_backdrop: true,
            wallpaper_colors: false,
            reduce_motion: false,
            live_lyrics: true,
            haptics: true,
            open_player_on_play: false,
            keep_screen_on_lyrics: false,
            theme: "night".into(),
            cover_colors: true,
            ambient_colors: false,
            moving_backdrop: true,
            backdrop_blur: 70.0,
            seek_style: "wave".into(),
            mini_player_colored: true,
            cover_shape: "square".into(),
            title_font: "flex".into(),
            text_scale: 1.0,
            density: "comfortable".into(),
            accent_color: String::new(),
            grain: 0.0,
            control_style: "expressive".into(),
            side_buttons: "skip".into(),
            volume_slider: false,
            welcomed: false,
        }
    }
}

#[derive(Clone, uniffi::Record)]
pub struct Accounts {
    /// Signed in to Last.fm as this user (empty name when Last.fm did not give one).
    pub lastfm_user: Option<String>,
    /// Scrobbles go to Last.fm.
    pub lastfm_on: bool,
    /// This build has a Last.fm application key, so signing in needs only the browser.
    pub lastfm_key_built_in: bool,
    pub listenbrainz_user: Option<String>,
    pub listenbrainz_on: bool,
    /// Listens waiting to be sent.
    pub waiting: u32,
    pub problem: Option<String>,
}

const APP_KEY: &str = "android.app";

/// A Last.fm sign-in waiting for the browser.
pub(crate) static PENDING: Mutex<Option<LastfmPending>> = Mutex::new(None);

fn failed(error: anyhow::Error) -> NeedleError {
    NeedleError::from(error)
}

#[uniffi::export]
impl Needle {
    pub fn playback_settings(&self) -> Result<PlaybackSettings> {
        let s = self.library.settings()?;
        Ok(PlaybackSettings {
            crossfade: s.crossfade,
            replay_gain: s.replay_gain,
            album_gain: s.album_gain,
            online_media: s.online_media,
            autoplay_query: s.autoplay_query,
            sound_analysis: s.sound_analysis,
            scrobble_corrections: s.scrobble_corrections,
            check_updates: s.check_updates,
        })
    }

    /// Changes the player's settings; the song playing goes on from where it was.
    pub fn set_playback_settings(&self, value: PlaybackSettings) -> Result<()> {
        let mut s = self.library.settings()?;
        s.crossfade = value.crossfade.clamp(0., 12.);
        s.replay_gain = value.replay_gain;
        s.album_gain = value.album_gain;
        s.online_media = value.online_media;
        s.autoplay_query = value.autoplay_query.trim().to_string();
        s.sound_analysis = value.sound_analysis;
        s.scrobble_corrections = value.scrobble_corrections;
        s.check_updates = value.check_updates;
        self.player.send(Command::Configure(Box::new(s)));
        if value.sound_analysis {
            self.measure_in_background();
        }
        Ok(())
    }

    /// The equalizer's band frequencies, in Hz.
    pub fn eq_bands(&self) -> Vec<f32> {
        dsp::BANDS.iter().map(|f| *f as f32).collect()
    }

    pub fn eq_presets(&self) -> Vec<EqPreset> {
        dsp::PRESETS
            .iter()
            .map(|(name, preamp, bands)| EqPreset {
                name: name.to_string(),
                preamp: *preamp,
                bands: bands.to_vec(),
            })
            .collect()
    }

    pub fn sound_settings(&self) -> Result<SoundSettings> {
        let d = self.library.settings()?.dsp;
        Ok(SoundSettings {
            eq: d.eq,
            preamp: d.preamp_db,
            bands: d.bands.to_vec(),
            preset: d.preset.clone(),
            balance: d.balance,
            mono: d.mono,
            crossfeed: d.crossfeed,
            mode: if d.parametric_mode() { "parametric".into() } else { "graphic".into() },
            parametric: d.parametric.iter().map(band).collect(),
        })
    }

    /// Changes the equalizer and effects at once, as the song plays.
    pub fn set_sound_settings(&self, value: SoundSettings) -> Result<()> {
        let mut d = self.library.settings()?.dsp;
        d.eq = value.eq;
        d.preamp_db = value.preamp.clamp(-24., 12.);
        for (band, gain) in d.bands.iter_mut().zip(value.bands) {
            *band = gain.clamp(-12., 12.);
        }
        d.preset = value.preset;
        d.balance = value.balance.clamp(-1., 1.);
        d.mono = value.mono;
        d.crossfeed = value.crossfeed;
        d.mode = if value.mode == "parametric" { "parametric".into() } else { "graphic".into() };
        d.parametric = value
            .parametric
            .into_iter()
            .take(20)
            .map(|b| dsp::ParamBand {
                uid: uuid::Uuid::new_v4().to_string(),
                kind: b.kind,
                frequency: b.frequency.clamp(20., 20000.),
                gain: b.gain.clamp(-24., 24.),
                q: b.q.clamp(0.1, 10.),
                on: b.on,
            })
            .collect();
        self.player.send(Command::Dsp(d));
        Ok(())
    }

    /// The listener's own saved equalizer settings, by name.
    pub fn my_presets(&self) -> Vec<String> {
        self.library.settings().map(|s| s.eq_presets.into_iter().map(|p| p.name).collect()).unwrap_or_default()
    }

    /// Saves the equalizer as it is now under `name` (replacing one of the same name).
    pub fn save_preset(&self, name: String) -> Result<()> {
        let name = name.trim().to_string();
        if name.is_empty() {
            return Err(NeedleError::Failed("Give the preset a name".into()));
        }
        let mut s = self.library.settings()?;
        let preset = dsp::UserPreset::from_dsp(&name, &s.dsp);
        s.eq_presets.retain(|p| p.name != name);
        s.eq_presets.push(preset);
        s.dsp.preset = name;
        self.library.save_settings(&s)?;
        self.player.send(Command::Dsp(s.dsp));
        Ok(())
    }

    /// Uses a saved preset.
    pub fn use_preset(&self, name: String) -> Result<()> {
        let s = self.library.settings()?;
        let preset = s
            .eq_presets
            .iter()
            .find(|p| p.name == name)
            .ok_or_else(|| NeedleError::Failed("That preset is gone".into()))?;
        self.player.send(Command::Dsp(preset.apply(&s.dsp)));
        Ok(())
    }

    pub fn delete_preset(&self, name: String) -> Result<()> {
        let mut s = self.library.settings()?;
        s.eq_presets.retain(|p| p.name != name);
        Ok(self.library.save_settings(&s)?)
    }

    /// Reads a headphone correction (an AutoEq or Equalizer APO "ParametricEQ.txt") into the
    /// parametric equalizer, and turns it on. Returns how many bands it has.
    pub fn load_eq_file(&self, path: String) -> Result<u32> {
        let text = std::fs::read_to_string(&path).map_err(anyhow::Error::from)?;
        let (preamp, bands) = dsp::parse_parametric(&text)?;
        let mut d = self.library.settings()?.dsp;
        let count = bands.len() as u32;
        d.eq = true;
        d.mode = "parametric".into();
        d.preamp_db = preamp.clamp(-24., 12.);
        d.parametric = bands;
        d.preset = std::path::Path::new(&path)
            .file_stem()
            .map(|s| s.to_string_lossy().trim_start_matches("picked-").to_string())
            .unwrap_or_else(|| "Correction".into());
        self.player.send(Command::Dsp(d));
        Ok(count)
    }

    pub fn app_settings(&self) -> AppSettings {
        self.library
            .get_json::<AppSettings>(APP_KEY)
            .ok()
            .flatten()
            .unwrap_or_default()
    }

    pub fn set_app_settings(&self, value: AppSettings) -> Result<()> {
        Ok(self.library.set_json(APP_KEY, &value)?)
    }

    /// Stops reading a music folder; its songs leave the library.
    pub fn remove_folder(&self, path: String) -> Result<()> {
        let db = self.library.connection()?;
        db.execute("DELETE FROM roots WHERE path=?", [&path])
            .map_err(|e| failed(e.into()))?;
        let prefix = format!("{}/", path.trim_end_matches('/'));
        db.execute(
            "DELETE FROM tracks WHERE substr(path,1,length(?1))=?1",
            [&prefix],
        )
        .map_err(|e| failed(e.into()))?;
        Ok(())
    }

    pub fn accounts(&self) -> Accounts {
        let status = integrations::secret_status();
        let settings = self.library.settings().unwrap_or_default();
        let credentials = Credentials::load();
        let waiting = integrations::scrobble_status(&self.library)
            .map(|rows| rows.len() as u32)
            .unwrap_or(0);
        Accounts {
            lastfm_user: status
                .lastfm
                .configured
                .then(|| status.lastfm.user.clone().unwrap_or_default()),
            lastfm_on: settings.lastfm_enabled,
            lastfm_key_built_in: !credentials.lastfm_api_key.is_empty()
                && !credentials.lastfm_secret.is_empty(),
            listenbrainz_user: status
                .listenbrainz
                .configured
                .then(|| status.listenbrainz.user.clone().unwrap_or_default()),
            listenbrainz_on: settings.listenbrainz_enabled,
            waiting,
            problem: status
                .store_error
                .or(status.lastfm.rejected)
                .or(status.listenbrainz.rejected),
        }
    }

    /// Starts signing in to Last.fm: returns the page to open in the browser. Without a key
    /// built into this build, `api_key` and `secret` are the listener's own.
    pub fn lastfm_begin(&self, api_key: String, secret: String) -> Result<String> {
        let credentials = Credentials::load();
        let (key, secret) = if api_key.trim().is_empty() {
            (credentials.lastfm_api_key, credentials.lastfm_secret)
        } else {
            (api_key, secret)
        };
        let pending = integrations::lastfm_begin(&key, &secret)?;
        let url = pending.auth_url.clone();
        *PENDING.lock().unwrap() = Some(pending);
        Ok(url)
    }

    /// Finishes signing in to Last.fm once the listener allowed Needle in the browser.
    pub fn lastfm_complete(&self) -> Result<String> {
        let pending = PENDING
            .lock()
            .unwrap()
            .take()
            .ok_or_else(|| NeedleError::Failed("Start signing in to Last.fm first".into()))?;
        match integrations::lastfm_complete(&pending) {
            Ok(user) => {
                self.set_scrobbling(Some(true), None)?;
                Ok(user)
            }
            Err(error) => {
                // Not allowed yet: keep it, so the listener can try again after allowing.
                *PENDING.lock().unwrap() = Some(pending);
                Err(error.into())
            }
        }
    }

    pub fn listenbrainz_sign_in(&self, token: String) -> Result<String> {
        let user = integrations::listenbrainz_sign_in(&token)?;
        self.set_scrobbling(None, Some(true))?;
        Ok(user)
    }

    /// `service` is "lastfm" or "listenbrainz".
    pub fn sign_out(&self, service: String) -> Result<()> {
        match service.as_str() {
            "lastfm" => {
                integrations::lastfm_sign_out()?;
                self.set_scrobbling(Some(false), None)
            }
            _ => {
                integrations::listenbrainz_sign_out()?;
                self.set_scrobbling(None, Some(false))
            }
        }
    }

    /// Turns sending to a service on or off (`None` leaves it as it is).
    pub fn set_scrobbling(&self, lastfm: Option<bool>, listenbrainz: Option<bool>) -> Result<()> {
        let mut s = self.library.settings()?;
        if let Some(on) = lastfm {
            s.lastfm_enabled = on;
        }
        if let Some(on) = listenbrainz {
            s.listenbrainz_enabled = on;
        }
        self.player.send(Command::Configure(Box::new(s)));
        Ok(())
    }
}

fn band(b: &dsp::ParamBand) -> EqBand {
    EqBand {
        kind: b.kind.clone(),
        frequency: b.frequency,
        gain: b.gain,
        q: b.q,
        on: b.on,
    }
}
