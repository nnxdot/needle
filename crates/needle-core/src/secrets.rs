//! Account secrets live in the operating system's credential store, never in the library database.
use anyhow::{Result, bail};
use std::{collections::HashMap, sync::Mutex};

/// Windows Credential Manager entries appear as `<account>.nnx.Needle`.
pub const SERVICE_NAME: &str = "nnx.Needle";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SecretKind {
    LastfmApiKey,
    LastfmSecret,
    LastfmSession,
    /// Stored beside the session so status can name the account; not itself secret.
    LastfmUser,
    ListenbrainzToken,
    ListenbrainzUser,
    AcoustidKey,
}
impl SecretKind {
    pub const ALL: [SecretKind; 7] = [
        Self::LastfmApiKey,
        Self::LastfmSecret,
        Self::LastfmSession,
        Self::LastfmUser,
        Self::ListenbrainzToken,
        Self::ListenbrainzUser,
        Self::AcoustidKey,
    ];
    pub fn account(self) -> &'static str {
        match self {
            Self::LastfmApiKey => "lastfm-api-key",
            Self::LastfmSecret => "lastfm-secret",
            Self::LastfmSession => "lastfm-session",
            Self::LastfmUser => "lastfm-user",
            Self::ListenbrainzToken => "listenbrainz-token",
            Self::ListenbrainzUser => "listenbrainz-user",
            Self::AcoustidKey => "acoustid-key",
        }
    }
    /// Environment variables override stored values when set and nonempty.
    pub fn env_var(self) -> Option<&'static str> {
        match self {
            Self::LastfmApiKey => Some("NEEDLE_LASTFM_API_KEY"),
            Self::LastfmSecret => Some("NEEDLE_LASTFM_SECRET"),
            Self::LastfmSession => Some("NEEDLE_LASTFM_SESSION"),
            Self::ListenbrainzToken => Some("NEEDLE_LISTENBRAINZ_TOKEN"),
            Self::AcoustidKey => Some("NEEDLE_ACOUSTID_API_KEY"),
            Self::LastfmUser | Self::ListenbrainzUser => None,
        }
    }
    /// Optional defaults compiled into a distributed build.
    pub fn built_in(self) -> Option<&'static str> {
        match self {
            Self::LastfmApiKey => option_env!("NEEDLE_LASTFM_API_KEY"),
            Self::LastfmSecret => option_env!("NEEDLE_LASTFM_SECRET"),
            _ => None,
        }
        .filter(|s| !s.trim().is_empty())
    }
}

pub trait SecretStore: Send + Sync {
    fn get(&self, kind: SecretKind) -> Result<Option<String>>;
    fn set(&self, kind: SecretKind, value: &str) -> Result<()>;
    /// Deleting an absent entry succeeds.
    fn delete(&self, kind: SecretKind) -> Result<()>;
}

/// The Windows Credential Manager. Other platforms report that no secure store is available.
pub struct SystemStore;
#[cfg(windows)]
impl SystemStore {
    fn entry(kind: SecretKind) -> Result<keyring::Entry> {
        keyring::Entry::new(SERVICE_NAME, kind.account())
            .map_err(|e| anyhow::anyhow!("Credential Manager: {e}"))
    }
}
#[cfg(windows)]
impl SecretStore for SystemStore {
    fn get(&self, kind: SecretKind) -> Result<Option<String>> {
        match Self::entry(kind)?.get_password() {
            Ok(value) => Ok(Some(value).filter(|v| !v.is_empty())),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => bail!("Credential Manager could not read {}: {e}", kind.account()),
        }
    }
    fn set(&self, kind: SecretKind, value: &str) -> Result<()> {
        Self::entry(kind)?.set_password(value).map_err(|e| {
            anyhow::anyhow!("Credential Manager could not save {}: {e}", kind.account())
        })
    }
    fn delete(&self, kind: SecretKind) -> Result<()> {
        match Self::entry(kind)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => bail!(
                "Credential Manager could not remove {}: {e}",
                kind.account()
            ),
        }
    }
}
#[cfg(not(windows))]
impl SecretStore for SystemStore {
    fn get(&self, _: SecretKind) -> Result<Option<String>> {
        Ok(None)
    }
    fn set(&self, _: SecretKind, _: &str) -> Result<()> {
        bail!("No secure credential store is supported on this platform; use environment variables")
    }
    fn delete(&self, _: SecretKind) -> Result<()> {
        Ok(())
    }
}

/// An in-process store for tests and embedding.
#[derive(Default)]
pub struct MemoryStore(Mutex<HashMap<SecretKind, String>>);
impl SecretStore for MemoryStore {
    fn get(&self, kind: SecretKind) -> Result<Option<String>> {
        Ok(self.0.lock().unwrap().get(&kind).cloned())
    }
    fn set(&self, kind: SecretKind, value: &str) -> Result<()> {
        self.0.lock().unwrap().insert(kind, value.into());
        Ok(())
    }
    fn delete(&self, kind: SecretKind) -> Result<()> {
        self.0.lock().unwrap().remove(&kind);
        Ok(())
    }
}

/// Replace every nonempty secret in `text` so it can be shown or stored safely.
pub fn redact(text: &str, secrets: &[&str]) -> String {
    let mut text = text.to_string();
    for secret in secrets.iter().filter(|s| s.len() >= 4) {
        text = text.replace(secret, "[redacted]");
    }
    text
}
