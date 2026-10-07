//! Connect: Needle on a computer, over the phone remote's address (its QR code in the
//! computer's Settings). The phone can control it, search it, play its songs here (streamed
//! over the home network), and move what plays from one to the other.
use crate::{Needle, NeedleError, Result};
use needle_core::{
    audio::{Command, QueueItem},
    model::Track,
    sources,
};
use serde_json::{Value, json};
use std::{sync::Mutex, time::Duration};

/// The plugin name streamed computer songs go by (`source://needle-pc/<id>`).
pub(crate) const PC: &str = "needle-pc";

/// The computer's remote address, such as `http://192.168.1.5:47380/r/<key>/`.
pub(crate) static ADDRESS: Mutex<Option<String>> = Mutex::new(None);

#[derive(Clone, uniffi::Record)]
pub struct PcSong {
    pub id: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration: f64,
    pub format: String,
    /// Where to load its cover from, when it has one.
    pub cover: Option<String>,
}

#[derive(Clone, uniffi::Record)]
pub struct PcState {
    pub playing: bool,
    pub position: f64,
    pub volume: f32,
    /// "off", "all", or "one".
    pub repeat: String,
    pub current: Option<PcSong>,
    pub up_next: Vec<PcSong>,
}

fn client() -> Result<reqwest::blocking::Client> {
    reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(8))
        .build()
        .map_err(|e| NeedleError::Failed(format!("{e}")))
}

fn address() -> Result<String> {
    ADDRESS
        .lock()
        .unwrap()
        .clone()
        .ok_or_else(|| NeedleError::Failed("Connect to Needle on your computer first".into()))
}

fn song(base: &str, v: &Value) -> PcSong {
    let id = v["id"].as_str().unwrap_or_default().to_string();
    PcSong {
        cover: (v["cover"].as_bool() != Some(false)).then(|| format!("{base}api/cover/{id}")),
        title: v["title"].as_str().unwrap_or_default().into(),
        artist: v["artist"].as_str().unwrap_or_default().into(),
        album: v["album"].as_str().unwrap_or_default().into(),
        duration: v["duration"].as_f64().unwrap_or_default(),
        format: v["format"].as_str().unwrap_or_default().into(),
        id,
    }
}

/// A computer song as a track this phone can play: streamed from the computer.
pub(crate) fn streamed(song: &PcSong) -> Track {
    Track {
        id: sources::track_id(PC, &song.id),
        path: sources::path_for(PC, &song.id),
        title: song.title.clone(),
        artist: song.artist.clone(),
        album: song.album.clone(),
        duration: song.duration,
        format: song.format.clone(),
        ..Default::default()
    }
}

/// The link to stream one of the computer's songs.
pub(crate) fn stream_link(id: &str) -> anyhow::Result<String> {
    let base = ADDRESS
        .lock()
        .unwrap()
        .clone()
        .ok_or_else(|| anyhow::anyhow!("Not connected to Needle on your computer"))?;
    Ok(format!("{base}api/audio/{id}"))
}

impl Needle {
    fn pc_post(&self, action: &str, body: Value) -> Result<()> {
        let base = address()?;
        let response = client()?
            .post(format!("{base}api/{action}"))
            .body(body.to_string())
            .send()
            .map_err(|e| NeedleError::Failed(format!("Your computer did not answer: {e}")))?;
        if response.status().is_success() {
            Ok(())
        } else {
            let text = response.text().unwrap_or_default();
            let message = serde_json::from_str::<Value>(&text)
                .ok()
                .and_then(|v| v["error"].as_str().map(str::to_string))
                .unwrap_or(text);
            Err(NeedleError::Failed(message))
        }
    }
}

#[uniffi::export]
impl Needle {
    /// Connects to Needle on a computer with its phone remote address (from the QR code in
    /// its Settings › Remote). Returns the song it plays, if any.
    pub fn connect_pc(&self, link: String) -> Result<Option<String>> {
        let mut base = link.trim().to_string();
        if !base.starts_with("http://") && !base.starts_with("https://") {
            base = format!("http://{base}");
        }
        if !base.ends_with('/') {
            base.push('/');
        }
        if !base.contains("/r/") {
            return Err(NeedleError::Failed(
                "That is not a Needle remote address. It looks like http://192.168.1.5:47380/r/…/"
                    .into(),
            ));
        }
        let state: Value = client()?
            .get(format!("{base}api/state"))
            .send()
            .and_then(|r| r.error_for_status())
            .and_then(|r| r.json())
            .map_err(|e| {
                NeedleError::Failed(format!(
                    "Needle on your computer did not answer ({e}). Check that the phone remote is on and both are on the same network."
                ))
            })?;
        *ADDRESS.lock().unwrap() = Some(base);
        self.library
            .set_json("android.pc", &ADDRESS.lock().unwrap().clone())
            .ok();
        Ok(state["current"]["title"].as_str().map(str::to_string))
    }

