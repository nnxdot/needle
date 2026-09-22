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
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

pub const EXTENSIONS: &[&str] = &[
    "flac", "mp3", "m4a", "mp4", "aac", "wav", "wave", "aif", "aiff", "ogg", "oga", "opus",
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
        // Prefer the front cover when a file carries several pictures.
        let pictures = tag.pictures();
        if let Some(picture) = pictures
            .iter()
            .find(|p| p.pic_type() == lofty::picture::PictureType::CoverFront)
            .or_else(|| pictures.first())
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
        track.artwork = folder_cover(path.parent().unwrap_or(&path));
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
    /// An empty value removes the tag; Needle then shows the track artist.
    #[serde(default)]
    pub album_artist: Option<String>,
    /// Zero removes the track number.
    #[serde(default)]
    pub track_number: Option<u32>,
}

impl TagEdit {
    pub fn is_empty(&self) -> bool {
        self.title.is_none()
            && self.artist.is_none()
            && self.album.is_none()
            && self.genre.is_none()
            && self.year.is_none()
            && self.musicbrainz_id.is_none()
            && self.album_artist.is_none()
            && self.track_number.is_none()
    }
    fn validate(&self) -> Result<()> {
        if let Some(v) = &self.musicbrainz_id {
            uuid::Uuid::parse_str(v).context("MusicBrainz recording ID must be a UUID")?;
        }
        Ok(())
    }
}

fn ensure_unchanged(track: &Track) -> Result<()> {
    let current = fs::metadata(&track.path)?;
    let modified = current
        .modified()?
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
        .min(i64::MAX as u128) as i64;
    if modified != track.modified_at || current.len() as i64 != track.file_size {
        bail!("File changed outside Needle. Rescan before editing its tags.")
    }
    Ok(())
}

