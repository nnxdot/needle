//! Account secrets live in the operating system's credential store, never in the library database.
#[cfg_attr(any(target_os = "android", target_os = "macos"), allow(unused_imports))]
use anyhow::{Result, bail};
use std::{collections::HashMap, sync::Mutex};

/// Windows Credential Manager entries appear as `<account>.nnx.Needle`; on Linux the
/// Secret Service item has the service `nnx.Needle` and the account as its user.
#[cfg_attr(
    not(any(windows, target_os = "linux", target_vendor = "apple")),
    allow(dead_code)
)]
pub const SERVICE_NAME: &str = "nnx.Needle";

/// Where secrets are kept, as people know it.
pub const STORE_NAME: &str = if cfg!(windows) {
    "Windows Credential Manager"
} else if cfg!(target_os = "macos") {
    "the macOS Keychain"
} else if cfg!(target_os = "ios") {
    "the iPhone Keychain"
} else if cfg!(target_os = "android") {
    "Needle's private app storage"
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
    /// fanart.tv's key, for artists' logos.
    FanartKey,
}
impl SecretKind {
    pub const ALL: [SecretKind; 8] = [
        Self::LastfmApiKey,
        Self::LastfmSecret,
        Self::LastfmSession,
        Self::LastfmUser,
        Self::ListenbrainzToken,
        Self::ListenbrainzUser,
        Self::AcoustidKey,
        Self::FanartKey,
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
            Self::FanartKey => "fanart-key",
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
            Self::FanartKey => Some("NEEDLE_FANART_KEY"),
            Self::LastfmUser | Self::ListenbrainzUser => None,
        }
    }
    /// Optional defaults compiled into a distributed build.
    pub fn built_in(self) -> Option<&'static str> {
        match self {
            Self::LastfmApiKey => option_env!("NEEDLE_LASTFM_API_KEY"),
            Self::LastfmSecret => option_env!("NEEDLE_LASTFM_SECRET"),
            Self::FanartKey => option_env!("NEEDLE_FANART_KEY"),
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
#[cfg(any(windows, target_os = "linux", target_vendor = "apple"))]
impl SystemStore {
    #[cfg_attr(target_os = "macos", allow(dead_code))]
    fn entry(kind: SecretKind) -> Result<keyring::Entry> {
        keyring::Entry::new(SERVICE_NAME, kind.account())
            .map_err(|e| anyhow::anyhow!("{STORE_NAME}: {e}"))
    }
}
#[cfg(any(windows, target_os = "linux", target_vendor = "apple"))]
impl SecretStore for SystemStore {
    fn get(&self, kind: SecretKind) -> Result<Option<String>> {
        if file_store::active() {
            return Ok(file_store::get(kind.account()));
        }
        #[cfg(target_os = "macos")]
        {
            keychain::get(kind.account())
        }
        #[cfg(not(target_os = "macos"))]
        match Self::entry(kind)?.get_password() {
            Ok(value) => Ok(Some(value).filter(|v| !v.is_empty())),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => bail!("{STORE_NAME} could not read {}: {e}", kind.account()),
        }
    }
    fn set(&self, kind: SecretKind, value: &str) -> Result<()> {
        if file_store::active() {
            return file_store::set(kind.account(), Some(value));
        }
        #[cfg(target_os = "macos")]
        {
            keychain::set(kind.account(), value)
        }
        #[cfg(not(target_os = "macos"))]
        Self::entry(kind)?
            .set_password(value)
            .map_err(|e| anyhow::anyhow!("{STORE_NAME} could not save {}: {e}", kind.account()))
    }
    fn delete(&self, kind: SecretKind) -> Result<()> {
        if file_store::active() {
            return file_store::set(kind.account(), None);
        }
        #[cfg(target_os = "macos")]
        {
            keychain::delete(kind.account())
        }
        #[cfg(not(target_os = "macos"))]
        match Self::entry(kind)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => bail!("{STORE_NAME} could not remove {}: {e}", kind.account()),
        }
    }
}

/// The Mac keeps all of Needle's secrets in one Keychain item. Without an Apple developer
/// team, the Keychain trusts an item only to the exact build that may read it, so each new
/// build (an update too) asks the person once per item it reads: one item, one question.
#[cfg(target_os = "macos")]
mod keychain {
    use super::{SERVICE_NAME, STORE_NAME, bundle};
    use anyhow::{Result, anyhow};
    use std::sync::{LazyLock, Mutex};

