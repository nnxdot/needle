use crate::database::Library;
pub use crate::secrets::{MemoryStore, SecretKind, SecretStore, SystemStore, redact};
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
pub(crate) fn client() -> Result<Client> {
    Ok(Client::builder()
        .timeout(Duration::from_secs(20))
        .user_agent(std::env::var("NEEDLE_HTTP_USER_AGENT").unwrap_or_else(|_| {
            concat!(
                "Needle/",
                env!("CARGO_PKG_VERSION"),
                " (local desktop music player)"
            )
            .into()
        }))
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

/// A MusicBrainz web-service GET that honours the one-request-per-second limit shared by all callers.
pub(crate) fn musicbrainz_get(path: &str, query: &[(&str, &str)]) -> Result<Value> {
    let mut last = MB_LAST.lock().unwrap();
    if let Some(time) = *last {
        let elapsed = time.elapsed();
        if elapsed < Duration::from_secs(1) {
            std::thread::sleep(Duration::from_secs(1) - elapsed);
        }
    }
    *last = Some(Instant::now());
    Ok(client()?
        .get(format!("https://musicbrainz.org/ws/2/{path}"))
        .query(query)
        .query(&[("fmt", "json")])
        .send()?
        .error_for_status()?
        .json()?)
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
        bail!("Add an AcoustID API key in Settings (or set NEEDLE_ACOUSTID_API_KEY) first");
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
    let response = client()?
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
        .send()
        .map_err(transport)?;
    drop(last);
    let (status, response) = read_json(response)?;
    let response = response.unwrap_or_default();
    if response["status"].as_str() != Some("ok") {
        if response["error"]["code"].as_i64() == Some(4) {
            return Err(ServiceError::SignInRequired(
                "AcoustID rejected the API key; enter a valid key in Settings".into(),
            )
            .into());
        }
        let message = response["error"]["message"].as_str();
        bail!(
            "AcoustID: {}",
            redact(
                &message.map_or_else(|| format!("HTTP {status}"), String::from),
                &[api_key]
            )
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
                let _ = flush_scrobbles(&library, &Credentials::load());
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

/// Credentials are supplied at runtime, never embedded in the library database or exports.
/// `Debug` output is redacted.
#[derive(Clone, Default)]
pub struct Credentials {
    pub lastfm_api_key: String,
    pub lastfm_secret: String,
    pub lastfm_session: String,
    pub listenbrainz_token: String,
    pub lastfm_user: String,
    pub listenbrainz_user: String,
    pub acoustid_key: String,
}
impl std::fmt::Debug for Credentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let set = |s: &str| if s.is_empty() { "unset" } else { "[redacted]" };
        f.debug_struct("Credentials")
            .field("lastfm_api_key", &set(&self.lastfm_api_key))
            .field("lastfm_secret", &set(&self.lastfm_secret))
            .field("lastfm_session", &set(&self.lastfm_session))
            .field("listenbrainz_token", &set(&self.listenbrainz_token))
            .field("lastfm_user", &self.lastfm_user)
            .field("listenbrainz_user", &self.listenbrainz_user)
            .field("acoustid_key", &set(&self.acoustid_key))
            .finish()
    }
}
impl Credentials {
    /// Environment variables only. Prefer [`Credentials::load`].
    pub fn from_environment() -> Self {
        let var = |name| std::env::var(name).unwrap_or_default();
        Self {
            lastfm_api_key: var("NEEDLE_LASTFM_API_KEY"),
            lastfm_secret: var("NEEDLE_LASTFM_SECRET"),
            lastfm_session: var("NEEDLE_LASTFM_SESSION"),
            listenbrainz_token: var("NEEDLE_LISTENBRAINZ_TOKEN"),
            acoustid_key: var("NEEDLE_ACOUSTID_API_KEY"),
            ..Self::default()
        }
    }
    /// Nonempty environment variables, then Windows Credential Manager, then build-time defaults.
    pub fn load() -> Self {
        Self::load_from(&SystemStore, |name| std::env::var(name).ok())
    }
    pub fn load_from(store: &dyn SecretStore, env: impl Fn(&str) -> Option<String>) -> Self {
        let get = |kind| {
            resolve(store, &env, kind)
                .map(|(value, _)| value)
                .unwrap_or_default()
        };
        Self {
            lastfm_api_key: get(SecretKind::LastfmApiKey),
            lastfm_secret: get(SecretKind::LastfmSecret),
            lastfm_session: get(SecretKind::LastfmSession),
            listenbrainz_token: get(SecretKind::ListenbrainzToken),
            lastfm_user: get(SecretKind::LastfmUser),
            listenbrainz_user: get(SecretKind::ListenbrainzUser),
            acoustid_key: get(SecretKind::AcoustidKey),
        }
    }
    fn secrets(&self) -> [&str; 5] {
        [
            &self.lastfm_api_key,
            &self.lastfm_secret,
            &self.lastfm_session,
            &self.listenbrainz_token,
            &self.acoustid_key,
        ]
    }
    fn fingerprint(&self, service: &str) -> blake3::Hash {
        let mut hasher = blake3::Hasher::new();
        let parts = if service == "lastfm" {
            vec![
                &self.lastfm_api_key,
                &self.lastfm_secret,
                &self.lastfm_session,
            ]
        } else {
            vec![&self.listenbrainz_token]
        };
        for part in parts {
            hasher.update(part.as_bytes());
            hasher.update(&[0]);
        }
        hasher.finalize()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum SecretSource {
    Environment,
    CredentialManager,
    BuiltIn,
}
fn resolve(
    store: &dyn SecretStore,
    env: &dyn Fn(&str) -> Option<String>,
    kind: SecretKind,
) -> Option<(String, SecretSource)> {
    if let Some(value) = kind
        .env_var()
        .and_then(env)
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
    {
        return Some((value, SecretSource::Environment));
    }
    if let Ok(Some(value)) = store.get(kind) {
        return Some((value, SecretSource::CredentialManager));
    }
    kind.built_in()
        .map(|value| (value.to_string(), SecretSource::BuiltIn))
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct ServiceState {
    pub configured: bool,
    /// Where the effective secret came from. Environment values cannot be signed out from the app.
    pub source: Option<SecretSource>,
    pub user: Option<String>,
    /// Set when the service rejected the current credential during this session.
    pub rejected: Option<String>,
}
#[derive(Clone, Debug, Default, Serialize)]
pub struct ServiceStatus {
    /// A usable Last.fm session: application key and secret plus a session key.
    pub lastfm: ServiceState,
    /// The Last.fm application API key and shared secret used to sign in.
    pub lastfm_app: ServiceState,
    pub listenbrainz: ServiceState,
    pub acoustid: ServiceState,
    /// Credential Manager could not be read; stored secrets are unavailable.
    pub store_error: Option<String>,
}
pub fn secret_status() -> ServiceStatus {
    secret_status_from(&SystemStore, |name| std::env::var(name).ok())
}
pub fn secret_status_from(
    store: &dyn SecretStore,
    env: impl Fn(&str) -> Option<String>,
) -> ServiceStatus {
    let credentials = Credentials::load_from(store, &env);
    let source = |kind| resolve(store, &env, kind).map(|(_, source)| source);
    let user = |kind| store.get(kind).ok().flatten();
    let key = source(SecretKind::LastfmApiKey);
    let app = key.is_some() && source(SecretKind::LastfmSecret).is_some();
    let session = source(SecretKind::LastfmSession);
    let token = source(SecretKind::ListenbrainzToken);
    let acoustid = source(SecretKind::AcoustidKey);
    ServiceStatus {
        lastfm: ServiceState {
            configured: app && session.is_some(),
            source: session,
            user: user(SecretKind::LastfmUser),
            rejected: rejected("lastfm", &credentials),
        },
        lastfm_app: ServiceState {
            configured: app,
            source: key,
            ..ServiceState::default()
        },
        listenbrainz: ServiceState {
            configured: token.is_some(),
            source: token,
            user: user(SecretKind::ListenbrainzUser),
            rejected: rejected("listenbrainz", &credentials),
        },
        acoustid: ServiceState {
            configured: acoustid.is_some(),
            source: acoustid,
            ..ServiceState::default()
        },
        store_error: store
            .get(SecretKind::LastfmSession)
            .err()
            .map(|e| format!("{e:#}")),
    }
}

/// Save a secret in Windows Credential Manager. An environment variable of the same kind still wins.
pub fn save_secret(kind: SecretKind, value: &str) -> Result<()> {
    save_secret_in(&SystemStore, kind, value)?;
    forget_rejections();
    Ok(())
}
fn save_secret_in(store: &dyn SecretStore, kind: SecretKind, value: &str) -> Result<()> {
    let value = value.trim();
    if value.is_empty() {
        bail!("Enter a value before saving");
    }
    store.set(kind, value)
}
pub fn clear_secret(kind: SecretKind) -> Result<()> {
    SystemStore.delete(kind)?;
    forget_rejections();
    Ok(())
}
/// Remove the Last.fm session and user name. The application key and secret remain.
pub fn lastfm_sign_out() -> Result<()> {
    clear_secret(SecretKind::LastfmSession)?;
    clear_secret(SecretKind::LastfmUser)
}
pub fn listenbrainz_sign_out() -> Result<()> {
    clear_secret(SecretKind::ListenbrainzToken)?;
    clear_secret(SecretKind::ListenbrainzUser)
}
/// The effective AcoustID API key (environment, then Credential Manager).
pub fn acoustid_key() -> Option<String> {
    Some(Credentials::load().acoustid_key).filter(|k| !k.is_empty())
}

/// Service failures the UI can recognise with `error.downcast_ref::<ServiceError>()`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ServiceError {
    /// The Last.fm token has not been approved in the browser yet.
    NotYetAuthorized,
    /// The Last.fm token expired or was used; start sign-in again.
    TokenExpired,
    /// The Last.fm application API key or shared secret was rejected.
    InvalidApplication(String),
    /// The stored session, token, or key was rejected; sign in again.
    SignInRequired(String),
    Api {
        code: i64,
        message: String,
    },
}
impl std::fmt::Display for ServiceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotYetAuthorized => {
                write!(
                    f,
                    "Approve Needle on Last.fm in your browser, then continue"
                )
            }
            Self::TokenExpired => write!(f, "The Last.fm sign-in request expired; start again"),
            Self::InvalidApplication(m) | Self::SignInRequired(m) => write!(f, "{m}"),
            Self::Api { code, message } => write!(f, "{message} (code {code})"),
        }
    }
}
impl std::error::Error for ServiceError {}
pub fn is_not_yet_authorized(error: &anyhow::Error) -> bool {
    error.downcast_ref::<ServiceError>() == Some(&ServiceError::NotYetAuthorized)
}

/// Transport errors without request URLs, which may contain keys.
fn transport(error: reqwest::Error) -> anyhow::Error {
    anyhow::anyhow!("{}", error.without_url())
}
fn read_json(response: reqwest::blocking::Response) -> Result<(u16, Option<Value>)> {
    let status = response.status().as_u16();
    let text = response.text().map_err(transport)?;
    Ok((status, serde_json::from_str(&text).ok()))
}

fn lastfm_error(code: i64, message: &str) -> ServiceError {
    match code {
        14 => ServiceError::NotYetAuthorized,
        4 | 15 => ServiceError::TokenExpired,
        10 | 13 | 26 => ServiceError::InvalidApplication(format!(
            "Last.fm rejected the application API key or shared secret: {message}"
        )),
        9 => ServiceError::SignInRequired("Last.fm rejected the session; sign in again".into()),
        _ => ServiceError::Api {
            code,
            message: format!("Last.fm: {message}"),
        },
    }
}
fn lastfm_checked(status: u16, body: Option<Value>) -> Result<Value> {
    let Some(body) = body else {
        bail!("Last.fm returned HTTP {status} without a readable response")
    };
    if let Some(code) = body["error"].as_i64() {
        let message = body["message"].as_str().unwrap_or("request rejected");
        return Err(lastfm_error(code, message).into());
    }
    if status >= 400 {
        bail!("Last.fm returned HTTP {status}");
    }
    Ok(body)
}
fn lastfm_post(fields: BTreeMap<String, String>, secret: &str) -> Result<(u16, Option<Value>)> {
    let secrets = [fields["api_key"].clone(), secret.to_string()];
    let redacted = |e: anyhow::Error| {
        let secrets = secrets.iter().map(String::as_str).collect::<Vec<_>>();
        anyhow::anyhow!("{}", redact(&format!("{e:#}"), &secrets))
    };
    let response = client()?
        .post("https://ws.audioscrobbler.com/2.0/")
        .form(&signed(fields, secret))
        .send()
        .map_err(transport)
        .map_err(redacted)?;
    read_json(response).map_err(redacted)
}

/// A Last.fm sign-in waiting for browser approval. Tokens expire after 60 minutes.
#[derive(Clone)]
pub struct LastfmPending {
    pub token: String,
    /// Open this in the user's browser.
    pub auth_url: String,
    api_key: String,
    secret: String,
}
impl std::fmt::Debug for LastfmPending {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LastfmPending").finish_non_exhaustive()
    }
}
fn parse_lastfm_token(body: &Value) -> Result<String> {
    Ok(body["token"]
        .as_str()
        .filter(|t| !t.is_empty())
        .context("Last.fm did not return an authorization token")?
        .into())
}
fn parse_lastfm_session(body: &Value) -> Result<(String, String)> {
    let key = body["session"]["key"]
        .as_str()
        .filter(|k| !k.is_empty())
        .context("Last.fm did not return a session")?;
    Ok((
        key.into(),
        body["session"]["name"].as_str().unwrap_or_default().into(),
    ))
}
/// Step 1: request a token (auth.getToken) and build the browser approval URL.
pub fn lastfm_begin(api_key: &str, secret: &str) -> Result<LastfmPending> {
    let (api_key, secret) = (api_key.trim(), secret.trim());
    if api_key.is_empty() || secret.is_empty() {
        bail!("Enter the Last.fm application API key and shared secret first");
    }
    let fields = BTreeMap::from([
        ("method".into(), "auth.getToken".into()),
        ("api_key".into(), api_key.into()),
    ]);
    let (status, body) = lastfm_post(fields, secret)?;
    let token = parse_lastfm_token(&lastfm_checked(status, body)?)?;
    Ok(LastfmPending {
        auth_url: format!("https://www.last.fm/api/auth/?api_key={api_key}&token={token}"),
        token,
        api_key: api_key.into(),
        secret: secret.into(),
    })
}
/// Step 2: after browser approval, exchange the token (auth.getSession) and save the session.
/// Returns [`ServiceError::NotYetAuthorized`] until the user approves; call again after that.
pub fn lastfm_complete(pending: &LastfmPending) -> Result<String> {
    let fields = BTreeMap::from([
        ("method".into(), "auth.getSession".into()),
        ("api_key".into(), pending.api_key.clone()),
        ("token".into(), pending.token.clone()),
    ]);
    let (status, body) = lastfm_post(fields, &pending.secret)?;
    let user = finish_lastfm(
        &SystemStore,
        |name| std::env::var(name).ok(),
        pending,
        lastfm_checked(status, body)?,
    )?;
    forget_rejections();
    Ok(user)
}
fn finish_lastfm(
    store: &dyn SecretStore,
    env: impl Fn(&str) -> Option<String>,
    pending: &LastfmPending,
    body: Value,
) -> Result<String> {
    let (session, user) = parse_lastfm_session(&body)?;
    // The session belongs to the application key that created it; remember user-entered keys.
    let current = Credentials::load_from(store, env);
    if current.lastfm_api_key != pending.api_key || current.lastfm_secret != pending.secret {
        save_secret_in(store, SecretKind::LastfmApiKey, &pending.api_key)?;
        save_secret_in(store, SecretKind::LastfmSecret, &pending.secret)?;
    }
    save_secret_in(store, SecretKind::LastfmSession, &session)?;
    if user.is_empty() {
        store.delete(SecretKind::LastfmUser)?;
    } else {
        save_secret_in(store, SecretKind::LastfmUser, &user)?;
    }
    Ok(user)
}
/// Older two-step API; prefer [`lastfm_begin`].
pub fn lastfm_authorize(credentials: &Credentials) -> Result<String> {
    Ok(lastfm_begin(&credentials.lastfm_api_key, &credentials.lastfm_secret)?.token)
}
/// Older two-step API; prefer [`lastfm_complete`]. Returns the session key without saving it.
pub fn lastfm_session(credentials: &Credentials, token: &str) -> Result<String> {
    let fields = BTreeMap::from([
        ("method".into(), "auth.getSession".into()),
        ("api_key".into(), credentials.lastfm_api_key.clone()),
        ("token".into(), token.into()),
    ]);
    let (status, body) = lastfm_post(fields, &credentials.lastfm_secret)?;
    Ok(parse_lastfm_session(&lastfm_checked(status, body)?)?.0)
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

fn parse_listenbrainz_validation(status: u16, body: Option<&Value>) -> Result<String> {
    let rejected = || ServiceError::SignInRequired("ListenBrainz rejected this token".into());
    match body {
        Some(body) if status < 400 => {
            if body["valid"].as_bool() == Some(true) {
                Ok(body["user_name"].as_str().unwrap_or_default().into())
            } else {
                Err(rejected().into())
            }
        }
        _ if status == 401 => Err(rejected().into()),
        _ => bail!("ListenBrainz returned HTTP {status}"),
    }
}
/// Check a user token (GET /1/validate-token) and return its user name. Nothing is saved.
pub fn listenbrainz_validate(token: &str) -> Result<String> {
    let token = token.trim();
    if token.is_empty() {
        bail!("Paste your ListenBrainz user token first");
    }
    let response = client()?
        .get("https://api.listenbrainz.org/1/validate-token")
        .header("Authorization", format!("Token {token}"))
        .send()
        .map_err(transport)?;
    let (status, body) = read_json(response)?;
    parse_listenbrainz_validation(status, body.as_ref())
}
/// Validate, then save the token and user name in Credential Manager.
pub fn listenbrainz_sign_in(token: &str) -> Result<String> {
    let user = listenbrainz_validate(token)?;
    save_secret(SecretKind::ListenbrainzToken, token)?;
    if user.is_empty() {
        clear_secret(SecretKind::ListenbrainzUser)?;
    } else {
        save_secret(SecretKind::ListenbrainzUser, &user)?;
    }
    Ok(user)
}

/// Credentials rejected during this process, by service and fingerprint. Never persisted.
static REJECTED: Mutex<Vec<(String, blake3::Hash, String)>> = Mutex::new(Vec::new());
fn rejected(service: &str, credentials: &Credentials) -> Option<String> {
    let fingerprint = credentials.fingerprint(service);
    REJECTED
        .lock()
        .unwrap()
        .iter()
        .find(|(s, f, _)| s == service && *f == fingerprint)
        .map(|(_, _, message)| message.clone())
}
fn reject(service: &str, credentials: &Credentials, message: &str) {
    let mut list = REJECTED.lock().unwrap();
    list.retain(|(s, _, _)| s != service);
    list.push((
        service.into(),
        credentials.fingerprint(service),
        message.into(),
    ));
}
fn forget_rejections() {
    REJECTED.lock().unwrap().clear();
}

#[derive(Debug, PartialEq)]
enum Outcome {
    Sent,
    /// Temporary; retry with backoff.
    Retry(String),
    /// This listen can never be accepted.
    Failed(String),
    /// The credential is bad; stop submitting to this service until the user signs in again.
    SignIn(String),
}
fn lastfm_outcome(status: u16, body: Option<&Value>) -> Outcome {
    let Some(body) = body else {
        return Outcome::Retry(format!("Last.fm returned HTTP {status}"));
    };
    if let Some(code) = body["error"].as_i64() {
        let message = body["message"].as_str().unwrap_or("request rejected");
        return match code {
            4 | 9 | 14 | 15 => {
                Outcome::SignIn("Last.fm rejected the session; sign in again".into())
            }
            10 | 13 | 26 => Outcome::SignIn(lastfm_error(code, message).to_string()),
            6 | 7 => Outcome::Failed(format!("Last.fm: {message} (code {code})")),
            _ => Outcome::Retry(format!("Last.fm: {message} (code {code})")),
        };
    }
    if status >= 400 {
        return Outcome::Retry(format!("Last.fm returned HTTP {status}"));
    }
    let number = |v: &Value| v.as_i64().or_else(|| v.as_str()?.parse().ok());
    if number(&body["scrobbles"]["@attr"]["accepted"]) == Some(1) {
        return Outcome::Sent;
    }
    let scrobble = &body["scrobbles"]["scrobble"];
    let ignored = if scrobble.is_array() {
        &scrobble[0]["ignoredMessage"]
    } else {
        &scrobble["ignoredMessage"]
    };
    let text = ignored["#text"].as_str().unwrap_or_default();
    match number(&ignored["code"]) {
        Some(5) => Outcome::Retry("Last.fm daily scrobble limit reached".into()),
        Some(code) if code > 0 => Outcome::Failed(
            format!("Last.fm ignored this scrobble (code {code}) {text}")
                .trim()
                .into(),
        ),
        _ => Outcome::Retry("Last.fm did not accept this scrobble".into()),
    }
}
fn listenbrainz_outcome(status: u16, body: Option<&Value>) -> Outcome {
    let message = body
        .and_then(|b| b["error"].as_str())
        .map(|m| format!("ListenBrainz: {m}"))
        .unwrap_or_else(|| format!("ListenBrainz returned HTTP {status}"));
    match status {
        200..=299 => Outcome::Sent,
        401 | 403 => Outcome::SignIn("ListenBrainz rejected the token; sign in again".into()),
        400 => Outcome::Failed(message),
        _ => Outcome::Retry(message),
    }
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
    let secrets = credentials.secrets();
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
            || rejected(&service, credentials).is_some()
        {
            continue;
        }
        let outcome = if listen.artist.trim().is_empty() || listen.title.trim().is_empty() {
            Outcome::Failed("Artist and title tags are required".into())
        } else {
            let response = if service == "listenbrainz" {
                client()?.post("https://api.listenbrainz.org/1/submit-listens").header("Authorization",format!("Token {}",credentials.listenbrainz_token))
                    .json(&json!({"listen_type":"single","payload":[{"listened_at":listen.started_at,"track_metadata":{"artist_name":listen.artist,"track_name":listen.title,"release_name":listen.album,"additional_info":{"submission_client":"Needle","submission_client_version":"0.1.0","duration":listen.duration as i64}}}]})).send()
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
                client()?
                    .post("https://ws.audioscrobbler.com/2.0/")
                    .form(&signed(fields, &credentials.lastfm_secret))
                    .send()
            };
            match response.map_err(transport).and_then(read_json) {
                Err(error) => Outcome::Retry(format!("{error:#}")),
                Ok((status, body)) if service == "listenbrainz" => {
                    listenbrainz_outcome(status, body.as_ref())
                }
                Ok((status, body)) => lastfm_outcome(status, body.as_ref()),
            }
        };
        match outcome {
            Outcome::Sent => {
                db.execute(
                    "UPDATE scrobbles SET status='sent',error='' WHERE listen_id=? AND service=?",
                    params![id, service],
                )?;
                sent += 1;
            }
            Outcome::Retry(error) => {
                let next = chrono::Utc::now().timestamp()
                    + (30 * 2i64.pow((attempts as u32).min(10))).min(86400);
                db.execute("UPDATE scrobbles SET attempts=attempts+1,next_attempt=?,error=? WHERE listen_id=? AND service=?",params![next,redact(&error,&secrets),id,service])?;
            }
            Outcome::Failed(error) => {
                db.execute("UPDATE scrobbles SET status='failed',attempts=attempts+1,error=? WHERE listen_id=? AND service=?",params![redact(&error,&secrets),id,service])?;
            }
            Outcome::SignIn(error) => {
                let error = redact(&error, &secrets);
                reject(&service, credentials, &error);
                db.execute(
                    "UPDATE scrobbles SET error=? WHERE listen_id=? AND service=?",
                    params![error, id, service],
                )?;
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

#[derive(Clone, Debug, Default, Serialize)]
pub struct QueueSummary {
    pub pending: i64,
    pub sent: i64,
    /// Permanently rejected listens; they are not retried unless the user asks.
    pub failed: i64,
    pub last_error: Option<String>,
    /// Earliest Unix time a backed-off pending listen will be retried.
    pub next_retry: Option<i64>,
    /// The service rejected the current credential; the queue waits until the user signs in again.
    pub sign_in_required: Option<String>,
}
#[derive(Clone, Debug, Default, Serialize)]
pub struct ScrobbleSummary {
    pub lastfm: QueueSummary,
    pub listenbrainz: QueueSummary,
}
pub fn scrobble_summary(library: &Library) -> Result<ScrobbleSummary> {
    scrobble_summary_with(library, &Credentials::load())
}
fn scrobble_summary_with(library: &Library, credentials: &Credentials) -> Result<ScrobbleSummary> {
    let db = library.connection()?;
    let queue = |service: &str| -> Result<QueueSummary> {
        let count = |status: &str| -> Result<i64> {
            Ok(db.query_row(
                "SELECT count(*) FROM scrobbles WHERE service=? AND status=?",
                params![service, status],
                |r| r.get(0),
            )?)
        };
        Ok(QueueSummary {
            pending: count("pending")?,
            sent: count("sent")?,
            failed: count("failed")?,
            last_error: db
                .query_row(
                    "SELECT error FROM scrobbles WHERE service=? AND status!='sent' AND error!='' ORDER BY next_attempt DESC LIMIT 1",
                    [service],
                    |r| r.get(0),
                )
                .optional()?,
            next_retry: db.query_row(
                "SELECT min(next_attempt) FROM scrobbles WHERE service=? AND status='pending' AND attempts>0",
                [service],
                |r| r.get(0),
            )?,
            sign_in_required: rejected(service, credentials),
        })
    };
    Ok(ScrobbleSummary {
        lastfm: queue("lastfm")?,
        listenbrainz: queue("listenbrainz")?,
    })
}
/// Return permanently failed listens to the queue, for one service or all.
pub fn retry_failed_scrobbles(library: &Library, service: Option<&str>) -> Result<usize> {
    Ok(library.connection()?.execute(
        "UPDATE scrobbles SET status='pending',next_attempt=0 WHERE status='failed' AND (?1 IS NULL OR service=?1)",
        params![service],
    )?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pending() -> LastfmPending {
        LastfmPending {
            token: "pending-token-777".into(),
            auth_url: String::new(),
            api_key: "user-key-1234".into(),
            secret: "user-secret-5678".into(),
        }
    }

    #[test]
    fn lastfm_signature_matches_reference() {
        let fields = BTreeMap::from([
            ("method".into(), "auth.getSession".into()),
            ("api_key".into(), "abc".into()),
            ("token".into(), "xyz".into()),
        ]);
        let signed_fields = signed(fields, "s3cret");
        assert_eq!(signed_fields["api_sig"], "4bb17654033fa2d5b549adac21d25520");
        assert_eq!(signed_fields["format"], "json");
        let fields = BTreeMap::from([
            ("method".into(), "track.scrobble".into()),
            ("api_key".into(), "abc".into()),
            ("sk".into(), "sk1".into()),
            ("artist".into(), "Björk".into()),
            ("track".into(), "Jóga".into()),
            ("timestamp".into(), "1700000000".into()),
        ]);
        assert_eq!(
            signed(fields, "s3cret")["api_sig"],
            "23b39928ad1ac1db9c502baf51996fc4"
        );
    }

    #[test]
    fn parses_lastfm_auth_responses() {
        let token = json!({"token": "cf45fe5a3e3cebe168480a086d7fe481"});
        assert_eq!(
            parse_lastfm_token(&token).unwrap(),
            "cf45fe5a3e3cebe168480a086d7fe481"
        );
        let session: Value = serde_json::from_str(
            r#"{"session":{"subscriber":0,"name":"needle-listener","key":"d580d57f32848f5dcf574d1ce18d78b2"}}"#,
        )
        .unwrap();
        assert_eq!(
            parse_lastfm_session(&lastfm_checked(200, Some(session)).unwrap()).unwrap(),
            (
                "d580d57f32848f5dcf574d1ce18d78b2".into(),
                "needle-listener".into()
            )
        );
        let error = |json: &str, status| {
            lastfm_checked(status, serde_json::from_str(json).ok())
                .unwrap_err()
                .downcast::<ServiceError>()
                .unwrap()
        };
        let unauthorized =
            r#"{"error":14,"message":"Unauthorized Token - This token has not been authorized"}"#;
        assert_eq!(error(unauthorized, 403), ServiceError::NotYetAuthorized);
        assert!(is_not_yet_authorized(
            &lastfm_checked(403, serde_json::from_str(unauthorized).ok()).unwrap_err()
        ));
        assert_eq!(
            error(r#"{"error":15,"message":"This token has expired"}"#, 403),
            ServiceError::TokenExpired
        );
        let invalid_key = r#"{"error":10,"message":"Invalid API key - You must be granted a valid key by last.fm"}"#;
        assert!(matches!(
            error(invalid_key, 403),
            ServiceError::InvalidApplication(_)
        ));
        assert!(lastfm_checked(502, None).is_err());
        assert!(parse_lastfm_token(&json!({})).is_err());
    }

    #[test]
    fn parses_listenbrainz_validation() {
        let valid: Value = serde_json::from_str(
            r#"{"code":200,"message":"Token valid.","valid":true,"user_name":"needle-listener"}"#,
        )
        .unwrap();
        assert_eq!(
            parse_listenbrainz_validation(200, Some(&valid)).unwrap(),
            "needle-listener"
        );
        let invalid: Value =
            serde_json::from_str(r#"{"code":200,"message":"Token invalid.","valid":false}"#)
                .unwrap();
        let error = parse_listenbrainz_validation(200, Some(&invalid)).unwrap_err();
        assert!(matches!(
            error.downcast_ref::<ServiceError>(),
            Some(ServiceError::SignInRequired(_))
        ));
        assert!(parse_listenbrainz_validation(401, None).is_err());
        assert!(parse_listenbrainz_validation(503, None).is_err());
    }

    #[test]
    fn classifies_scrobble_responses() {
        let parse = |s: &str| serde_json::from_str::<Value>(s).unwrap();
        let accepted = parse(
            r##"{"scrobbles":{"scrobble":{"ignoredMessage":{"code":"0","#text":""}},"@attr":{"ignored":0,"accepted":1}}}"##,
        );
        assert_eq!(lastfm_outcome(200, Some(&accepted)), Outcome::Sent);
        let ignored = parse(
            r##"{"scrobbles":{"scrobble":{"ignoredMessage":{"code":"3","#text":"Timestamp too old"}},"@attr":{"ignored":1,"accepted":0}}}"##,
        );
        assert!(
            matches!(lastfm_outcome(200, Some(&ignored)), Outcome::Failed(m) if m.contains("Timestamp too old"))
        );
        let limit = parse(
            r##"{"scrobbles":{"scrobble":[{"ignoredMessage":{"code":"5","#text":""}}],"@attr":{"ignored":"1","accepted":"0"}}}"##,
        );
        assert!(matches!(
            lastfm_outcome(200, Some(&limit)),
            Outcome::Retry(_)
        ));
        let session =
            parse(r#"{"error":9,"message":"Invalid session key - Please re-authenticate"}"#);
        assert!(matches!(
            lastfm_outcome(403, Some(&session)),
            Outcome::SignIn(_)
        ));
        let offline = parse(r#"{"error":11,"message":"Service Offline"}"#);
        assert!(matches!(
            lastfm_outcome(503, Some(&offline)),
            Outcome::Retry(_)
        ));
        assert!(matches!(lastfm_outcome(502, None), Outcome::Retry(_)));
        let ok = parse(r#"{"status":"ok"}"#);
        assert_eq!(listenbrainz_outcome(200, Some(&ok)), Outcome::Sent);
        assert!(matches!(
            listenbrainz_outcome(401, None),
            Outcome::SignIn(_)
        ));
        let bad = parse(r#"{"code":400,"error":"JSON document may only contain listened_at"}"#);
        assert!(
            matches!(listenbrainz_outcome(400, Some(&bad)), Outcome::Failed(m) if m.contains("listened_at"))
        );
        assert!(matches!(listenbrainz_outcome(429, None), Outcome::Retry(_)));
    }

    #[test]
    fn environment_overrides_stored_secrets() {
        let store = MemoryStore::default();
        store
            .set(SecretKind::ListenbrainzToken, "stored-token")
            .unwrap();
        store
            .set(SecretKind::ListenbrainzUser, "stored-user")
            .unwrap();
        store
            .set(SecretKind::AcoustidKey, "stored-acoustid")
            .unwrap();
        let env = |name: &str| match name {
            "NEEDLE_LISTENBRAINZ_TOKEN" => Some("env-token".to_string()),
            "NEEDLE_ACOUSTID_API_KEY" => Some("   ".to_string()),
            _ => None,
        };
        let credentials = Credentials::load_from(&store, env);
        assert_eq!(credentials.listenbrainz_token, "env-token");
        assert_eq!(credentials.listenbrainz_user, "stored-user");
        assert_eq!(
            credentials.acoustid_key, "stored-acoustid",
            "blank environment values are ignored"
        );
        let status = secret_status_from(&store, env);
        assert_eq!(status.listenbrainz.source, Some(SecretSource::Environment));
        assert_eq!(status.listenbrainz.user.as_deref(), Some("stored-user"));
        assert_eq!(
            status.acoustid.source,
            Some(SecretSource::CredentialManager)
        );
        assert!(!status.lastfm.configured);
        let stored_only = Credentials::load_from(&store, |_| None);
        assert_eq!(stored_only.listenbrainz_token, "stored-token");
    }

    #[test]
    fn completing_lastfm_saves_session_and_user_keys() {
        let store = MemoryStore::default();
        let body = json!({"session":{"name":"needle-listener","key":"session-key-9999"}});
        let user = finish_lastfm(&store, |_| None, &pending(), body).unwrap();
        assert_eq!(user, "needle-listener");
        let credentials = Credentials::load_from(&store, |_| None);
        assert_eq!(credentials.lastfm_session, "session-key-9999");
        assert_eq!(credentials.lastfm_api_key, "user-key-1234");
        assert_eq!(credentials.lastfm_secret, "user-secret-5678");
        let status = secret_status_from(&store, |_| None);
        assert!(status.lastfm.configured);
        assert_eq!(status.lastfm.user.as_deref(), Some("needle-listener"));
        store.delete(SecretKind::LastfmSession).unwrap();
        assert!(!secret_status_from(&store, |_| None).lastfm.configured);
        assert!(finish_lastfm(&store, |_| None, &pending(), json!({"session":{}})).is_err());
    }

    #[test]
    fn secrets_are_redacted() {
        let credentials = Credentials {
            lastfm_session: "session-key-9999".into(),
            listenbrainz_token: "lb-token-abcdef".into(),
            ..Credentials::default()
        };
        let debug = format!("{credentials:?} {:?}", pending());
        assert!(!debug.contains("session-key-9999") && !debug.contains("lb-token-abcdef"));
        assert!(!debug.contains("user-secret-5678") && !debug.contains("pending-token-777"));
        let message = redact(
            "request to ?sk=session-key-9999&token=lb-token-abcdef failed",
            &credentials.secrets(),
        );
        assert_eq!(message, "request to ?sk=[redacted]&token=[redacted] failed");
    }

    #[test]
    fn rejected_credentials_pause_until_they_change() {
        let old = Credentials {
            listenbrainz_token: "rejected-token-for-test".into(),
            ..Credentials::default()
        };
        reject(
            "listenbrainz",
            &old,
            "ListenBrainz rejected the token; sign in again",
        );
        assert!(rejected("listenbrainz", &old).is_some());
        let new = Credentials {
            listenbrainz_token: "replacement-token".into(),
            ..Credentials::default()
        };
        assert!(rejected("listenbrainz", &new).is_none());
    }

    #[test]
    fn summarizes_scrobble_queue() {
        let directory = tempfile::tempdir().unwrap();
        let library = Library::open(directory.path()).unwrap();
        library
            .connection()
            .unwrap()
            .execute_batch(
                "INSERT INTO scrobbles VALUES ('a','lastfm','pending',2,4000000000,'Last.fm returned HTTP 503');
                 INSERT INTO scrobbles VALUES ('b','lastfm','sent',1,0,'');
                 INSERT INTO scrobbles VALUES ('c','lastfm','failed',1,0,'Artist and title tags are required');
                 INSERT INTO scrobbles VALUES ('d','listenbrainz','pending',0,0,'');",
            )
            .unwrap();
        let summary = scrobble_summary_with(&library, &Credentials::default()).unwrap();
        let lastfm = &summary.lastfm;
        assert_eq!((lastfm.pending, lastfm.sent, lastfm.failed), (1, 1, 1));
        assert_eq!(lastfm.next_retry, Some(4000000000));
        assert_eq!(
            lastfm.last_error.as_deref(),
            Some("Last.fm returned HTTP 503")
        );
        assert_eq!(summary.listenbrainz.pending, 1);
        assert_eq!(summary.listenbrainz.next_retry, None);
        assert_eq!(retry_failed_scrobbles(&library, Some("lastfm")).unwrap(), 1);
        let summary = scrobble_summary_with(&library, &Credentials::default()).unwrap();
        assert_eq!(summary.lastfm.pending, 2);
    }
}
