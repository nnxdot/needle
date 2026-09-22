#![recursion_limit = "1024"]
mod ui;
use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use needle_core::{audio, database::Library, demo, integrations, model::Playlist, scan, sync};
use std::{
    path::PathBuf,
    sync::{Arc, atomic::AtomicBool},
};

#[derive(Parser)]
#[command(
    name = "needle",
    version,
    about = "A library-first desktop music player"
)]
struct Cli {
    /// Store the library in an alternate directory.
    #[arg(long, global = true)]
    data_dir: Option<PathBuf>,
    #[command(subcommand)]
    command: Option<Commands>,
}
#[derive(Subcommand)]
enum Commands {
    Scan {
        folder: PathBuf,
    },
    Search {
        #[arg(default_value = "")]
        expression: String,
        #[arg(long)]
        json: bool,
    },
    Play {
        file: PathBuf,
    },
    Exclusive {
        file: PathBuf,
    },
    Devices,
    Demo,
    History {
        #[arg(long, default_value_t = 100)]
        limit: usize,
    },
    Playlist {
        name: String,
        #[arg(long)]
        query: String,
    },
    Export {
        playlist: String,
        destination: PathBuf,
    },
    ImportPlaylist {
        file: PathBuf,
    },
    Backup {
        destination: PathBuf,
    },
    SyncExport {
        destination: PathBuf,
    },
    SyncImport {
        file: PathBuf,
    },
    Lookup {
        artist: String,
        title: String,
    },
    Scrobble,
    Loudness {
        expression: String,
    },
    Fingerprint {
        file: PathBuf,
    },
    Duplicates {
        #[arg(default_value = "")]
        expression: String,
    },
    Doctor,
    /// Preview tag edits across matching tracks. Pass --apply to write with backups.
    Tag {
        expression: String,
        changes: PathBuf,
        #[arg(long)]
        apply: bool,
    },
    /// Identify a file using AcoustID (requires NEEDLE_ACOUSTID_API_KEY).
    Identify {
        file: PathBuf,
    },
    LayoutExport {
        destination: PathBuf,
    },
    LayoutImport {
        file: PathBuf,
    },
    Cover {
        track_id: String,
        release_id: String,
    },
}
fn main() {
    if let Err(error) = run() {
        eprintln!("Needle: {error:#}");
        std::process::exit(1);
    }
}
fn run() -> Result<()> {
    let cli = Cli::parse();
    let library = Library::open(cli.data_dir.unwrap_or_else(Library::default_directory))?;
    match cli.command {
        None => ui::run(library),
        Some(Commands::Tag {
            expression,
            changes,
            apply,
        }) => {
            let edit: scan::TagEdit = serde_json::from_slice(&std::fs::read(changes)?)?;
            let tracks = library.search(&expression)?;
            for track in &tracks {
                println!(
                    "{}",
                    serde_json::to_string(
                        &serde_json::json!({"id":track.id,"path":track.path,"before":{"title":track.title,"artist":track.artist,"album":track.album,"album_artist":track.album_artist,"genre":track.genre,"year":track.year,"track_number":track.track_number,"musicbrainz_id":track.musicbrainz_id},"changes":edit})
                    )?
                );
            }
            if apply {
                let ids: Vec<String> = tracks.into_iter().map(|t| t.id).collect();
                let report = scan::write_tags_batch(
                    &library,
                    &ids,
                    &edit,
                    Arc::new(AtomicBool::new(false)),
                    |p| {
                        if !p.current.is_empty() {
                            eprintln!("[{}/{}] {}", p.done + 1, p.total, p.current);
                        }
                    },
                )?;
                println!(
                    "Saved {} files with backups; {} errors",
                    report.saved.len(),
                    report.failed.len()
                );
                if !report.failed.is_empty() {
                    anyhow::bail!(
                        "{}",
                        report
                            .failed
                            .iter()
                            .map(|f| format!("{}: {}", f.path, f.error))
                            .collect::<Vec<_>>()
                            .join("\n")
                    );
                }
            } else {
                println!(
                    "Preview only. Add --apply to write these tags with original-file backups."
                );
            }
            Ok(())
        }
        Some(Commands::Identify { file }) => {
            let key = std::env::var("NEEDLE_ACOUSTID_API_KEY").unwrap_or_default();
            println!(
                "{}",
                serde_json::to_string_pretty(&integrations::acoustid_lookup(
                    &library, &file, &key
                )?)?
            );
            Ok(())
        }
        Some(Commands::Cover {
            track_id,
            release_id,
        }) => {
            println!(
                "{}",
                integrations::cover_art(&library, &track_id, &release_id)?.display()
            );
            Ok(())
        }
        Some(Commands::LayoutExport { destination }) => {
            std::fs::write(
                destination,
                serde_json::to_vec_pretty(&library.settings()?.layout)?,
            )?;
            Ok(())
        }
        Some(Commands::LayoutImport { file }) => {
            let layout: needle_core::model::Layout = serde_json::from_slice(&std::fs::read(file)?)?;
            layout.validate()?;
            let mut settings = library.settings()?;
            settings.layout = layout;
            library.save_settings(&settings)?;
            println!("Layout imported; reopen Needle to apply it.");
            Ok(())
        }
        Some(Commands::Scan { folder }) => {
            let report = scan::import(&library, &folder, Arc::new(AtomicBool::new(false)), |p| {
                if p.done {
                    println!(
                        "{} scanned; {} imported; {} unchanged; {} errors",
                        p.scanned,
                        p.imported,
                        p.unchanged,
                        p.errors.len()
                    );
                }
            })?;
            for error in &report.errors {
                eprintln!("{error}");
            }
            Ok(())
        }
        Some(Commands::Search { expression, json }) => {
            let tracks = library.search(&expression)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&tracks)?)
            } else {
                for t in tracks {
                    println!(
                        "{}\t{}\t{}\t{}\t{}",
                        t.id,
                        t.artist,
                        t.title,
                        t.album,
                        needle_core::model::format_duration(t.duration)
                    );
                }
            }
            Ok(())
        }
        Some(Commands::Play { file }) => {
            scan::import_one(&library, &file)?;
            let track = library
                .track_by_path(&file.canonicalize()?.to_string_lossy())?
                .context("Track was not imported")?;
            let player = audio::Player::new(library);
            player.send(audio::Command::Play(vec![audio::QueueItem {
                track,
                reason: "Selected from command line".into(),
            }]));
            std::thread::sleep(std::time::Duration::from_millis(500));
            loop {
                let state = player.state();
                if let Some(error) = state.error {
                    anyhow::bail!("{error}")
                }
                if !state.playing {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(250));
            }
            player.shutdown();
            Ok(())
        }
        Some(Commands::Exclusive { file }) => audio::play_exclusive(&file),
        Some(Commands::Devices) => {
            for device in audio::devices()? {
                println!("{device}")
            }
            Ok(())
        }
        Some(Commands::Demo) => {
            let path = library.directory.join("demo");
            demo::create(&path)?;
            scan::import(&library, &path, Arc::new(AtomicBool::new(false)), |_| {})?;
            println!("Demo library ready: {}", path.display());
            Ok(())
        }
        Some(Commands::History { limit }) => {
            println!(
                "{}",
                serde_json::to_string_pretty(&library.history(limit)?)?
            );
            Ok(())
        }
        Some(Commands::Playlist { name, query }) => {
            let playlist = Playlist {
                id: uuid_string(),
                name,
                query: Some(query),
                track_ids: vec![],
                updated_at: chrono::Utc::now().timestamp(),
            };
            library.save_playlist(&playlist)?;
            println!("Saved {}", playlist.name);
            Ok(())
        }
        Some(Commands::Export {
            playlist,
            destination,
        }) => {
            let list = library
                .playlists()?
                .into_iter()
                .find(|p| p.id == playlist || p.name == playlist)
                .context("Playlist not found")?;
            library.export_playlist(&list, &destination)
        }
        Some(Commands::ImportPlaylist { file }) => {
            let p = library.import_playlist(&file)?;
            println!("Imported {} tracks into {}", p.track_ids.len(), p.name);
            Ok(())
        }
        Some(Commands::Backup { destination }) => library.backup(&destination),
        Some(Commands::SyncExport { destination }) => sync::export(
            &library,
            &destination,
            &std::env::var("NEEDLE_SYNC_PASSPHRASE")
                .context("Set NEEDLE_SYNC_PASSPHRASE (at least 12 characters)")?,
        ),
        Some(Commands::SyncImport { file }) => {
            println!(
                "{:?}",
                sync::import(
                    &library,
                    &file,
                    &std::env::var("NEEDLE_SYNC_PASSPHRASE")
                        .context("Set NEEDLE_SYNC_PASSPHRASE")?
                )?
            );
            Ok(())
        }
        Some(Commands::Lookup { artist, title }) => {
            println!(
                "{}",
                serde_json::to_string_pretty(&integrations::musicbrainz_search(
                    &library, &artist, &title
                )?)?
            );
            Ok(())
        }
        Some(Commands::Scrobble) => {
            println!(
                "Submitted {} listens",
                integrations::flush_scrobbles(
                    &library,
                    &integrations::Credentials::from_environment()
                )?
            );
            println!("{:?}", integrations::scrobble_status(&library)?);
            Ok(())
        }
        Some(Commands::Loudness { expression }) => {
            for track in library.search(&expression)? {
                let measurement = needle_core::analysis::scan_loudness(&library, &track.id)?;
                println!(
                    "{}\t{:.2} LUFS\t{:+.2} dB",
                    track.title, measurement.integrated_lufs, measurement.replay_gain_db
                );
            }
            Ok(())
        }
        Some(Commands::Fingerprint { file }) => {
            println!(
                "{}",
                serde_json::to_string_pretty(&needle_core::analysis::fingerprint(&file)?)?
            );
            Ok(())
        }
        Some(Commands::Duplicates { expression }) => {
            println!(
                "{}",
                serde_json::to_string_pretty(&needle_core::analysis::duplicates(
                    &library,
                    &expression
                )?)?
            );
            Ok(())
        }
        Some(Commands::Doctor) => {
            println!(
                "Needle {}\nLibrary: {}\nTracks: {}\nPlatform: {}\nAudio outputs: {}\nPlayback: shared; Windows WASAPI exclusive\n",
                env!("CARGO_PKG_VERSION"),
                library.directory.display(),
                library.count()?,
                std::env::consts::OS,
                audio::devices()?.join(", ")
            );
            Ok(())
        }
    }
}
fn uuid_string() -> String {
    format!(
        "{}-{}",
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default(),
        std::process::id()
    )
}