    /// Items in the login Keychain, by account, under Needle's service name.
    struct Keychain;
    impl bundle::Items for Keychain {
        fn read(&self, account: &str) -> Result<Option<String>, String> {
            let entry = keyring::Entry::new(SERVICE_NAME, account).map_err(|e| e.to_string())?;
            match entry.get_password() {
                Ok(value) => Ok(Some(value)),
                Err(keyring::Error::NoEntry) => Ok(None),
                Err(e) => Err(e.to_string()),
            }
        }
        fn write(&self, account: &str, value: &str) -> Result<(), String> {
            keyring::Entry::new(SERVICE_NAME, account)
                .and_then(|entry| entry.set_password(value))
                .map_err(|e| e.to_string())
        }
    }

    static SECRETS: LazyLock<Mutex<bundle::Bundle<Keychain>>> =
        LazyLock::new(|| Mutex::new(bundle::Bundle::new(Keychain)));

    fn secrets() -> std::sync::MutexGuard<'static, bundle::Bundle<Keychain>> {
        SECRETS.lock().unwrap_or_else(|p| p.into_inner())
    }

    pub fn get(account: &str) -> Result<Option<String>> {
        secrets()
            .get(account)
            .map_err(|e| anyhow!("{STORE_NAME} could not read {account}: {e}"))
    }

    pub fn set(account: &str, value: &str) -> Result<()> {
        secrets()
            .set(account, Some(value))
            .map_err(|e| anyhow!("{STORE_NAME} could not save {account}: {e}"))
    }

    pub fn delete(account: &str) -> Result<()> {
        secrets()
            .set(account, None)
            .map_err(|e| anyhow!("{STORE_NAME} could not remove {account}: {e}"))
    }
}

/// All of a store's secrets in one item (the Mac's Keychain; see `keychain`), read once a run
/// and kept in memory. Secrets an earlier Needle kept as items of their own are moved in the
/// first time each is asked for. Their old items are left as they are (removing one could make
/// the Keychain ask again), and are not read again.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
mod bundle {
    use serde::{Deserialize, Serialize};
    use std::collections::{BTreeMap, BTreeSet};

    /// The item that holds the rest.
    pub const ACCOUNT: &str = "secrets";

    /// A store of items by account (an item's value, or none).
    pub trait Items {
        fn read(&self, account: &str) -> Result<Option<String>, String>;
        fn write(&self, account: &str, value: &str) -> Result<(), String>;
    }

    #[derive(Default, Serialize, Deserialize)]
    struct Saved {
        #[serde(default)]
        values: BTreeMap<String, String>,
        /// Accounts whose old item of their own was looked for already (moved, or none).
        #[serde(default)]
        moved: BTreeSet<String>,
    }

    pub struct Bundle<I: Items> {
        items: I,
        /// Read once a run; a failed read is not tried again, so it asks the person once.
        saved: Option<Result<Saved, String>>,
    }

    impl<I: Items> Bundle<I> {
        pub fn new(items: I) -> Self {
            Self { items, saved: None }
        }

        fn saved(&mut self) -> Result<&mut Saved, String> {
            let items = &self.items;
            let loaded = self.saved.get_or_insert_with(|| {
                Ok(items
                    .read(ACCOUNT)?
                    .and_then(|text| serde_json::from_str(&text).ok())
                    .unwrap_or_default())
            });
            loaded.as_mut().map_err(|e| e.clone())
        }

        fn save(&mut self) -> Result<(), String> {
            let text = serde_json::to_string(self.saved()?).map_err(|e| e.to_string())?;
            self.items.write(ACCOUNT, &text)
        }

        pub fn get(&mut self, account: &str) -> Result<Option<String>, String> {
            let saved = self.saved()?;
            if let Some(value) = saved.values.get(account) {
                return Ok(Some(value.clone()));
            }
            if saved.moved.contains(account) {
                return Ok(None);
            }
            // Kept by an earlier Needle as an item of its own: moved in, once.
            let old = self.items.read(account)?.filter(|v| !v.is_empty());
            let saved = self.saved()?;
            saved.moved.insert(account.to_string());
            if let Some(value) = &old {
                saved.values.insert(account.to_string(), value.clone());
            }
            // Not saved now, it is looked for again next run; the value is right either way.
            let _ = self.save();
            Ok(old)
        }

