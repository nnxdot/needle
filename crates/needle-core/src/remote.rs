//! The phone remote: a small web page on this computer that phones on the same network open
//! in their browser, to see what plays and control it. Nothing to install on the phone.
//!
//! Every address holds a long random key (`/r/<key>/`), so only phones given the address (by
//! its QR code in Settings) can use it. The remote is off until turned on, and it answers
//! only on the local network.
use crate::{
    audio::{Command, Player, QueueItem, Repeat},
    database::Library,
};
use anyhow::{Context, Result};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader, Read, Write},
    net::{IpAddr, TcpListener, TcpStream},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::Duration,
};

/// The port Needle tries first, so a phone's bookmark keeps working.
pub const PORT: u16 = 47380;
const MAX_BODY: usize = 64 * 1024;
const MAX_CONNECTIONS: usize = 32;

/// A new random key for the remote's address.
pub fn new_key() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    (0..24)
        .map(|_| {
            let n = rng.gen_range(0..36u8);
            (if n < 10 { b'0' + n } else { b'a' + n - 10 }) as char
        })
        .collect()
}

/// This computer's address on the local network.
pub fn local_ip() -> IpAddr {
    crate::cast::local_ip_for("192.168.1.1:9".parse().unwrap())
        .ok()
        .filter(|ip| !ip.is_unspecified())
        .unwrap_or(IpAddr::from([127, 0, 0, 1]))
}

/// The running remote. Dropping it stops it.
pub struct Server {
    pub port: u16,
    key: String,
    stop: Arc<AtomicBool>,
}

impl Server {
    /// The address to open on a phone.
    pub fn address(&self) -> String {
        format!("http://{}:{}/r/{}/", local_ip(), self.port, self.key)
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

pub fn start(player: Player, library: Library, key: String) -> Result<Server> {
    anyhow::ensure!(key.len() >= 16, "The remote's key is too short");
    let listener = TcpListener::bind(("0.0.0.0", PORT))
        .or_else(|_| TcpListener::bind(("0.0.0.0", 0)))
        .context("Could not start the phone remote")?;
    listener.set_nonblocking(true)?;
    let port = listener.local_addr()?.port();
    let stop = Arc::new(AtomicBool::new(false));
    let (flag, prefix) = (stop.clone(), format!("/r/{key}/"));
    let open = Arc::new(AtomicUsize::new(0));
    std::thread::Builder::new()
        .name("needle-remote".into())
        .spawn(move || {
            crate::logfile::info(format!("Phone remote listening on port {port}"));
            while !flag.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((stream, peer)) => {
                        // Only the local network: private, link-local, and loopback addresses.
                        let local = match peer.ip() {
                            IpAddr::V4(ip) => {
                                ip.is_private() || ip.is_loopback() || ip.is_link_local()
                            }
                            IpAddr::V6(ip) => {
                                ip.is_loopback()
                                    || (ip.segments()[0] & 0xfe00) == 0xfc00
                                    || (ip.segments()[0] & 0xffc0) == 0xfe80
                            }
                        };
                        if !local || open.load(Ordering::Relaxed) >= MAX_CONNECTIONS {
                            continue;
                        }
                        let (player, library, prefix, open) = (
                            player.clone(),
                            library.clone(),
                            prefix.clone(),
                            open.clone(),
                        );
                        open.fetch_add(1, Ordering::Relaxed);
                        std::thread::spawn(move || {
                            let _ = serve(stream, &player, &library, &prefix);
                            open.fetch_sub(1, Ordering::Relaxed);
                        });
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(100));
                    }
                    Err(_) => std::thread::sleep(Duration::from_millis(100)),
                }
            }
            crate::logfile::info("Phone remote stopped");
        })?;
    Ok(Server { port, key, stop })
}

struct Request {
    method: String,
    path: String,
    query: String,
    body: Vec<u8>,
}

fn read_request(stream: &TcpStream) -> Result<Request> {
    stream.set_read_timeout(Some(Duration::from_secs(10)))?;
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line)?;
    let mut parts = line.split_whitespace();
    let method = parts.next().unwrap_or_default().to_string();
    let target = parts.next().unwrap_or_default().to_string();
    let mut length = 0usize;
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header)? == 0 || header.trim().is_empty() {
            break;
        }
        if let Some((name, value)) = header.split_once(':')
            && name.trim().eq_ignore_ascii_case("content-length")
        {
            length = value.trim().parse().unwrap_or(0);
        }
    }
    anyhow::ensure!(length <= MAX_BODY, "Request too large");
    let mut body = vec![0; length];
    reader.read_exact(&mut body)?;
    let (path, query) = match target.split_once('?') {
        Some((p, q)) => (p.to_string(), q.to_string()),
        None => (target, String::new()),
    };
    Ok(Request {
        method,
        path,
        query,
        body,
    })
}

