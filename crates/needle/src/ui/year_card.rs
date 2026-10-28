//! "Your 2026 in music" as a picture to share: the hours, the year by month, the top artist,
//! and the top songs, in Needle's own fonts. Saved to Pictures › Needle, as lyric cards are.
use super::{
    AppView, Event,
    lyric_card::{escape, folder, fonts},
    widgets::count,
};
use anyhow::{Context as _, Result};
use image::{Rgba, RgbaImage};
use needle_core::wrapped::Wrapped;
use std::path::PathBuf;

const WIDTH: u32 = 1080;
const HEIGHT: u32 = 1350;
const MARGIN: u32 = 84;

/// The big number and what it counts: hours, or minutes for a short year.
pub fn time_listened(seconds: f64) -> (String, &'static str) {
    let hours = (seconds / 3600.).round() as usize;
    if hours >= 2 {
        (count(hours), "hours of music")
    } else {
        (count((seconds / 60.).round() as usize), "minutes of music")
    }
}

/// "53 days · 21,880 plays · 412 new songs".
pub fn year_line(w: &Wrapped) -> String {
    let mut parts = vec![];
    let days = (w.stats.seconds / 86400.).floor() as usize;
    if days >= 1 {
        parts.push(if days == 1 {
            "1 day".to_string()
        } else {
            format!("{} days", count(days))
        });
    }
    parts.push(if w.stats.plays == 1 {
        "1 play".into()
    } else {
        format!("{} plays", count(w.stats.plays))
    });
    if w.new_songs > 0 {
        parts.push(if w.new_songs == 1 {
            "1 new song".into()
        } else {
            format!("{} new songs", count(w.new_songs))
        });
    }
    parts.join(" · ")
}

/// "212 hours · in your top 5 every month".
pub fn artist_line(w: &Wrapped) -> String {
    let Some(top) = w.stats.top_artists.first() else {
        return String::new();
    };
    let hours = (top.seconds / 3600.).round() as usize;
    let time = if hours >= 2 {
        format!("{} hours", count(hours))
    } else {
        format!("{} min", (top.seconds / 60.).round() as usize)
    };
    let months = if w.listening_months > 1 && w.top_artist_months == w.listening_months {
        "in your top 5 every month".to_string()
    } else if w.top_artist_months > 1 {
        format!("in your top 5 for {} months", w.top_artist_months)
    } else if top.plays == 1 {
        "1 play".into()
    } else {
        format!("{} plays", count(top.plays))
    };
    format!("{time} · {months}")
}

/// Draw the picture.
pub fn draw(w: &Wrapped) -> Result<RgbaImage> {
    // Night with an amber glow at the top right and violet at the foot, as on the page.
    let card = RgbaImage::from_fn(WIDTH, HEIGHT, |x, y| {
        let (fx, fy) = (x as f32 / WIDTH as f32, y as f32 / HEIGHT as f32);
        let glow = |cx: f32, cy: f32, r: f32| {
            (1. - (((fx - cx).powi(2) + (fy - cy).powi(2)).sqrt() / r)).clamp(0., 1.)
        };
        let (a, v) = (glow(0.95, 0., 0.75), glow(0., 1., 0.8));
        let base = [26., 20., 16.];
        let amber = [162., 100., 58.];
        let violet = [59., 42., 85.];
        let mix = |i: usize| {
            (base[i] + (amber[i] - base[i]) * a * 0.8 + (violet[i] - base[i]) * v * 0.8) as u8
        };
        Rgba([mix(0), mix(1), mix(2), 255])
    });
    let (number, what) = time_listened(w.stats.seconds);
    let mut svg = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{WIDTH}" height="{HEIGHT}">
<text x="{MARGIN}" y="150" font-family="Inter" font-weight="600" font-size="30" letter-spacing="6" fill="#e2b46c">YOUR {} IN MUSIC</text>
<text x="{MARGIN}" y="360" font-family="'Fraunces 72pt Soft', Fraunces, Inter" font-weight="600" font-size="210" fill="#e2b46c">{}</text>
<text x="{MARGIN}" y="430" font-family="Inter" font-weight="600" font-size="46" fill="#ffffff">{}</text>
<text x="{MARGIN}" y="482" font-family="Inter" font-weight="400" font-size="32" fill="#ffffff" fill-opacity="0.65">{}</text>
"##,
        w.year,
        escape(&number),
        what,
        escape(&year_line(w)),
    );
    // The year by month.
    let plays: Vec<f64> = w.months.iter().map(|m| m.plays as f64).collect();
    let most = plays.iter().cloned().fold(0., f64::max).max(1.);
    let top = plays
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.total_cmp(b.1))
        .map(|(i, _)| i);
    let (bars_top, bars_height, gap) = (530., 150., 12.);
    let bar = (WIDTH as f64 - 2. * MARGIN as f64 - 11. * gap) / 12.;
    for (i, value) in plays.iter().enumerate() {
        let h = (value / most * bars_height).max(4.);
        svg.push_str(&format!(
            r##"<rect x="{:.1}" y="{:.1}" width="{bar:.1}" height="{h:.1}" rx="6" fill="#e2b46c" fill-opacity="{}"/>"##,
            MARGIN as f64 + i as f64 * (bar + gap),
            bars_top + bars_height - h,
            if Some(i) == top { "1" } else { "0.35" },
        ));
    }
    let mut y = 800;
    if let Some(artist) = w.stats.top_artists.first() {
        svg.push_str(&format!(
            r##"<text x="{MARGIN}" y="{y}" font-family="Inter" font-size="28" fill="#ffffff" fill-opacity="0.6">Top artist</text>
<text x="{MARGIN}" y="{}" font-family="Inter" font-weight="600" font-size="54" fill="#ffffff">{}</text>
<text x="{MARGIN}" y="{}" font-family="Inter" font-size="30" fill="#ffffff" fill-opacity="0.65">{}</text>
"##,
            y + 64,
            escape(&artist.artist),
            y + 110,
            escape(&artist_line(w)),
        ));
        y += 190;
    }
    svg.push_str(&format!(
        r##"<text x="{MARGIN}" y="{y}" font-family="Inter" font-size="28" fill="#ffffff" fill-opacity="0.6">Top songs</text>"##
    ));
    for (i, (id, plays)) in w
        .top_track_ids
        .iter()
        .zip(&w.top_track_plays)
        .take(3)
        .enumerate()
    {
        let Some(track) = w.tracks.get(id) else {
            continue;
        };
        let row = y + 58 + i as u32 * 56;
        let mut title = track.title.clone();
        if title.chars().count() > 30 {
            title = title.chars().take(29).collect::<String>() + "…";
        }
        svg.push_str(&format!(
            r##"<text x="{MARGIN}" y="{row}" font-family="Inter" font-weight="600" font-size="34" fill="#e2b46c">{}</text>
<text x="{}" y="{row}" font-family="Inter" font-weight="600" font-size="34" fill="#ffffff">{}</text>
<text x="{}" y="{row}" text-anchor="end" font-family="Inter" font-size="30" fill="#ffffff" fill-opacity="0.6">{}</text>
"##,
            i + 1,
            MARGIN + 50,
            escape(&title),
            WIDTH - MARGIN,
            count(*plays),
        ));
    }
    svg.push_str(&format!(
        r##"<circle cx="{}" cy="{}" r="11" fill="#e2b46c"/>
<text x="{}" y="{}" font-family="Inter" font-weight="600" font-size="30" fill="#e2b46c">Needle</text>
</svg>"##,
        MARGIN + 11,
        HEIGHT - MARGIN - 10,
        MARGIN + 34,
        HEIGHT - MARGIN,
    ));
    let options = usvg::Options {
        fontdb: std::sync::Arc::new(fonts()),
        ..Default::default()
    };
    let tree = usvg::Tree::from_str(&svg, &options).context("Drawing the picture")?;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(WIDTH, HEIGHT).context("No room to draw")?;
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::default(),
        &mut pixmap.as_mut(),
    );
    let words = RgbaImage::from_fn(WIDTH, HEIGHT, |x, y| {
        let c = pixmap
            .pixel(x, y)
            .map(|p| p.demultiply())
            .unwrap_or_else(|| resvg::tiny_skia::PremultipliedColorU8::TRANSPARENT.demultiply());
        Rgba([c.red(), c.green(), c.blue(), c.alpha()])
    });
    let mut card = card;
    image::imageops::overlay(&mut card, &words, 0, 0);
    Ok(card)
}

