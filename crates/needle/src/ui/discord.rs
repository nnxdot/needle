//! Keeps Discord's "Listening to" status in step with playback. Needle finds Discord by
//! itself; there is nothing to set up.
use super::AppView;
use needle_core::discord::{Activity, Presence};

/// The Discord application Needle appears as (discord.com/developers/applications).
pub const APP_ID: &str = "";

impl AppView {
    /// Presence runs when it is turned on and this build knows its Discord application.
    pub(super) fn discord_available() -> bool {
        !APP_ID.is_empty()
    }

    /// Called on every poll; tells Discord only when the song, pause state, or position jumps.
    pub(super) fn update_discord(&mut self) {
        if !self.settings.discord_presence || !Self::discord_available() {
            self.discord = None;
            self.discord_sent = None;
            return;
        }
        let presence = self
            .discord
            .get_or_insert_with(|| Presence::start(APP_ID.to_string()));
        let now = chrono::Utc::now().timestamp();
        let activity = self.playback.current.as_ref().map(|item| {
            let track = &item.track;
            let started = now - self.playback.position.round() as i64;
            Activity {
                title: track.title.clone(),
                artist: track.display_artist().to_string(),
                album: track.album.clone(),
                started,
                ends: (track.duration > 0.).then(|| started + track.duration.round() as i64),
                paused: !self.playback.playing,
            }
        });
        let key = activity
            .as_ref()
            .map(|a| (format!("{}\u{1}{}", a.title, a.artist), a.paused, a.started));
        let changed = match (&self.discord_sent, &key) {
            (None, None) => false,
            (Some((song, paused, started)), Some((new_song, new_paused, new_started))) => {
                song != new_song
                    || paused != new_paused
                    || (!new_paused && (started - new_started).abs() > 2)
            }
            _ => true,
        };
        if changed {
            presence.set(activity);
            self.discord_sent = key;
        }
    }
}