fn respond(mut stream: &TcpStream, status: &str, kind: &str, body: &[u8]) -> Result<()> {
    write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nReferrer-Policy: no-referrer\r\nConnection: close\r\n\r\n",
        body.len()
    )?;
    stream.write_all(body)?;
    Ok(())
}

fn serve(stream: TcpStream, player: &Player, library: &Library, prefix: &str) -> Result<()> {
    let request = read_request(&stream)?;
    let Some(route) = request.path.strip_prefix(prefix) else {
        // A wrong key looks the same as a page that does not exist.
        std::thread::sleep(Duration::from_millis(300));
        return respond(&stream, "404 Not Found", "text/plain", b"Not found");
    };
    match (request.method.as_str(), route) {
        ("GET", "") => respond(
            &stream,
            "200 OK",
            "text/html; charset=utf-8",
            PAGE.as_bytes(),
        ),
        ("GET", "api/state") => {
            let body = serde_json::to_vec(&state(player))?;
            respond(&stream, "200 OK", "application/json", &body)
        }
        ("GET", "api/search") => {
            let query = decode(
                request
                    .query
                    .split('&')
                    .find_map(|p| p.strip_prefix("q="))
                    .unwrap_or_default(),
            );
            let tracks = library.search(&query).unwrap_or_default();
            let found: Vec<Value> = tracks
                .iter()
                .filter(|t| !t.missing)
                .take(50)
                .map(|t| {
                    json!({ "id": t.id, "title": t.title, "artist": t.display_artist(),
                            "album": t.album, "duration": t.duration })
                })
                .collect();
            respond(
                &stream,
                "200 OK",
                "application/json",
                &serde_json::to_vec(&found)?,
            )
        }
        ("GET", cover) if cover.starts_with("api/cover/") => {
            let id = &cover["api/cover/".len()..];
            let image = library
                .track(id)
                .ok()
                .flatten()
                .and_then(|t| t.artwork)
                .and_then(|path| std::fs::read(path).ok())
                .filter(|bytes| bytes.len() < 20 << 20);
            match image {
                Some(bytes) => {
                    let kind = if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
                        "image/png"
                    } else if bytes.starts_with(b"RIFF") {
                        "image/webp"
                    } else {
                        "image/jpeg"
                    };
                    respond(&stream, "200 OK", kind, &bytes)
                }
                None => respond(&stream, "404 Not Found", "text/plain", b"No cover"),
            }
        }
        ("POST", action) if action.starts_with("api/") => {
            match act(&action[4..], &request.body, player, library) {
                Ok(()) => respond(&stream, "200 OK", "application/json", b"{}"),
                Err(e) => respond(
                    &stream,
                    "400 Bad Request",
                    "application/json",
                    serde_json::to_vec(&json!({ "error": format!("{e:#}") }))?.as_slice(),
                ),
            }
        }
        _ => respond(&stream, "404 Not Found", "text/plain", b"Not found"),
    }
}

