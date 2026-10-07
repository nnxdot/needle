use needle_core::{
    analysis, cue,
    database::Library,
    doctor,
    formats::Dsf,
    import, integrations,
    model::{Listen, Playlist, Track},
    scan::{self, TagEdit},
    sync,
};
use std::{fs, path::Path};

fn library(dir: &Path) -> Library {
    Library::open(dir.join("data")).unwrap()
}

#[test]
fn bounded_pages_preserve_limits_missing_entries_duplicates_and_album_sorting() {
    let dir = tempfile::tempdir().unwrap();
    let lib = library(dir.path());
    for (i, artist) in [
        "The Amber",
        "amber",
        "Élodie",
        "Zulu",
        "水",
        "Various artists",
    ]
    .into_iter()
    .enumerate()
    {
        for n in 0..4 {
            lib.upsert(&Track {
                id: format!("{i}-{n}"),
                path: format!("/{i}-{n}.wav"),
                title: format!("Title {n}"),
                artist: artist.into(),
                album_artist: artist.into(),
                album: format!("Album {}", n % 2),
                track_number: n,
                missing: n == 1,
                ..Default::default()
            })
            .unwrap();
        }
    }
    for expression in [
        "",
        "order by title limit 8",
        "order by title limit 2 per artist limit 10",
        "order by title limit 1 per album",
    ] {
        let all: Vec<_> = lib
            .search(expression)
            .unwrap()
            .into_iter()
            .filter(|t| !t.missing)
            .collect();
        assert_eq!(lib.available_count(expression).unwrap(), all.len());
        for offset in [0, 2, 40] {
            assert_eq!(
                lib.available_page(expression, offset, 3).unwrap(),
                all.iter().skip(offset).take(3).cloned().collect::<Vec<_>>()
            );
        }
    }
    let all = lib.albums("").unwrap();
    for offset in [0, 2, 8, 40] {
        assert_eq!(
            lib.albums_page(offset, 3).unwrap(),
            all.iter().skip(offset).take(3).cloned().collect::<Vec<_>>()
        );
    }
    let playlist = Playlist {
        id: "page".into(),
        name: "Paging".into(),
        track_ids: ["0-1", "0-0", "gone", "0-0", "0-2"]
            .map(str::to_owned)
            .into(),
        ..Default::default()
    };
    assert_eq!(
        lib.playlist_available_page(&playlist, 1, 2)
            .unwrap()
            .iter()
            .map(|t| t.id.as_str())
            .collect::<Vec<_>>(),
        ["0-0", "0-2"]
    );
}

#[test]
#[ignore = "disposable-library performance measurement, run explicitly"]
fn measure_bounded_search_on_a_large_disposable_library() {
    let dir = tempfile::tempdir().unwrap();
    let lib = library(dir.path());
    let mut db = lib.connection().unwrap();
    let tx = db.transaction().unwrap();
    for n in 0..50_000 {
        Library::upsert_on(
            &tx,
            &Track {
                id: format!("{n:06}"),
                path: format!("/song-{n}.wav"),
                title: format!("Song {n}"),
                artist: format!("Artist {}", n % 500),
                album_artist: format!("Artist {}", n % 500),
                album: format!("Album {}", n % 2000),
                duration: 200.,
                ..Default::default()
            },
        )
        .unwrap();
    }
    tx.commit().unwrap();
    let before = std::time::Instant::now();
    let old: Vec<_> = lib
        .search("")
        .unwrap()
        .into_iter()
        .filter(|t| !t.missing)
        .take(300)
        .collect();
    let old_elapsed = before.elapsed();
    let after = std::time::Instant::now();
    let new = lib.available_page("", 0, 300).unwrap();
    let new_elapsed = after.elapsed();
    assert_eq!(new, old);
    println!(
        "50,000 tracks: full search then take = {old_elapsed:?}; bounded page = {new_elapsed:?}; hydrated rows 50,000 -> {}",
        new.len()
    );
}

