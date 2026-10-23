//! Navidrome and other servers that speak the Subsonic API (Airsonic-Advanced, Gonic,
//! Ampache, LMS), built into Needle as a music source. The plugin host drives it like a
//! source plugin (see [`crate::plugins`]): songs are listed in the library and streamed
//! from the server, and plays and ratings are saved on the server too.
//!
//! It keeps what the Navidrome / Subsonic plugin kept, under the same names: its settings
//! (`plugin:subsonic` in the library), its password (the `subsonic` plugin's secret), and its
//! songs (`source://subsonic/<id>`). So people who used the plugin stay signed in, and their
//! songs keep their ratings, plays, and playlists.
use crate::database::Library;
use serde_json::{Map, Value, json};
use std::{collections::BTreeMap, collections::HashMap, time::Duration};

/// Its id as a source: the plugin's, so songs and settings carry over.
pub const ID: &str = "subsonic";
/// The name people see.
pub const NAME: &str = "Navidrome / Subsonic";
pub const DESCRIPTION: &str = "Plays the music on your Navidrome or other Subsonic server. Songs are listed with your own and streamed; plays and ratings are saved on the server.";
/// The switch for searching the server as you type.
pub const SEARCH_SERVER: &str = "search_server";

/// Songs a page when listing the whole server.
const PAGE: i64 = 250;
/// Albums a page, for servers that find nothing for an empty search (Ampache). Short pages,
/// so songs waiting to play are not held up for long.
const ALBUM_PAGE: i64 = 5;
/// The longest a request may take.
const TIMEOUT: Duration = Duration::from_secs(15);
/// The most a server's answer may hold, so a broken server cannot fill the memory.
const MAX_ANSWER: u64 = 64 << 20;
/// The API version Needle speaks: 1.13.0 brought salted passwords.
const API_VERSION: &str = "1.16.1";

/// Lyrics the server has for a song.
#[derive(Clone, Debug, PartialEq)]
pub enum Lyrics {
    /// LRC: `[mm:ss.cc]` before each line.
    Timed(String),
    Plain(String),
}

/// The server source. Cheap to make: everything it knows is kept in the library and the
/// system's password store.
pub struct Subsonic {
    library: Library,
}

impl Subsonic {
    pub fn new(library: Library) -> Self {
        Self { library }
    }

    /// The sign-in form and the switches, as a source plugin's `source()` gives them.
    pub fn about(&self) -> Value {
        json!({
            "name": NAME,
            "fields": [
                { "id": "server", "label": "Server address", "placeholder": "https://music.example.com" },
                { "id": "username", "label": "User name", "placeholder": "" },
                { "id": "password", "label": "Password", "placeholder": "", "secret": true },
            ],
            "switches": [{
                "id": SEARCH_SERVER,
                "label": "Search the server as you type (experimental)",
                "detail": "For octo-fiesta: songs your server can fetch show above your search results, and play from there. Off by default.",
            }],
        })
    }

    pub fn signed_in(&self) -> bool {
        !self.text("server").is_empty() && self.password().is_some()
    }

    /// Sign in with what was typed in the form: `server`, `username`, and `password`.
    pub fn sign_in(&self, fields: &BTreeMap<String, String>) -> Result<(), String> {
        let field = |key: &str| fields.get(key).map(String::as_str).unwrap_or_default();
        let typed = address(field("server"));
        if typed.is_empty() {
            return Err("Type your server's address".into());
        }
        // Without http:// or https://, only the secure address is tried: the sign-in part of
        // each request could be copied off a plain-http connection and used again.
        let lower = typed.to_ascii_lowercase();
        let plain = lower.starts_with("http://");
        let server = if plain || lower.starts_with("https://") {
            typed
        } else {
            format!("https://{typed}")
        };
        self.set("server", json!(server))?;
        self.set("username", json!(field("username").trim()))?;
        self.set("list_by", json!(""))?;
        crate::secrets::plugin::set(ID, "password", field("password"))
            .map_err(|e| format!("{e:#}"))?;
        if let Err(error) = self.ask("ping", &[]) {
            self.sign_out();
            if !plain && error.starts_with("Could not connect") {
                return Err(format!(
                    "{error} over https. If your server only uses plain http (Navidrome on :4533 often does), type http:// in front of the address. Plain http is not encrypted, so only use it at home or through a VPN."
                ));
            }
            return Err(error);
        }
        Ok(())
    }

    /// Forget the address and the password. The user name stays, to fill the form next time.
    pub fn sign_out(&self) {
        let _ = self.set("server", json!(""));
        let _ = crate::secrets::plugin::delete(ID, "password");
    }

