use anyhow::Result;
use needle_core::{database::Library, model::Track};
use std::time::Instant;
fn main() -> Result<()> {
    let directory = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "artifacts/benchmark-library".into());
    let count: usize = std::env::args()
        .nth(2)
        .and_then(|s| s.parse().ok())
        .unwrap_or(500_000);
    let library = Library::open(directory)?;
    if library.count()? < count {
        let start = Instant::now();
        let mut db = library.connection()?;
        let tx = db.transaction()?;
        for index in library.count()?..count {
            let track = Track {
                id: format!("bench-{index:07}"),
                path: format!("/benchmark/{index}.flac"),
                title: format!("Track {index:07}"),
                artist: format!("Artist {:05}", index % 10000),
                album: format!("Album {:06}", index / 12),
                album_artist: format!("Artist {:05}", index % 10000),
                year: 1980 + (index % 47) as i64,
                bpm: Some(60. + (index % 120) as f64),
                rating: (index % 6) as i64,
                track_number: (index % 12 + 1) as i64,
                duration: 180. + (index % 120) as f64,
                format: "FLAC".into(),
                ..Default::default()
            };
            Library::upsert_on(&tx, &track)?;
        }
        tx.commit()?;
        db.execute_batch("ANALYZE;")?;
        println!(
            "Indexed {count} tracks in {:.2}s",
            start.elapsed().as_secs_f64()
        );
    }
    for expression in [
        "Artist 00420",
        "Track 0123456",
        "year >= 2020 and rating >= 4 order by rating desc limit 100",
        "bpm > 170 limit 100",
        "",
    ] {
        let mut times = vec![];
        let mut total = 0;
        for _ in 0..7 {
            let start = Instant::now();
            let page = library.search_page(expression, 0, 100)?;
            total = page.total;
            times.push(start.elapsed().as_secs_f64() * 1000.);
        }
        if count == 500_000 && expression == "Artist 00420" {
            assert_eq!(total, 50);
        }
        if count == 500_000 && expression == "Track 0123456" {
            assert_eq!(total, 1);
        }
        times.sort_by(f64::total_cmp);
        println!(
            "{expression:?}: {} matches; median {:.1} ms; max {:.1} ms; first 100 results",
            total, times[3], times[6]
        );
    }
    Ok(())
}
