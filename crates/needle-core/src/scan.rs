use crate::{database::Library, model::Track};
use anyhow::{Context, Result, bail};
use lofty::{
    config::WriteOptions,
    file::{AudioFile, TaggedFileExt},
    prelude::{Accessor, ItemKey, TagExt},
    probe::Probe,
};
use std::{
    fs::{self, File},
    io::Read,
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

pub const EXTENSIONS: &[&str] = &[
    "flac", "mp3", "m4a", "mp4", "aac", "wav", "wave", "aif", "aiff", "ogg", "oga",
];
#[derive(Clone, Debug, Default)]
pub struct ScanProgress {
    pub scanned: usize,
    pub imported: usize,
    pub unchanged: usize,
    pub errors: Vec<String>,
    pub current: String,
    pub done: bool,
}

pub fn import(
    library: &Library,
    root: &Path,
    cancel: Arc<AtomicBool>,
    mut progress: impl FnMut(ScanProgress),
) -> Result<ScanProgress> {
    let root = root
        .canonicalize()
        .with_context(|| format!("Cannot access {}", root.display()))?;
    if !root.is_dir() {
        bail!("Choose a music folder")
    }
    fs::read_dir(&root).context("Cannot read this music folder")?;
    let mut state = ScanProgress::default();
    for entry in walkdir::WalkDir::new(&root).follow_links(false) {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                if state.errors.len() < 100 {
                    state.errors.push(e.to_string());
                }
                continue;
            }
        };
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        if !path
            .extension()
            .is_some_and(|e| EXTENSIONS.contains(&e.to_string_lossy().to_lowercase().as_str()))
        {
            continue;
        }
        state.scanned += 1;
        state.current = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into();
        match import_one(library, path) {
            Ok(true) => state.imported += 1,
            Ok(false) => state.unchanged += 1,
            Err(e) => {
                if state.errors.len() < 100 {
                    state.errors.push(format!("{}: {e:#}", path.display()));
                }
            }
        }
        if state.scanned % 10 == 0 || state.scanned == 1 {
            progress(state.clone());
        }
    }
    library.add_root(&root.to_string_lossy())?;
    if !cancel.load(Ordering::Relaxed) {
        for track in library.search("")? {
            if Path::new(&track.path).starts_with(&root)
                && fs::metadata(&track.path)
                    .is_err_and(|e| e.kind() == std::io::ErrorKind::NotFound)
            {
                library
                    .connection()?
                    .execute("UPDATE tracks SET missing=1 WHERE id=?", [track.id])?;
            }
        }
    }
    state.done = true;
    progress(state.clone());
    Ok(state)
}

