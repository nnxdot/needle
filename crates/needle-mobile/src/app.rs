//! The app itself: updates from needle.nnx.fyi, and music files opened from other apps.
use crate::{Needle, NeedleError, Result, Song};
use needle_core::{
    audio::{Command, QueueItem},
    update,
};
use std::sync::Mutex;

#[derive(Clone, uniffi::Record)]
pub struct Update {
    pub version: String,
    /// The download page, for when the app cannot install it by itself.
    pub page: String,
    /// The release has the Android app, with a checksum, so it can be installed from here.
    pub installable: bool,
}

static FOUND: Mutex<Option<update::Release>> = Mutex::new(None);

/// How far an update has got (see `needle_core::update::Progress`): `stage` is "" (none),
/// "list", "download", "check", or "install"; `done` of `total` bytes (`total` 0 when unknown).
#[derive(Clone, uniffi::Record)]
pub struct UpdateProgress {
    pub stage: String,
    pub done: u64,
    pub total: u64,
}

#[uniffi::export]
impl Needle {
    /// The app says whether its network is metered now (mobile data, a hotspot, Low Data Mode):
    /// background lookups then wait, when Fewer lookups on metered connections is on.
    pub fn set_metered(&self, on: bool) {
        needle_core::network::set_metered(on);
    }

    /// Whether the network is metered, as the app last said.
    pub fn metered(&self) -> bool {
        needle_core::network::metered()
    }

    /// How far the update running now has got, for a progress bar.
    pub fn update_progress(&self) -> UpdateProgress {
        let p = update::progress();
        UpdateProgress {
            stage: p.stage,
            done: p.done,
            total: p.total,
        }
    }

    /// Asks needle.nnx.fyi whether a newer Needle is out. Sends nothing but the request.
    pub fn check_update(&self) -> Result<Option<Update>> {
        let found = update::check(env!("CARGO_PKG_VERSION"))?;
        let info = found.as_ref().map(|r| Update {
            version: r.version.clone(),
            page: r.page.clone(),
            installable: r.package.is_some() && r.checksums_url.is_some(),
        });
        *FOUND.lock().unwrap() = found;
        Ok(info)
    }

    /// Downloads the update found by [`Needle::check_update`] into `folder` and checks it
    /// against the release's checksums. Returns the file for Android's installer.
    pub fn download_update(&self, folder: String) -> Result<String> {
        let release = FOUND
            .lock()
            .unwrap()
            .clone()
            .ok_or_else(|| NeedleError::Failed("Check for updates first".into()))?;
        let path = update::download(&release, std::path::Path::new(&folder))?;
        Ok(path.to_string_lossy().to_string())
    }

    /// Plays a music file opened from another app (a file manager, a download), adding it to
    /// the library first. Returns the song, or `None` when Needle cannot play it.
    pub fn open_file(&self, path: String) -> Result<Option<Song>> {
        let path = std::path::Path::new(&path);
        let _ = needle_core::scan::import_one(&self.library, path);
        let full = path.canonicalize().unwrap_or(path.to_path_buf());
        let Some(track) = self
            .library
            .track_by_path(&full.to_string_lossy())?
            .or(self.library.track_by_path(&path.to_string_lossy())?)
        else {
            return Ok(None);
        };
        let song = Song::from(&track);
        self.player.send(Command::Play(vec![QueueItem {
            track,
            reason: "Opened".into(),
        }]));
        Ok(Some(song))
    }

    /// Where Needle's Dolby decoder is: the app's own library folder, where Android lets it
    /// run (`libneedle_ffmpeg.so`).
    pub fn set_decoder(&self, path: String) {
        if std::path::Path::new(&path).is_file() {
            // SAFETY: set once at start, before any song opens and needs it.
            unsafe { std::env::set_var("NEEDLE_FFMPEG", path) };
        }
    }

    /// Needle's version.
    pub fn version(&self) -> String {
        env!("CARGO_PKG_VERSION").into()
    }
}
