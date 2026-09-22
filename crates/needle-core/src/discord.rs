//! Discord Rich Presence: "Listening to …" on your Discord profile.
//!
//! Talks to the Discord desktop app over its local IPC pipe (`\\.\pipe\discord-ipc-N` on
//! Windows, a Unix socket elsewhere). Nothing goes over the network from Needle itself. A
//! background thread keeps the latest activity and reconnects when Discord starts later.
use serde_json::{Value, json};
use std::{
    io::{Read, Write},
    sync::mpsc::{self, RecvTimeoutError, Sender},
    time::Duration,
};

/// What to show. Times are Unix seconds.
#[derive(Clone, Debug, PartialEq)]
pub struct Activity {
    pub title: String,
    pub artist: String,
    pub album: String,
    /// When the song started, as if it had played without pauses.
    pub started: i64,
    pub ends: Option<i64>,
    pub paused: bool,
    /// Look up the cover online so Discord can show it (Discord needs a public image link).
    pub find_cover: bool,
    /// The cover's public link, once known.
    pub cover: Option<String>,
}

enum Message {
    Set(Option<Activity>),
    Stop,
}

/// A handle to the presence thread. Dropping it clears the presence.
pub struct Presence {
    sender: Sender<Message>,
}

impl Presence {
    /// Start talking to Discord as the application `client_id`.
    pub fn start(client_id: String) -> Self {
        let (sender, receiver) = mpsc::channel::<Message>();
        std::thread::spawn(move || {
            let mut pipe: Option<Pipe> = None;
            let mut wanted: Option<Activity> = None;
            let mut covers: std::collections::HashMap<String, Option<String>> = Default::default();
            let mut shown: Option<Option<Activity>> = None;
            loop {
                match receiver.recv_timeout(Duration::from_secs(15)) {
                    Ok(Message::Set(activity)) => {
                        wanted = activity;
                        if let Some(a) = wanted.as_mut().filter(|a| a.find_cover) {
                            a.cover = covers.get(&cover_key(a)).cloned().flatten();
                        }
                    }
                    Ok(Message::Stop) | Err(RecvTimeoutError::Disconnected) => {
                        if let Some(p) = pipe.as_mut() {
                            let _ = p.set_activity(None);
                        }
                        return;
                    }
                    // Periodically try again, so presence appears once Discord is running.
                    Err(RecvTimeoutError::Timeout) => {}
                }
                if shown.as_ref() == Some(&wanted) && pipe.is_some() {
                    continue;
                }
                if pipe.is_none() {
                    pipe = Pipe::connect(&client_id);
                    shown = None;
                }
                if let Some(p) = pipe.as_mut() {
                    if p.set_activity(wanted.as_ref()).is_ok() {
                        shown = Some(wanted.clone());
                    } else {
                        pipe = None;
                    }
                }
                // Show the song straight away, then add its cover once it has been found.
                if pipe.is_some()
                    && let Some(a) = wanted.as_mut().filter(|a| a.find_cover)
                    && !covers.contains_key(&cover_key(a))
                {
                    let found = find_cover(&a.artist, &a.title, &a.album);
                    covers.insert(cover_key(a), found.clone());
                    if found.is_some() {
                        a.cover = found;
                        if let Some(p) = pipe.as_mut() {
                            if p.set_activity(Some(a)).is_ok() {
                                shown = Some(Some(a.clone()));
                            } else {
                                pipe = None;
                            }
                        }
                    }
                }
            }
        });
        Self { sender }
    }

    pub fn set(&self, activity: Option<Activity>) {
        let _ = self.sender.send(Message::Set(activity));
    }
}

impl Drop for Presence {
    fn drop(&mut self) {
        let _ = self.sender.send(Message::Stop);
    }
}

