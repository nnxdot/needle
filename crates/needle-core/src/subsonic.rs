//! Navidrome and other servers that speak the Subsonic API (Airsonic-Advanced, Gonic,
//! Ampache, LMS), built into Needle as a music source. The plugin host drives it like a
//! source plugin (see [`crate::plugins`]): songs are listed in the library and streamed
//! from the server, and plays and ratings are saved on the server too.
//!
//! It keeps what the Navidrome / Subsonic plugin kept, under the same names: its settings
//! (`plugin:subsonic` in the library), its password (the `subsonic` plugin's secret), and its
//! songs (`source://subsonic/<id>`). So people who used the plugin stay signed in, and their
//! songs keep their ratings, plays, and playlists.
use crate::{
    database::Library,
    server_playlists::{Listed, Remote, ServerError},
};
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

/// Albums asked for at a time when reading the albums' own cover ids.
const COVER_PAGE: i64 = 500;

/// The albums' own cover ids (album id to cover id), read at the first page of a sync. An
/// album's cover id changes when its picture changes on the server; a song's own cover id
/// does not, and the server keeps its small copy under it, so a new album picture would
/// never come in through the songs' ids.
static ALBUM_COVERS: std::sync::Mutex<Option<HashMap<String, String>>> =
    std::sync::Mutex::new(None);