        /// Keep `value` for `account`, or forget it.
        pub fn set(&mut self, account: &str, value: Option<&str>) -> Result<(), String> {
            let saved = self.saved()?;
            saved.moved.insert(account.to_string());
            match value.filter(|v| !v.is_empty()) {
                Some(value) => saved.values.insert(account.to_string(), value.to_string()),
                None => saved.values.remove(account),
            };
            self.save()
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use std::{cell::RefCell, collections::HashMap};

        /// A pretend Keychain that counts reads (each would ask the person).
        #[derive(Default)]
        struct Fake {
            items: RefCell<HashMap<String, String>>,
            reads: RefCell<Vec<String>>,
        }
        impl Items for &Fake {
            fn read(&self, account: &str) -> Result<Option<String>, String> {
                self.reads.borrow_mut().push(account.to_string());
                Ok(self.items.borrow().get(account).cloned())
            }
            fn write(&self, account: &str, value: &str) -> Result<(), String> {
                self.items
                    .borrow_mut()
                    .insert(account.to_string(), value.to_string());
                Ok(())
            }
        }

        #[test]
        fn old_items_move_into_one_and_a_new_build_reads_only_that() {
            let keychain = Fake::default();
            for (account, value) in [
                ("lastfm-session", "s3ss"),
                ("plugin:subsonic:password", "pw"),
            ] {
                keychain
                    .items
                    .borrow_mut()
                    .insert(account.into(), value.into());
            }
            // The first run of the new Needle: each old item is read once, and moved.
            let mut first = Bundle::new(&keychain);
            assert_eq!(
                first.get("lastfm-session").unwrap().as_deref(),
                Some("s3ss")
            );
            assert_eq!(
                first.get("lastfm-session").unwrap().as_deref(),
                Some("s3ss")
            );
            assert_eq!(
                first.get("plugin:subsonic:password").unwrap().as_deref(),
                Some("pw")
            );
            assert_eq!(first.get("listenbrainz-token").unwrap(), None);
            assert_eq!(
                *keychain.reads.borrow(),
                [
                    "secrets",
                    "lastfm-session",
                    "plugin:subsonic:password",
                    "listenbrainz-token"
                ]
            );
            // The old items stay where they were (removing one could ask).
            assert!(keychain.items.borrow().contains_key("lastfm-session"));

            // A later build (an update): one read, of the one item, for all of them.
            keychain.reads.borrow_mut().clear();
            let mut next = Bundle::new(&keychain);
            assert_eq!(
                next.get("plugin:subsonic:password").unwrap().as_deref(),
                Some("pw")
            );
            assert_eq!(next.get("lastfm-session").unwrap().as_deref(), Some("s3ss"));
            assert_eq!(next.get("listenbrainz-token").unwrap(), None);
            assert_eq!(*keychain.reads.borrow(), ["secrets"]);

            // Signing out forgets it for good: the old item is not read again.
            next.set("lastfm-session", None).unwrap();
            next.set("acoustid-key", Some("k3y")).unwrap();
            let mut later = Bundle::new(&keychain);
            assert_eq!(later.get("lastfm-session").unwrap(), None);
            assert_eq!(later.get("acoustid-key").unwrap().as_deref(), Some("k3y"));
            assert!(!keychain.items.borrow()["secrets"].contains("s3ss"));
        }
    }
}

#[cfg(target_os = "android")]
impl SecretStore for SystemStore {
    fn get(&self, kind: SecretKind) -> Result<Option<String>> {
        Ok(file_store::get(kind.account()))
    }
    fn set(&self, kind: SecretKind, value: &str) -> Result<()> {
        file_store::set(kind.account(), Some(value))
    }
    fn delete(&self, kind: SecretKind) -> Result<()> {
        file_store::set(kind.account(), None)
    }
}

/// Secrets kept in `secrets.json` in a folder, readable only by this user (0600):
/// - Android has no keyring for apps, so its secrets are kept so in the app's own storage,
///   which Android lets no other app read ([`set_android_folder`]).
/// - Elsewhere, a test copy's library folder with a `secrets.json` in it ([`set_secrets_folder`])
///   keeps its secrets there, apart from the person's own: a test build then never asks for the
///   Keychain, nor reads or changes the real Needle's sign-ins.
mod file_store {
    use anyhow::{Context, Result};
    use std::{
        collections::HashMap,
        path::PathBuf,
        sync::{Mutex, OnceLock},
    };

    static FOLDER: OnceLock<PathBuf> = OnceLock::new();
    static LOCK: Mutex<()> = Mutex::new(());

    pub fn set_folder(folder: PathBuf) {
        let _ = FOLDER.set(folder);
    }

    fn file() -> Option<PathBuf> {
        FOLDER.get().map(|f| f.join("secrets.json"))
    }

    /// Whether secrets are kept in the file: always on Android, and elsewhere when the file is
    /// there (only a test copy's library has one).
    pub fn active() -> bool {
        cfg!(target_os = "android") || file().is_some_and(|f| f.is_file())
    }