/// The JSON Discord expects for an activity (or `null` to clear it).
pub fn activity_json(activity: Option<&Activity>) -> Value {
    let Some(a) = activity else {
        return Value::Null;
    };
    // Discord needs 2–128 characters in each text field.
    let text = |s: &str, fallback: &str| {
        let s = if s.trim().is_empty() {
            fallback
        } else {
            s.trim()
        };
        let mut s: String = s.chars().take(128).collect();
        while s.chars().count() < 2 {
            s.push(' ');
        }
        s
    };
    // Header "Listening to <song>", then the artist, then the album (or "Paused").
    let album = if a.album.trim().is_empty() {
        None
    } else {
        Some(text(&a.album, ""))
    };
    let mut value = json!({
        "name": text(&a.title, "Needle"),
        "type": 2,
        "status_display_type": 0,
        "details": text(&a.artist, "Unknown artist"),
        "assets": {
            "large_image": a.cover.clone().unwrap_or_else(|| "needle".into()),
            "large_text": album.clone().unwrap_or_else(|| "Needle".into()),
            "small_image": if a.cover.is_some() { json!("needle") } else { Value::Null },
            "small_text": if a.cover.is_some() { json!("Needle") } else { Value::Null },
        },
    });
    let state = match (a.paused, album) {
        (true, Some(album)) => Some(format!("Paused · {album}")),
        (true, None) => Some("Paused".to_string()),
        (false, album) => album,
    };
    if let Some(state) = state {
        value["state"] = json!(state);
    }
    if a.cover.is_none() {
        let assets = value["assets"].as_object_mut().unwrap();
        assets.remove("small_image");
        assets.remove("small_text");
    }
    if !a.paused {
        let mut timestamps = json!({ "start": a.started * 1000 });
        if let Some(end) = a.ends {
            timestamps["end"] = json!(end * 1000);
        }
        value["timestamps"] = timestamps;
    }
    value
}

fn cover_key(a: &Activity) -> String {
    format!(
        "{}\u{1}{}\u{1}{}",
        a.artist.to_lowercase(),
        a.album.to_lowercase(),
        a.title.to_lowercase()
    )
}

/// Find a public link to the song's cover in Apple's iTunes catalog, searching by artist and
/// song name only. Returns `None` unless the artist matches, so a wrong cover is never shown.
pub fn find_cover(artist: &str, title: &str, album: &str) -> Option<String> {
    if artist.trim().is_empty() || title.trim().is_empty() {
        return None;
    }
    let client = crate::integrations::client().ok()?;
    let response: Value = client
        .get("https://itunes.apple.com/search")
        .query(&[
            ("term", format!("{artist} {title}").as_str()),
            ("entity", "song"),
            ("limit", "10"),
        ])
        .timeout(Duration::from_secs(6))
        .send()
        .ok()?
        .json()
        .ok()?;
    best_cover(response["results"].as_array()?, artist, title, album)
}

/// Pick the best result: the artist must match; then prefer the same song on the same album.
pub fn best_cover(results: &[Value], artist: &str, title: &str, album: &str) -> Option<String> {
    let norm = |s: &str| {
        s.to_lowercase()
            .chars()
            .filter(|c| c.is_alphanumeric())
            .collect::<String>()
    };
    let (artist, title, album) = (norm(artist), norm(title), norm(album));
    let artist_ok = |r: &Value| {
        let found = norm(r["artistName"].as_str().unwrap_or_default());
        !found.is_empty() && (found.contains(&artist) || artist.contains(&found))
    };
    let score = |r: &Value| {
        let same_title = norm(r["trackName"].as_str().unwrap_or_default()) == title;
        let same_album =
            !album.is_empty() && norm(r["collectionName"].as_str().unwrap_or_default()) == album;
        same_title as u8 * 2 + same_album as u8
    };
    let best = results
        .iter()
        .filter(|r| artist_ok(r))
        .max_by_key(|r| score(r))?;
    let url = best["artworkUrl100"].as_str()?;
    Some(url.replace("100x100bb", "600x600bb"))
}

/// Executable names of the Discord desktop app and well-known Discord clients that offer
/// Rich Presence.
pub fn is_discord_client(exe: &str) -> bool {
    matches!(
        exe.to_ascii_lowercase().as_str(),
        "discord.exe"
            | "discordcanary.exe"
            | "discordptb.exe"
            | "discorddevelopment.exe"
            | "vesktop.exe"
            | "equibop.exe"
            | "legcord.exe"
            | "webcord.exe"
    )
}

