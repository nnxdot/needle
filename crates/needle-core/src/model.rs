use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Track {
    pub id: String,
    pub path: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub album_artist: String,
    pub genre: String,
    pub year: i64,
    pub track_number: i64,
    pub disc: i64,
    pub duration: f64,
    pub sample_rate: i64,
    pub bit_depth: i64,
    pub channels: i64,
    pub format: String,
    pub bitrate: i64,
    pub bpm: Option<f64>,
    pub rating: i64,
    pub added_at: i64,
    pub modified_at: i64,
    pub file_size: i64,
    pub content_hash: String,
    pub artwork: Option<String>,
    pub replay_gain: Option<f64>,
    pub replay_peak: Option<f64>,
    pub album_replay_gain: Option<f64>,
    pub album_peak: Option<f64>,
    pub musicbrainz_id: Option<String>,
    pub play_count: i64,
    pub last_played: Option<i64>,
    pub missing: bool,
    pub metadata_version: u32,
    /// For a track of a CUE sheet: the stretch of the album file it plays.
    pub cue: Option<CueSpan>,
}

/// Where a CUE sheet's track lives in its album file.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CueSpan {
    /// The album's audio file.
    pub audio: String,
    /// Seconds into it where the track starts, and ends (`None`: at the end of the file).
    pub start: f64,
    pub end: Option<f64>,
}

impl Track {
    /// The audio file to decode: the album file for a CUE track, else the track's own file.
    pub fn audio_path(&self) -> &str {
        self.cue.as_ref().map_or(&self.path, |c| &c.audio)
    }
    /// The file on disk this track comes from: the `.cue` sheet for a CUE track.
    pub fn file_path(&self) -> &str {
        match &self.cue {
            Some(_) => self
                .path
                .rsplit_once('#')
                .map_or(&self.path, |(file, _)| file),
            None => &self.path,
        }
    }
    pub fn display_artist(&self) -> &str {
        if self.artist.is_empty() {
            "Unknown artist"
        } else {
            &self.artist
        }
    }
    pub fn display_album(&self) -> &str {
        if self.album.is_empty() {
            "Unknown album"
        } else {
            &self.album
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Playlist {
    pub id: String,
    pub name: String,
    pub query: Option<String>,
    pub track_ids: Vec<String>,
    pub updated_at: i64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Listen {
    pub id: String,
    pub track_id: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub started_at: i64,
    pub listened_seconds: f64,
    pub duration: f64,
    pub qualified: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub volume: f32,
    pub output_device: Option<String>,
    pub exclusive: bool,
    pub replay_gain: bool,
    /// Prefer album gain over track gain when ReplayGain is on.
    pub album_gain: bool,
    pub autoplay_query: String,
    pub theme: String,
    pub show_inspector: bool,
    pub lastfm_enabled: bool,
    pub listenbrainz_enabled: bool,
    pub layout: Layout,
    /// Look up lyrics, artist photos, and missing album art online.
    pub online_media: bool,
    pub dsp: crate::dsp::Dsp,
    /// Turn off interface animations (Windows' own setting also turns them off).
    pub reduce_motion: bool,
    /// Tint the interface with the colours of the playing song's cover.
    pub music_colors: bool,
    /// What Windows draws behind the back layer: "mica", "acrylic", "clear", or "solid".
    pub window_material: String,
    /// How see-through the glass is, 0 (solid) to 1 (most see-through).
    pub glass_amount: f32,
    /// Let the glass show through the page too, not only the sidebar and bars.
    pub glass_page: bool,
    /// Film grain over the window, 0 (off) to 1.
    pub grain: f32,
    /// Show what is playing on Discord, whenever Discord is running.
    pub discord_presence: bool,
    /// Look up covers online (artist and song name only) so Discord can show them.
    pub discord_covers: bool,
    /// Discord card: "Listening to <title>", then three lines ("song", "artist", "album",
    /// "needle", or "none"), and whether to show the Needle logo.
    pub discord_title: String,
    pub discord_top: String,
    pub discord_middle: String,
    pub discord_bottom: String,
    pub discord_logo: bool,
    /// Font for page titles: "system", "bahnschrift", or "fraunces".
    pub display_font: String,
    /// Color for the interface when colors from the music are off: a hex color like
    /// "#3e63dd", or empty for Needle's amber.
    pub accent_color: String,
    /// The cover (or the chosen color) fills the whole background, with any look.
    pub ambient: bool,
    /// Seconds songs overlap as one ends and the next begins (0 = off).
    pub crossfade: f32,
    /// The song table's columns, in order, with their widths.
    pub columns: Vec<ColumnSetting>,
}

/// One column of the song table.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ColumnSetting {
    pub key: String,
    pub width: f32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            volume: 0.65,
            output_device: None,
            exclusive: false,
            replay_gain: false,
            album_gain: false,
            autoplay_query: String::new(),
            theme: "dark".into(),
            show_inspector: true,
            lastfm_enabled: false,
            listenbrainz_enabled: false,
            layout: Layout::default(),
            online_media: false,
            dsp: crate::dsp::Dsp::default(),
            reduce_motion: false,
            music_colors: true,
            window_material: "mica".into(),
            glass_amount: 0.6,
            glass_page: false,
            grain: 0.,
            discord_presence: true,
            discord_covers: true,
            discord_title: "song".into(),
            discord_top: "artist".into(),
            discord_middle: "song".into(),
            discord_bottom: "album".into(),
            discord_logo: true,
            display_font: "system".into(),
            accent_color: String::new(),
            ambient: false,
            crossfade: 0.,
            columns: [("album", 220.), ("quality", 92.), ("time", 52.)]
                .into_iter()
                .map(|(key, width)| ColumnSetting { key: key.into(), width })
                .collect(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Layout {
    pub version: u32,
    pub sidebar_width: f32,
    pub inspector_width: f32,
    pub row_height: f32,
}

impl Default for Layout {
    fn default() -> Self {
        Self {
            version: 1,
            sidebar_width: 212.0,
            inspector_width: 284.0,
            row_height: 52.0,
        }
    }
}
impl Layout {
    pub fn validate(&self) -> anyhow::Result<()> {
        if self.version != 1 {
            anyhow::bail!("Unsupported layout version");
        }
        for (name, value, min, max) in [
            ("sidebar_width", self.sidebar_width, 175., 260.),
            ("inspector_width", self.inspector_width, 240., 340.),
            ("row_height", self.row_height, 32., 76.),
        ] {
            if !value.is_finite() || value < min || value > max {
                anyhow::bail!("{name} must be between {min} and {max}");
            }
        }
        Ok(())
    }
}

pub fn format_duration(seconds: f64) -> String {
    let secs = seconds.max(0.0) as u64;
    if secs >= 3600 {
        format!("{}:{:02}:{:02}", secs / 3600, secs / 60 % 60, secs % 60)
    } else {
        format!("{}:{:02}", secs / 60, secs % 60)
    }
}