fn song(library: &Library, path: &Path, id: &str) -> Track {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, b"disposable audio").unwrap();
    let track = Track {
        id: id.into(),
        path: path.to_string_lossy().into(),
        title: id.into(),
        ..Default::default()
    };
    library.upsert(&track).unwrap();
    track
}

fn wave(path: &Path, frequency: f64) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut writer = hound::WavWriter::create(
        path,
        hound::WavSpec {
            channels: 1,
            sample_rate: 44100,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        },
    )
    .unwrap();
    for n in 0..44100 {
        writer
            .write_sample(
                (8000. * (n as f64 * frequency * std::f64::consts::TAU / 44100.).sin()) as i16,
            )
            .unwrap();
    }
    writer.finalize().unwrap();
}

fn sheet(path: &Path, two: bool) {
    let mut text = String::from(
        "PERFORMER \"Artist\"\nTITLE \"Album\"\nFILE \"album.wav\" WAVE\nTRACK 01 AUDIO\nTITLE \"One\"\nINDEX 01 00:00:00\n",
    );
    if two {
        text.push_str("TRACK 02 AUDIO\nTITLE \"Two\"\nINDEX 01 00:00:30\n");
    }
    fs::write(path, text).unwrap();
}

#[test]
fn organize_preserves_colliding_lyrics_and_undo_retries_blocked_files() {
    let dir = tempfile::tempdir().unwrap();
    let lib = library(dir.path());
    let root = dir.path().join("Music");
    let root_text = root.to_string_lossy().to_string();
    lib.add_root(&root_text).unwrap();
    let source = root.join("old/song.wav");
    let destination = root.join("new/song.wav");
    let track = song(&lib, &source, "a");
    fs::create_dir_all(destination.parent().unwrap()).unwrap();
    fs::write(source.with_extension("lrc"), "source lyric").unwrap();
    fs::write(destination.with_extension("lrc"), "existing lyric").unwrap();
    let movement = doctor::Move {
        id: track.id.clone(),
        from: track.path.clone(),
        to: destination.to_string_lossy().into(),
    };
    let (done, failures) = lib
        .organize(
            std::slice::from_ref(&movement),
            std::slice::from_ref(&root_text),
        )
        .unwrap();
    assert!(done.is_empty());
    assert_eq!(failures.len(), 1);
    assert!(source.is_file());
    assert_eq!(
        fs::read_to_string(source.with_extension("lrc")).unwrap(),
        "source lyric"
    );
    assert_eq!(
        fs::read_to_string(destination.with_extension("lrc")).unwrap(),
        "existing lyric"
    );

    fs::remove_file(destination.with_extension("lrc")).unwrap();
    let (done, failures) = lib.organize(&[movement], &[root_text]).unwrap();
    assert_eq!(done.len(), 1);
    assert!(failures.is_empty());
    fs::create_dir_all(source.parent().unwrap()).unwrap();
    fs::write(&source, "obstruction").unwrap();
    let _ = lib.undo_organize();
    assert!(lib.can_undo_organize());
    assert_eq!(fs::read_to_string(&source).unwrap(), "obstruction");
    fs::remove_file(&source).unwrap();
    assert_eq!(lib.undo_organize().unwrap(), 1);
    assert!(!lib.can_undo_organize());
    assert_eq!(fs::read(source).unwrap(), b"disposable audio");
    assert_eq!(
        fs::read_to_string(root.join("old/song.lrc")).unwrap(),
        "source lyric"
    );
}

#[test]
fn organize_does_not_include_sibling_roots() {
    let dir = tempfile::tempdir().unwrap();
    let lib = library(dir.path());
    let root = dir.path().join("Music");
    let inside = song(&lib, &root.join("inside.wav"), "inside");
    let sibling = song(
        &lib,
        &dir.path().join("MusicArchive/outside.wav"),
        "outside",
    );
    let plan = doctor::plan_organize(
        &[inside, sibling],
        &[root.to_string_lossy().into()],
        "{title}",
    );
    assert_eq!(plan.skipped.len(), 1);
    assert!(plan.skipped[0].0.contains("MusicArchive"));
    assert!(plan.moves.iter().all(|m| !m.from.contains("MusicArchive")));
}