pub fn import_one(library: &Library, path: &Path) -> Result<bool> {
    let path = path.canonicalize()?;
    let path_string = path.to_string_lossy().to_string();
    let metadata = fs::metadata(&path)?;
    let modified = metadata
        .modified()?
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
        .min(i64::MAX as u128) as i64;
    let previous = library.track_by_path(&path_string)?;
    if previous.as_ref().is_some_and(|t| {
        t.modified_at == modified
            && t.file_size == metadata.len() as i64
            && !t.missing
            && t.metadata_version == 1
    }) {
        return Ok(false);
    }
    let tagged = Probe::open(&path)?
        .read()
        .context("Unable to read audio metadata")?;
    let properties = tagged.properties();
    let mut hasher = blake3::Hasher::new();
    let mut file = File::open(&path)?;
    let mut buffer = [0u8; 65536];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    let hash = hasher.finalize().to_hex().to_string();
    let previous = match previous {
        Some(t) => Some(t),
        None => library.moved_track(&hash)?,
    };
    let now = chrono::Utc::now().timestamp();
    let mut track = Track {
        id: previous
            .as_ref()
            .map(|t| t.id.clone())
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
        path: path_string,
        title: path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into(),
        duration: properties.duration().as_secs_f64(),
        sample_rate: properties.sample_rate().unwrap_or(0) as i64,
        bit_depth: properties.bit_depth().unwrap_or(0) as i64,
        channels: properties.channels().unwrap_or(0) as i64,
        bitrate: properties.audio_bitrate().unwrap_or(0) as i64,
        format: path
            .extension()
            .unwrap_or_default()
            .to_string_lossy()
            .to_uppercase(),
        added_at: previous.as_ref().map(|t| t.added_at).unwrap_or(now),
        modified_at: modified,
        file_size: metadata.len() as i64,
        content_hash: hash,
        rating: previous.as_ref().map(|t| t.rating).unwrap_or(0),
        play_count: previous.as_ref().map(|t| t.play_count).unwrap_or(0),
        last_played: previous.as_ref().and_then(|t| t.last_played),
        metadata_version: 1,
        ..Default::default()
    };
    if let Some(tag) = tagged.primary_tag().or_else(|| tagged.first_tag()) {
        if let Some(title) = tag.title() {
            track.title = title.into();
        }
        track.artist = tag.artist().unwrap_or_default().into();
        track.album = tag.album().unwrap_or_default().into();
        track.album_artist = tag
            .get_string(ItemKey::AlbumArtist)
            .unwrap_or(&track.artist)
            .into();
        track.genre = tag.genre().unwrap_or_default().into();
        track.year = tag
            .get_string(ItemKey::RecordingDate)
            .or_else(|| tag.get_string(ItemKey::Year))
            .and_then(|s| s.get(..4))
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        track.track_number = tag.track().unwrap_or(0) as i64;
        track.disc = tag.disk().unwrap_or(1) as i64;
        track.bpm = tag
            .get_string(ItemKey::Bpm)
            .or_else(|| tag.get_string(ItemKey::IntegerBpm))
            .and_then(|s| s.parse().ok());
        track.replay_gain = tag
            .get_string(ItemKey::ReplayGainTrackGain)
            .and_then(parse_gain);
        track.replay_peak = tag
            .get_string(ItemKey::ReplayGainTrackPeak)
            .and_then(|s| s.parse().ok());
        track.album_replay_gain = tag
            .get_string(ItemKey::ReplayGainAlbumGain)
            .and_then(parse_gain);
        track.album_peak = tag
            .get_string(ItemKey::ReplayGainAlbumPeak)
            .and_then(|s| s.parse().ok());
        track.musicbrainz_id = tag
            .get_string(ItemKey::MusicBrainzRecordingId)
            .map(String::from);
        if let Some(picture) = tag
            .pictures()
            .first()
            .filter(|p| p.data().len() <= 20 * 1024 * 1024)
        {
            let filename = format!("{}.img", blake3::hash(picture.data()).to_hex());
            let destination = library.directory.join("artwork").join(filename);
            if !destination.exists() {
                fs::write(&destination, picture.data())?;
            }
            track.artwork = Some(destination.to_string_lossy().into());
        }
    }
    if track.artwork.is_none() {
        for filename in [
            "cover.jpg",
            "folder.jpg",
            "cover.png",
            "front.jpg",
            "Folder.jpg",
        ] {
            let cover = path.parent().unwrap().join(filename);
            if cover.is_file() {
                track.artwork = Some(cover.to_string_lossy().into());
                break;
            }
        }
    }
    library.upsert(&track)?;
    Ok(true)
}

fn parse_gain(value: &str) -> Option<f64> {
    value.trim().trim_end_matches("dB").trim().parse().ok()
}

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct TagEdit {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub genre: Option<String>,
    pub year: Option<u32>,
    pub musicbrainz_id: Option<String>,
}

