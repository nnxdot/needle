//! Needle for AI assistants: a Model Context Protocol server (MCP, "Streamable HTTP"), so an
//! assistant such as Claude can see what plays, control it, search the library, play albums,
//! artists, genres, and playlists, make playlists, and read the lyrics.
//!
//! It is off until turned on (Settings), answers only this computer (127.0.0.1), and only with
//! the long random key in its address (`/mcp/<key>`) or as `Authorization: Bearer <key>`.
//! Requests that a web page sends (an `Origin` other than this computer's) are refused, so a
//! site open in a browser cannot reach it.
use crate::{
    audio::{Command, Player, QueueItem, Repeat},
    database::Library,
    model::{Playlist, Track},
    remote::{read_request, respond},
};
use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::{
    net::TcpListener,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::Duration,
};

/// The port Needle tries first, so an assistant's settings keep working.
pub const PORT: u16 = 47381;
/// The newest protocol version Needle speaks; older clients get their own back.
const PROTOCOL: &str = "2025-06-18";
const KNOWN: [&str; 3] = ["2025-06-18", "2025-03-26", "2024-11-05"];
const MAX_CONNECTIONS: usize = 16;
/// The most songs a tool plays or lists at once.
const MOST: usize = 500;

/// Called after a tool changed the library (a playlist made, a song favorited), so the app
/// shows it.
pub type Changed = Arc<dyn Fn() + Send + Sync>;

/// The running server; it stops when dropped.
pub struct Server {
    pub port: u16,
    key: String,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Server {
    /// The address to give an assistant (its key inside).
    pub fn address(&self) -> String {
        format!("http://127.0.0.1:{}/mcp/{}", self.port, self.key)
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Start answering on this computer only.
pub fn start(player: Player, library: Library, key: String, changed: Changed) -> Result<Server> {
    anyhow::ensure!(key.len() >= 16, "The key is too short");
    let listener = TcpListener::bind(("127.0.0.1", PORT))
        .or_else(|_| TcpListener::bind(("127.0.0.1", 0)))
        .context("Could not start Needle for AI assistants")?;
    listener.set_nonblocking(true)?;
    let port = listener.local_addr()?.port();
    let stop = Arc::new(AtomicBool::new(false));
    let flag = stop.clone();
    let open = Arc::new(AtomicUsize::new(0));
    let secret = key.clone();
    let thread = std::thread::Builder::new()
        .name("needle-mcp".into())
        .spawn(move || {
            crate::logfile::info(format!("Needle for AI assistants on port {port}"));
            while !flag.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((stream, peer)) => {
                        let _ = stream.set_nonblocking(false);
                        if !peer.ip().is_loopback()
                            || open.load(Ordering::Relaxed) >= MAX_CONNECTIONS
                        {
                            continue;
                        }
                        let (player, library, key, open, changed) = (
                            player.clone(),
                            library.clone(),
                            secret.clone(),
                            open.clone(),
                            changed.clone(),
                        );
                        open.fetch_add(1, Ordering::Relaxed);
                        std::thread::spawn(move || {
                            let _ = serve(&stream, &player, &library, &key, &changed);
                            open.fetch_sub(1, Ordering::Relaxed);
                        });
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(50));
                    }
                    Err(_) => std::thread::sleep(Duration::from_millis(200)),
                }
            }
        })?;
    Ok(Server {
        port,
        key,
        stop,
        thread: Some(thread),
    })
}

/// A page's origin that may talk to Needle: none (an assistant is not a browser), or this
/// computer's own.
fn origin_allowed(origin: Option<&str>) -> bool {
    match origin {
        None => true,
        Some(o) => {
            let host = o
                .trim_start_matches("http://")
                .trim_start_matches("https://")
                .split([':', '/'])
                .next()
                .unwrap_or("");
            matches!(host, "localhost" | "127.0.0.1" | "[::1]")
        }
    }
}