#[test]
fn failed_organize_undo_keeps_a_retry_record() {
    let dir = tempfile::tempdir().unwrap();
    let lib = library(dir.path());
    let root = dir.path().join("Music");
    let source = root.join("old/song.wav");
    let destination = root.join("new/song.wav");
    let track = song(&lib, &source, "retry");
    let movement = doctor::Move {
        id: track.id,
        from: track.path,
        to: destination.to_string_lossy().into(),
    };
    let (done, failures) = lib
        .organize(&[movement], &[root.to_string_lossy().into()])
        .unwrap();
    assert_eq!(done.len(), 1);
    assert!(failures.is_empty());
    fs::create_dir_all(source.parent().unwrap()).unwrap();
    fs::write(&source, "keep me").unwrap();
    let _ = lib.undo_organize();
    assert!(lib.can_undo_organize());
    assert_eq!(fs::read_to_string(&source).unwrap(), "keep me");
    fs::remove_file(source).unwrap();
    assert_eq!(lib.undo_organize().unwrap(), 1);
}

#[test]
fn cue_identity_distinguishes_equal_size_recordings_and_matches_copies() {
    let dir = tempfile::tempdir().unwrap();
    let lib = library(dir.path());
    for (folder, hz) in [("a", 440.), ("b", 880.), ("copy", 440.)] {
        let folder = dir.path().join(folder);
        wave(&folder.join("album.wav"), hz);
        sheet(&folder.join("album.cue"), false);
        cue::import_sheet(&lib, &folder.join("album.cue")).unwrap();
    }
    let tracks = lib.search("").unwrap();
    let find = |name: &str| {
        tracks
            .iter()
            .find(|t| {
                t.audio_path()
                    == dir
                        .path()
                        .join(name)
                        .join("album.wav")
                        .canonicalize()
                        .unwrap()
                        .to_string_lossy()
            })
            .unwrap()
    };
    assert_ne!(find("a").content_hash, find("b").content_hash);
    assert_eq!(find("a").content_hash, find("copy").content_hash);
    let duplicates = analysis::duplicates(&lib, "").unwrap();
    assert_eq!(duplicates.len(), 1);
    assert!(duplicates[0].exact_file);
}

#[test]
fn cue_sync_migrates_weak_identities_before_associating_history_and_playlists() {
    let dir = tempfile::tempdir().unwrap();
    let from = library(&dir.path().join("from"));
    let other = library(&dir.path().join("other"));
    let copied = library(&dir.path().join("copied"));
    for (lib, name, hz) in [
        (&from, "original", 440.),
        (&other, "different", 880.),
        (&copied, "copy", 440.),
    ] {
        let folder = dir.path().join(name);
        wave(&folder.join("album.wav"), hz);
        sheet(&folder.join("album.cue"), false);
        cue::import_sheet(lib, &folder.join("album.cue")).unwrap();
        let mut track = lib.search("").unwrap().remove(0);
        track.content_hash = "legacy-size-only-identity".into();
        track.metadata_version = 1;
        lib.upsert(&track).unwrap();
    }
    let track = from.search("").unwrap().remove(0);
    from.rate(&track.id, 5).unwrap();
    from.record_listen(&Listen {
        id: "history".into(),
        track_id: track.id.clone(),
        started_at: 100,
        duration: 1.,
        listened_seconds: 1.,
        qualified: true,
        ..Default::default()
    })
    .unwrap();
    from.save_playlist(&Playlist {
        id: "collection".into(),
        name: "Copies".into(),
        track_ids: vec![track.id.clone(), track.id],
        ..Default::default()
    })
    .unwrap();
    let bundle = dir.path().join("transfer.needle");
    sync::export(&from, &bundle, "disposable-passphrase").unwrap();
    let different = sync::import(&other, &bundle, "disposable-passphrase").unwrap();
    assert_eq!(different.matched_tracks, 0);
    assert_eq!(different.imported_listens, 0);
    assert_eq!(other.search("").unwrap()[0].rating, 0);
    let same = sync::import(&copied, &bundle, "disposable-passphrase").unwrap();
    assert_eq!(same.matched_tracks, 1);
    assert_eq!(same.imported_listens, 1);
    assert_eq!(copied.search("").unwrap()[0].rating, 5);
    assert_eq!(copied.playlists().unwrap()[0].track_ids.len(), 2);
}