/// Write through a same-directory temporary file; retain a backup before replacing the original.
pub fn write_tags(library: &Library, id: &str, edit: &TagEdit) -> Result<()> {
    let track = library.track(id)?.context("Track no longer exists")?;
    let original = Path::new(&track.path);
    let current = fs::metadata(original)?;
    let modified = current
        .modified()?
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
        .min(i64::MAX as u128) as i64;
    if modified != track.modified_at || current.len() as i64 != track.file_size {
        bail!("File changed outside Needle. Rescan before editing its tags.")
    }
    let mut tagged = Probe::open(original)?.read()?;
    let primary_type = tagged.primary_tag_type();
    if tagged.primary_tag().is_none() {
        tagged.insert_tag(lofty::tag::Tag::new(primary_type));
    }
    let tag = tagged
        .primary_tag_mut()
        .context("This format does not support tag editing")?;
    if let Some(v) = &edit.title {
        tag.set_title(v.clone());
    }
    if let Some(v) = &edit.artist {
        tag.set_artist(v.clone());
    }
    if let Some(v) = &edit.album {
        tag.set_album(v.clone());
    }
    if let Some(v) = &edit.genre {
        tag.set_genre(v.clone());
    }
    if let Some(v) = edit.year {
        tag.insert_text(ItemKey::RecordingDate, v.to_string());
    }
    if let Some(v) = &edit.musicbrainz_id {
        uuid::Uuid::parse_str(v).context("MusicBrainz recording ID must be a UUID")?;
        // ID3 stores the recording ID in UFID, which is a conversion special
        // case rather than an ItemKey string mapping in Lofty.
        tag.insert_unchecked(lofty::tag::TagItem::new(
            ItemKey::MusicBrainzRecordingId,
            lofty::tag::ItemValue::Text(v.clone()),
        ));
    }
    let temporary = original.with_file_name(format!(".needle-{}.tmp", uuid::Uuid::new_v4()));
    fs::copy(original, &temporary)?;
    let operation = (|| -> Result<()> {
        tag.save_to_path(&temporary, WriteOptions::default())?;
        if audio_digest(original)? != audio_digest(&temporary)? {
            bail!(
                "The tag writer would change decoded audio. The original file has been preserved."
            );
        }
        std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&temporary)?
            .sync_all()?;
        let backup = library.directory.join("backups").join(format!(
            "{}-{}.{}",
            track.id,
            uuid::Uuid::new_v4(),
            track.format.to_lowercase()
        ));
        fs::copy(original, &backup)?;
        fs::rename(&temporary, original)
            .context("Unable to replace audio file; original and backup are intact")?;
        import_one(library, original)?;
        Ok(())
    })();
    if temporary.exists() {
        let _ = fs::remove_file(temporary);
    }
    operation
}
fn audio_digest(path: &Path) -> Result<(u32, u16, u64, blake3::Hash)> {
    use rodio::Source;
    let decoder =
        crate::audio_file::decode(path).context("Cannot verify audio before writing tags")?;
    let rate = decoder.sample_rate();
    let channels = decoder.channels();
    let mut hasher = blake3::Hasher::new();
    let mut count = 0;
    for sample in decoder {
        hasher.update(&sample.to_bits().to_le_bytes());
        count += 1;
    }
    if count == 0 {
        bail!("Cannot verify an empty audio stream");
    }
    Ok((rate, channels, count, hasher.finalize()))
}

