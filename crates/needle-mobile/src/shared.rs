//! What the phones and the Mac share beyond the phones' first set: speaker timing for a group,
//! the equalizer as a file, a blend of two artists, the log, and theme files. Moved here from
//! the Mac's part, with the same names, so both call the same core.
use crate::{Needle, NeedleError, Result};
use needle_core::{
    audio::{Command, QueueItem},
    cast::Group,
};

/// How many songs a blend queues, as on the desktop.
const STATION: usize = 50;

fn failed(text: impl Into<String>) -> NeedleError {
    NeedleError::Failed(text.into())
}

#[derive(Clone, uniffi::Record)]
pub struct GroupMember {
    /// The member's address (empty for this computer or phone), for
    /// [`Needle::set_speaker_delay`].
    pub key: String,
    pub name: String,
    pub delay: i32,
}

/// A theme file to share: what to save it as, and its text (and where it is kept now).
#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct ThemeExport {
    /// A file name for it: the theme's name, `.toml`.
    pub file_name: String,
    /// The file's text: what to save, or send to a friend (dropped on Needle, it is added).
    pub text: String,
    /// Where Needle keeps it.
    pub path: String,
}

#[uniffi::export]
impl Needle {
    // ---------- Speakers

    /// The members of the speaker group playing now, with their delays (ms).
    pub fn group_members(&self) -> Vec<GroupMember> {
        let Some(group) = self
            .library
            .settings()
            .ok()
            .and_then(|s| s.output_device)
            .as_deref()
            .and_then(Group::from_device_name)
        else {
            return vec![];
        };
        group
            .members()
            .into_iter()
            .map(|(key, name)| GroupMember {
                delay: *group.delays.get(&key).unwrap_or(&0),
                key,
                name,
            })
            .collect()
    }

    /// Moves one member of the group playing now earlier or later, in ms (-1000 to 1000),
    /// while it plays.
    pub fn set_speaker_delay(&self, key: String, ms: i32) -> Result<()> {
        let settings = self.library.settings()?;
        let Some(mut group) = settings
            .output_device
            .as_deref()
            .and_then(Group::from_device_name)
        else {
            return Ok(());
        };
        group.delays.insert(key, ms.clamp(-1000, 1000));
        group.delays.retain(|_, ms| *ms != 0);
        let device = group.device_name();
        self.library
            .update_settings(|s| s.output_device = Some(device))?;
        self.player
            .send(Command::SpeakerDelays(group.delays.clone()));
        Ok(())
    }

    // ---------- The equalizer as a file

    /// The equalizer as an Equalizer APO file's text ("ParametricEQ.txt", which AutoEq and
    /// other players read), for an app that saves files its own way.
    pub fn eq_file_text(&self) -> Result<String> {
        Ok(self.library.settings()?.dsp.eq_file())
    }

    /// Writes the equalizer as an Equalizer APO file ("ParametricEQ.txt") at `path`.
    pub fn save_eq_file(&self, path: String) -> Result<()> {
        std::fs::write(&path, self.library.settings()?.dsp.eq_file())
            .map_err(anyhow::Error::from)?;
        Ok(())
    }

    // ---------- A blend of two artists

    /// The artists that sound nearest to `artist`, for a blend.
    pub fn similar_artists(&self, artist: String) -> Vec<String> {
        self.library
            .similar_artists(&artist, 10)
            .unwrap_or_default()
    }

    /// Plays songs from both artists that flow from one to the other. Returns how many.
    pub fn start_blend(&self, first: String, second: String) -> Result<u32> {
        let station = self.library.blend(&first, &second, STATION)?;
        if station.len() < 2 {
            return Err(failed(
                "Not enough songs are measured yet to make a station. Needle measures them in the background; try again soon.",
            ));
        }
        let reason = format!("Blend: {first} × {second}");
        let count = station.len() as u32;
        self.player.send(Command::Play(
            station
                .into_iter()
                .map(|track| QueueItem {
                    track,
                    reason: reason.clone(),
                })
                .collect(),
        ));
        Ok(count)
    }

    // ---------- The log

    pub fn log_folder(&self) -> String {
        needle_core::logfile::log_folder(&self.library.directory)
            .to_string_lossy()
            .to_string()
    }

    /// The log file being written now (Settings › Your data › Log file).
    pub fn log_file(&self) -> String {
        needle_core::logfile::log_file(&self.library.directory)
            .to_string_lossy()
            .to_string()
    }

    /// An error report to paste in an email: the recent log and the last crash, without file
    /// paths or names ("Copy error report").
    pub fn error_report(&self) -> String {
        needle_core::logfile::error_report(&self.library.directory)
    }

    // ---------- Theme files

    /// A custom theme (`id` as in `themes()`: `custom:name`, or `custom:plugin/name` for a
    /// plugin's) as a file to share.
    pub fn export_theme(&self, id: String) -> Result<ThemeExport> {
        let name = id
            .strip_prefix("custom:")
            .ok_or_else(|| failed("Only your own and plugins' themes are files"))?;
        let (folder, stem) = match name.split_once('/') {
            Some((plugin, stem)) => (
                self.plugins
                    .plugins()
                    .into_iter()
                    .find(|p| p.enabled && p.manifest.id == plugin)
                    .ok_or_else(|| failed("That theme's plugin is off"))?
                    .folder
                    .join("themes"),
                stem,
            ),
            None => (self.library.directory.join("themes"), name),
        };
        if stem.is_empty() || stem.contains(['/', '\\']) || stem.starts_with('.') {
            return Err(failed("That is not a theme"));
        }
        let path = folder.join(format!("{stem}.toml"));
        if std::fs::metadata(&path).map_or(true, |m| m.len() > 64 * 1024) {
            return Err(failed("That theme's file is gone"));
        }
        let text = std::fs::read_to_string(&path).map_err(anyhow::Error::from)?;
        let title = toml::from_str::<toml::Value>(&text)
            .ok()
            .and_then(|v| v.get("name")?.as_str().map(str::to_string))
            .unwrap_or_else(|| stem.to_string());
        Ok(ThemeExport {
            file_name: format!("{}.toml", file_slug(&title)),
            text,
            path: path.to_string_lossy().to_string(),
        })
    }
}

/// A theme's name as a file name: letters and digits, the rest as single dashes.
fn file_slug(name: &str) -> String {
    let mut slug = String::new();
    for c in name.trim().chars() {
        if c.is_alphanumeric() {
            slug.extend(c.to_lowercase());
        } else if !slug.ends_with('-') {
            slug.push('-');
        }
    }
    let slug = slug.trim_matches('-');
    if slug.is_empty() {
        "theme".into()
    } else {
        slug.into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_theme_file_is_named_after_its_theme() {
        assert_eq!(file_slug("Midnight Rose!"), "midnight-rose");
        assert_eq!(file_slug("  "), "theme");
        assert_eq!(file_slug("Ünïcode  Night"), "ünïcode-night");
    }
}