#[test]
fn cue_rescan_retires_removed_entries_and_unavailable_audio() {
    let dir = tempfile::tempdir().unwrap();
    let lib = library(dir.path());
    let audio = dir.path().join("album.wav");
    let cue_path = dir.path().join("album.cue");
    wave(&audio, 440.);
    sheet(&cue_path, true);
    cue::import_sheet(&lib, &cue_path).unwrap();
    let tracks = lib.search("").unwrap();
    let second = tracks.iter().find(|t| t.track_number == 2).unwrap();
    sheet(&cue_path, false);
    cue::import_sheet(&lib, &cue_path).unwrap();
    assert!(lib.track(&second.id).unwrap().unwrap().missing);
    fs::remove_file(audio).unwrap();
    cue::import_sheet(&lib, &cue_path).unwrap();
    assert!(lib.search("").unwrap().iter().all(|t| t.missing));
}

#[test]
fn cue_playlists_round_trip_including_repeated_entries() {
    let dir = tempfile::tempdir().unwrap();
    let lib = library(dir.path());
    wave(&dir.path().join("album.wav"), 440.);
    sheet(&dir.path().join("album.cue"), true);
    cue::import_sheet(&lib, &dir.path().join("album.cue")).unwrap();
    let tracks = lib.search("").unwrap();
    let ids = vec![
        tracks[1].id.clone(),
        tracks[0].id.clone(),
        tracks[1].id.clone(),
    ];
    let playlist = Playlist {
        id: "cue-list".into(),
        name: "CUE".into(),
        track_ids: ids.clone(),
        ..Default::default()
    };
    let destination = dir.path().join("export.m3u8");
    lib.export_playlist(&playlist, &destination).unwrap();
    assert_eq!(lib.import_playlist(&destination).unwrap().track_ids, ids);
}

#[test]
fn metadata_edits_keep_measured_loudness_but_changed_audio_invalidates_it() {
    let dir = tempfile::tempdir().unwrap();
    let lib = library(dir.path());
    let path = dir.path().join("tone.wav");
    wave(&path, 440.);
    scan::import_one(&lib, &path).unwrap();
    let track = lib.search("").unwrap().remove(0);
    scan::write_tags(
        &lib,
        &track.id,
        &TagEdit {
            artist: Some("Artist".into()),
            album: Some("Album".into()),
            ..Default::default()
        },
    )
    .unwrap();
    analysis::scan_loudness(&lib, &track.id).unwrap();
    analysis::scan_album_loudness(&lib, &track.id).unwrap();
    let measured = lib.track(&track.id).unwrap().unwrap();
    scan::write_tags(
        &lib,
        &track.id,
        &TagEdit {
            title: Some("New title".into()),
            ..Default::default()
        },
    )
    .unwrap();
    let edited = lib.track(&track.id).unwrap().unwrap();
    assert_eq!(
        (
            edited.replay_gain,
            edited.replay_peak,
            edited.album_replay_gain,
            edited.album_peak
        ),
        (
            measured.replay_gain,
            measured.replay_peak,
            measured.album_replay_gain,
            measured.album_peak
        )
    );
    wave(&path, 880.);
    scan::import_one(&lib, &path).unwrap();
    let changed = lib.track(&track.id).unwrap().unwrap();
    assert_eq!(
        (
            changed.replay_gain,
            changed.replay_peak,
            changed.album_replay_gain,
            changed.album_peak
        ),
        (None, None, None, None)
    );
}

