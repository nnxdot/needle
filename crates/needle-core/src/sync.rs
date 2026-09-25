//! Authenticated encrypted transfer of listening history, ratings, and playlists.
//! Audio files and credentials are never included.
use crate::{
    database::Library,
    model::{Listen, Playlist},
};
use anyhow::{Context, Result, bail};
use chacha20poly1305::{KeyInit, XChaCha20Poly1305, XNonce, aead::Aead};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, path::Path};

#[derive(Serialize, Deserialize)]
struct Bundle {
    version: u32,
    tracks: Vec<Identity>,
    listens: Vec<Listen>,
    playlists: Vec<Playlist>,
}
#[derive(Serialize, Deserialize)]
struct Identity {
    id: String,
    hash: String,
    rating: i64,
}

fn key(passphrase: &str, salt: &[u8]) -> Result<[u8; 32]> {
    if passphrase.chars().count() < 12 {
        bail!("Use a sync passphrase with at least 12 characters")
    }
    let mut result = [0u8; 32];
    argon2::Argon2::default()
        .hash_password_into(passphrase.as_bytes(), salt, &mut result)
        .map_err(|e| anyhow::anyhow!("Key derivation failed: {e}"))?;
    Ok(result)
}
pub fn export(library: &Library, path: &Path, passphrase: &str) -> Result<()> {
    let tracks = library
        .search("")?
        .into_iter()
        .map(|t| Identity {
            id: t.id,
            hash: t.content_hash,
            rating: t.rating,
        })
        .collect();
    let bundle = Bundle {
        version: 1,
        tracks,
        listens: library.history(i64::MAX as usize)?,
        playlists: library.playlists()?,
    };
    let payload = serde_json::to_vec(&bundle)?;
    let mut salt = [0u8; 16];
    let mut nonce = [0u8; 24];
    rand::rngs::OsRng.fill_bytes(&mut salt);
    rand::rngs::OsRng.fill_bytes(&mut nonce);
    let cipher = XChaCha20Poly1305::new_from_slice(&key(passphrase, &salt)?).unwrap();
    let encrypted = cipher
        .encrypt(XNonce::from_slice(&nonce), payload.as_ref())
        .map_err(|_| anyhow::anyhow!("Unable to encrypt library data"))?;
    let mut output = b"NEEDLE01".to_vec();
    output.extend_from_slice(&salt);
    output.extend_from_slice(&nonce);
    output.extend_from_slice(&encrypted);
    std::fs::write(path, output)?;
    Ok(())
}
#[derive(Debug, Default)]
pub struct ImportReport {
    pub matched_tracks: usize,
    pub imported_listens: usize,
    pub imported_playlists: usize,
    pub unmatched_tracks: usize,
}
pub fn import(library: &Library, path: &Path, passphrase: &str) -> Result<ImportReport> {
    let size = std::fs::metadata(path)?.len();
    if !(64..=256 * 1024 * 1024).contains(&size) {
        bail!("Invalid sync file size")
    }
    let bytes = std::fs::read(path)?;
    if &bytes[..8] != b"NEEDLE01" {
        bail!("This is not a Needle sync bundle")
    }
    let cipher = XChaCha20Poly1305::new_from_slice(&key(passphrase, &bytes[8..24])?).unwrap();
    let plaintext = cipher
        .decrypt(XNonce::from_slice(&bytes[24..48]), &bytes[48..])
        .map_err(|_| anyhow::anyhow!("Incorrect passphrase or damaged sync file"))?;
    let bundle: Bundle = serde_json::from_slice(&plaintext).context("Invalid sync data")?;
    if bundle.version != 1 {
        bail!("Unsupported sync version")
    }
    for playlist in &bundle.playlists {
        if playlist.id.is_empty() || playlist.name.trim().is_empty() {
            bail!("Invalid playlist identity");
        }
        if let Some(query) = &playlist.query {
            crate::query::compile(query, chrono::Utc::now().timestamp())?;
        }
    }
    for listen in &bundle.listens {
        if listen.id.is_empty()
            || listen.listened_seconds < 0.
            || listen.duration < 0.
            || !listen.listened_seconds.is_finite()
            || !listen.duration.is_finite()
        {
            bail!("Invalid listening history");
        }
    }
    let local: HashMap<_, _> = library
        .search("")?
        .into_iter()
        .filter(|t| !t.content_hash.is_empty())
        .map(|t| (t.content_hash.clone(), t))
        .collect();
    let existing: HashMap<_, _> = library
        .playlists()?
        .into_iter()
        .map(|p| (p.id, p.updated_at))
        .collect();
    let mut mapping = HashMap::new();
    let mut report = ImportReport::default();
    let mut db = library.connection()?;
    let tx = db.transaction()?;
    for remote in bundle.tracks {
        if let Some(track) = local.get(&remote.hash) {
            mapping.insert(remote.id, track.id.clone());
            report.matched_tracks += 1;
            if track.rating == 0 && (0..=5).contains(&remote.rating) {
                tx.execute(
                    "UPDATE tracks SET rating=? WHERE id=? AND rating=0",
                    rusqlite::params![remote.rating, track.id],
                )?;
            }
        } else {
            report.unmatched_tracks += 1;
        }
    }
    for mut listen in bundle.listens {
        if let Some(local) = mapping.get(&listen.track_id) {
            listen.track_id = local.clone();
            let changed = tx.execute(
                "INSERT OR IGNORE INTO listens VALUES (?,?,?,?,?)",
                rusqlite::params![
                    listen.id,
                    listen.track_id,
                    listen.started_at,
                    listen.qualified,
                    serde_json::to_string(&listen)?
                ],
            )?;
            if changed > 0 {
                if listen.qualified {
                    tx.execute("UPDATE tracks SET play_count=play_count+1,last_played=max(coalesce(last_played,0),?) WHERE id=?",rusqlite::params![listen.started_at,listen.track_id])?;
                }
                report.imported_listens += 1;
            }
        }
    }
    for mut playlist in bundle.playlists {
        if existing
            .get(&playlist.id)
            .is_some_and(|updated| *updated >= playlist.updated_at)
        {
            continue;
        }
        playlist.track_ids = playlist
            .track_ids
            .iter()
            .filter_map(|id| mapping.get(id).cloned())
            .collect();
        tx.execute("INSERT INTO playlists VALUES (?,?,?) ON CONFLICT(id) DO UPDATE SET name=excluded.name,data=excluded.data",rusqlite::params![playlist.id,playlist.name,serde_json::to_string(&playlist)?])?;
        report.imported_playlists += 1;
    }
    tx.commit()?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Track;
    #[test]
    fn invalid_playlist_leaves_all_local_data_unchanged() {
        let dir = tempfile::tempdir().unwrap();
        let a = Library::open(dir.path().join("a")).unwrap();
        let b = Library::open(dir.path().join("b")).unwrap();
        a.upsert(&Track {
            id: "remote".into(),
            path: "remote.flac".into(),
            content_hash: "identical".into(),
            rating: 5,
            ..Default::default()
        })
        .unwrap();
        b.upsert(&Track {
            id: "local".into(),
            path: "local.flac".into(),
            content_hash: "identical".into(),
            ..Default::default()
        })
        .unwrap();
        let invalid = Playlist {
            id: "bad".into(),
            name: "Invalid imported rule".into(),
            query: Some("year >".into()),
            track_ids: vec![],
            updated_at: 1,
            ..Default::default()
        };
        a.connection()
            .unwrap()
            .execute(
                "INSERT INTO playlists VALUES(?,?,?)",
                rusqlite::params![
                    invalid.id,
                    invalid.name,
                    serde_json::to_string(&invalid).unwrap()
                ],
            )
            .unwrap();
        let path = dir.path().join("invalid.needle");
        export(&a, &path, "a long test passphrase").unwrap();
        assert!(import(&b, &path, "a long test passphrase").is_err());
        assert_eq!(b.track("local").unwrap().unwrap().rating, 0);
        assert!(b.playlists().unwrap().is_empty());
        assert!(b.history(100).unwrap().is_empty());
    }
    #[test]
    fn authenticated_sync_remaps_local_paths_and_is_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let a = Library::open(dir.path().join("a")).unwrap();
        let b = Library::open(dir.path().join("b")).unwrap();
        a.upsert(&Track {
            id: "a".into(),
            path: "/a/file".into(),
            content_hash: "same".into(),
            rating: 4,
            ..Default::default()
        })
        .unwrap();
        b.upsert(&Track {
            id: "b".into(),
            path: "/b/file".into(),
            content_hash: "same".into(),
            ..Default::default()
        })
        .unwrap();
        a.record_listen(&Listen {
            id: "listen".into(),
            track_id: "a".into(),
            title: "title".into(),
            artist: "artist".into(),
            album: String::new(),
            started_at: 1000,
            listened_seconds: 100.0,
            duration: 150.0,
            qualified: true,
        })
        .unwrap();
        let path = dir.path().join("sync.needle");
        export(&a, &path, "a long test passphrase").unwrap();
        assert!(import(&b, &path, "incorrect passphrase").is_err());
        assert_eq!(b.count().unwrap(), 1);
        assert_eq!(
            import(&b, &path, "a long test passphrase")
                .unwrap()
                .imported_listens,
            1
        );
        assert_eq!(
            import(&b, &path, "a long test passphrase")
                .unwrap()
                .imported_listens,
            0
        );
        assert_eq!(b.track("b").unwrap().unwrap().play_count, 1);
        assert_eq!(b.track("b").unwrap().unwrap().rating, 4);
    }
}
