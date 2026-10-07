use crate::*;
use needle_core::model::Playlist as StoredPlaylist;
use std::collections::HashMap;

#[test]
fn private_file_access_accepts_canonical_paths_and_excludes_shared_files() {
    let dir = tempfile::tempdir().unwrap();
    let owned = dir.path().join("app-files");
    std::fs::create_dir_all(&owned).unwrap();
    #[cfg(unix)]
    let data = {
        let alias = dir.path().join("app-alias");
        std::os::unix::fs::symlink(&owned, &alias).unwrap();
        alias.join("needle")
    };
    #[cfg(not(unix))]
    let data = owned.join("needle");
    let needle = Needle::new(data.to_string_lossy().into()).unwrap();
    for (id, path) in [
        ("private", owned.join("song.wav")),
        ("shared", dir.path().join("shared.wav")),
    ] {
        std::fs::write(&path, b"disposable fixture").unwrap();
        needle
            .library
            .upsert(&Track {
                id: id.into(),
                path: path.canonicalize().unwrap().to_string_lossy().into(),
                ..Default::default()
            })
            .unwrap();
    }
    assert!(needle.songs_are_private(vec!["private".into()]).unwrap());
    assert!(!needle.songs_are_private(vec!["shared".into()]).unwrap());
    assert!(
        !needle
            .songs_are_private(vec!["private".into(), "shared".into()])
            .unwrap()
    );
    needle.shutdown();
}

#[test]
fn visible_playlist_occurrences_and_theme_ownership_are_stable() {
    let dir = tempfile::tempdir().unwrap();
    let needle = Needle::new(dir.path().to_string_lossy().into()).unwrap();
    for (id, missing) in [("missing", true), ("a", false), ("b", false)] {
        needle
            .library
            .upsert(&Track {
                id: id.into(),
                path: format!("/{id}.wav"),
                title: id.into(),
                missing,
                ..Default::default()
            })
            .unwrap();
    }
    let p = StoredPlaylist {
        id: "p".into(),
        name: "Occurrences".into(),
        track_ids: ["missing", "a", "gone", "b", "a", "missing"]
            .map(str::to_owned)
            .into(),
        ..Default::default()
    };
    needle.library.save_playlist(&p).unwrap();
    let detail = needle.playlist_detail("p".into()).unwrap();
    assert_eq!(detail.positions, [1, 3, 4]);
    assert_eq!(
        detail
            .songs
            .iter()
            .map(|s| s.id.as_str())
            .collect::<Vec<_>>(),
        ["a", "b", "a"]
    );
    needle
        .remove_from_playlist("p".into(), detail.positions[2])
        .unwrap();
    assert_eq!(
        needle
            .library
            .playlist_by_id("p")
            .unwrap()
            .unwrap()
            .track_ids,
        ["missing", "a", "gone", "b", "missing"]
    );
    let detail = needle.playlist_detail("p".into()).unwrap();
    needle
        .move_in_playlist("p".into(), detail.positions[1], detail.positions[0])
        .unwrap();
    assert_eq!(
        needle
            .playlist_detail("p".into())
            .unwrap()
            .songs
            .iter()
            .map(|s| s.id.as_str())
            .collect::<Vec<_>>(),
        ["b", "a"]
    );

    let first = needle
        .save_theme(None, "Night".into(), "dark".into(), HashMap::new())
        .unwrap();
    let second = needle
        .save_theme(None, "Night".into(), "light".into(), HashMap::new())
        .unwrap();
    assert_ne!(first, second);
    assert!(first.starts_with("custom:"));
    let source = dir.path().join("source.toml");
    std::fs::write(&source, "name='Night'\nbase='midnight'\n").unwrap();
    needle
        .import_theme(source.to_string_lossy().into())
        .unwrap();
    needle
        .import_theme(source.to_string_lossy().into())
        .unwrap();
    assert_eq!(needle.themes().len(), 4);
    needle
        .save_theme(
            Some(first.clone()),
            "Renamed".into(),
            "midnight".into(),
            HashMap::new(),
        )
        .unwrap();
    assert_eq!(
        needle.themes().iter().find(|t| t.id == first).unwrap().name,
        "Renamed"
    );
    assert_eq!(
        needle
            .themes()
            .iter()
            .find(|t| t.id == second)
            .unwrap()
            .base,
        "light"
    );
    assert!(
        needle
            .save_theme(
                Some("night".into()),
                "Bad".into(),
                "dark".into(),
                HashMap::new()
            )
            .is_err()
    );
    assert!(needle.delete_theme("custom:plugin/night".into()).is_err());
    needle.delete_theme(first).unwrap();
    assert_eq!(needle.themes().len(), 3);
    for id in ["one", "two"] {
        let folder = dir.path().join("plugins").join(id);
        std::fs::create_dir_all(folder.join("themes")).unwrap();
        std::fs::write(
            folder.join("plugin.toml"),
            format!("id='{id}'\nname='{id}'\n"),
        )
        .unwrap();
        std::fs::write(
            folder.join("themes/night.toml"),
            "name='Night'\nbase='dark'\n",
        )
        .unwrap();
    }
    needle
        .library
        .set_json("plugins_enabled", &vec!["one", "two"])
        .unwrap();
    needle
        .plugins
        .send(needle_core::plugins::PluginEvent::Reload);
    let started = std::time::Instant::now();
    while needle.themes().len() != 5 {
        assert!(
            started.elapsed().as_secs() < 5,
            "theme plugins did not load"
        );
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let themes = needle.themes();
    assert!(
        themes
            .iter()
            .any(|t| t.id == "custom:one/night" && t.read_only)
    );
    assert!(
        themes
            .iter()
            .any(|t| t.id == "custom:two/night" && t.read_only)
    );
    assert!(
        needle
            .save_theme(
                Some("custom:one/night".into()),
                "Changed".into(),
                "light".into(),
                HashMap::new()
            )
            .is_err()
    );
    let root = dir.path().join("Music");
    std::fs::create_dir_all(root.join("old")).unwrap();
    needle.library.add_root(&root.to_string_lossy()).unwrap();
    for (id, title) in [("tidy-good", "Good"), ("tidy-blocked", "Blocked")] {
        let file = root.join("old").join(format!("{id}.wav"));
        std::fs::write(&file, b"disposable audio").unwrap();
        needle
            .library
            .upsert(&Track {
                id: id.into(),
                title: title.into(),
                path: file.to_string_lossy().into(),
                ..Default::default()
            })
            .unwrap();
    }
    std::fs::write(root.join("old/tidy-blocked.lrc"), "original lyric").unwrap();
    assert_eq!(needle.plan_tidy("{title}".into()).moves, 2);
    std::fs::write(root.join("Blocked.lrc"), "keep this lyric").unwrap();
    let result = needle.tidy().unwrap();
    assert_eq!(result.moved, 1);
    assert_eq!(result.failures.len(), 1);
    assert!(result.failures[0].reason.contains("already exists"));
    assert_eq!(
        std::fs::read_to_string(root.join("Blocked.lrc")).unwrap(),
        "keep this lyric"
    );
    assert!(needle.can_undo_tidy());
    assert_eq!(needle.undo_tidy().unwrap(), 1);
    needle.shutdown();
}