/// Whether the program serving this pipe is a Discord client.
#[cfg(windows)]
fn served_by_discord(pipe: &Stream) -> bool {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::{
        Foundation::CloseHandle,
        System::{
            Pipes::GetNamedPipeServerProcessId,
            Threading::{
                OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
                QueryFullProcessImageNameW,
            },
        },
    };
    let mut pid = 0u32;
    if unsafe { GetNamedPipeServerProcessId(pipe.as_raw_handle() as _, &mut pid) } == 0 {
        return false;
    }
    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if process == 0 {
        return false;
    }
    let mut path = [0u16; 1024];
    let mut len = path.len() as u32;
    let ok = unsafe {
        QueryFullProcessImageNameW(process, PROCESS_NAME_WIN32, path.as_mut_ptr(), &mut len)
    };
    unsafe { CloseHandle(process) };
    if ok == 0 {
        return false;
    }
    let path = String::from_utf16_lossy(&path[..len as usize]);
    let exe = path.rsplit(['\\', '/']).next().unwrap_or_default();
    let discord = is_discord_client(exe);
    if !discord {
        eprintln!("Not showing presence: the Discord pipe belongs to {exe}");
    }
    discord
}

/// On other systems the socket lives in this user's own runtime folder.
#[cfg(unix)]
fn served_by_discord(_: &Stream) -> bool {
    true
}

#[cfg(windows)]
type Stream = std::fs::File;
#[cfg(unix)]
type Stream = std::os::unix::net::UnixStream;

struct Pipe {
    stream: Stream,
    nonce: u64,
}

impl Pipe {
    fn open(n: u32) -> Option<Stream> {
        #[cfg(windows)]
        {
            std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open(format!(r"\\.\pipe\discord-ipc-{n}"))
                .ok()
        }
        #[cfg(unix)]
        {
            let dir = ["XDG_RUNTIME_DIR", "TMPDIR", "TMP", "TEMP"]
                .iter()
                .find_map(|k| std::env::var(k).ok())
                .unwrap_or_else(|| "/tmp".into());
            std::os::unix::net::UnixStream::connect(format!("{dir}/discord-ipc-{n}")).ok()
        }
    }

    fn connect(client_id: &str) -> Option<Self> {
        // Only talk to a pipe that Discord itself serves, so no other program on this PC can
        // pose as Discord to learn what you play.
        let stream = (0..10).filter_map(Self::open).find(served_by_discord)?;
        let mut pipe = Self { stream, nonce: 0 };
        pipe.send(0, &json!({ "v": 1, "client_id": client_id }))
            .ok()?;
        let (_, ready) = pipe.receive().ok()?;
        (ready["evt"] == "READY").then_some(pipe)
    }

    fn set_activity(&mut self, activity: Option<&Activity>) -> std::io::Result<()> {
        self.set_activity_reply(activity).map(|_| ())
    }

    fn set_activity_reply(&mut self, activity: Option<&Activity>) -> std::io::Result<Value> {
        self.nonce += 1;
        let message = json!({
            "cmd": "SET_ACTIVITY",
            "args": { "pid": std::process::id(), "activity": activity_json(activity) },
            "nonce": format!("needle-{}", self.nonce),
        });
        self.send(1, &message)?;
        let (_, reply) = self.receive()?;
        if reply["evt"] == "ERROR" {
            eprintln!("Discord refused the presence: {}", reply["data"]);
        }
        Ok(reply)
    }

    fn send(&mut self, op: u32, value: &Value) -> std::io::Result<()> {
        let body = serde_json::to_vec(value)?;
        let mut frame = Vec::with_capacity(8 + body.len());
        frame.extend_from_slice(&op.to_le_bytes());
        frame.extend_from_slice(&(body.len() as u32).to_le_bytes());
        frame.extend_from_slice(&body);
        self.stream.write_all(&frame)?;
        self.stream.flush()
    }