    /// Connects again to the computer used last time; `false` when there was none or it
    /// did not answer.
    pub fn reconnect_pc(&self) -> bool {
        let saved: Option<Option<String>> = self.library.get_json("android.pc").ok().flatten();
        match saved.flatten() {
            Some(link) => self.connect_pc(link).is_ok(),
            None => false,
        }
    }

    pub fn disconnect_pc(&self) {
        *ADDRESS.lock().unwrap() = None;
        let _ = self.library.set_json("android.pc", &None::<String>);
    }

    pub fn pc_connected(&self) -> bool {
        ADDRESS.lock().unwrap().is_some()
    }

    /// What Needle on the computer plays.
    pub fn pc_state(&self) -> Result<PcState> {
        let base = address()?;
        let v: Value = client()?
            .get(format!("{base}api/state"))
            .send()
            .and_then(|r| r.error_for_status())
            .and_then(|r| r.json())
            .map_err(|e| NeedleError::Failed(format!("Your computer did not answer: {e}")))?;
        Ok(PcState {
            playing: v["playing"].as_bool().unwrap_or(false),
            position: v["position"].as_f64().unwrap_or(0.),
            volume: v["volume"].as_f64().unwrap_or(1.) as f32,
            repeat: v["repeat"].as_str().unwrap_or("off").into(),
            current: v["current"].is_object().then(|| song(&base, &v["current"])),
            up_next: v["queue"]
                .as_array()
                .map(|q| q.iter().map(|s| song(&base, s)).collect())
                .unwrap_or_default(),
        })
    }

    /// "toggle", "next", "previous", or "shuffle" on the computer.
    pub fn pc_do(&self, action: String) -> Result<()> {
        self.pc_post(&action, json!({}))
    }

    pub fn pc_seek(&self, seconds: f64) -> Result<()> {
        self.pc_post("seek", json!({ "seconds": seconds }))
    }

    /// "off", "all", or "one".
    pub fn pc_repeat(&self, mode: String) -> Result<()> {
        self.pc_post("repeat", json!({ "mode": mode }))
    }

    /// Plays the song `index` places into what is up next on the computer (`id` makes sure
    /// the list has not changed meanwhile).
    pub fn pc_jump(&self, index: u32, id: String) -> Result<()> {
        self.pc_post("jump", json!({ "index": index, "id": id }))
    }

    /// Adds the computer's songs `ids` to the end of what is up next there.
    pub fn pc_enqueue(&self, ids: Vec<String>) -> Result<()> {
        self.pc_post("enqueue", json!({ "ids": ids }))
    }

    /// Puts the computer's songs `ids` next in line there.
    pub fn pc_play_next(&self, ids: Vec<String>) -> Result<()> {
        self.pc_post("next-up", json!({ "ids": ids }))
    }

    pub fn pc_volume(&self, value: f32) -> Result<()> {
        self.pc_post("volume", json!({ "value": value.clamp(0., 1.) }))
    }

    /// Plays the computer's songs `ids` on the computer.
    pub fn pc_play(&self, ids: Vec<String>) -> Result<()> {
        self.pc_post("play", json!({ "ids": ids }))
    }

    pub fn pc_search(&self, query: String) -> Result<Vec<PcSong>> {
        let base = address()?;
        let found: Vec<Value> = client()?
            .get(format!("{base}api/search"))
            .query(&[("q", query)])
            .send()
            .and_then(|r| r.error_for_status())
            .and_then(|r| r.json())
            .map_err(|e| NeedleError::Failed(format!("Your computer did not answer: {e}")))?;
        Ok(found.iter().map(|v| song(&base, v)).collect())
    }

    /// Plays the computer's songs here, streamed over the network, from `start`.
    pub fn play_from_pc(&self, songs: Vec<PcSong>, start: u32, at: f64) {
        let items: Vec<QueueItem> = songs
            .iter()
            .map(|s| QueueItem {
                track: streamed(s),
                reason: "From your computer".into(),
            })
            .collect();
        if items.is_empty() {
            return;
        }
        let start = (start as usize).min(items.len() - 1);
        self.player.send(Command::PlayAt(items, start));
        if at > 1. {
            self.player.send(Command::Seek(at));
        }
    }

