//! Times album/artist/genre summaries, value completion, and history statistics on a
//! synthetic library: `cargo run --release -p needle-core --example browse_benchmark -- DIR [TRACKS] [LISTENS]`
use anyhow::Result;
use needle_core::{
    database::Library,
    model::{Listen, Track},
};
use std::time::Instant;

fn time<T>(
    label: &str,
    mut run: impl FnMut() -> Result<T>,
    describe: impl Fn(&T) -> String,
) -> Result<()> {
    let mut times = vec![];
    let mut last = None;
    for _ in 0..5 {
        let start = Instant::now();
        last = Some(run()?);
        times.push(start.elapsed().as_secs_f64() * 1000.);
    }
    times.sort_by(f64::total_cmp);
    println!(
        "{label:<40} median {:>8.1} ms  max {:>8.1} ms  ({})",
        times[2],
        times[4],
        describe(last.as_ref().unwrap())
    );
    Ok(())
}

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let directory = args
        .next()
        .unwrap_or_else(|| "artifacts/browse-benchmark".into());
    let tracks: usize = args.next().and_then(|s| s.parse().ok()).unwrap_or(500_000);
    let listens: usize = args
        .next()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1_000_000);
    let library = Library::open(directory)?;
    let mut db = library.connection()?;
    if library.count()? < tracks {
        let tx = db.transaction()?;
        for index in library.count()?..tracks {
            Library::upsert_on(
                &tx,
                &Track {
                    id: format!("bench-{index:07}"),
                    path: format!("/benchmark/{index}.flac"),
                    title: format!("Track {index:07}"),
                    artist: format!("Artist {:05}", index % 10000),
                    album: format!("Album {:06}", index / 12),
                    album_artist: format!("Artist {:05}", (index / 12) % 10000),
                    genre: format!("Genre {:02}", index % 40),
                    year: 1980 + (index % 47) as i64,
                    track_number: (index % 12 + 1) as i64,
                    duration: 180. + (index % 120) as f64,
                    format: if index % 3 == 0 { "MP3" } else { "FLAC" }.into(),
                    artwork: (index % 12 == 0).then(|| format!("/art/{index}.jpg")),
                    ..Default::default()
                },
            )?;
        }
        tx.commit()?;
    }
    let existing = library.history_count()?;
    if existing < listens {
        let now = chrono::Utc::now().timestamp();
        let tx = db.transaction()?;
        for index in existing..listens {
            // Listening concentrates on a subset of the library.
            let track = (index * 7919) % tracks.min(50_000);
            let listen = Listen {
                id: format!("listen-{index:08}"),
                track_id: format!("bench-{track:07}"),
                title: format!("Track {track:07}"),
                artist: format!("Artist {:05}", track % 10000),
                album: format!("Album {:06}", track / 12),
                started_at: now - (index as i64 * 97) % (3 * 365 * 86400),
                listened_seconds: 30. + (index % 200) as f64,
                duration: 200.,
                qualified: index % 4 != 0,
            };
            tx.execute(
                "INSERT INTO listens(id,track_id,started_at,qualified,data) VALUES (?,?,?,?,?)",
                rusqlite::params![
                    listen.id,
                    listen.track_id,
                    listen.started_at,
                    listen.qualified,
                    serde_json::to_string(&listen)?
                ],
            )?;
        }
        tx.commit()?;
    }
    db.execute_batch("ANALYZE;")?;
    println!(
        "{} tracks, {} listens",
        library.count()?,
        library.history_count()?
    );
    let now = chrono::Utc::now().timestamp();
    time(
        "history_stats(all)",
        || library.history_stats(None),
        |s| format!("{} listens, {} days", s.listens, s.days.len()),
    )?;
    time(
        "history_stats(30 days)",
        || library.history_stats(Some(now - 30 * 86400)),
        |s| format!("{} listens", s.listens),
    )?;
    time(
        "history_page(10000, 200)",
        || library.history_page(10_000, 200),
        |p| format!("{} rows", p.len()),
    )?;
    time(
        "albums(\"\")",
        || library.albums(""),
        |a| format!("{} albums", a.len()),
    )?;
    time(
        "albums(\"year >= 2020\")",
        || library.albums("year >= 2020"),
        |a| format!("{} albums", a.len()),
    )?;
    time(
        "artists(\"\")",
        || library.artists(""),
        |a| format!("{} artists", a.len()),
    )?;
    time(
        "artist_albums",
        || library.artist_albums("Artist 00042"),
        |a| format!("{} albums", a.len()),
    )?;
    time(
        "top_tracks",
        || library.top_tracks("Artist 00042", 10),
        |a| format!("{} tracks", a.len()),
    )?;
    time(
        "genres",
        || library.genres(),
        |g| format!("{} genres", g.len()),
    )?;
    time(
        "field_values(artist, \"Artist 004\")",
        || library.field_values("artist", "Artist 004", 10),
        |v| format!("{} values", v.len()),
    )?;
    time(
        "field_values(album, \"\")",
        || library.field_values("album", "", 10),
        |v| format!("{} values", v.len()),
    )?;
    time(
        "suggest_with_library(genre = \"Ge)",
        || {
            Ok(needle_core::query::suggest_with_library(
                &library,
                "genre = \"Ge",
                11,
                10,
            ))
        },
        |v| format!("{} suggestions", v.len()),
    )?;
    Ok(())
}
