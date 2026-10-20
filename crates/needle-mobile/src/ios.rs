//! The iPhone keeps each app in a folder whose name can change when the app is updated or put
//! back from a backup. The library keeps full paths (songs, covers, the saved queue), so the app
//! calls [`relocate`] at start, before [`crate::Needle::new`], when its folder has moved.
use crate::Result;
use needle_core::database::Library;

/// Replaces `old` with `new` at the start of every path Needle keeps in `data_dir`'s library.
/// Returns how many rows changed.
#[uniffi::export]
pub fn relocate(data_dir: String, old: String, new: String) -> Result<u32> {
    let old = old.trim_end_matches('/');
    let new = new.trim_end_matches('/');
    if old.is_empty() || old == new {
        return Ok(0);
    }
    let library = Library::open(std::path::PathBuf::from(data_dir))?;
    let mut db = library.connection()?;
    let tx = db.transaction().map_err(anyhow::Error::from)?;
    let mut changed = 0;
    let real = (format!("/private{old}"), format!("/private{new}"));
    let mut pairs = vec![(old.to_string(), new.to_string())];
    if old.starts_with("/var/") && new.starts_with("/var/") {
        pairs.push(real);
    }
    for (old, new) in &pairs {
        let (old, new) = (old.as_str(), new.as_str());
        // Paths begin with the folder; text (song data, playlists, settings) holds it anywhere.
        for table in ["tracks", "roots"] {
            changed += tx
                .execute(
                    &format!(
                        "UPDATE {table} SET path = ?2 || substr(path, length(?1) + 1)
                     WHERE substr(path, 1, length(?1) + 1) = ?1 || '/'"
                    ),
                    [old, new],
                )
                .map_err(anyhow::Error::from)?;
        }
        for (table, column) in [
            ("tracks", "data"),
            ("playlists", "data"),
            ("settings", "value"),
        ] {
            changed += tx
                .execute(
                    &format!(
                        "UPDATE {table} SET {column} = replace({column}, ?1 || '/', ?2 || '/')
                     WHERE instr({column}, ?1 || '/') > 0"
                    ),
                    [old, new],
                )
                .map_err(anyhow::Error::from)?;
        }
    }
    tx.commit().map_err(anyhow::Error::from)?;
    Ok(changed as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moves_songs_folders_and_saved_settings() {
        let dir = tempfile::tempdir().unwrap();
        let data = dir.path().to_string_lossy().to_string();
        {
            let library = Library::open(dir.path()).unwrap();
            library.add_root("/old/app/Documents").unwrap();
            library.add_root("/old/apples").unwrap();
            library
                .set_json("playback_session", &"/old/app/Documents/a.flac")
                .unwrap();
        }
        assert!(relocate(data.clone(), "/old/app".into(), "/new/app/".into()).unwrap() >= 2);
        let library = Library::open(dir.path()).unwrap();
        assert_eq!(
            library.roots().unwrap(),
            vec!["/new/app/Documents", "/old/apples"]
        );
        let session: String = library.get_json("playback_session").unwrap().unwrap();
        assert_eq!(session, "/new/app/Documents/a.flac");
        assert_eq!(relocate(data, "/x".into(), "/x".into()).unwrap(), 0);
    }
}