    /// One page of the server's songs (from 0), until a page comes back empty. Each song is
    /// a map as a source plugin gives it (see [`crate::sources::Song::from_value`]).
    pub fn songs(&self, page: i64) -> Result<Vec<Value>, String> {
        let session = self.session()?;
        if self.text("list_by") != "albums" {
            let reply = session.ask(
                "search3",
                &[
                    ("query", String::new()),
                    ("songCount", PAGE.to_string()),
                    ("songOffset", (page * PAGE).to_string()),
                    ("artistCount", "0".into()),
                    ("albumCount", "0".into()),
                ],
            )?;
            let found = list(&reply["searchResult3"]["song"]);
            if !found.is_empty() {
                return Ok(to_songs(&found, &session));
            }
            if page > 0 {
                return Ok(vec![]);
            }
            // Nothing for an empty search: this server is read album by album from now on.
            self.set("list_by", json!("albums"))?;
        }
        let reply = session.ask(
            "getAlbumList2",
            &[
                ("type", "alphabeticalByName".into()),
                ("size", ALBUM_PAGE.to_string()),
                ("offset", (page * ALBUM_PAGE).to_string()),
            ],
        )?;
        let albums = list(&reply["albumList2"]["album"]);
        // The albums' songs, asked for side by side.
        let links: Vec<String> = albums
            .iter()
            .map(|album| session.link("getAlbum", &[("id", text(&album["id"]))]))
            .collect();
        let answers: Vec<Result<Value, String>> = std::thread::scope(|scope| {
            let asks: Vec<_> = links
                .into_iter()
                .map(|link| scope.spawn(move || get(&link)))
                .collect();
            asks.into_iter()
                .map(|a| {
                    a.join()
                        .unwrap_or_else(|_| Err("Reading an album failed".into()))
                })
                .collect()
        });
        let mut found = vec![];
        for answer in answers {
            found.extend(list(&answer?["album"]["song"]));
        }
        Ok(to_songs(&found, &session))
    }

    /// Songs on the server for a search, when the switch is on. Through octo-fiesta (a
    /// Subsonic proxy) these include songs from its streaming services, which it fetches when
    /// they are played.
    pub fn search(&self, query: &str) -> Result<Vec<Value>, String> {
        let words = query.trim();
        if words.is_empty() || !self.switch(SEARCH_SERVER) {
            return Ok(vec![]);
        }
        let session = self.session()?;
        let reply = session.ask(
            "search3",
            &[
                ("query", words.to_string()),
                ("songCount", "25".into()),
                ("artistCount", "0".into()),
                ("albumCount", "0".into()),
            ],
        )?;
        Ok(to_songs(&list(&reply["searchResult3"]["song"]), &session))
    }

    /// A link to the original file, not a smaller copy.
    pub fn stream(&self, id: &str) -> Result<String, String> {
        Ok(self.session()?.link(
            "stream",
            &[("id", id.to_string()), ("format", "raw".into())],
        ))
    }

    /// Tell the server a song started.
    pub fn playing(&self, id: &str) -> Result<(), String> {
        self.ask(
            "scrobble",
            &[("id", id.to_string()), ("submission", "false".into())],
        )
        .map(drop)
    }

    /// Count a play on the server. `started_at` is in Unix seconds.
    pub fn played(&self, id: &str, started_at: i64) -> Result<(), String> {
        self.ask(
            "scrobble",
            &[
                ("id", id.to_string()),
                ("time", (started_at * 1000).to_string()),
                ("submission", "true".into()),
            ],
        )
        .map(drop)
    }

    /// Save a rating (0 to 5 stars) on the server.
    pub fn rate(&self, id: &str, stars: i64) -> Result<(), String> {
        self.ask(
            "setRating",
            &[("id", id.to_string()), ("rating", stars.to_string())],
        )
        .map(drop)
    }

    /// The server's lyrics for one of its songs (Navidrome reads them from the files and from
    /// .lrc files beside them): timed ones when it has them. Only servers that speak
    /// OpenSubsonic have lyrics by song; from others, none.
    pub fn lyrics(&self, id: &str) -> Option<Lyrics> {
        let reply = self
            .ask("getLyricsBySongId", &[("id", id.to_string())])
            .ok()?;
        let all = list(&reply["lyricsList"]["structuredLyrics"]);
        // The song's own words first, timed or not: a server can also list a translation or a
        // romanization, which comes only when the song's own are missing.
        let (main, other): (Vec<&Value>, Vec<&Value>) = all
            .iter()
            .partition(|l| matches!(l["kind"].as_str(), None | Some("main")));
        pick_lyrics(&main).or_else(|| pick_lyrics(&other))
    }

