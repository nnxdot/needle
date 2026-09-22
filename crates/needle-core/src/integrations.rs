use crate::database::Library;
use anyhow::{Context, Result, bail};
use reqwest::blocking::Client;
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    sync::Mutex,
    time::{Duration, Instant},
};

static MB_LAST: Mutex<Option<Instant>> = Mutex::new(None);
static ACOUSTID_LAST: Mutex<Option<Instant>> = Mutex::new(None);
static SCROBBLE_LOCK: Mutex<()> = Mutex::new(());
fn client() -> Result<Client> {
    Ok(Client::builder()
        .timeout(Duration::from_secs(20))
        .user_agent(
            std::env::var("NEEDLE_HTTP_USER_AGENT")
                .unwrap_or_else(|_| "Needle/0.1.0 (local desktop music player)".into()),
        )
        .build()?)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RecordingMatch {
    pub id: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub score: i64,
    #[serde(default)]
    pub release_id: Option<String>,
}

pub fn musicbrainz_search(
    library: &Library,
    artist: &str,
    title: &str,
) -> Result<Vec<RecordingMatch>> {
    let key = format!("mb:recording:{artist}:{title}");
    let cached: Option<String> = library
        .connection()?
        .query_row(
            "SELECT data FROM cache WHERE key=? AND expires>?",
            params![key, chrono::Utc::now().timestamp()],
            |r| r.get(0),
        )
        .optional()?;
    if let Some(data) = cached {
        return Ok(serde_json::from_str(&data)?);
    }
    // Serialize access across callers, including requests waiting for the rate limit.
    let mut last = MB_LAST.lock().unwrap();
    if let Some(time) = *last {
        let elapsed = time.elapsed();
        if elapsed < Duration::from_secs(1) {
            std::thread::sleep(Duration::from_secs(1) - elapsed);
        }
    }
    let escape = |s: &str| s.replace('\\', "\\\\").replace('"', "\\\"");
    let query = format!(
        "recording:\"{}\" AND artist:\"{}\"",
        escape(title),
        escape(artist)
    );
    *last = Some(Instant::now());
    let response: Value = client()?
        .get("https://musicbrainz.org/ws/2/recording/")
        .query(&[("query", query.as_str()), ("fmt", "json"), ("limit", "8")])
        .send()?
        .error_for_status()?
        .json()?;
    drop(last);
    let result = response["recordings"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|r| RecordingMatch {
            id: r["id"].as_str().unwrap_or_default().into(),
            title: r["title"].as_str().unwrap_or_default().into(),
            artist: r["artist-credit"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|a| a["name"].as_str().unwrap_or_default())
                .collect::<Vec<_>>()
                .join(", "),
            album: r["releases"][0]["title"]
                .as_str()
                .unwrap_or_default()
                .into(),
            score: r["score"].as_i64().unwrap_or(0),
            release_id: r["releases"][0]["id"].as_str().map(String::from),
        })
        .collect::<Vec<_>>();
    library.connection()?.execute("INSERT INTO cache VALUES (?,?,?) ON CONFLICT(key) DO UPDATE SET expires=excluded.expires,data=excluded.data",params![key,chrono::Utc::now().timestamp()+604800,serde_json::to_string(&result)?])?;
    Ok(result)
}

/// Credentials are supplied at runtime, never embedded in a released application.
pub fn acoustid_lookup(
    library: &Library,
    path: &std::path::Path,
    api_key: &str,
) -> Result<Vec<RecordingMatch>> {
    if api_key.trim().is_empty() {
        bail!(
            "Set NEEDLE_ACOUSTID_API_KEY for your registered application before fingerprint lookup"
        );
    }
    let fingerprint = crate::analysis::fingerprint(path)?;
    let key = format!("acoustid:{}", blake3::hash(fingerprint.encoded.as_bytes()));
    let cached: Option<String> = library
        .connection()?
        .query_row(
            "SELECT data FROM cache WHERE key=? AND expires>?",
            params![key, chrono::Utc::now().timestamp()],
            |r| r.get(0),
        )
        .optional()?;
    if let Some(cached) = cached {
        return Ok(serde_json::from_str(&cached)?);
    }
    let mut last = ACOUSTID_LAST.lock().unwrap();
    if let Some(time) = *last {
        let elapsed = time.elapsed();
        if elapsed < Duration::from_millis(350) {
            std::thread::sleep(Duration::from_millis(350) - elapsed);
        }
    }
    *last = Some(Instant::now());
    let response: Value = client()?
        .post("https://api.acoustid.org/v2/lookup")
        .form(&[
            ("client", api_key.to_string()),
            (
                "duration",
                (fingerprint.duration.round() as u64).to_string(),
            ),
            ("fingerprint", fingerprint.encoded),
            ("meta", "recordings releases".into()),
            ("format", "json".into()),
        ])
        .send()?
        .error_for_status()?
        .json()?;
    drop(last);
    if response["status"].as_str() != Some("ok") {
        bail!(
            "AcoustID: {}",
            response["error"]["message"]
                .as_str()
                .unwrap_or("lookup rejected")
        );
    }
    let mut matches = vec![];
    for result in response["results"].as_array().into_iter().flatten() {
        for recording in result["recordings"].as_array().into_iter().flatten() {
            matches.push(RecordingMatch {
                id: recording["id"].as_str().unwrap_or_default().into(),
                title: recording["title"].as_str().unwrap_or_default().into(),
                artist: recording["artists"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|a| a["name"].as_str())
                    .collect::<Vec<_>>()
                    .join(", "),
                album: recording["releases"][0]["title"]
                    .as_str()
                    .unwrap_or_default()
                    .into(),
                score: (result["score"].as_f64().unwrap_or(0.) * 100.).round() as i64,
                release_id: recording["releases"][0]["id"].as_str().map(String::from),
            });
        }
    }
    matches.sort_by_key(|m| std::cmp::Reverse(m.score));
    matches.dedup_by(|a, b| a.id == b.id);
    matches.truncate(12);
    library.connection()?.execute(
        "INSERT OR REPLACE INTO cache VALUES (?,?,?)",
        params![
            key,
            chrono::Utc::now().timestamp() + 604800,
            serde_json::to_string(&matches)?
        ],
    )?;
    Ok(matches)
}

/// Cache cover art locally. This never modifies the music file's tags.
pub fn cover_art(
    library: &Library,
    track_id: &str,
    release_id: &str,
) -> Result<std::path::PathBuf> {
    use std::io::Read;
    let release = uuid::Uuid::parse_str(release_id).context("Invalid MusicBrainz release ID")?;
    let destination = library
        .directory
        .join("artwork")
        .join(format!("caa-{release}.jpg"));
    if !destination.exists() {
        let response = client()?
            .get(format!(
                "https://coverartarchive.org/release/{release}/front-500"
            ))
            .send()?
            .error_for_status()?;
        if !response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|h| h.to_str().ok())
            .is_some_and(|s| s.starts_with("image/"))
        {
            bail!("Cover Art Archive did not return an image");
        }
        let mut bytes = vec![];
        response
            .take(10 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() > 10 * 1024 * 1024 {
            bail!("Cover image exceeds 10 MB");
        }
        std::fs::write(&destination, bytes)?;
    }
    let mut track = library.track(track_id)?.context("Track no longer exists")?;
    track.artwork = Some(destination.to_string_lossy().into());
    library.upsert(&track)?;
    Ok(destination)
}

pub struct ScrobbleWorker {
    stop: crossbeam_channel::Sender<()>,
}
impl ScrobbleWorker {
    pub fn start(library: Library) -> Self {
        let (stop, receiver) = crossbeam_channel::bounded(1);
        std::thread::spawn(move || {
            loop {
                let _ = flush_scrobbles(&library, &Credentials::from_environment());
                if !matches!(
                    receiver.recv_timeout(Duration::from_secs(60)),
                    Err(crossbeam_channel::RecvTimeoutError::Timeout)
                ) {
                    break;
                }
            }
        });
        Self { stop }
    }
}
impl Drop for ScrobbleWorker {
    fn drop(&mut self) {
        let _ = self.stop.try_send(());
    }
}

/// Credentials are supplied at runtime, never embedded in a released application.
#[derive(Clone, Default)]
pub struct Credentials {
    pub lastfm_api_key: String,
    pub lastfm_secret: String,
    pub lastfm_session: String,
    pub listenbrainz_token: String,
}
impl Credentials {
    pub fn from_environment() -> Self {
        Self {
            lastfm_api_key: std::env::var("NEEDLE_LASTFM_API_KEY").unwrap_or_default(),
            lastfm_secret: std::env::var("NEEDLE_LASTFM_SECRET").unwrap_or_default(),
            lastfm_session: std::env::var("NEEDLE_LASTFM_SESSION").unwrap_or_default(),
            listenbrainz_token: std::env::var("NEEDLE_LISTENBRAINZ_TOKEN").unwrap_or_default(),
        }
    }
}
pub fn lastfm_authorize(credentials: &Credentials) -> Result<String> {
    if credentials.lastfm_api_key.is_empty() {
        bail!(
            "Set NEEDLE_LASTFM_API_KEY and NEEDLE_LASTFM_SECRET for your registered Last.fm application"
        )
    }
    let response: Value = client()?
        .get("https://ws.audioscrobbler.com/2.0/")
        .query(&[
            ("method", "auth.getToken"),
            ("api_key", credentials.lastfm_api_key.as_str()),
            ("format", "json"),
        ])
        .send()?
        .error_for_status()?
        .json()?;
    Ok(response["token"]
        .as_str()
        .context("Last.fm did not return an authorization token")?
        .into())
}
fn signed(mut fields: BTreeMap<String, String>, secret: &str) -> BTreeMap<String, String> {
    let mut input = String::new();
    for (k, v) in &fields {
        input.push_str(k);
        input.push_str(v);
    }
    input.push_str(secret);
    fields.insert("api_sig".into(), format!("{:x}", md5::compute(input)));
    fields.insert("format".into(), "json".into());
    fields
}
pub fn lastfm_session(credentials: &Credentials, token: &str) -> Result<String> {
    let fields = BTreeMap::from([
        ("method".into(), "auth.getSession".into()),
        ("api_key".into(), credentials.lastfm_api_key.clone()),
        ("token".into(), token.into()),
    ]);
    let response: Value = client()?
        .post("https://ws.audioscrobbler.com/2.0/")
        .form(&signed(fields, &credentials.lastfm_secret))
        .send()?
        .error_for_status()?
        .json()?;
    Ok(response["session"]["key"]
        .as_str()
        .context("Authorize the application on Last.fm before exchanging the token")?
        .into())
}

pub fn flush_scrobbles(library: &Library, credentials: &Credentials) -> Result<usize> {
    let _guard = SCROBBLE_LOCK.lock().unwrap();
    let settings = library.settings()?;
    let db = library.connection()?;
    let mut statement=db.prepare("SELECT s.listen_id,s.service,h.data,s.attempts FROM scrobbles s JOIN listens h ON h.id=s.listen_id WHERE s.status='pending' AND s.next_attempt<=? ORDER BY h.started_at LIMIT 100")?;
    let rows = statement
        .query_map([chrono::Utc::now().timestamp()], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, i64>(3)?,
            ))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let mut sent = 0;
    for (id, service, data, attempts) in rows {
        if (service == "lastfm" && !settings.lastfm_enabled)
            || (service == "listenbrainz" && !settings.listenbrainz_enabled)
        {
            continue;
        }
        let listen: crate::model::Listen = serde_json::from_str(&data)?;
        if (service == "listenbrainz" && credentials.listenbrainz_token.is_empty())
            || (service == "lastfm"
                && (credentials.lastfm_session.is_empty()
                    || credentials.lastfm_api_key.is_empty()
                    || credentials.lastfm_secret.is_empty()))
        {
            continue;
        }
        let result = (|| -> Result<()> {
            if listen.artist.trim().is_empty() || listen.title.trim().is_empty() {
                bail!("Artist and title tags are required")
            }
            if service == "listenbrainz" {
                client()?.post("https://api.listenbrainz.org/1/submit-listens").header("Authorization",format!("Token {}",credentials.listenbrainz_token))
                    .json(&json!({"listen_type":"single","payload":[{"listened_at":listen.started_at,"track_metadata":{"artist_name":listen.artist,"track_name":listen.title,"release_name":listen.album,"additional_info":{"submission_client":"Needle","submission_client_version":"0.1.0","duration":listen.duration as i64}}}]})).send()?.error_for_status()?;
            } else {
                let fields = BTreeMap::from([
                    ("method".into(), "track.scrobble".into()),
                    ("api_key".into(), credentials.lastfm_api_key.clone()),
                    ("sk".into(), credentials.lastfm_session.clone()),
                    ("artist".into(), listen.artist.clone()),
                    ("track".into(), listen.title.clone()),
                    ("album".into(), listen.album.clone()),
                    ("timestamp".into(), listen.started_at.to_string()),
                    ("duration".into(), (listen.duration as i64).to_string()),
                ]);
                let response: Value = client()?
                    .post("https://ws.audioscrobbler.com/2.0/")
                    .form(&signed(fields, &credentials.lastfm_secret))
                    .send()?
                    .error_for_status()?
                    .json()?;
                if response.get("error").is_some() {
                    bail!(
                        "Last.fm: {}",
                        response["message"].as_str().unwrap_or("request rejected")
                    )
                }
                let accepted = response["scrobbles"]["@attr"]["accepted"].as_str() == Some("1");
                if !accepted {
                    bail!(
                        "Last.fm did not accept this scrobble; inspect its timestamp and metadata"
                    )
                }
            }
            Ok(())
        })();
        match result {
            Ok(()) => {
                db.execute(
                    "UPDATE scrobbles SET status='sent',error='' WHERE listen_id=? AND service=?",
                    params![id, service],
                )?;
                sent += 1;
            }
            Err(error) => {
                let next = chrono::Utc::now().timestamp()
                    + (30 * 2i64.pow((attempts as u32).min(10))).min(86400);
                db.execute("UPDATE scrobbles SET attempts=attempts+1,next_attempt=?,error=? WHERE listen_id=? AND service=?",params![next,format!("{error:#}"),id,service])?;
            }
        }
    }
    Ok(sent)
}

pub fn scrobble_status(library: &Library) -> Result<Vec<(String, String, i64)>> {
    let db = library.connection()?;
    let mut statement =
        db.prepare("SELECT service,status,count(*) FROM scrobbles GROUP BY service,status")?;
    Ok(statement
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
        .collect::<std::result::Result<_, _>>()?)
}