pub fn watch(
    library: Library,
    on_change: impl Fn() + Send + 'static,
) -> Result<notify::RecommendedWatcher> {
    use notify::Watcher;
    let worker_library = library.clone();
    let (tx, rx) = crossbeam_channel::bounded::<()>(1);
    let mut watcher = notify::recommended_watcher(move |result: notify::Result<notify::Event>| {
        if let Ok(event) = result
            && matches!(
                event.kind,
                notify::EventKind::Create(_)
                    | notify::EventKind::Modify(_)
                    | notify::EventKind::Remove(_)
            )
        {
            let _ = tx.try_send(());
        }
    })?;
    for root in library.roots()? {
        if Path::new(&root).is_dir() {
            watcher.watch(Path::new(&root), notify::RecursiveMode::Recursive)?;
        }
    }
    std::thread::spawn(move || {
        while rx.recv().is_ok() {
            std::thread::sleep(std::time::Duration::from_millis(800));
            while rx.try_recv().is_ok() {}
            if let Ok(roots) = worker_library.roots() {
                for root in roots {
                    let _ = import(
                        &worker_library,
                        Path::new(&root),
                        Arc::new(AtomicBool::new(false)),
                        |_| {},
                    );
                }
            }
            on_change();
        }
    });
    Ok(watcher)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn watching_imports_new_files_and_notifies_the_view() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("music");
        std::fs::create_dir(&root).unwrap();
        let library = Library::open(dir.path().join("db")).unwrap();
        library
            .add_root(&root.canonicalize().unwrap().to_string_lossy())
            .unwrap();
        let (tx, rx) = crossbeam_channel::bounded(8);
        let _watcher = watch(library.clone(), move || {
            let _ = tx.try_send(());
        })
        .unwrap();
        let mut wav = hound::WavWriter::create(
            root.join("new.wav"),
            hound::WavSpec {
                channels: 1,
                sample_rate: 44100,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            },
        )
        .unwrap();
        for _ in 0..4410 {
            wav.write_sample(0i16).unwrap();
        }
        wav.finalize().unwrap();
        rx.recv_timeout(std::time::Duration::from_secs(8)).unwrap();
        assert_eq!(library.count().unwrap(), 1);
    }
    #[test]
    fn editing_tags_preserves_samples_and_original_backup() {
        use rodio::Decoder;
        let dir = tempfile::tempdir().unwrap();
        let music = dir.path().join("music");
        crate::demo::create(&music).unwrap();
        let library = Library::open(dir.path().join("db")).unwrap();
        let path = music.join("01 - A room with a view.wav");
        import_one(&library, &path).unwrap();
        let track = library.search("").unwrap().remove(0);
        assert_eq!(track.bpm, Some(92.));
        let before: Vec<f32> = Decoder::try_from(File::open(&path).unwrap())
            .unwrap()
            .collect();
        write_tags(
            &library,
            &track.id,
            &TagEdit {
                title: Some("An edited title".into()),
                artist: Some("Björk テスト".into()),
                musicbrainz_id: Some("00000000-0000-4000-8000-000000000001".into()),
                ..Default::default()
            },
        )
        .unwrap();
        let after: Vec<f32> = Decoder::try_from(File::open(&path).unwrap())
            .unwrap()
            .collect();
        assert_eq!(before, after);
        assert_eq!(library.search("edited").unwrap().len(), 1);
        assert_eq!(library.search("BJÖRK").unwrap().len(), 1);
        assert_eq!(
            library.track(&track.id).unwrap().unwrap().title,
            "An edited title"
        );
        assert_eq!(
            fs::read_dir(library.directory.join("backups"))
                .unwrap()
                .count(),
            1
        );
        assert_eq!(library.count().unwrap(), 1);
        assert_eq!(
            library
                .track(&track.id)
                .unwrap()
                .unwrap()
                .musicbrainz_id
                .as_deref(),
            Some("00000000-0000-4000-8000-000000000001")
        );
    }
    #[test]
    fn imports_incrementally_and_preserves_identity_on_move() {
        let dir = tempfile::tempdir().unwrap();
        let music = dir.path().join("music");
        fs::create_dir(&music).unwrap();
        let path = music.join("test.wav");
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 44100,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::create(&path, spec).unwrap();
        for _ in 0..4410 {
            writer.write_sample(0i16).unwrap();
        }
        writer.finalize().unwrap();
        let library = Library::open(dir.path().join("db")).unwrap();
        assert!(import_one(&library, &path).unwrap());
        let first = library.search("").unwrap().remove(0);
        library.rate(&first.id, 4).unwrap();
        assert!(!import_one(&library, &path).unwrap());
        let moved = music.join("renamed.wav");
        fs::rename(path, &moved).unwrap();
        import_one(&library, &moved).unwrap();
        let second = library.search("").unwrap().remove(0);
        assert_eq!(first.id, second.id);
        assert_eq!(second.rating, 4);
        assert_eq!(library.count().unwrap(), 1);
    }
}