/// What the phone shows.
pub fn state(player: &Player) -> Value {
    let state = player.state();
    let current = state.current.as_ref().map(|item| {
        let t = &item.track;
        json!({ "id": t.id, "title": t.title, "artist": t.display_artist(), "album": t.album,
                "duration": t.duration, "cover": t.artwork.is_some() })
    });
    let queue: Vec<Value> = state
        .queue
        .iter()
        .take(50)
        .enumerate()
        .map(|(i, item)| {
            json!({ "index": i, "id": item.track.id, "title": item.track.title,
                    "artist": item.track.display_artist() })
        })
        .collect();
    json!({
        "playing": state.playing,
        "position": state.position,
        "volume": state.volume,
        "output": state.output,
        "repeat": match state.repeat { Repeat::Off => "off", Repeat::All => "all", Repeat::One => "one" },
        "current": current,
        "queue": queue,
    })
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Body {
    seconds: f64,
    value: f32,
    index: usize,
    ids: Vec<String>,
}

/// Carry out what the phone asked for.
pub fn act(action: &str, body: &[u8], player: &Player, library: &Library) -> Result<()> {
    let body: Body = if body.is_empty() {
        Body::default()
    } else {
        serde_json::from_slice(body).context("The request is not valid")?
    };
    let items = |ids: &[String]| -> Result<Vec<QueueItem>> {
        Ok(library
            .tracks_by_ids(&ids.iter().take(500).cloned().collect::<Vec<_>>())?
            .into_iter()
            .filter(|t| !t.missing)
            .map(|track| QueueItem {
                track,
                reason: "Chosen on a phone".into(),
            })
            .collect())
    };
    let command = match action {
        "toggle" => Command::Toggle,
        "next" => Command::Next,
        "previous" => Command::Previous,
        "seek" if body.seconds.is_finite() && body.seconds >= 0. => Command::Seek(body.seconds),
        "volume" if (0.0..=1.0).contains(&body.value) => Command::Volume(body.value),
        "jump" => Command::Jump(body.index),
        "play" => Command::Play(items(&body.ids)?),
        "next-up" => Command::PlayNext(items(&body.ids)?),
        "enqueue" => Command::Enqueue(items(&body.ids)?),
        _ => anyhow::bail!("Unknown action"),
    };
    player.send(command);
    Ok(())
}

/// Undo URL encoding (`%20` and `+`).
fn decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < bytes.len() => {
                match std::str::from_utf8(&bytes[i + 1..i + 3])
                    .ok()
                    .and_then(|hex| u8::from_str_radix(hex, 16).ok())
                {
                    Some(b) => {
                        out.push(b);
                        i += 2;
                    }
                    None => out.push(b'%'),
                }
            }
            b => out.push(b),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

const PAGE: &str = include_str!("remote.html");

#[cfg(test)]
mod tests {
    use super::*;

    fn get(port: u16, path: &str) -> (u16, String) {
        let response = reqwest::blocking::get(format!("http://127.0.0.1:{port}{path}")).unwrap();
        (response.status().as_u16(), response.text().unwrap())
    }

    #[test]
    fn the_remote_needs_its_key_and_controls_the_player() {
        let dir = tempfile::tempdir().unwrap();
        let library = Library::open(dir.path()).unwrap();
        library
            .upsert(&crate::model::Track {
                id: "t1".into(),
                path: "C:/nowhere/song.flac".into(),
                title: "Harbor Lights".into(),
                artist: "Mara Quinn".into(),
                duration: 200.,
                ..Default::default()
            })
            .unwrap();
        let player = Player::new(library.clone());
        let key = new_key();
        assert_eq!(key.len(), 24);
        let server = start(player.clone(), library.clone(), key.clone()).unwrap();
        assert!(server.address().contains(&format!("/r/{key}/")));
        let port = server.port;
        let (status, page) = get(port, &format!("/r/{key}/"));
        assert_eq!(status, 200);
        assert!(page.contains("<title>Needle</title>"));
        assert_eq!(get(port, "/r/wrongkeywrongkeywrong/").0, 404);
        assert_eq!(get(port, &format!("/r/{key}/api/nothing")).0, 404);
        let (status, state) = get(port, &format!("/r/{key}/api/state"));
        assert_eq!(status, 200);
        let state: Value = serde_json::from_str(&state).unwrap();
        assert_eq!(state["playing"], false);
        let (_, found) = get(port, &format!("/r/{key}/api/search?q=harbor+lights"));
        let found: Value = serde_json::from_str(&found).unwrap();
        assert_eq!(found[0]["title"], "Harbor Lights");
        let client = reqwest::blocking::Client::new();
        let post = |action: &str, body: &str| {
            client
                .post(format!("http://127.0.0.1:{port}/r/{key}/api/{action}"))
                .body(body.to_string())
                .send()
                .unwrap()
                .status()
                .as_u16()
        };
        assert_eq!(post("volume", r#"{"value":0.3}"#), 200);
        assert_eq!(post("volume", r#"{"value":7}"#), 400);
        assert_eq!(post("enqueue", r#"{"ids":["t1"]}"#), 200);
        assert_eq!(post("explode", ""), 400);
        let start = std::time::Instant::now();
        while (player.state().volume - 0.3).abs() > 1e-6 && start.elapsed().as_secs() < 5 {
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!((player.state().volume - 0.3).abs() < 1e-6);
        drop(server);
        player.shutdown();
        assert_eq!(decode("a%20b+c%C3%A9%"), "a b cé%");
    }
}