fn serve(
    stream: &std::net::TcpStream,
    player: &Player,
    library: &Library,
    key: &str,
    changed: &Changed,
) -> Result<()> {
    let request = read_request(stream)?;
    let keyed = request.path == format!("/mcp/{key}")
        || (request.path == "/mcp"
            && request.authorization.as_deref() == Some(format!("Bearer {key}").as_str()));
    if !keyed {
        // A wrong key looks the same as a page that does not exist.
        return respond(stream, "404 Not Found", "text/plain", b"Not found");
    }
    if !origin_allowed(request.origin.as_deref()) {
        return respond(
            stream,
            "403 Forbidden",
            "text/plain",
            b"Not from a web page",
        );
    }
    if request.method != "POST" {
        // No stream of messages from Needle: answers come back to each request.
        return respond(
            stream,
            "405 Method Not Allowed",
            "text/plain",
            b"POST JSON-RPC messages here",
        );
    }
    let message: Value = match serde_json::from_slice(&request.body) {
        Ok(v) => v,
        Err(_) => {
            let error = json!({"jsonrpc": "2.0", "id": null,
                "error": {"code": -32700, "message": "Not JSON"}});
            return respond(
                stream,
                "400 Bad Request",
                "application/json",
                &serde_json::to_vec(&error)?,
            );
        }
    };
    let tools = Tools {
        player,
        library,
        changed,
    };
    let answer = match message {
        Value::Array(batch) => {
            let answers: Vec<Value> = batch.iter().filter_map(|m| tools.answer(m)).collect();
            (!answers.is_empty()).then_some(Value::Array(answers))
        }
        one => tools.answer(&one),
    };
    match answer {
        Some(a) => respond(
            stream,
            "200 OK",
            "application/json",
            &serde_json::to_vec(&a)?,
        ),
        // Only notifications: nothing to answer.
        None => respond(stream, "202 Accepted", "application/json", b""),
    }
}

struct Tools<'a> {
    player: &'a Player,
    library: &'a Library,
    changed: &'a Changed,
}

