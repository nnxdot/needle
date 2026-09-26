//! Account secrets live in the operating system's credential store, never in the library database.
use anyhow::{Result, bail};
use std::{collections::HashMap, sync::Mutex};

/// Windows Credential Manager entries appear as `<account>.nnx.Needle`; on Linux the
/// Secret Service item has the service `nnx.Needle` and the account as its user.
pub const SERVICE_NAME: &str = "nnx.Needle";

/// Where secrets are kept, as people know it.
pub const STORE_NAME: &str = if cfg!(windows) {
    "Windows Credential Manager"
} else if cfg!(target_os = "macos") {
    "the macOS Keychain"
} else {
    "GNOME Keyring or KWallet"
};

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

/// Windows Credential Manager, or on Linux the Secret Service (GNOME Keyring, KWallet). Other
/// platforms report that no secure store is available.
pub struct SystemStore;
#[cfg(any(windows, target_os = "linux", target_os = "macos"))]
impl SystemStore {
    fn entry(kind: SecretKind) -> Result<keyring::Entry> {
        keyring::Entry::new(SERVICE_NAME, kind.account())
            .map_err(|e| anyhow::anyhow!("{STORE_NAME}: {e}"))
    }
}
#[cfg(any(windows, target_os = "linux", target_os = "macos"))]
impl SecretStore for SystemStore {
    fn get(&self, kind: SecretKind) -> Result<Option<String>> {
        match Self::entry(kind)?.get_password() {
            Ok(value) => Ok(Some(value).filter(|v| !v.is_empty())),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => bail!("{STORE_NAME} could not read {}: {e}", kind.account()),
        }
    }
    fn set(&self, kind: SecretKind, value: &str) -> Result<()> {
        Self::entry(kind)?
            .set_password(value)
            .map_err(|e| anyhow::anyhow!("{STORE_NAME} could not save {}: {e}", kind.account()))
    }
    fn delete(&self, kind: SecretKind) -> Result<()> {
        match Self::entry(kind)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => bail!("{STORE_NAME} could not remove {}: {e}", kind.account()),
        }
    }
}
#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
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

/// Secrets a plugin keeps (a source's password, for example): in the system store (see
/// [`STORE_NAME`]) as `plugin:<id>:<key>`, and in memory in tests.
pub mod plugin {
    use anyhow::Result;

    #[cfg(all(
        not(test),
        not(windows),
        not(target_os = "linux"),
        not(target_os = "macos")
    ))]
    fn memory() -> &'static std::sync::Mutex<std::collections::HashMap<String, String>> {
        static STORE: std::sync::LazyLock<
            std::sync::Mutex<std::collections::HashMap<String, String>>,
        > = std::sync::LazyLock::new(Default::default);
        &STORE
    }

    /// In tests, one store per thread: each plugin host runs its plugins on its own thread, so
    /// tests running side by side do not share (or sign out) each other's passwords.
    #[cfg(test)]
    fn memory() -> &'static std::sync::Mutex<std::collections::HashMap<String, String>> {
        thread_local! {
            static STORE: &'static std::sync::Mutex<std::collections::HashMap<String, String>> =
                Box::leak(Box::default());
        }
        STORE.with(|store| *store)
    }

    fn account(plugin: &str, key: &str) -> String {
        format!("plugin:{plugin}:{key}")
    }

    pub fn get(plugin: &str, key: &str) -> Result<Option<String>> {
        #[cfg(all(any(windows, target_os = "linux", target_os = "macos"), not(test)))]
        {
            let entry = keyring::Entry::new(super::SERVICE_NAME, &account(plugin, key))
                .map_err(|e| anyhow::anyhow!("{}: {e}", super::STORE_NAME))?;
            match entry.get_password() {
                Ok(value) => Ok(Some(value).filter(|v| !v.is_empty())),
                Err(keyring::Error::NoEntry) => Ok(None),
                Err(e) => {
                    anyhow::bail!("{} could not read a plugin secret: {e}", super::STORE_NAME)
                }
            }
        }
        #[cfg(any(test, not(any(windows, target_os = "linux", target_os = "macos"))))]
        Ok(memory().lock().unwrap().get(&account(plugin, key)).cloned())
    }

    pub fn set(plugin: &str, key: &str, value: &str) -> Result<()> {
        #[cfg(all(any(windows, target_os = "linux", target_os = "macos"), not(test)))]
        {
            keyring::Entry::new(super::SERVICE_NAME, &account(plugin, key))
                .and_then(|e| e.set_password(value))
                .map_err(|e| {
                    anyhow::anyhow!("{} could not save a plugin secret: {e}", super::STORE_NAME)
                })
        }
        #[cfg(any(test, not(any(windows, target_os = "linux", target_os = "macos"))))]
        {
            memory()
                .lock()
                .unwrap()
                .insert(account(plugin, key), value.into());
            Ok(())
        }
    }

    pub fn delete(plugin: &str, key: &str) -> Result<()> {
        #[cfg(all(any(windows, target_os = "linux", target_os = "macos"), not(test)))]
        {
            let entry = keyring::Entry::new(super::SERVICE_NAME, &account(plugin, key))
                .map_err(|e| anyhow::anyhow!("{}: {e}", super::STORE_NAME))?;
            match entry.delete_credential() {
                Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
                Err(e) => anyhow::bail!(
                    "{} could not remove a plugin secret: {e}",
                    super::STORE_NAME
                ),
            }
        }
        #[cfg(any(test, not(any(windows, target_os = "linux", target_os = "macos"))))]
        {
            memory().lock().unwrap().remove(&account(plugin, key));
            Ok(())
        }
    }
}