/// Prepares a same-directory temporary file, requires its decoded audio to match the
/// original, keeps a backup of the original, then replaces it and rescans the track.
fn replace_verified(
    library: &Library,
    track: &Track,
    mismatch: &str,
    prepare: impl FnOnce(&Path) -> Result<()>,
) -> Result<()> {
    let original = Path::new(&track.path);
    let temporary = original.with_file_name(format!(".needle-{}.tmp", uuid::Uuid::new_v4()));
    let operation = (|| -> Result<()> {
        prepare(&temporary)?;
        if audio_digest(original)? != audio_digest(&temporary)? {
            bail!("{mismatch}");
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

/// Write through a same-directory temporary file; retain a backup before replacing the original.
pub fn write_tags(library: &Library, id: &str, edit: &TagEdit) -> Result<()> {
    edit.validate()?;
    let track = library.track(id)?.context("Track no longer exists")?;
    let original = Path::new(&track.path);
    ensure_unchanged(&track)?;
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
    if let Some(v) = &edit.album_artist {
        if v.is_empty() {
            tag.remove_key(ItemKey::AlbumArtist);
        } else if !tag.insert_text(ItemKey::AlbumArtist, v.clone()) {
            bail!("This file's tag format cannot store an album artist");
        }
    }
    match edit.track_number {
        Some(0) => tag.remove_track(),
        Some(v) => tag.set_track(v),
        None => {}
    }
    if let Some(v) = &edit.musicbrainz_id {
        // ID3 stores the recording ID in UFID, which is a conversion special
        // case rather than an ItemKey string mapping in Lofty.
        tag.insert_unchecked(lofty::tag::TagItem::new(
            ItemKey::MusicBrainzRecordingId,
            lofty::tag::ItemValue::Text(v.clone()),
        ));
    }
    replace_verified(
        library,
        &track,
        "The tag writer would change decoded audio. The original file has been preserved.",
        |temporary| {
            fs::copy(original, temporary)?;
            tag.save_to_path(temporary, WriteOptions::default())?;
            Ok(())
        },
    )
}

#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
pub struct BatchProgress {
    /// Files attempted so far.
    pub done: usize,
    pub total: usize,
    pub saved: usize,
    pub failed: usize,
    /// Path of the file being written, empty once finished.
    pub current: String,
}

#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
pub struct BatchFailure {
    pub id: String,
    pub path: String,
    pub error: String,
}

#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
pub struct BatchReport {
    pub total: usize,
    /// Track IDs written successfully; these writes stay saved even if others fail.
    pub saved: Vec<String>,
    pub failed: Vec<BatchFailure>,
    /// Cancelled before every file was attempted.
    pub cancelled: bool,
}

/// Applies one edit to many tracks with the same backup and audio-verification guarantees
/// as [`write_tags`]. Per-file failures are collected; an invalid edit fails before any write.
pub fn write_tags_batch(
    library: &Library,
    ids: &[String],
    edit: &TagEdit,
    cancel: Arc<AtomicBool>,
    mut progress: impl FnMut(BatchProgress),
) -> Result<BatchReport> {
    if edit.is_empty() {
        bail!("Choose at least one tag to change")
    }
    edit.validate()?;
    let mut seen = std::collections::HashSet::new();
    let ids: Vec<&String> = ids.iter().filter(|id| seen.insert(id.as_str())).collect();
    let mut report = BatchReport {
        total: ids.len(),
        ..Default::default()
    };
    let mut state = BatchProgress {
        total: ids.len(),
        ..Default::default()
    };
    for id in ids {
        if cancel.load(Ordering::Relaxed) {
            report.cancelled = true;
            break;
        }
        let path = library
            .track(id)
            .ok()
            .flatten()
            .map(|t| t.path)
            .unwrap_or_default();
        state.current = path.clone();
        progress(state.clone());
        match write_tags(library, id, edit) {
            Ok(()) => {
                report.saved.push(id.clone());
                state.saved += 1;
            }
            Err(e) => {
                report.failed.push(BatchFailure {
                    id: id.clone(),
                    path,
                    error: format!("{e:#}"),
                });
                state.failed += 1;
            }
        }
        state.done += 1;
    }
    state.current.clear();
    progress(state);
    Ok(report)
}

/// A field's value across a selection.
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(tag = "state", content = "value", rename_all = "snake_case")]
pub enum Common<T> {
    Same(T),
    /// The selection holds different values (or is empty).
    Mixed,
}
impl<T> Common<T> {
    pub fn value(&self) -> Option<&T> {
        match self {
            Self::Same(value) => Some(value),
            Self::Mixed => None,
        }
    }
    pub fn is_mixed(&self) -> bool {
        matches!(self, Self::Mixed)
    }
}

/// Shared editable values of a multi-track selection, for prefilling a tag editor.
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct CommonTags {
    pub count: usize,
    pub title: Common<String>,
    pub artist: Common<String>,
    pub album: Common<String>,
    pub album_artist: Common<String>,
    pub genre: Common<String>,
    pub year: Common<i64>,
    pub track_number: Common<i64>,
}

pub fn common_tags(tracks: &[Track]) -> CommonTags {
    fn common<T: PartialEq + Clone>(tracks: &[Track], field: impl Fn(&Track) -> &T) -> Common<T> {
        match tracks.split_first() {
            Some((first, rest)) if rest.iter().all(|t| field(t) == field(first)) => {
                Common::Same(field(first).clone())
            }
            _ => Common::Mixed,
        }
    }
    CommonTags {
        count: tracks.len(),
        title: common(tracks, |t| &t.title),
        artist: common(tracks, |t| &t.artist),
        album: common(tracks, |t| &t.album),
        album_artist: common(tracks, |t| &t.album_artist),
        genre: common(tracks, |t| &t.genre),
        year: common(tracks, |t| &t.year),
        track_number: common(tracks, |t| &t.track_number),
    }
}

/// An original-file copy kept before a tag write or restore replaced the track's file.
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct TagBackup {
    pub path: std::path::PathBuf,
    /// Unix seconds when the backup was made.
    pub created_at: i64,
    pub size: u64,
}

fn backup_belongs(library: &Library, track_id: &str, path: &Path) -> bool {
    let directory = library.directory.join("backups");
    path.parent()
        .and_then(|p| p.canonicalize().ok())
        .is_some_and(|p| directory.canonicalize().is_ok_and(|d| d == p))
        && path.is_file()
        && path
            .file_stem()
            .and_then(|s| s.to_str())
            .and_then(|s| s.strip_prefix(track_id)?.strip_prefix('-'))
            .is_some_and(|rest| uuid::Uuid::parse_str(rest).is_ok())
}

/// Backups of a track's file, newest first.
pub fn tag_backups(library: &Library, track_id: &str) -> Result<Vec<TagBackup>> {
    let mut found = vec![];
    for entry in fs::read_dir(library.directory.join("backups"))? {
        let path = entry?.path();
        if !backup_belongs(library, track_id, &path) {
            continue;
        }
        let metadata = fs::metadata(&path)?;
        let created = metadata.created().or_else(|_| metadata.modified())?;
        found.push((created, path, metadata.len()));
    }
    found.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    Ok(found
        .into_iter()
        .map(|(created, path, size)| TagBackup {
            path,
            created_at: created
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_secs() as i64),
            size,
        })
        .collect())
}

/// Restores a backup's tags by replacing the track's file with the backup, only when both
/// decode to identical audio. The current file is backed up first, so a restore can be undone.
pub fn restore_tag_backup(library: &Library, track_id: &str, backup: &TagBackup) -> Result<()> {
    if !backup_belongs(library, track_id, &backup.path) {
        bail!("This backup does not belong to the track")
    }
    let track = library.track(track_id)?.context("Track no longer exists")?;
    ensure_unchanged(&track)?;
    replace_verified(
        library,
        &track,
        "The backup's decoded audio differs from the current file. Nothing was restored.",
        |temporary| {
            fs::copy(&backup.path, temporary)?;
            Ok(())
        },
    )
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
    fn demo_library() -> (tempfile::TempDir, Library, Vec<Track>) {
        let dir = tempfile::tempdir().unwrap();
        let music = dir.path().join("music");
        crate::demo::create(&music).unwrap();
        let library = Library::open(dir.path().join("db")).unwrap();
        import(&library, &music, Arc::new(AtomicBool::new(false)), |_| {}).unwrap();
        let mut tracks = library.search("").unwrap();
        tracks.sort_by_key(|t| t.track_number);
        assert_eq!(tracks.len(), 3);
        (dir, library, tracks)
    }
    fn samples(path: &str) -> Vec<f32> {
        crate::audio_file::decode(Path::new(path))
            .unwrap()
            .collect()
    }

    #[test]
    fn batch_edits_report_per_file_results_and_keep_successes() {
        let (_dir, library, tracks) = demo_library();
        let mut ids: Vec<String> = tracks.iter().map(|t| t.id.clone()).collect();
        ids.insert(1, "no-such-track".into());
        ids.push(ids[0].clone());
        let before = samples(&tracks[0].path);
        let mut updates = vec![];
        let report = write_tags_batch(
            &library,
            &ids,
            &TagEdit {
                album: Some("Collected".into()),
                album_artist: Some("Various Needles".into()),
                ..Default::default()
            },
            Arc::new(AtomicBool::new(false)),
            |p| updates.push(p),
        )
        .unwrap();
        assert_eq!(report.total, 4);
        assert_eq!(report.saved.len(), 3);
        assert_eq!(report.failed.len(), 1);
        assert_eq!(report.failed[0].id, "no-such-track");
        assert!(!report.cancelled);
        assert_eq!(updates.len(), 5);
        assert_eq!(updates[0].current, tracks[0].path);
        let last = updates.last().unwrap();
        assert_eq!((last.done, last.saved, last.failed), (4, 3, 1));
        assert!(last.current.is_empty());
        for track in library.search("").unwrap() {
            assert_eq!(track.album, "Collected");
            assert_eq!(track.album_artist, "Various Needles");
            assert_eq!(track.artist, "Needle Studio");
        }
        assert_eq!(samples(&tracks[0].path), before);
        assert_eq!(library.albums("").unwrap().len(), 1);

        let cancel = Arc::new(AtomicBool::new(true));
        let report = write_tags_batch(
            &library,
            &ids,
            &TagEdit {
                genre: Some("x".into()),
                ..Default::default()
            },
            cancel,
            |_| {},
        )
        .unwrap();
        assert!(report.cancelled);
        assert!(report.saved.is_empty() && report.failed.is_empty());

        let backups = fs::read_dir(library.directory.join("backups"))
            .unwrap()
            .count();
        let invalid = TagEdit {
            title: Some("never written".into()),
            musicbrainz_id: Some("not a uuid".into()),
            ..Default::default()
        };
        assert!(
            write_tags_batch(
                &library,
                &ids,
                &invalid,
                Arc::new(AtomicBool::new(false)),
                |_| {}
            )
            .is_err()
        );
        assert!(
            write_tags_batch(
                &library,
                &ids,
                &TagEdit::default(),
                Arc::new(AtomicBool::new(false)),
                |_| {}
            )
            .is_err()
        );
        assert_eq!(
            fs::read_dir(library.directory.join("backups"))
                .unwrap()
                .count(),
            backups
        );
        assert!(library.search("never").unwrap().is_empty());
    }

    #[test]
    fn album_artist_and_track_number_round_trip_and_clear() {
        let (_dir, library, tracks) = demo_library();
        let id = &tracks[2].id;
        let edit = TagEdit {
            album_artist: Some("Someone Else".into()),
            track_number: Some(9),
            ..Default::default()
        };
        write_tags(&library, id, &edit).unwrap();
        let track = library.track(id).unwrap().unwrap();
        assert_eq!(
            (track.album_artist.as_str(), track.track_number),
            ("Someone Else", 9)
        );
        let edit = TagEdit {
            album_artist: Some(String::new()),
            track_number: Some(0),
            ..Default::default()
        };
        write_tags(&library, id, &edit).unwrap();
        let track = library.track(id).unwrap().unwrap();
        assert_eq!(
            (track.album_artist.as_str(), track.track_number),
            ("Needle Studio", 0)
        );
        let edit: TagEdit = serde_json::from_str(r#"{"album":"Only album"}"#).unwrap();
        assert!(edit.album_artist.is_none() && edit.track_number.is_none());
    }

    #[test]
    fn common_tags_mark_mixed_fields() {
        let (_dir, _library, tracks) = demo_library();
        let common = common_tags(&tracks);
        assert_eq!(common.count, 3);
        assert!(common.title.is_mixed());
        assert_eq!(common.artist, Common::Same("Needle Studio".into()));
        assert_eq!(
            common.album.value().map(String::as_str),
            Some("First listening · Demo recordings")
        );
        assert_eq!(common.album_artist, Common::Same("Needle Studio".into()));
        assert_eq!(common.genre, Common::Same("Ambient".into()));
        assert_eq!(common.year, Common::Same(2026));
        assert!(common.track_number.is_mixed());
        assert_eq!(
            common_tags(&tracks[..1]).title,
            Common::Same("A room with a view".into())
        );
        let empty = common_tags(&[]);
        assert_eq!(empty.count, 0);
        assert!(empty.artist.is_mixed());
        let json = serde_json::to_value(&common).unwrap();
        assert_eq!(json["title"], serde_json::json!({"state": "mixed"}));
        assert_eq!(
            json["year"],
            serde_json::json!({"state": "same", "value": 2026})
        );
    }

    #[test]
    fn backups_restore_tags_verify_audio_and_are_undoable() {
        let (_dir, library, tracks) = demo_library();
        let track = &tracks[0];
        assert!(tag_backups(&library, &track.id).unwrap().is_empty());
        let before = samples(&track.path);
        for title in ["Second title", "Third title"] {
            write_tags(
                &library,
                &track.id,
                &TagEdit {
                    title: Some(title.into()),
                    ..Default::default()
                },
            )
            .unwrap();
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        write_tags(
            &library,
            &tracks[1].id,
            &TagEdit {
                title: Some("Other".into()),
                ..Default::default()
            },
        )
        .unwrap();
        let backups = tag_backups(&library, &track.id).unwrap();
        assert_eq!(backups.len(), 2);
        assert!(backups[0].created_at >= backups[1].created_at);
        assert!(backups[0].size > 0);

        restore_tag_backup(&library, &track.id, &backups[1]).unwrap();
        let restored = library.track(&track.id).unwrap().unwrap();
        assert_eq!(restored.title, "A room with a view");
        assert_eq!(samples(&track.path), before);
        assert_eq!(library.count().unwrap(), 3);
        let after = tag_backups(&library, &track.id).unwrap();
        assert_eq!(after.len(), 3);
        restore_tag_backup(&library, &track.id, &after[0]).unwrap();
        assert_eq!(
            library.track(&track.id).unwrap().unwrap().title,
            "Third title"
        );

        let other = tag_backups(&library, &tracks[1].id).unwrap();
        assert_eq!(other.len(), 1);
        assert!(restore_tag_backup(&library, &track.id, &other[0]).is_err());
        let outside = TagBackup {
            path: Path::new(&tracks[1].path).to_path_buf(),
            created_at: 0,
            size: 0,
        };
        assert!(restore_tag_backup(&library, &track.id, &outside).is_err());

        let forged = library.directory.join("backups").join(format!(
            "{}-{}.wav",
            track.id,
            uuid::Uuid::new_v4()
        ));
        fs::copy(&tracks[2].path, &forged).unwrap();
        let forged = tag_backups(&library, &track.id)
            .unwrap()
            .into_iter()
            .find(|b| b.path == forged)
            .unwrap();
        let count = tag_backups(&library, &track.id).unwrap().len();
        let error = restore_tag_backup(&library, &track.id, &forged).unwrap_err();
        assert!(format!("{error:#}").contains("differs"), "{error:#}");
        assert_eq!(
            library.track(&track.id).unwrap().unwrap().title,
            "Third title"
        );
        assert_eq!(tag_backups(&library, &track.id).unwrap().len(), count);
        assert_eq!(samples(&track.path), before);
    }
}

/// A cover image kept beside the audio files: cover, folder, front, or album art, in any case,
/// as JPEG, PNG or WebP. A folder with exactly one image uses that image.
pub fn folder_cover(folder: &Path) -> Option<String> {
    let images: Vec<PathBuf> = fs::read_dir(folder)
        .ok()?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.extension().and_then(|e| e.to_str()).is_some_and(|e| {
                matches!(
                    e.to_ascii_lowercase().as_str(),
                    "jpg" | "jpeg" | "png" | "webp"
                )
            })
        })
        .collect();
    let rank = |p: &PathBuf| {
        let stem = p
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        ["cover", "folder", "front", "albumart", "album"]
            .iter()
            .position(|name| {
                stem == *name
                    || stem.starts_with(&format!("{name}_"))
                    || stem.starts_with(&format!("{name} "))
                    || (*name == "albumart" && stem.starts_with("albumart"))
            })
    };
    images
        .iter()
        .filter_map(|p| rank(p).map(|r| (r, p)))
        .min_by_key(|(r, _)| *r)
        .map(|(_, p)| p.clone())
        .or_else(|| (images.len() == 1).then(|| images[0].clone()))
        .map(|p| p.to_string_lossy().into())
}

#[cfg(test)]
mod cover_tests {
    use super::folder_cover;

    #[test]
    fn finds_named_and_lone_covers() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("back.jpg"), b"x").unwrap();
        std::fs::write(dir.path().join("Front.PNG"), b"x").unwrap();
        std::fs::write(dir.path().join("Cover.jpg"), b"x").unwrap();
        assert!(folder_cover(dir.path()).unwrap().ends_with("Cover.jpg"));
        let lone = tempfile::tempdir().unwrap();
        std::fs::write(lone.path().join("scan0001.webp"), b"x").unwrap();
        assert!(
            folder_cover(lone.path())
                .unwrap()
                .ends_with("scan0001.webp")
        );
        let two = tempfile::tempdir().unwrap();
        std::fs::write(two.path().join("a.jpg"), b"x").unwrap();
        std::fs::write(two.path().join("b.jpg"), b"x").unwrap();
        assert_eq!(folder_cover(two.path()), None);
    }
}