    fn read() -> HashMap<String, String> {
        file()
            .and_then(|f| std::fs::read(f).ok())
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    pub fn get(account: &str) -> Option<String> {
        let _guard = LOCK.lock().unwrap_or_else(|p| p.into_inner());
        read().remove(account).filter(|v| !v.is_empty())
    }

    pub fn set(account: &str, value: Option<&str>) -> Result<()> {
        let _guard = LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let path = file().context("Needle's storage is not ready yet")?;
        let mut all = read();
        match value {
            Some(v) => all.insert(account.to_string(), v.to_string()),
            None => all.remove(account),
        };
        // Written aside, then moved over, so a crash never leaves half a file.
        let partial = path.with_extension("part");
        std::fs::write(&partial, serde_json::to_vec(&all)?)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&partial, std::fs::Permissions::from_mode(0o600))?;
        }
        std::fs::rename(&partial, &path)?;
        Ok(())
    }
}

/// Android: the folder, in the app's own storage, where secrets are kept.
#[cfg(target_os = "android")]
pub fn set_android_folder(folder: std::path::PathBuf) {
    file_store::set_folder(folder);
}

/// The library folder. A test copy's library with a `secrets.json` file in it keeps its
/// secrets in that file, not in the system's store (see `file_store`); for a real library
/// (with no such file) nothing changes.
pub fn set_secrets_folder(folder: std::path::PathBuf) {
    file_store::set_folder(folder);
}

#[cfg(not(any(
    windows,
    target_os = "linux",
    target_vendor = "apple",
    target_os = "android"
)))]
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
        not(target_vendor = "apple"),
        not(target_os = "android")
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
        #[cfg(not(any(test, target_os = "android")))]
        if super::file_store::active() {
            return Ok(super::file_store::get(&account(plugin, key)));
        }
        #[cfg(all(target_os = "macos", not(test)))]
        return super::keychain::get(&account(plugin, key));
        #[cfg(all(
            any(windows, target_os = "linux", target_vendor = "apple"),
            not(target_os = "macos"),
            not(test)
        ))]
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
        #[cfg(all(target_os = "android", not(test)))]
        return Ok(super::file_store::get(&account(plugin, key)));
        #[cfg(any(
            test,
            not(any(
                windows,
                target_os = "linux",
                target_vendor = "apple",
                target_os = "android"
            ))
        ))]
        Ok(memory().lock().unwrap().get(&account(plugin, key)).cloned())
    }

    pub fn set(plugin: &str, key: &str, value: &str) -> Result<()> {
        #[cfg(not(any(test, target_os = "android")))]
        if super::file_store::active() {
            return super::file_store::set(&account(plugin, key), Some(value));
        }
        #[cfg(all(target_os = "macos", not(test)))]
        return super::keychain::set(&account(plugin, key), value);
        #[cfg(all(
            any(windows, target_os = "linux", target_vendor = "apple"),
            not(target_os = "macos"),
            not(test)
        ))]
        {
            keyring::Entry::new(super::SERVICE_NAME, &account(plugin, key))
                .and_then(|e| e.set_password(value))
                .map_err(|e| {
                    anyhow::anyhow!("{} could not save a plugin secret: {e}", super::STORE_NAME)
                })
        }
        #[cfg(all(target_os = "android", not(test)))]
        return super::file_store::set(&account(plugin, key), Some(value));
        #[cfg(any(
            test,
            not(any(
                windows,
                target_os = "linux",
                target_vendor = "apple",
                target_os = "android"
            ))
        ))]
        {
            memory()
                .lock()
                .unwrap()
                .insert(account(plugin, key), value.into());
            Ok(())
        }
    }

    pub fn delete(plugin: &str, key: &str) -> Result<()> {
        #[cfg(not(any(test, target_os = "android")))]
        if super::file_store::active() {
            return super::file_store::set(&account(plugin, key), None);
        }
        #[cfg(all(target_os = "macos", not(test)))]
        return super::keychain::delete(&account(plugin, key));
        #[cfg(all(
            any(windows, target_os = "linux", target_vendor = "apple"),
            not(target_os = "macos"),
            not(test)
        ))]
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
        #[cfg(all(target_os = "android", not(test)))]
        return super::file_store::set(&account(plugin, key), None);
        #[cfg(any(
            test,
            not(any(
                windows,
                target_os = "linux",
                target_vendor = "apple",
                target_os = "android"
            ))
        ))]
        {
            memory().lock().unwrap().remove(&account(plugin, key));
            Ok(())
        }
    }
}
