//! Decode and tag disposable fixtures without requiring an audio device.
use needle_core::{database::Library, scan};
use rodio::Source;
use std::{
    path::PathBuf,
    sync::{Arc, atomic::AtomicBool},
};
fn main() -> anyhow::Result<()> {
    let folder = PathBuf::from(
        std::env::args()
            .nth(1)
            .expect("Pass a folder of disposable audio fixtures"),
    );
    let temp = tempfile::tempdir()?;
    let library = Library::open(temp.path())?;
    let report = scan::import(&library, &folder, Arc::new(AtomicBool::new(false)), |_| {})?;
    anyhow::ensure!(
        report.errors.is_empty(),
        "Import errors: {:?}",
        report.errors
    );
    let mut failures = vec![];
    for track in library.search("")? {
        let decoder = needle_core::audio_file::decode(std::path::Path::new(&track.path))?;
        let channels = decoder.channels();
        let rate = decoder.sample_rate();
        let before: Vec<f32> = decoder.collect();
        anyhow::ensure!(!before.is_empty(), "No decoded audio in {}", track.path);
        let result = scan::write_tags(
            &library,
            &track.id,
            &scan::TagEdit {
                artist: Some("Needle format verification".into()),
                musicbrainz_id: Some("00000000-0000-4000-8000-000000000001".into()),
                ..Default::default()
            },
        );
        if let Err(error) = result {
            println!("{}: write safely rejected: {error:#}", track.path);
            failures.push(track.path.clone());
            continue;
        }
        let after: Vec<f32> =
            needle_core::audio_file::decode(std::path::Path::new(&track.path))?.collect();
        anyhow::ensure!(
            before == after,
            "PCM changed during tagging: {}",
            track.path
        );
        let saved = library.track(&track.id)?.unwrap();
        anyhow::ensure!(saved.artist == "Needle format verification");
        anyhow::ensure!(
            saved.musicbrainz_id.as_deref() == Some("00000000-0000-4000-8000-000000000001"),
            "Recording ID did not survive in {}",
            track.path
        );
        anyhow::ensure!(
            before.len() == 144000 * channels as usize,
            "Expected an exact three-second fixture: {}",
            track.path
        );
        println!(
            "{}: {} frames, {rate} Hz, {channels} ch; metadata import + tag write + identical decoded PCM passed",
            std::path::Path::new(&track.path)
                .file_name()
                .unwrap()
                .to_string_lossy(),
            before.len() / channels as usize
        );
    }
    anyhow::ensure!(failures.is_empty(), "Rejected tag writes: {failures:?}");
    Ok(())
}
