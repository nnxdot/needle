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
    /// Measure EBU R128 loudness and store ReplayGain for matching tracks.
    Loudness {
        expression: String,
        /// Also measure each matching track's whole album and store album gain.
        #[arg(long)]
        album: bool,
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
    /// Identify a file using AcoustID (requires `login acoustid` or NEEDLE_ACOUSTID_API_KEY).
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
    /// Sign in to a service; secrets are read from the terminal and kept in Credential Manager.
    Login {
        service: Service,
        /// Last.fm only: enter a new application API key and shared secret.
        #[arg(long)]
        new_app: bool,
    },
    /// Remove a service's stored credentials. Environment variables are not affected.
    Logout {
        service: Service,
    },
    /// Show account and scrobble-queue status without revealing secrets.
    Services,
}
#[derive(Clone, Copy, clap::ValueEnum)]
enum Service {
    Lastfm,
    /// The Last.fm application API key and shared secret.
    LastfmApp,
    Listenbrainz,
    Acoustid,
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
        None => ui::run(library, vec![]),
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
            let key = integrations::acoustid_key().unwrap_or_default();
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
                ..Default::default()
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
                integrations::flush_scrobbles(&library, &integrations::Credentials::load())?
            );
            println!(
                "{}",
                serde_json::to_string_pretty(&integrations::scrobble_summary(&library)?)?
            );
            Ok(())
        }
        Some(Commands::Login { service, new_app }) => login(service, new_app),
        Some(Commands::Logout { service }) => {
            use integrations::SecretKind;
            match service {
                Service::Lastfm => integrations::lastfm_sign_out()?,
                Service::LastfmApp => {
                    integrations::clear_secret(SecretKind::LastfmApiKey)?;
                    integrations::clear_secret(SecretKind::LastfmSecret)?;
                }
                Service::Listenbrainz => integrations::listenbrainz_sign_out()?,
                Service::Acoustid => integrations::clear_secret(SecretKind::AcoustidKey)?,
            }
            println!("Removed stored credentials. Environment variables, if set, still apply.");
            Ok(())
        }
        Some(Commands::Services) => {
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "accounts": integrations::secret_status(),
                    "scrobbles": integrations::scrobble_summary(&library)?,
                }))?
            );
            Ok(())
        }
        Some(Commands::Loudness { expression, album }) if album => {
            let mut measured = std::collections::HashSet::new();
            for track in library.search(&expression)? {
                let Some(key) = needle_core::analysis::album_key(&track) else {
                    eprintln!("{}: no album title; skipped", track.title);
                    continue;
                };
                if !measured.insert(key) {
                    continue;
                }
                let album = needle_core::analysis::scan_album_loudness(&library, &track.id)?;
                println!(
                    "{} — {}\t{:.2} LUFS\t{:+.2} dB album\tpeak {:.3}",
                    album.album_artist,
                    album.album,
                    album.integrated_lufs,
                    album.replay_gain_db,
                    album.true_peak
                );
                for track in album.tracks {
                    match track.loudness {
                        Some(l) => println!(
                            "  {}\t{:.2} LUFS\t{:+.2} dB",
                            track.title, l.integrated_lufs, l.replay_gain_db
                        ),
                        None => println!("  {}\ttoo quiet or short to measure", track.title),
                    }
                }
            }
            Ok(())
        }
        Some(Commands::Loudness { expression, .. }) => {
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
fn prompt(label: &str) -> Result<String> {
    use std::io::Write;
    eprint!("{label}: ");
    std::io::stderr().flush()?;
    let mut line = String::new();
    std::io::stdin().read_line(&mut line)?;
    let line = line.trim().to_string();
    anyhow::ensure!(!line.is_empty(), "Nothing entered");
    Ok(line)
}
fn login(service: Service, new_app: bool) -> Result<()> {
    use integrations::SecretKind;
    match service {
        Service::Listenbrainz => {
            let token = prompt("ListenBrainz user token (https://listenbrainz.org/settings/)")?;
            let user = integrations::listenbrainz_sign_in(&token)?;
            println!("Signed in to ListenBrainz as {user}");
        }
        Service::Acoustid => {
            integrations::save_secret(SecretKind::AcoustidKey, &prompt("AcoustID API key")?)?;
            println!("Saved. AcoustID checks the key on the first lookup.");
        }
        Service::LastfmApp => {
            integrations::save_secret(SecretKind::LastfmApiKey, &prompt("Last.fm API key")?)?;
            integrations::save_secret(SecretKind::LastfmSecret, &prompt("Last.fm shared secret")?)?;
            println!("Saved the Last.fm application key. Run `login lastfm` to sign in.");
        }
        Service::Lastfm => {
            let credentials = integrations::Credentials::load();
            let (key, secret) = if new_app
                || credentials.lastfm_api_key.is_empty()
                || credentials.lastfm_secret.is_empty()
            {
                (prompt("Last.fm API key")?, prompt("Last.fm shared secret")?)
            } else {
                (credentials.lastfm_api_key, credentials.lastfm_secret)
            };
            let pending = integrations::lastfm_begin(&key, &secret)?;
            println!(
                "Approve Needle in your browser:\n{}\nWaiting for approval (up to 10 minutes)...",
                pending.auth_url
            );
            #[cfg(windows)]
            let _ = std::process::Command::new("rundll32")
                .args(["url.dll,FileProtocolHandler", &pending.auth_url])
                .spawn();
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(600);
            loop {
                std::thread::sleep(std::time::Duration::from_secs(4));
                match integrations::lastfm_complete(&pending) {
                    Ok(user) => {
                        println!("Signed in to Last.fm as {user}");
                        break;
                    }
                    Err(error)
                        if integrations::is_not_yet_authorized(&error)
                            && std::time::Instant::now() < deadline => {}
                    Err(error) => return Err(error),
                }
            }
        }
    }
    Ok(())
}
fn uuid_string() -> String {
    format!(
        "{}-{}",
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default(),
        std::process::id()
    )
}