impl Tools<'_> {
    /// One JSON-RPC message's answer; none for a notification.
    fn answer(&self, message: &Value) -> Option<Value> {
        let id = message.get("id").cloned()?;
        let method = message["method"].as_str().unwrap_or("");
        let params = &message["params"];
        let result = match method {
            "initialize" => {
                let asked = params["protocolVersion"].as_str().unwrap_or(PROTOCOL);
                Ok(json!({
                    "protocolVersion": if KNOWN.contains(&asked) { asked } else { PROTOCOL },
                    "capabilities": {"tools": {"listChanged": false}},
                    "serverInfo": {"name": "Needle", "version": env!("CARGO_PKG_VERSION")},
                    "instructions": INSTRUCTIONS,
                }))
            }
            "ping" => Ok(json!({})),
            "tools/list" => Ok(json!({ "tools": tool_list() })),
            "tools/call" => {
                let name = params["name"].as_str().unwrap_or("");
                let args = params.get("arguments").cloned().unwrap_or(json!({}));
                Ok(match self.call(name, &args) {
                    Ok(value) => json!({
                        "content": [{"type": "text", "text": serde_json::to_string_pretty(&value).unwrap_or_default()}],
                        "isError": false,
                    }),
                    Err(e) => json!({
                        "content": [{"type": "text", "text": format!("{e:#}")}],
                        "isError": true,
                    }),
                })
            }
            _ => Err(json!({"code": -32601, "message": format!("Unknown method {method}")})),
        };
        Some(match result {
            Ok(result) => json!({"jsonrpc": "2.0", "id": id, "result": result}),
            Err(error) => json!({"jsonrpc": "2.0", "id": id, "error": error}),
        })
    }

    fn call(&self, name: &str, args: &Value) -> Result<Value> {
        let text = |key: &str| args[key].as_str().map(str::trim).filter(|s| !s.is_empty());
        match name {
            "now_playing" => Ok(crate::remote::state(self.player)),
            "control" => {
                let state = self.player.state();
                let command = match text("action").unwrap_or("") {
                    "play" if !state.playing => Command::Toggle,
                    "pause" if state.playing => Command::Toggle,
                    "play" | "pause" => return Ok(json!({"done": "It already was"})),
                    "toggle" => Command::Toggle,
                    "next" => Command::Next,
                    "previous" => Command::Previous,
                    "shuffle" => Command::Shuffle,
                    "seek" => {
                        let seconds = args["seconds"].as_f64().context("seek needs seconds")?;
                        anyhow::ensure!(seconds.is_finite() && seconds >= 0., "seconds must be 0 or more");
                        Command::Seek(seconds)
                    }
                    "volume" => {
                        let value = args["volume"].as_f64().context("volume needs volume (0 to 1)")?;
                        anyhow::ensure!((0.0..=1.0).contains(&value), "volume is 0 to 1");
                        Command::Volume(value as f32)
                    }
                    "repeat" => Command::Repeat(match text("repeat").unwrap_or("") {
                        "off" => Repeat::Off,
                        "all" => Repeat::All,
                        "one" => Repeat::One,
                        _ => bail!("repeat is off, all, or one"),
                    }),
                    other => bail!("Unknown action \"{other}\""),
                };
                self.player.send(command);
                Ok(json!({"done": true}))
            }
            "search" => {
                let query = text("query").context("search needs a query")?;
                let limit = args["limit"].as_u64().unwrap_or(10).clamp(1, 50) as usize;
                let found = self.library.find(query, limit)?;
                Ok(json!({
                    "best": found.best.map(|b| match b {
                        crate::find::Best::Song(t) => json!({"song": song(&t)}),
                        crate::find::Best::Album(a) => json!({"album": album(&a)}),
                        crate::find::Best::Artist(a) => json!({"artist": a.name}),
                    }),
                    "songs": found.songs.iter().map(song).collect::<Vec<_>>(),
                    "albums": found.albums.iter().map(album).collect::<Vec<_>>(),
                    "artists": found.artists.iter().map(|a| json!({"name": a.name, "albums": a.albums, "songs": a.tracks})).collect::<Vec<_>>(),
                    "lyrics": found.lyrics.iter().map(|m| json!({"song": song(&m.track), "line": m.line})).collect::<Vec<_>>(),
                }))
            }
            "play" => {
                let tracks = self.chosen(args)?;
                anyhow::ensure!(!tracks.is_empty(), "Nothing in the library matches that");
                let count = tracks.len();
                let reason = "Asked by an assistant".to_string();
                let items: Vec<QueueItem> = tracks
                    .into_iter()
                    .map(|track| QueueItem {
                        track,
                        reason: reason.clone(),
                    })
                    .collect();
                let when = text("when").unwrap_or("now");
                self.player.send(match when {
                    "now" => Command::Play(items),
                    "next" => Command::PlayNext(items),
                    "end" => Command::Enqueue(items),
                    _ => bail!("when is now, next, or end"),
                });
                Ok(json!({"songs": count, "when": when}))
            }
            "favorite" => {
                let id = match text("song_id") {
                    Some(id) => id.to_string(),
                    None => self
                        .player
                        .state()
                        .current
                        .map(|c| c.track.id)
                        .context("Nothing plays: give a song_id")?,
                };
                let on = args["on"].as_bool().unwrap_or(true);
                let rating = if on { 5 } else { 0 };
                let track = self.library.track(&id)?.context("No song with that id")?;
                self.library.rate(&id, rating)?;
                self.player.send(Command::Rated(id, rating));
                (self.changed)();
                Ok(json!({"song": song(&track), "favorite": on}))
            }
            "playlists" => Ok(json!(
                self.library
                    .playlists()?
                    .iter()
                    .map(|p| json!({"name": p.name, "rule": p.query, "songs": p.query.is_none().then_some(p.track_ids.len()), "about": p.description}))
                    .collect::<Vec<_>>()
            )),
            "make_playlist" => {
                let name = text("name").context("make_playlist needs a name")?;
                let rule = text("rule").map(String::from);
                let ids: Vec<String> = args["song_ids"]
                    .as_array()
                    .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).take(MOST).collect())
                    .unwrap_or_default();
                anyhow::ensure!(rule.is_some() || !ids.is_empty(), "Give a rule or song_ids");
                let playlist = Playlist {
                    id: uuid::Uuid::new_v4().to_string(),
                    name: name.to_string(),
                    query: rule,
                    track_ids: ids,
                    updated_at: chrono::Utc::now().timestamp(),
                    description: text("about").unwrap_or("").to_string(),
                    ..Default::default()
                };
                self.library.save_playlist(&playlist)?;
                let songs = self.library.playlist_tracks(&playlist)?.len();
                (self.changed)();
                Ok(json!({"made": playlist.name, "songs": songs}))
            }
            "library_numbers" => {
                let n = self.library.numbers()?;
                Ok(json!({
                    "songs": n.songs, "albums": n.albums, "artists": n.artists,
                    "hours": (n.duration / 3600.).round(), "server_songs": n.server_songs,
                    "lossless": n.lossless, "hires": n.hires, "formats": n.formats,
                    "decades": n.decades, "top_genres": n.genres, "top_artists": n.artists_by_songs,
                }))
            }
            "lyrics" => {
                let track = match text("song_id") {
                    Some(id) => self.library.track(id)?.context("No song with that id")?,
                    None => self
                        .player
                        .state()
                        .current
                        .map(|c| c.track)
                        .context("Nothing plays: give a song_id")?,
                };
                let online = self.library.settings().map(|s| s.online_media).unwrap_or(false);
                let lyrics = crate::media::lyrics(self.library, &track, online, None)?;
                Ok(match lyrics {
                    None => json!({"song": song(&track), "lyrics": null}),
                    Some(l) if l.instrumental => json!({"song": song(&track), "instrumental": true}),
                    Some(l) => {
                        let text = if l.lines.is_empty() {
                            l.plain.clone()
                        } else {
                            l.lines.iter().map(|x| x.text.as_str()).collect::<Vec<_>>().join("\n")
                        };
                        json!({"song": song(&track), "lyrics": text})
                    }
                })
            }
            _ => bail!("Unknown tool \"{name}\""),
        }
    }

    /// The songs `play` was asked for: by id, a rule, an album, an artist, a genre, or a
    /// playlist; shuffled when asked.
    fn chosen(&self, args: &Value) -> Result<Vec<Track>> {
        let text = |key: &str| args[key].as_str().map(str::trim).filter(|s| !s.is_empty());
        let quoted = |s: &str| format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""));
        let mut tracks = if let Some(ids) = args["song_ids"].as_array() {
            let ids: Vec<String> = ids
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .take(MOST)
                .collect();
            self.library.tracks_by_ids(&ids)?
        } else if let Some(rule) = text("rule") {
            self.library.search(rule)?
        } else if let Some(name) = text("album") {
            let found = self.library.find(name, 10)?;
            let wanted = text("artist").map(str::to_lowercase);
            let album = found
                .albums
                .iter()
                .find(|a| {
                    wanted
                        .as_ref()
                        .is_none_or(|w| a.artist.to_lowercase().contains(w.as_str()))
                })
                .context("No album by that name")?;
            self.library.search(&album.query)?
        } else if let Some(artist) = text("artist") {
            self.library
                .search(&format!("artist = {}", quoted(artist)))?
        } else if let Some(genre) = text("genre") {
            self.library.search(&format!("genre = {}", quoted(genre)))?
        } else if let Some(name) = text("playlist") {
            let playlist = self
                .library
                .playlists()?
                .into_iter()
                .find(|p| p.name.eq_ignore_ascii_case(name))
                .context("No playlist by that name")?;
            self.library.playlist_tracks(&playlist)?
        } else {
            bail!("Say what to play: song_ids, rule, album, artist, genre, or playlist")
        };
        tracks.retain(|t| !t.missing);
        if args["shuffle"].as_bool().unwrap_or(false) {
            use rand::seq::SliceRandom;
            tracks.shuffle(&mut rand::thread_rng());
        }
        tracks.truncate(MOST);
        Ok(tracks)
    }
}