    // The settings, shared with the plugin it replaces.

    fn settings(&self) -> HashMap<String, Value> {
        self.library
            .get_json(&format!("plugin:{ID}"))
            .ok()
            .flatten()
            .unwrap_or_default()
    }

    fn text(&self, key: &str) -> String {
        self.settings()
            .get(key)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    }

    fn switch(&self, key: &str) -> bool {
        self.settings().get(key) == Some(&Value::Bool(true))
    }

    fn set(&self, key: &str, value: Value) -> Result<(), String> {
        let mut all = self.settings();
        all.insert(key.into(), value);
        self.library
            .set_json(&format!("plugin:{ID}"), &all)
            .map_err(|e| format!("{e:#}"))
    }

    fn password(&self) -> Option<String> {
        crate::secrets::plugin::get(ID, "password").ok().flatten()
    }

    // Talking to the server.

    /// The server's address and the sign-in part of requests, read once for all the requests
    /// of one task (a page of songs and its cover links). The password itself is never sent:
    /// only a salted hash of it.
    fn session(&self) -> Result<Session, String> {
        use rand::Rng;
        let password = self
            .password()
            .ok_or_else(|| "Sign in to your server first".to_string())?;
        let settings = self.settings();
        let setting = |key: &str| {
            settings
                .get(key)
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string()
        };
        let salt: String = rand::thread_rng()
            .sample_iter(&rand::distributions::Alphanumeric)
            .take(12)
            .map(char::from)
            .collect();
        let token = format!("{:x}", md5::compute(format!("{password}{salt}")));
        Ok(Session {
            server: setting("server"),
            auth: format!(
                "u={}&t={token}&s={salt}&v={API_VERSION}&c=Needle&f=json",
                encode(&setting("username"))
            ),
        })
    }

    /// Ask the server, and return its `subsonic-response` when it says "ok".
    fn ask(&self, method: &str, params: &[(&str, String)]) -> Result<Value, String> {
        self.session()?.ask(method, params)
    }
}

/// What every request to the server needs (see `Subsonic::session`).
struct Session {
    server: String,
    auth: String,
}

impl Session {
    fn link(&self, method: &str, params: &[(&str, String)]) -> String {
        let mut url = format!("{}/rest/{method}.view?{}", self.server, self.auth);
        for (key, value) in params {
            url += &format!("&{key}={}", encode(value));
        }
        url
    }

    fn ask(&self, method: &str, params: &[(&str, String)]) -> Result<Value, String> {
        get(&self.link(method, params))
    }
}

/// The server's songs as Needle wants them. Their cover links share the session's sign-in, so
/// an album's songs share one link.
fn to_songs(found: &[Value], session: &Session) -> Vec<Value> {
    found
        .iter()
        .map(|song| {
            let cover = text(&song["coverArt"]);
            let album = text(&song["albumId"]);
            let mut map = Map::new();
            for (ours, theirs) in [
                ("id", "id"),
                ("title", "title"),
                ("artist", "artist"),
                ("album", "album"),
                ("album_artist", "displayAlbumArtist"),
                ("genre", "genre"),
                ("year", "year"),
                ("track", "track"),
                ("disc", "discNumber"),
                ("duration", "duration"),
                ("format", "suffix"),
                ("bitrate", "bitRate"),
                ("sample_rate", "samplingRate"),
                ("bit_depth", "bitDepth"),
                ("channels", "channelCount"),
                ("size", "size"),
                ("musicbrainz_id", "musicBrainzId"),
            ] {
                map.insert(ours.into(), song[theirs].clone());
            }
            // One cover per album: songs often carry their own copy of the same picture.
            map.insert(
                "cover_id".into(),
                json!(if album.is_empty() {
                    cover.clone()
                } else {
                    format!("album:{album}")
                }),
            );
            if !cover.is_empty() {
                map.insert(
                    "cover".into(),
                    json!(session.link("getCoverArt", &[("id", cover), ("size", "600".into())])),
                );
            }
            Value::Object(map)
        })
        .collect()
}

/// One client for all requests to the server: it keeps its connections (and its TLS setup)
/// for the next request, instead of making them again each time.
fn client() -> Result<&'static reqwest::blocking::Client, String> {
    static CLIENT: std::sync::OnceLock<reqwest::blocking::Client> = std::sync::OnceLock::new();
    if let Some(client) = CLIENT.get() {
        return Ok(client);
    }
    let made = crate::integrations::client().map_err(|e| e.to_string())?;
    Ok(CLIENT.get_or_init(|| made))
}

