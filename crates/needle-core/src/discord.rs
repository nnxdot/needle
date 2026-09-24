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
    pub layout: Layout,
}

/// How long the song must stay the same before Discord is told.
const SETTLE: Duration = Duration::from_millis(800);
/// How soon to try again after Discord refused a change.
const RETRY: Duration = Duration::from_secs(5);

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
                // Refused by Discord (too many changes at once): try again soon.
                let wait = if shown.as_ref() != Some(&wanted) && pipe.is_some() {
                    RETRY
                } else {
                    Duration::from_secs(15)
                };
                match receiver.recv_timeout(wait) {
                    Ok(Message::Set(activity)) => {
                        wanted = activity;
                        // Skipping through songs sends many changes at once; Discord allows only
                        // a few a minute and can leave an old song's card beside the new one.
                        // Wait until the song settles, then send one update.
                        loop {
                            match receiver.recv_timeout(SETTLE) {
                                Ok(Message::Set(activity)) => wanted = activity,
                                Ok(Message::Stop) | Err(RecvTimeoutError::Disconnected) => {
                                    if let Some(p) = pipe.as_mut() {
                                        let _ = p.set_activity(None);
                                    }
                                    return;
                                }
                                Err(RecvTimeoutError::Timeout) => break,
                            }
                        }
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
                if pipe.is_none() {
                    continue;
                }
                // Find the cover before telling Discord, so a new song is one update, not two.
                if let Some(a) = wanted.as_mut().filter(|a| a.find_cover)
                    && !covers.contains_key(&cover_key(a))
                {
                    let found = find_cover(&a.artist, &a.title, &a.album);
                    covers.insert(cover_key(a), found.clone());
                    a.cover = found;
                }
                if let Some(p) = pipe.as_mut() {
                    match p.set_activity_reply(wanted.as_ref()) {
                        Ok(reply) if reply["evt"] == "ERROR" => crate::logfile::warn(format!(
                            "Discord did not take the status; trying again: {}",
                            reply["data"]
                        )),
                        Ok(_) => shown = Some(wanted.clone()),
                        Err(_) => pipe = None,
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

/// What one part of the Discord card shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Field {
    Song,
    Artist,
    Album,
    Needle,
    Nothing,
}

impl Field {
    /// The settings value for this field.
    pub fn name(self) -> &'static str {
        match self {
            Self::Song => "song",
            Self::Artist => "artist",
            Self::Album => "album",
            Self::Needle => "needle",
            Self::Nothing => "none",
        }
    }
    pub fn from_name(name: &str) -> Self {
        match name {
            "song" => Self::Song,
            "artist" => Self::Artist,
            "album" => Self::Album,
            "needle" => Self::Needle,
            _ => Self::Nothing,
        }
    }
}

/// How the card is laid out: "Listening to <title>", then three lines.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Layout {
    pub title: Field,
    pub top: Field,
    pub middle: Field,
    /// The third line (Discord shows it only when there is a picture).
    pub bottom: Field,
    /// The Needle logo: as a badge on the cover, and as the picture when there is no cover.
    pub logo: bool,
}

impl Default for Layout {
    fn default() -> Self {
        Self {
            title: Field::Song,
            top: Field::Artist,
            middle: Field::Song,
            bottom: Field::Album,
            logo: true,
        }
    }
}

/// Discord needs 2–128 characters in each text field.
fn clip(s: &str) -> String {
    let mut s: String = s.trim().chars().take(128).collect();
    while s.chars().count() < 2 {
        s.push(' ');
    }
    s
}

impl Activity {
    fn text(&self, field: Field) -> Option<String> {
        let value = match field {
            Field::Song => self.title.as_str(),
            Field::Artist => self.artist.as_str(),
            Field::Album => self.album.as_str(),
            Field::Needle => "Needle",
            Field::Nothing => return None,
        };
        (!value.trim().is_empty()).then(|| clip(value))
    }
}

/// The JSON Discord expects for an activity (or `null` to clear it).
pub fn activity_json(activity: Option<&Activity>) -> Value {
    let Some(a) = activity else {
        return Value::Null;
    };
    let layout = a.layout;
    let mut value = json!({
        "name": a.text(layout.title).unwrap_or_else(|| "Needle".into()),
        "type": 2,
        "status_display_type": 0,
    });
    let top = a.text(layout.top);
    // When paused, say so on the second line (or the first, if the second is empty).
    let mut middle = a.text(layout.middle);
    let mut first = top;
    if a.paused {
        match (&middle, &first) {
            (Some(m), _) => middle = Some(clip(&format!("Paused · {m}"))),
            (None, Some(f)) => first = Some(clip(&format!("Paused · {f}"))),
            (None, None) => middle = Some("Paused".into()),
        }
    }
    if let Some(first) = first {
        value["details"] = json!(first);
    }
    if let Some(middle) = middle {
        value["state"] = json!(middle);
    }
    let picture = a
        .cover
        .clone()
        .or_else(|| layout.logo.then(|| "needle".to_string()));
    if let Some(picture) = picture {
        let mut assets = json!({ "large_image": picture });
        if let Some(bottom) = a.text(layout.bottom) {
            assets["large_text"] = json!(bottom);
        }
        if a.cover.is_some() && layout.logo {
            assets["small_image"] = json!("needle");
            assets["small_text"] = json!("Needle");
        }
        value["assets"] = assets;
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

/// Search Apple's iTunes catalog for a song by artist and song name.
fn itunes_songs(artist: &str, title: &str) -> Option<Vec<Value>> {
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
    response["results"].as_array().cloned()
}

/// Find a public link to the song's cover in Apple's iTunes catalog, searching by artist and
/// song name only. Returns `None` unless the artist matches, so a wrong cover is never shown.
pub fn find_cover(artist: &str, title: &str, album: &str) -> Option<String> {
    best_cover(&itunes_songs(artist, title)?, artist, title, album)
}

/// The artist's name as Apple writes it, when the tags write it another way (키키 for
/// KiiiKiii). Only when Apple has the same song on the same album; `None` when the names
/// already agree or nothing matches.
pub fn find_artist_spelling(artist: &str, title: &str, album: &str) -> Option<String> {
    artist_spelling(&itunes_songs(artist, title)?, artist, title, album)
}

pub fn artist_spelling(
    results: &[Value],
    artist: &str,
    title: &str,
    album: &str,
) -> Option<String> {
    let best = best_song(results, artist, title, album)?;
    let n = Names::new(artist, title, album);
    if n.artist_ok(best) {
        return None;
    }
    best["artistName"].as_str().map(str::to_string)
}

/// Names reduced to lowercase letters and digits, to compare tags with Apple's.
struct Names {
    artist: String,
    title: String,
    album: String,
}
fn norm(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect()
}
impl Names {
    fn new(artist: &str, title: &str, album: &str) -> Self {
        Self {
            artist: norm(artist),
            title: norm(title),
            album: norm(album),
        }
    }
    fn artist_ok(&self, r: &Value) -> bool {
        let found = norm(r["artistName"].as_str().unwrap_or_default());
        !found.is_empty() && (found.contains(&self.artist) || self.artist.contains(&found))
    }
    fn same_title(&self, r: &Value) -> bool {
        norm(r["trackName"].as_str().unwrap_or_default()) == self.title
    }
    fn same_album(&self, r: &Value) -> bool {
        !self.album.is_empty()
            && norm(r["collectionName"].as_str().unwrap_or_default()) == self.album
    }
}

/// The best result: the artist must match, or (when the artist is written another way, like
/// 키키 in the tags and KiiiKiii at Apple) the same song on the same album; then prefer the
/// same song on the same album.
fn best_song<'a>(
    results: &'a [Value],
    artist: &str,
    title: &str,
    album: &str,
) -> Option<&'a Value> {
    let n = Names::new(artist, title, album);
    results
        .iter()
        .filter(|r| n.artist_ok(r) || (n.same_title(r) && n.same_album(r)))
        .max_by_key(|r| n.same_title(r) as u8 * 2 + n.same_album(r) as u8)
}

/// Pick the best result's cover, at 600 × 600.
pub fn best_cover(results: &[Value], artist: &str, title: &str, album: &str) -> Option<String> {
    let best = best_song(results, artist, title, album)?;
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
            // Discord installed as a Flatpak or Snap keeps its socket in its own folder.
            [
                "",
                "app/com.discordapp.Discord/",
                "app/com.discordapp.DiscordCanary/",
                "app/dev.vencord.Vesktop/",
                "snap.discord/",
            ]
            .iter()
            .find_map(|sub| {
                std::os::unix::net::UnixStream::connect(format!("{dir}/{sub}discord-ipc-{n}")).ok()
            })
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
            album: "Armageddon - The 1st Album".into(),
            started: 1_000,
            ends: Some(1_196),
            paused: false,
            find_cover: true,
            cover: None,
            layout: Layout::default(),
        };
        // Default: "Listening to Armageddon", artist, song, album; Needle logo as the picture.
        let v = activity_json(Some(&a));
        assert_eq!(v["type"], 2);
        assert_eq!(v["name"], "Armageddon");
        assert_eq!(v["details"], "aespa");
        assert_eq!(v["state"], "Armageddon");
        assert_eq!(v["assets"]["large_image"], "needle");
        assert_eq!(v["assets"]["large_text"], "Armageddon - The 1st Album");
        assert!(v["assets"].get("small_image").is_none());
        assert_eq!(v["timestamps"]["start"], 1_000_000);
        assert_eq!(v["timestamps"]["end"], 1_196_000);
        // With a cover the logo becomes the small badge.
        let with_cover = activity_json(Some(&Activity {
            cover: Some("https://example.com/c.jpg".into()),
            ..a.clone()
        }));
        assert_eq!(
            with_cover["assets"]["large_image"],
            "https://example.com/c.jpg"
        );
        assert_eq!(with_cover["assets"]["small_image"], "needle");
        // Without the logo: no badge, and no picture at all when there is no cover.
        let no_logo = Layout {
            logo: false,
            ..Layout::default()
        };
        let v = activity_json(Some(&Activity {
            cover: Some("https://example.com/c.jpg".into()),
            layout: no_logo,
            ..a.clone()
        }));
        assert!(v["assets"].get("small_image").is_none());
        let v = activity_json(Some(&Activity {
            layout: no_logo,
            ..a.clone()
        }));
        assert!(v.get("assets").is_none());
        // Other layouts, and paused.
        let custom = Layout {
            title: Field::Needle,
            top: Field::Song,
            middle: Field::Nothing,
            bottom: Field::Artist,
            logo: true,
        };
        let v = activity_json(Some(&Activity {
            layout: custom,
            paused: true,
            ..a.clone()
        }));
        assert_eq!(v["name"], "Needle");
        assert_eq!(v["details"], "Paused · Armageddon");
        assert!(v.get("state").is_none());
        assert_eq!(v["assets"]["large_text"], "aespa");
        assert!(v.get("timestamps").is_none());
        let paused = activity_json(Some(&Activity { paused: true, ..a }));
        assert_eq!(paused["state"], "Paused · Armageddon");
        assert_eq!(activity_json(None), Value::Null);
        assert_eq!(Field::from_name(Field::Album.name()), Field::Album);
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
        // The artist in another script: the same song on the same album still counts, but the
        // song alone does not.
        let kiiikiii: Vec<Value> = serde_json::from_str(r#"[
            {"artistName":"KiiiKiii","trackName":"Hey Hi","collectionName":"WhyKiiiKiii - EP","artworkUrl100":"https://a/hey/100x100bb.jpg"}
        ]"#).unwrap();
        assert_eq!(
            best_cover(&kiiikiii, "키키", "Hey Hi", "WhyKiiiKiii - EP").as_deref(),
            Some("https://a/hey/600x600bb.jpg")
        );
        assert_eq!(best_cover(&kiiikiii, "키키", "Hey Hi", "Other"), None);
        assert_eq!(best_cover(&kiiikiii, "키키", "Hey Hi", ""), None);
    }

    #[test]
    fn spells_the_artist_as_apple_does() {
        let results: Vec<Value> = serde_json::from_str(
            r#"[
            {"artistName":"KiiiKiii","trackName":"Hey Hi","collectionName":"WhyKiiiKiii - EP"}
        ]"#,
        )
        .unwrap();
        assert_eq!(
            artist_spelling(&results, "키키", "Hey Hi", "WhyKiiiKiii - EP").as_deref(),
            Some("KiiiKiii")
        );
        // Names that already agree stay as tagged (no change of case or spacing).
        assert_eq!(
            artist_spelling(&results, "kiiikiii", "Hey Hi", "WhyKiiiKiii - EP"),
            None
        );
        // Without the same album, a different artist is never taken.
        assert_eq!(artist_spelling(&results, "키키", "Hey Hi", ""), None);
        assert_eq!(
            artist_spelling(&results, "Someone", "Hey Hi", "Other"),
            None
        );
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
                layout: Layout::default(),
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