impl AppView {
    /// Save the year as a picture, in the background, and say where it went.
    pub(super) fn save_year_picture(&mut self) {
        let Some(wrapped) = self.wrapped.clone() else {
            return;
        };
        let sender = self.sender.clone();
        self.notify("Making a picture of your year…");
        std::thread::spawn(move || {
            let result = (|| -> Result<PathBuf> {
                let card = draw(&wrapped)?;
                let dir = folder();
                std::fs::create_dir_all(&dir)?;
                let name = format!("Your {} in music", wrapped.year);
                let mut path = dir.join(format!("{name}.png"));
                let mut n = 2;
                while path.exists() {
                    path = dir.join(format!("{name} {n}.png"));
                    n += 1;
                }
                card.save(&path)?;
                Ok(path)
            })();
            let _ = sender.send(match result {
                Ok(path) => {
                    super::files::show_file(&path);
                    Event::Notice(format!(
                        "Saved {} in Pictures › Needle.",
                        path.file_name().unwrap_or_default().to_string_lossy()
                    ))
                }
                Err(e) => Event::Error(format!("The picture of your year: {e:#}")),
            });
        });
    }
}

#[cfg(test)]
mod tests {
    use needle_core::{
        history::{HistoryStats, HistoryTop},
        model::Track,
        wrapped::{MonthStat, Wrapped},
    };

    fn year() -> Wrapped {
        let mut w = Wrapped {
            year: 2026,
            stats: HistoryStats {
                plays: 21880,
                seconds: 1284. * 3600.,
                top_artists: vec![HistoryTop {
                    artist: "Ólafur Kett".into(),
                    plays: 1903,
                    seconds: 212. * 3600.,
                    ..Default::default()
                }],
                ..Default::default()
            },
            new_songs: 412,
            top_artist_months: 12,
            listening_months: 12,
            top_track_ids: vec!["a".into()],
            top_track_plays: vec![214],
            months: (1..=12)
                .map(|month| MonthStat {
                    month,
                    plays: month as usize * 10,
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        };
        w.tracks.insert(
            "a".into(),
            Track {
                id: "a".into(),
                title: "Slow Satellites & <friends>".into(),
                ..Default::default()
            },
        );
        w
    }

    #[test]
    fn the_lines_read_as_in_the_mockup() {
        let w = year();
        assert_eq!(
            super::time_listened(w.stats.seconds),
            ("1,284".to_string(), "hours of music")
        );
        assert_eq!(
            super::year_line(&w),
            "53 days · 21,880 plays · 412 new songs"
        );
        assert_eq!(
            super::artist_line(&w),
            "212 hours · in your top 5 every month"
        );
        assert_eq!(super::time_listened(600.).1, "minutes of music");
    }

    #[test]
    fn the_picture_draws() {
        let card = super::draw(&year()).unwrap();
        assert_eq!(card.dimensions(), (super::WIDTH, super::HEIGHT));
    }
}