/// The longest a request may take.
const TIMEOUT: Duration = Duration::from_secs(15);
/// The most a server's answer may hold, so a broken server cannot fill the memory.
const MAX_ANSWER: u64 = 64 << 20;
/// The most one playlist's answer may hold: less on 32-bit systems (TV boxes), whose memory runs
/// out sooner. A playlist of 10,000 songs is about 10 MB.
const MAX_PLAYLIST_ANSWER: u64 = if cfg!(target_pointer_width = "32") {
    24 << 20
} else {
    MAX_ANSWER
};
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
            if page == 0 {
                *ALBUM_COVERS.lock().unwrap_or_else(|e| e.into_inner()) =
                    Some(album_covers(&session));
            }
            let covers = ALBUM_COVERS
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clone()
                .unwrap_or_default();
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
                return Ok(to_songs(&found, &session, &covers));
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
        let covers: HashMap<String, String> = albums
            .iter()
            .map(|album| (text(&album["id"]), text(&album["coverArt"])))
            .filter(|(id, cover)| !id.is_empty() && !cover.is_empty())
            .collect();
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
        Ok(to_songs(&found, &session, &covers))
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
        // A search uses the albums' cover ids known from the last sync.
        let covers = ALBUM_COVERS
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
            .unwrap_or_default();
        Ok(to_songs(
            &list(&reply["searchResult3"]["song"]),
            &session,
            &covers,
        ))
    }

    /// A link to a song's cover as the server keeps it, not a smaller copy (a smaller copy of
    /// a moving cover is a still one).
    pub fn original_cover(&self, id: &str) -> Result<String, String> {
        Ok(self
            .session()?
            .link("getCoverArt", &[("id", id.to_string())]))
    }

    /// A link to a song's ALBUM's cover as the server keeps it, not a smaller copy. Asked by
    /// the album's id, a server gives the album's own picture (such as a moving cover.webp in
    /// its folder); asked by the song's id, Navidrome gives the picture inside the song's
    /// file first, often a still one.
    pub fn original_album_cover(&self, song_id: &str) -> Result<String, String> {
        let session = self.session()?;
        let reply = session.ask("getSong", &[("id", song_id.to_string())])?;
        let album = text(&reply["song"]["albumId"]);
        if album.is_empty() {
            return Err("The server gave no album for the song".into());
        }
        Ok(session.link("getCoverArt", &[("id", album)]))
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

/// The server's playlists (see [`crate::server_playlists`]).
impl crate::server_playlists::Server for Subsonic {
    fn playlists(&self) -> Result<Vec<Listed>, ServerError> {
        let session = self.session().map_err(ServerError::Unreachable)?;
        let reply = session.ask_within("getPlaylists", &[], MAX_ANSWER)?;
        let me = self.text("username");
        Ok(list(&reply["playlists"]["playlist"])
            .iter()
            .map(|p| Listed {
                id: text(&p["id"]),
                read_only: read_only(p, &me),
                changed: text(&p["changed"]),
                song_count: text(&p["songCount"]).parse().ok(),
            })
            .filter(|p| !p.id.is_empty())
            .collect())
    }

    fn playlist(&self, id: &str) -> Result<Remote, ServerError> {
        let session = self.session().map_err(ServerError::Unreachable)?;
        let reply = session.ask_within(
            "getPlaylist",
            &[("id", id.to_string())],
            MAX_PLAYLIST_ANSWER,
        )?;
        let playlist = &reply["playlist"];
        let entries: Vec<Value> = list(&playlist["entry"])
            .into_iter()
            .filter(|e| !text(&e["id"]).is_empty())
            .collect();
        // The albums' cover ids, read where they are kept, not copied for each playlist.
        let found = {
            let covers = ALBUM_COVERS.lock().unwrap_or_else(|e| e.into_inner());
            let none = HashMap::new();
            to_songs(&entries, &session, covers.as_ref().unwrap_or(&none))
        };
        let (mut songs, mut unread) = (vec![], vec![]);
        for (at, (entry, value)) in entries.iter().zip(&found).enumerate() {
            match crate::sources::Song::from_value(value) {
                Some(song) => songs.push(song),
                // No title: not a song Needle can show, but kept on the server at a push.
                None => unread.push((at, text(&entry["id"]))),
            }
        }
        Ok(Remote {
            id: id.to_string(),
            name: text(&playlist["name"]),
            comment: text(&playlist["comment"]),
            songs,
            unread,
            read_only: read_only(playlist, &self.text("username")),
            changed: text(&playlist["changed"]),
        })
    }

    fn save_playlist(
        &self,
        id: &str,
        name: Option<&str>,
        songs: &[String],
    ) -> Result<(), ServerError> {
        let session = self.session().map_err(ServerError::Unreachable)?;
        // createPlaylist with a playlist's id gives it exactly these songs. Sent as a form:
        // a long playlist does not fit in a link.
        let mut form = vec![("playlistId", id.to_string())];
        form.extend(songs.iter().map(|song| ("songId", song.clone())));
        session.post_within("createPlaylist", &form)?;
        if let Some(name) = name {
            session.ask_within(
                "updatePlaylist",
                &[("playlistId", id.to_string()), ("name", name.to_string())],
                MAX_ANSWER,
            )?;
        }
        Ok(())
    }

    fn delete_playlist(&self, id: &str) -> Result<(), ServerError> {
        self.session()
            .map_err(ServerError::Unreachable)?
            .ask_within("deletePlaylist", &[("id", id.to_string())], MAX_ANSWER)
            .map(drop)
    }

    fn account(&self) -> String {
        let server = self.text("server");
        if server.is_empty() {
            return String::new();
        }
        format!(
            "{}|{}",
            server.to_lowercase(),
            self.text("username").to_lowercase()
        )
    }
}

/// Whether the server lets the person signed in (`me`) only read a playlist: Navidrome marks
/// smart playlists `readonly`, and another person's public playlist has another `owner`.
fn read_only(playlist: &Value, me: &str) -> bool {
    let owner = text(&playlist["owner"]);
    playlist["readonly"].as_bool() == Some(true)
        || (!owner.is_empty() && !me.is_empty() && !owner.eq_ignore_ascii_case(me))
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

    /// [`Self::ask`], telling a refusal from no answer, for answers of at most `cap` bytes.
    fn ask_within(
        &self,
        method: &str,
        params: &[(&str, String)],
        cap: u64,
    ) -> Result<Value, ServerError> {
        let client = client().map_err(ServerError::Unreachable)?;
        answer_within(client.get(self.link(method, params)), cap)
    }

    /// Ask with the values in a form instead of the link.
    fn post_within(&self, method: &str, params: &[(&str, String)]) -> Result<Value, ServerError> {
        let body: Vec<String> = params
            .iter()
            .map(|(key, value)| format!("{key}={}", encode(value)))
            .collect();
        let request = client()
            .map_err(ServerError::Unreachable)?
            .post(self.link(method, &[]))
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(body.join("&"));
        answer_within(request, MAX_ANSWER)
    }
}

/// Every album's own cover id (see `ALBUM_COVERS`), a few requests for a whole library. A
/// server that cannot list them gives none, and songs keep their own ids.
fn album_covers(session: &Session) -> HashMap<String, String> {
    let mut covers = HashMap::new();
    for page in 0..200 {
        let Ok(reply) = session.ask(
            "getAlbumList2",
            &[
                ("type", "alphabeticalByName".into()),
                ("size", COVER_PAGE.to_string()),
                ("offset", (page * COVER_PAGE).to_string()),
            ],
        ) else {
            break;
        };
        let albums = list(&reply["albumList2"]["album"]);
        if albums.is_empty() {
            break;
        }
        for album in &albums {
            let (id, cover) = (text(&album["id"]), text(&album["coverArt"]));
            if !id.is_empty() && !cover.is_empty() {
                covers.insert(id, cover);
            }
        }
        if (albums.len() as i64) < COVER_PAGE {
            break;
        }
    }
    covers
}

/// The server's songs as Needle wants them. Their cover links share the session's sign-in, so
/// an album's songs share one link: the album's own cover id when it is known (it changes
/// with the album's picture), else the song's.
fn to_songs(
    found: &[Value],
    session: &Session,
    album_covers: &HashMap<String, String>,
) -> Vec<Value> {
    found
        .iter()
        .map(|song| {
            let album = text(&song["albumId"]);
            let cover = album_covers
                .get(&album)
                .cloned()
                .unwrap_or_else(|| text(&song["coverArt"]));
            let mut map = Map::new();
            // The album's own id on the server (its cover is asked for by it).
            map.insert("album_id".into(), json!(album));
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
    answer(client()?.get(url))
}

/// Send a request to the server, and return its `subsonic-response` when it says "ok".
fn answer(request: reqwest::blocking::RequestBuilder) -> Result<Value, String> {
    answer_within(request, MAX_ANSWER).map_err(|e| e.to_string())
}

/// [`answer`], reading at most `cap` bytes, and telling a real "no" from the server (a
/// Subsonic `failed` answer about what was asked) from no answer: no connection, a timeout, an
/// error page, an answer cut short or not from a Subsonic server, or a sign-in the server does
/// not take (that is about the password, not what was asked).
fn answer_within(
    request: reqwest::blocking::RequestBuilder,
    cap: u64,
) -> Result<Value, ServerError> {
    use ServerError::{Refused, Unreachable};
    use std::io::Read;
    let response = request
        .timeout(TIMEOUT)
        .send()
        .map_err(|e| Unreachable(crate::sources::http_error(&e)))?;
    let status = response.status();
    if status == reqwest::StatusCode::NOT_FOUND {
        // Something answered, but not a music server: often the port is missing.
        return Err(Unreachable("No music server at this address. Check it, and the port: Navidrome uses :4533 unless it was changed".into()));
    }
    if !status.is_success() {
        return Err(Unreachable(format!("The server answered {status}")));
    }
    let mut body = vec![];
    response
        .take(cap)
        .read_to_end(&mut body)
        .map_err(|_| Unreachable("The connection to the server failed".into()))?;
    let not_subsonic = || Unreachable("That address did not answer like a Subsonic server".into());
    let answer: Value = serde_json::from_slice(&body).map_err(|_| not_subsonic())?;
    let reply = answer
        .get("subsonic-response")
        .filter(|r| r.is_object())
        .ok_or_else(not_subsonic)?;
    if reply["status"] != "ok" {
        let message = reply["error"]["message"]
            .as_str()
            .filter(|m| !m.is_empty())
            .unwrap_or("The server refused the request")
            .to_string();
        // 20 and 30: versions that do not match; 40 to 44: the sign-in. Neither is about what
        // was asked: it may work once that is put right.
        return Err(match reply["error"]["code"].as_i64() {
            Some(20 | 30 | 40..=44) => Unreachable(message),
            _ => Refused(message),
        });
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

    /// Against a real, throwaway Navidrome (never a person's own): set NEEDLE_TEST_NAVIDROME
    /// to its address, and NEEDLE_TEST_NAVIDROME_USER and _PASSWORD, with two playlists made
    /// in its web page ("Web Road Trip" with 3 songs, "Web Gym" with 2), then run
    /// `cargo test -p needle-core navidrome_playlists_live -- --ignored`.
    #[test]
    #[ignore]
    fn navidrome_playlists_live() {
        use crate::server_playlists::{Server, playlist_id, sync};
        let env = |key: &str| std::env::var(key).unwrap_or_default();
        let dir = tempfile::tempdir().unwrap();
        let library = Library::open(dir.path()).unwrap();
        let server = Subsonic::new(library.clone());
        server
            .sign_in(&BTreeMap::from([
                ("server".to_string(), env("NEEDLE_TEST_NAVIDROME")),
                ("username".to_string(), env("NEEDLE_TEST_NAVIDROME_USER")),
                (
                    "password".to_string(),
                    env("NEEDLE_TEST_NAVIDROME_PASSWORD"),
                ),
            ]))
            .unwrap();
        let mut songs = vec![];
        for page in 0.. {
            let list = server.songs(page).unwrap();
            if list.is_empty() {
                break;
            }
            songs.extend(list.iter().filter_map(crate::sources::Song::from_value));
        }
        crate::sources::apply(&library, ID, &songs).unwrap();
        let ours = |name: &str| {
            library
                .playlists()
                .unwrap()
                .into_iter()
                .find(|p| p.name == name)
        };
        let titles = |p: &crate::model::Playlist| -> Vec<String> {
            p.track_ids
                .iter()
                .map(|id| library.track(id).unwrap().unwrap().title)
                .collect()
        };

        // Made in the web page: they come in, with their songs and comment.
        let report = sync(&library, ID, &server, false).unwrap();
        assert_eq!(report.added, 2, "{report:?}");
        let trip = ours("Web Road Trip").unwrap();
        assert_eq!(trip.track_ids.len(), 3);
        assert_eq!(trip.description, "made in the web UI");
        let gym = ours("Web Gym").unwrap();
        let (trip_id, gym_id) = (
            trip.server.clone().unwrap().id,
            gym.server.clone().unwrap().id,
        );
        assert_eq!(trip.id, playlist_id(ID, &trip_id));

        // Changed in Needle: renamed, one song out, the rest the other way round.
        let mut edited = trip.clone();
        edited.name = "Road Trip from Needle".into();
        edited.track_ids.remove(0);
        edited.track_ids.reverse();
        library.save_playlist(&edited).unwrap();
        let report = sync(&library, ID, &server, true).unwrap();
        assert_eq!(report.sent, 1, "{report:?}");
        let there = server.playlist(&trip_id).unwrap();
        assert_eq!(there.name, "Road Trip from Needle");
        assert_eq!(
            there
                .songs
                .iter()
                .map(|s| s.title.clone())
                .collect::<Vec<_>>(),
            titles(&edited)
        );

        // Changed on the server: renamed, a song added.
        let extra = songs
            .iter()
            .find(|s| !gym.track_ids.contains(&crate::sources::track_id(ID, &s.id)))
            .unwrap();
        server
            .ask(
                "updatePlaylist",
                &[
                    ("playlistId", gym_id.clone()),
                    ("name", "Gym, renamed on the server".into()),
                    ("songIdToAdd", extra.id.clone()),
                ],
            )
            .unwrap();
        let report = sync(&library, ID, &server, false).unwrap();
        assert_eq!(report.updated, 1, "{report:?}");
        let gym = ours("Gym, renamed on the server").unwrap();
        assert_eq!(gym.track_ids.len(), 3);
        assert_eq!(titles(&gym).last(), Some(&extra.title));

        // Deleted on the server: gone from Needle.
        server
            .ask("deletePlaylist", &[("id", gym_id.clone())])
            .unwrap();
        let report = sync(&library, ID, &server, false).unwrap();
        assert_eq!(report.removed, 1, "{report:?}");
        assert!(ours("Gym, renamed on the server").is_none());

        // Deleted in Needle: gone from the server.
        library.delete_playlist(&edited.id).unwrap();
        let report = sync(&library, ID, &server, true).unwrap();
        assert_eq!(report.sent, 1, "{report:?}");
        assert!(server.playlists().unwrap().is_empty());

        // A new one whose song is not in the library: the song comes with it.
        let missing = &songs[0];
        server
            .ask(
                "createPlaylist",
                &[("name", "Late".into()), ("songId", missing.id.clone())],
            )
            .unwrap();
        library
            .connection()
            .unwrap()
            .execute(
                "DELETE FROM tracks WHERE id=?",
                [crate::sources::track_id(ID, &missing.id)],
            )
            .unwrap();
        sync(&library, ID, &server, false).unwrap();
        let late = ours("Late").unwrap();
        assert_eq!(titles(&late), std::slice::from_ref(&missing.title));

        // Signing out takes it from Needle, not from the server.
        crate::server_playlists::forget(&library, ID).unwrap();
        assert!(library.playlists().unwrap().is_empty());
        assert_eq!(server.playlists().unwrap().len(), 1);
        server
            .ask(
                "deletePlaylist",
                &[("id", server.playlists().unwrap()[0].id.clone())],
            )
            .unwrap();
    }

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