    /// Moves what the computer plays to this phone: the same song and place, and what is up
    /// next; the computer pauses.
    pub fn move_from_pc(&self) -> Result<bool> {
        let state = self.pc_state()?;
        let Some(current) = state.current.clone() else {
            return Ok(false);
        };
        let mut songs = vec![current];
        songs.extend(state.up_next);
        if state.playing {
            self.pc_do("toggle".into())?;
        }
        self.play_from_pc(songs, 0, state.position);
        Ok(true)
    }

    /// Moves what this phone plays to the computer: the computer finds the same songs in its
    /// library (by title and artist), plays them from the same place, and the phone pauses.
    pub fn move_to_pc(&self) -> Result<u32> {
        let state = self.player.state();
        let Some(current) = state.current.clone() else {
            return Ok(0);
        };
        let mut wanted: Vec<Track> = vec![current.track.clone()];
        wanted.extend(state.queue.iter().take(40).map(|q| q.track.clone()));
        let mut ids = vec![];
        for (index, track) in wanted.iter().enumerate() {
            // A song streamed from the computer is already one of its own.
            if let Some((PC, id)) = sources::parse_path(&track.path) {
                ids.push(id);
                continue;
            }
            let found = self.pc_search(handoff_query(track))?;
            if let Some(same) = found.iter().find(|s| {
                s.title.eq_ignore_ascii_case(&track.title)
                    && s.artist.eq_ignore_ascii_case(track.display_artist())
            }) {
                ids.push(same.id.clone());
            } else if index == 0 {
                return Err(NeedleError::Failed(
                    "Your computer does not have this song".into(),
                ));
            }
        }
        if ids.is_empty() {
            return Err(NeedleError::Failed(
                "Your computer does not have this song".into(),
            ));
        }
        self.pc_play(ids.clone())?;
        if state.position > 1. {
            // The computer opens the song first; then it can move to the place.
            std::thread::sleep(Duration::from_millis(700));
            let _ = self.pc_seek(state.position);
        }
        if state.playing {
            self.player.send(Command::Toggle);
        }
        Ok(ids.len() as u32)
    }
}

fn handoff_query(track: &Track) -> String {
    format!(
        "title = {} and artist = {}",
        needle_core::query::quote(&track.title),
        needle_core::query::quote(track.display_artist())
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use needle_core::{audio::Player, database::Library, remote};

    /// The phone finds, controls, and streams from Needle on a computer through its remote.
    #[test]
    fn the_phone_connects_to_a_computer() {
        let computer = tempfile::tempdir().unwrap();
        let library = Library::open(computer.path()).unwrap();
        let song = computer.path().join("harbor.flac");
        std::fs::write(&song, b"fLaC the music itself").unwrap();
        library
            .upsert(&Track {
                id: "t1".into(),
                path: song.to_string_lossy().into(),
                title: "Harbor Lights".into(),
                artist: "Mara Quinn".into(),
                duration: 200.,
                format: "FLAC".into(),
                ..Default::default()
            })
            .unwrap();
        let player = Player::new(library.clone());
        let key = remote::new_key();
        let server = remote::start(player.clone(), library, key.clone()).unwrap();
        let link = format!("127.0.0.1:{}/r/{key}", server.port);

        let phone = tempfile::tempdir().unwrap();
        let needle = Needle::new(phone.path().to_string_lossy().into()).unwrap();
        assert!(
            needle
                .connect_pc("192.168.1.5:47380/nothing".into())
                .is_err()
        );
        needle.connect_pc(link).unwrap();
        assert!(needle.pc_connected());
        let state = needle.pc_state().unwrap();
        assert!(!state.playing);
        let found = needle.pc_search("harbor".into()).unwrap();
        assert_eq!(found[0].title, "Harbor Lights");
        assert_eq!(found[0].format, "FLAC");
        assert_eq!(streamed(&found[0]).format, "FLAC");
        let track = streamed(&found[0]);
        assert_eq!(needle.pc_search(handoff_query(&track)).unwrap()[0].id, "t1");
        // A computer song streams from its own address.
        let url = stream_link("t1").unwrap();
        let bytes = reqwest::blocking::get(url).unwrap().bytes().unwrap();
        assert_eq!(&bytes[..], b"fLaC the music itself");
        needle.pc_volume(0.4).unwrap();
        let start = std::time::Instant::now();
        while (player.state().volume - 0.4).abs() > 1e-6 && start.elapsed().as_secs() < 5 {
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!((player.state().volume - 0.4).abs() < 1e-6);
        needle.disconnect_pc();
        assert!(!needle.pc_connected());
        player.shutdown();
        needle.shutdown();
    }
}