/// Ask the server at `url`, and return its `subsonic-response` when it says "ok".
fn get(url: &str) -> Result<Value, String> {
    use std::io::Read;
    let response = client()?
        .get(url)
        .timeout(TIMEOUT)
        .send()
        .map_err(|e| crate::sources::http_error(&e))?;
    let status = response.status();
    if status == reqwest::StatusCode::NOT_FOUND {
        // Something answered, but not a music server: often the port is missing.
        return Err("No music server at this address. Check it, and the port: Navidrome uses :4533 unless it was changed".into());
    }
    if !status.is_success() {
        return Err(format!("The server answered {status}"));
    }
    let mut body = vec![];
    response
        .take(MAX_ANSWER)
        .read_to_end(&mut body)
        .map_err(|_| "The connection to the server failed".to_string())?;
    let answer: Value = serde_json::from_slice(&body)
        .map_err(|_| "That address did not answer like a Subsonic server".to_string())?;
    let reply = answer
        .get("subsonic-response")
        .filter(|r| r.is_object())
        .ok_or_else(|| "That address did not answer like a Subsonic server".to_string())?;
    if reply["status"] != "ok" {
        return Err(reply["error"]["message"]
            .as_str()
            .filter(|m| !m.is_empty())
            .unwrap_or("The server refused the request")
            .to_string());
    }
    Ok(reply.clone())
}

/// "music.example.com/ " becomes "music.example.com".
fn address(typed: &str) -> String {
    typed.trim().trim_end_matches('/').to_string()
}

/// A value of the server's as text: ids can come as numbers.
fn text(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        _ => String::new(),
    }
}

/// A list in the server's answer; a missing one is empty.
fn list(value: &Value) -> Vec<Value> {
    value.as_array().cloned().unwrap_or_default()
}

/// Percent-encoding for a link: all but letters, digits, and `-_.~`.
fn encode(text: &str) -> String {
    let mut out = String::new();
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || b"-_.~".contains(&byte) {
            out.push(byte as char);
        } else {
            out += &format!("%{byte:02X}");
        }
    }
    out
}

/// Timed lyrics from the list when it has them, else its plain ones.
fn pick_lyrics(all: &[&Value]) -> Option<Lyrics> {
    let mut plain = String::new();
    for lyrics in all {
        let Some(lines) = lyrics["line"].as_array() else {
            continue;
        };
        if lyrics["synced"] == true {
            // OpenSubsonic: a positive offset shows the lyrics sooner.
            let offset = lyrics["offset"].as_f64().unwrap_or(0.);
            let mut timed = String::new();
            for line in lines {
                let ms = (line["start"].as_f64().unwrap_or(0.) - offset).max(0.) as i64;
                timed += &format!(
                    "[{:02}:{:02}.{:02}]{}\n",
                    ms / 60_000,
                    ms % 60_000 / 1000,
                    ms % 1000 / 10,
                    line["value"].as_str().unwrap_or_default()
                );
            }
            if !timed.is_empty() {
                return Some(Lyrics::Timed(timed));
            }
        } else if plain.is_empty() {
            for line in lines {
                if let Some(value) = line["value"].as_str() {
                    plain += value;
                    plain.push('\n');
                }
            }
        }
    }
    (!plain.is_empty()).then_some(Lyrics::Plain(plain))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn addresses_lose_spaces_and_end_slashes() {
        assert_eq!(address(" music.example.com// "), "music.example.com");
        assert_eq!(encode("a b/ü"), "a%20b%2F%C3%BC");
    }

    #[test]
    fn timed_lyrics_come_before_plain_ones_and_keep_their_offset() {
        let timed = json!({"synced": true, "offset": 500, "line": [
            {"start": 1000, "value": "Hey"}, {"start": 62_250.0, "value": "Hi"}, {"start": 100}
        ]});
        let plain = json!({"synced": false, "line": [{"value": "Hey"}, {"value": "Hi"}]});
        assert_eq!(
            pick_lyrics(&[&plain, &timed]),
            Some(Lyrics::Timed(
                "[00:00.50]Hey\n[01:01.75]Hi\n[00:00.00]\n".into()
            ))
        );
        assert_eq!(
            pick_lyrics(&[&plain]),
            Some(Lyrics::Plain("Hey\nHi\n".into()))
        );
        assert_eq!(pick_lyrics(&[&json!({"synced": true})]), None);
    }
}
