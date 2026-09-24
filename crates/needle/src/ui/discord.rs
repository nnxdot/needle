//! Keeps Discord's "Listening to" status in step with playback. Needle finds Discord by
//! itself; there is nothing to set up.
use super::AppView;
use needle_core::discord::{Activity, Field, Layout, Presence};

/// The Discord application Needle appears as (discord.com/developers/applications).
pub const APP_ID: &str = "1552097597454295110";

impl AppView {
    /// Presence runs when it is turned on and this build knows its Discord application.
    pub(super) fn discord_available() -> bool {
        !APP_ID.is_empty()
    }

    /// Tells Last.fm and ListenBrainz what is playing when a song starts or plays again after
    /// a pause, so they show it live. Played songs are still scrobbled when they finish.
    pub(super) fn update_now_playing(&mut self) {
        let enabled = self.settings.lastfm_enabled || self.settings.listenbrainz_enabled;
        let song = self
            .playback
            .current
            .as_ref()
            .filter(|_| enabled && self.playback.playing)
            .map(|item| &item.track);
        let key = song.map(|t| t.id.clone());
        if key == self.now_playing_sent {
            return;
        }
        self.now_playing_sent = key;
        let Some(track) = song else {
            return;
        };
        let song = needle_core::integrations::NowPlaying {
            artist: track.display_artist().to_string(),
            title: track.title.clone(),
            album: track.album.clone(),
            duration: track.duration,
        };
        let settings = self.settings.clone();
        std::thread::spawn(move || {
            let credentials = needle_core::integrations::Credentials::load();
            if let Err(error) =
                needle_core::integrations::send_now_playing(&settings, &credentials, &song)
            {
                needle_core::logfile::error(format!("Now playing: {error:#}"));
            }
        });
    }

    /// Called on every poll; tells Discord only when the song, pause state, or position jumps.
    pub(super) fn update_discord(&mut self) {
        if !self.settings.discord_presence || !Self::discord_available() {
            self.discord = None;
            self.discord_sent = None;
            return;
        }
        // A server song that is still connecting has no start time yet (its clock stands at
        // 0:00): wait until it plays, then tell Discord once, with the right time.
        if self.playback.loading {
            return;
        }
        let presence = self
            .discord
            .get_or_insert_with(|| Presence::start(APP_ID.to_string()));
        let now = chrono::Utc::now().timestamp();
        // Nothing playing for a while (paused or stopped): the status goes away, so it does
        // not sit on the profile. Playing again brings it back.
        if self.playback.playing {
            self.discord_idle_since = None;
        } else if self.discord_idle_since.is_none() {
            self.discord_idle_since = Some(now);
        }
        let idle = self.settings.discord_idle_minutes > 0
            && self
                .discord_idle_since
                .is_some_and(|since| now - since >= self.settings.discord_idle_minutes as i64 * 60);
        let hidden = idle || (!self.playback.playing && !self.settings.discord_paused);
        let activity = self
            .playback
            .current
            .as_ref()
            .filter(|_| !hidden)
            .map(|item| {
                let track = &item.track;
                let started = now - self.playback.position.round() as i64;
                Activity {
                    title: track.title.clone(),
                    artist: track.display_artist().to_string(),
                    album: track.album.clone(),
                    started,
                    ends: (track.duration > 0.).then(|| started + track.duration.round() as i64),
                    paused: !self.playback.playing,
                    find_cover: self.settings.discord_covers,
                    cover: None,
                    layout: Layout {
                        title: Field::from_name(&self.settings.discord_title),
                        top: Field::from_name(&self.settings.discord_top),
                        middle: Field::from_name(&self.settings.discord_middle),
                        bottom: Field::from_name(&self.settings.discord_bottom),
                        logo: self.settings.discord_logo,
                    },
                }
            });
        let key = activity
            .as_ref()
            .map(|a| (format!("{}\u{1}{}", a.title, a.artist), a.paused, a.started));
        let refresh = std::mem::take(&mut self.discord_refresh);
        let changed = refresh
            || match (&self.discord_sent, &key) {
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