#[test]
fn unicode_credit_markers_normalize_without_panicking() {
    for (text, expected) in [
        ("K feat. Guest", "k"),
        ("İ ft. Guest", "i"),
        ("Song feat Guest", "song"),
        ("Song ft. Guest", "song"),
    ] {
        assert_eq!(import::normalize(text), expected);
    }
}

#[test]
fn malformed_dsf_layouts_are_rejected_before_seeking() {
    let dir = tempfile::tempdir().unwrap();
    for (rate, block) in [
        (2_822_400u32, 1u32),
        (2_822_400, 2),
        (2_822_400, 3),
        (1, 4096),
        (31, 4096),
    ] {
        let path = dir.path().join(format!("{rate}-{block}.dsf"));
        let mut bytes = vec![0u8; 28 + 52 + 12 + block as usize * 2];
        bytes[..4].copy_from_slice(b"DSD ");
        bytes[4..12].copy_from_slice(&28u64.to_le_bytes());
        bytes[28..32].copy_from_slice(b"fmt ");
        bytes[32..40].copy_from_slice(&52u64.to_le_bytes());
        for (offset, value) in [
            (40, 1u32),
            (48, 2),
            (52, 2),
            (56, rate),
            (60, 1),
            (72, block),
        ] {
            bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        }
        bytes[64..72].copy_from_slice(&(block as u64 * 8).to_le_bytes());
        bytes[80..84].copy_from_slice(b"data");
        fs::write(&path, bytes).unwrap();
        assert!(
            Dsf::open(&path).is_err(),
            "accepted rate {rate}, block {block}"
        );
    }
}

#[test]
fn disabled_scrobble_backlogs_do_not_starve_enabled_services() {
    let dir = tempfile::tempdir().unwrap();
    let lib = library(dir.path());
    let mut settings = lib.settings().unwrap();
    settings.lastfm_enabled = true;
    lib.save_settings(&settings).unwrap();
    for n in 0..100 {
        lib.record_listen(&Listen {
            id: format!("old-{n}"),
            track_id: format!("t-{n}"),
            title: "Old".into(),
            artist: "Artist".into(),
            duration: 60.,
            listened_seconds: 60.,
            started_at: n,
            qualified: true,
            ..Default::default()
        })
        .unwrap();
    }
    settings.lastfm_enabled = false;
    settings.listenbrainz_enabled = true;
    lib.save_settings(&settings).unwrap();
    // Missing title/artist is handled locally, so this test never sends a network request.
    lib.record_listen(&Listen {
        id: "new".into(),
        track_id: "new-track".into(),
        duration: 60.,
        listened_seconds: 60.,
        started_at: 101,
        qualified: true,
        ..Default::default()
    })
    .unwrap();
    integrations::flush_scrobbles(
        &lib,
        &integrations::Credentials {
            listenbrainz_token: "disposable-test-token".into(),
            ..Default::default()
        },
    )
    .unwrap();
    let status: String = lib
        .connection()
        .unwrap()
        .query_row(
            "SELECT status FROM scrobbles WHERE listen_id='new'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(status, "failed");
}

#[test]
fn browse_summaries_use_the_same_per_group_limits_as_song_search() {
    let dir = tempfile::tempdir().unwrap();
    let lib = library(dir.path());
    for n in 0..8 {
        lib.upsert(&Track {
            id: n.to_string(),
            path: format!("song-{n}.wav"),
            artist: format!("Artist {}", n / 4),
            album_artist: format!("Artist {}", n / 4),
            album: format!("Album {}", n / 2),
            duration: 30.,
            ..Default::default()
        })
        .unwrap();
    }
    for query in ["limit 1 per artist", "limit 1 per album"] {
        let expected = lib.search(query).unwrap().len();
        assert_eq!(
            lib.albums(query)
                .unwrap()
                .iter()
                .map(|a| a.tracks)
                .sum::<usize>(),
            expected
        );
        assert_eq!(
            lib.artists(query)
                .unwrap()
                .iter()
                .map(|a| a.tracks)
                .sum::<usize>(),
            expected
        );
    }
}