fn song(t: &Track) -> Value {
    json!({"id": t.id, "title": t.title, "artist": t.display_artist(), "album": t.album,
           "year": t.year, "seconds": t.duration.round(), "favorite": t.rating >= 4, "plays": t.play_count})
}

fn album(a: &crate::browse::AlbumSummary) -> Value {
    json!({"album": a.album, "artist": a.artist, "year": a.year, "songs": a.tracks})
}

const INSTRUCTIONS: &str = "Needle is the music player on this computer. Use search to find songs, albums, artists, and lines of lyrics in the listener's own library; play to play them (now, next, or at the end); control for play, pause, skipping, seeking, volume, and repeat; now_playing for what plays and what is up next. Rules (for play and make_playlist) look like: rating >= 4, played(30d), play_count = 0, bit_depth >= 24, year < 2000, genre = \"Jazz\", artist = \"Name\"; combine with and/or; add 'shuffle limit 50'.";

fn tool_list() -> Value {
    let examples: Vec<String> = crate::find::RULE_EXAMPLES
        .iter()
        .map(|(name, rule)| format!("{name}: {rule}"))
        .collect();
    let rule_help = format!(
        "A Needle rule, for example {}. Fields include title, artist, album, genre, year, rating, play_count, bit_depth, sample_rate, format, bpm; combine with and/or; end with 'shuffle limit 50' to pick at random.",
        examples.join("; ")
    );
    json!([
        {
            "name": "now_playing",
            "title": "What plays",
            "description": "The song playing now, whether it plays, where it is, the volume, the repeat mode, and the next 50 songs up next.",
            "inputSchema": {"type": "object", "properties": {}},
            "annotations": {"readOnlyHint": true},
        },
        {
            "name": "control",
            "title": "Control the player",
            "description": "Play, pause, toggle, next, previous, shuffle what is up next, seek to a second, set the volume (0 to 1), or set repeat (off, all, one).",
            "inputSchema": {"type": "object", "properties": {
                "action": {"type": "string", "enum": ["play", "pause", "toggle", "next", "previous", "shuffle", "seek", "volume", "repeat"]},
                "seconds": {"type": "number", "description": "For seek."},
                "volume": {"type": "number", "minimum": 0, "maximum": 1, "description": "For volume."},
                "repeat": {"type": "string", "enum": ["off", "all", "one"], "description": "For repeat."},
            }, "required": ["action"]},
        },
        {
            "name": "search",
            "title": "Search the library",
            "description": "Find songs, albums, and artists in the listener's library by name, and songs whose lyrics hold the words. The best match comes first.",
            "inputSchema": {"type": "object", "properties": {
                "query": {"type": "string"},
                "limit": {"type": "integer", "minimum": 1, "maximum": 50, "description": "How many of each (10 if left out)."},
            }, "required": ["query"]},
            "annotations": {"readOnlyHint": true},
        },
        {
            "name": "play",
            "title": "Play music",
            "description": "Play songs now, next, or at the end of what is up next. Choose them by song_ids (from search), a rule, an album (with its artist to tell albums apart), an artist, a genre, or a playlist's name.",
            "inputSchema": {"type": "object", "properties": {
                "song_ids": {"type": "array", "items": {"type": "string"}},
                "rule": {"type": "string", "description": rule_help},
                "album": {"type": "string"},
                "artist": {"type": "string"},
                "genre": {"type": "string"},
                "playlist": {"type": "string"},
                "shuffle": {"type": "boolean"},
                "when": {"type": "string", "enum": ["now", "next", "end"], "description": "now (the default) replaces what plays."},
            }},
        },
        {
            "name": "favorite",
            "title": "Favorite a song",
            "description": "Make a song a favorite (or not, with on: false). Without song_id, the song playing now.",
            "inputSchema": {"type": "object", "properties": {
                "song_id": {"type": "string"},
                "on": {"type": "boolean"},
            }},
        },
        {
            "name": "playlists",
            "title": "List playlists",
            "description": "The listener's playlists: smart ones with their rule, the others with their number of songs.",
            "inputSchema": {"type": "object", "properties": {}},
            "annotations": {"readOnlyHint": true},
        },
        {
            "name": "make_playlist",
            "title": "Make a playlist",
            "description": "Make a playlist: a smart one from a rule (it keeps itself up to date), or one of the songs given by id.",
            "inputSchema": {"type": "object", "properties": {
                "name": {"type": "string"},
                "rule": {"type": "string", "description": rule_help},
                "song_ids": {"type": "array", "items": {"type": "string"}},
                "about": {"type": "string", "description": "A line about it."},
            }, "required": ["name"]},
        },
        {
            "name": "library_numbers",
            "title": "The library in numbers",
            "description": "How many songs, albums, and artists, hours of music, lossless and hi-res songs, formats, decades, the top genres, and the top artists.",
            "inputSchema": {"type": "object", "properties": {}},
            "annotations": {"readOnlyHint": true},
        },
        {
            "name": "lyrics",
            "title": "Lyrics",
            "description": "The lyrics of the song playing now, or of the song given by id.",
            "inputSchema": {"type": "object", "properties": {"song_id": {"type": "string"}}},
            "annotations": {"readOnlyHint": true},
        },
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn post(port: u16, path: &str, body: Value, origin: Option<&str>) -> (u16, Value) {
        let client = reqwest::blocking::Client::new();
        let mut request = client
            .post(format!("http://127.0.0.1:{port}{path}"))
            .json(&body);
        if let Some(o) = origin {
            request = request.header("Origin", o);
        }
        let response = request.send().unwrap();
        let status = response.status().as_u16();
        (status, response.json().unwrap_or(Value::Null))
    }

    #[test]
    fn an_assistant_needs_the_key_and_can_search_and_make_a_playlist() {
        let dir = tempfile::tempdir().unwrap();
        let library = Library::open(dir.path().join("data")).unwrap();
        for (id, title, genre) in [
            ("a", "Harbor Lights", "Jazz"),
            ("b", "Low Tide", "Jazz"),
            ("c", "Neon", "Pop"),
        ] {
            library
                .upsert(&Track {
                    id: id.into(),
                    path: dir
                        .path()
                        .join(format!("{id}.flac"))
                        .to_string_lossy()
                        .into(),
                    title: title.into(),
                    artist: "Mara Quinn".into(),
                    album: "Harbor".into(),
                    album_artist: "Mara Quinn".into(),
                    genre: genre.into(),
                    duration: 200.,
                    ..Default::default()
                })
                .unwrap();
        }
        let player = Player::new(library.clone());
        let made = Arc::new(AtomicUsize::new(0));
        let counter = made.clone();
        let key = "k".repeat(32);
        let server = start(
            player,
            library.clone(),
            key.clone(),
            Arc::new(move || {
                counter.fetch_add(1, Ordering::Relaxed);
            }),
        )
        .unwrap();
        let port = server.port;
        let path = format!("/mcp/{key}");
        // Without the key: nothing there.
        let init = json!({"jsonrpc": "2.0", "id": 1, "method": "initialize",
            "params": {"protocolVersion": "2025-03-26", "capabilities": {}, "clientInfo": {"name": "t", "version": "1"}}});
        assert_eq!(post(port, "/mcp/wrong", init.clone(), None).0, 404);
        // A web page may not.
        assert_eq!(
            post(port, &path, init.clone(), Some("https://evil.example")).0,
            403
        );
        // With it: the client's version back, and the tools.
        let (status, answer) = post(port, &path, init, None);
        assert_eq!(status, 200);
        assert_eq!(answer["result"]["protocolVersion"], "2025-03-26");
        assert_eq!(answer["result"]["serverInfo"]["name"], "Needle");
        let notified = post(
            port,
            &path,
            json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
            None,
        );
        assert_eq!(notified.0, 202);
        let (_, list) = post(
            port,
            &path,
            json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}),
            None,
        );
        assert!(
            list["result"]["tools"]
                .as_array()
                .unwrap()
                .iter()
                .any(|t| t["name"] == "play")
        );
        // Search finds the song.
        let (_, found) = post(
            port,
            &path,
            json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call",
            "params": {"name": "search", "arguments": {"query": "harbor lights"}}}),
            None,
        );
        assert_eq!(found["result"]["isError"], false);
        assert!(
            found["result"]["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains("Harbor Lights")
        );
        // A smart playlist from a rule.
        let (_, made_one) = post(
            port,
            &path,
            json!({"jsonrpc": "2.0", "id": 4, "method": "tools/call",
            "params": {"name": "make_playlist", "arguments": {"name": "Jazz now", "rule": "genre = \"Jazz\""}}}),
            None,
        );
        assert_eq!(made_one["result"]["isError"], false, "{made_one}");
        assert!(
            made_one["result"]["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains("\"songs\": 2")
        );
        assert_eq!(made.load(Ordering::Relaxed), 1);
        assert_eq!(library.playlists().unwrap()[0].name, "Jazz now");
        // A bad rule is the tool's error, not the protocol's.
        let (_, bad) = post(
            port,
            &path,
            json!({"jsonrpc": "2.0", "id": 5, "method": "tools/call",
            "params": {"name": "make_playlist", "arguments": {"name": "Bad", "rule": "nonsense ==="}}}),
            None,
        );
        assert_eq!(bad["result"]["isError"], true);
        // An unknown method.
        let (_, unknown) = post(
            port,
            &path,
            json!({"jsonrpc": "2.0", "id": 6, "method": "nope"}),
            None,
        );
        assert_eq!(unknown["error"]["code"], -32601);
    }

    #[test]
    fn only_this_computers_pages_may_call() {
        assert!(origin_allowed(None));
        assert!(origin_allowed(Some("http://localhost:6274")));
        assert!(origin_allowed(Some("http://127.0.0.1")));
        assert!(!origin_allowed(Some("https://example.com")));
        assert!(!origin_allowed(Some("http://localhost.example.com")));
    }
}