    fn receive(&mut self) -> std::io::Result<(u32, Value)> {
        let mut header = [0u8; 8];
        self.stream.read_exact(&mut header)?;
        let op = u32::from_le_bytes(header[..4].try_into().unwrap());
        let len = u32::from_le_bytes(header[4..].try_into().unwrap()) as usize;
        if len > 1 << 20 {
            return Err(std::io::Error::other("Discord sent an oversized frame"));
        }
        let mut body = vec![0u8; len];
        self.stream.read_exact(&mut body)?;
        Ok((op, serde_json::from_slice(&body).unwrap_or(Value::Null)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn activity_shape() {
        let a = Activity {
            title: "Armageddon".into(),
            artist: "aespa".into(),
            album: "".into(),
            started: 1_000,
            ends: Some(1_196),
            paused: false,
            find_cover: true,
            cover: None,
        };
        let v = activity_json(Some(&a));
        assert_eq!(v["type"], 2);
        assert_eq!(v["name"], "Armageddon");
        assert_eq!(v["details"], "aespa");
        assert!(v.get("state").is_none());
        assert_eq!(v["assets"]["large_image"], "needle");
        assert_eq!(v["assets"]["large_text"], "Needle");
        assert!(v["assets"].get("small_image").is_none());
        assert_eq!(v["timestamps"]["start"], 1_000_000);
        assert_eq!(v["timestamps"]["end"], 1_196_000);
        let with_cover = activity_json(Some(&Activity {
            album: "Armageddon - The 1st Album".into(),
            cover: Some("https://example.com/c.jpg".into()),
            ..a.clone()
        }));
        assert_eq!(
            with_cover["assets"]["large_image"],
            "https://example.com/c.jpg"
        );
        assert_eq!(with_cover["assets"]["small_image"], "needle");
        assert_eq!(with_cover["state"], "Armageddon - The 1st Album");
        let paused = activity_json(Some(&Activity {
            paused: true,
            title: "X".into(),
            ..a
        }));
        assert_eq!(paused["name"], "X ");
        assert_eq!(paused["state"], "Paused");
        assert!(paused.get("timestamps").is_none());
        assert_eq!(activity_json(None), Value::Null);
    }

    #[test]
    fn picks_the_right_cover() {
        let results: Vec<Value> = serde_json::from_str(r#"[
            {"artistName":"Someone Else","trackName":"Armageddon","collectionName":"X","artworkUrl100":"https://a/wrong/100x100bb.jpg"},
            {"artistName":"aespa","trackName":"Armageddon (Remix)","collectionName":"K-POP","artworkUrl100":"https://a/remix/100x100bb.jpg"},
            {"artistName":"aespa","trackName":"Armageddon","collectionName":"Armageddon - The 1st Album","artworkUrl100":"https://a/album/100x100bb.jpg"}
        ]"#).unwrap();
        assert_eq!(
            best_cover(
                &results,
                "aespa",
                "Armageddon",
                "Armageddon - The 1st Album"
            )
            .as_deref(),
            Some("https://a/album/600x600bb.jpg")
        );
        assert_eq!(best_cover(&results[..1], "aespa", "Armageddon", ""), None);
    }

    #[test]
    fn recognises_discord_clients_only() {
        assert!(is_discord_client("Discord.exe"));
        assert!(is_discord_client("DiscordPTB.exe"));
        assert!(is_discord_client("Vesktop.exe"));
        assert!(!is_discord_client("notdiscord.exe"));
        assert!(!is_discord_client("python.exe"));
    }

    /// Run by hand with Discord open and NEEDLE_DISCORD_ID set: shows a presence for a few
    /// seconds, checks that Discord accepted it, then clears it.
    #[test]
    #[ignore]
    fn live_presence() {
        let id = std::env::var("NEEDLE_DISCORD_ID").expect("set NEEDLE_DISCORD_ID");
        let mut pipe = Pipe::connect(&id).expect("Discord did not accept the connection");
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let reply = pipe
            .set_activity_reply(Some(&Activity {
                title: "Armageddon".into(),
                artist: "aespa".into(),
                album: "Armageddon - The 1st Album".into(),
                started: now,
                ends: Some(now + 60),
                paused: false,
                find_cover: false,
                cover: find_cover("aespa", "Armageddon", "Armageddon - The 1st Album"),
            }))
            .unwrap();
        println!("{reply}");
        assert_ne!(reply["evt"], "ERROR", "{reply}");
        std::thread::sleep(Duration::from_secs(8));
        pipe.set_activity(None).unwrap();
    }

    /// Run by hand with Discord open: finds its pipe and confirms who serves it.
    #[test]
    #[ignore]
    fn live_pipe_is_discord() {
        let found = (0..10)
            .filter_map(Pipe::open)
            .any(|p| served_by_discord(&p));
        assert!(found, "no Discord pipe served by a Discord client");
    }
}
