//! Live check of the online media sources: `cargo run -p needle-core --example media_probe`.
use needle_core::{database::Library, media, model::Track};

fn main() -> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    let library = Library::open(dir.path())?;
    let track = Track {
        title: "Karma Police".into(),
        artist: "Radiohead".into(),
        album: "OK Computer".into(),
        duration: 264.,
        ..Default::default()
    };
    let lyrics = media::lyrics(&library, &track, true, None)?;
    println!(
        "lyrics: {:?} lines, source {:?}",
        lyrics.as_ref().map(|l| l.lines.len()),
        lyrics.map(|l| l.source)
    );
    let image = media::artist_image(&library, "Björk", true)?;
    println!(
        "artist image: {:?} ({} bytes)",
        image.is_some(),
        image
            .map(|p| std::fs::metadata(p).map(|m| m.len()).unwrap_or(0))
            .unwrap_or(0)
    );
    Ok(())
}
