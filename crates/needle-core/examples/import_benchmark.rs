//! Times adding a folder of generated songs, as a first scan and as a rescan.
//!
//! `cargo run --release -p needle-core --example import_benchmark -- DIR [SONGS] [MB]`
use needle_core::{database::Library, scan};
use std::{
    path::PathBuf,
    sync::{Arc, atomic::AtomicBool},
    time::Instant,
};

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let dir = PathBuf::from(args.next().expect("a folder for the test songs"));
    let songs: usize = args.next().map_or(Ok(600), |a| a.parse())?;
    let megabytes: f64 = args.next().map_or(Ok(4.), |a| a.parse())?;
    let music = dir.join("music");
    std::fs::create_dir_all(&music)?;
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: 44100,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let frames = (megabytes * 1024. * 1024. / 4.) as usize;
    for n in 0..songs {
        let path = music.join(format!("Album {:03}/{:02} Song.wav", n / 12, n % 12));
        std::fs::create_dir_all(path.parent().unwrap())?;
        let mut writer = hound::WavWriter::create(&path, spec)?;
        for i in 0..frames {
            let s = ((i * (n + 3)) % 2000) as i16;
            writer.write_sample(s)?;
            writer.write_sample(-s)?;
        }
        writer.finalize()?;
    }
    let library = Library::open(dir.join("library"))?;
    let start = Instant::now();
    let first = scan::import(&library, &music, Arc::new(AtomicBool::new(false)), |_| {})?;
    let took = start.elapsed().as_secs_f64();
    let mut again = first.clone();
    let mut rescan = 0.;
    for round in 1..=3 {
        let start = Instant::now();
        again = scan::import(&library, &music, Arc::new(AtomicBool::new(false)), |_| {})?;
        rescan = start.elapsed().as_secs_f64();
        println!("rescan {round}: {rescan:.2} s");
    }
    let start = Instant::now();
    for entry in walkdir::WalkDir::new(&music).into_iter().flatten() {
        if entry.file_type().is_file() {
            scan::full_hash(entry.path())?;
        }
    }
    let whole = start.elapsed().as_secs_f64();
    println!(
        "{songs} songs of {megabytes} MB: first scan {took:.2} s ({:.1} ms a song, {} added, {} errors); rescan {rescan:.2} s ({} unchanged); reading every whole file, as 1.0.0 did, {whole:.2} s",
        took * 1000. / songs as f64,
        first.imported,
        first.errors.len(),
        again.unchanged,
    );
    Ok(())
}
