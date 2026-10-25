//! Lyric cards: one line of a song's lyrics as a picture to share, in the colours of its cover,
//! with the cover, the song's name, and Needle's mark. Saved to Pictures › Needle.
use super::{AppView, Event};
use anyhow::{Context as _, Result};
use image::{Rgba, RgbaImage, imageops};
use needle_core::model::Track;
use std::path::{Path, PathBuf};

const WIDTH: u32 = 1080;
const HEIGHT: u32 = 1350;
const COVER: u32 = 300;
const MARGIN: u32 = 84;

/// The fonts the card is written in: Needle's own, so it looks the same everywhere.
fn fonts() -> usvg::fontdb::Database {
    let mut db = usvg::fontdb::Database::new();
    for data in [
        include_bytes!("../../../../assets/fonts/Fraunces72ptSoft-SemiBold.ttf").as_slice(),
        include_bytes!("../../../../assets/fonts/Inter-400.ttf").as_slice(),
        include_bytes!("../../../../assets/fonts/Inter-600.ttf").as_slice(),
    ] {
        db.load_font_data(data.to_vec());
    }
    db
}

/// Text made safe inside SVG.
fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// The line cut into rows of at most `width` characters, at spaces.
fn wrap(text: &str, width: usize) -> Vec<String> {
    let mut rows: Vec<String> = vec![];
    for word in text.split_whitespace() {
        match rows.last_mut() {
            Some(row) if row.chars().count() + 1 + word.chars().count() <= width => {
                row.push(' ');
                row.push_str(word);
            }
            _ => rows.push(word.to_string()),
        }
    }
    rows
}

/// The cover's colour: its average, made deep enough for white words to read on it.
fn cover_tone(cover: &image::DynamicImage) -> [u8; 3] {
    let small = cover.thumbnail_exact(1, 1).to_rgb8();
    let [r, g, b] = small.get_pixel(0, 0).0;
    let (r, g, b) = (r as f32, g as f32, b as f32);
    let light = 0.2126 * r + 0.7152 * g + 0.0722 * b;
    let scale = if light > 70. { 70. / light } else { 1. };
    [(r * scale) as u8, (g * scale) as u8, (b * scale) as u8]
}

/// Draw the card.
pub fn draw(track: &Track, line: &str) -> Result<RgbaImage> {
    let cover = track
        .artwork
        .as_deref()
        .and_then(|path| image::open(path).ok());
    let tone = cover.as_ref().map(cover_tone).unwrap_or([46, 38, 30]);
    // Top to bottom: the cover's colour, then a darker shade of it.
    let mut card = RgbaImage::new(WIDTH, HEIGHT);
    for (_, y, pixel) in card.enumerate_pixels_mut() {
        let t = y as f32 / HEIGHT as f32;
        let k = 1. - 0.55 * t;
        *pixel = Rgba([
            (tone[0] as f32 * k) as u8,
            (tone[1] as f32 * k) as u8,
            (tone[2] as f32 * k) as u8,
            255,
        ]);
    }
    if let Some(cover) = &cover {
        let square = cover
            .resize_to_fill(COVER, COVER, imageops::FilterType::Lanczos3)
            .to_rgba8();
        // Round corners, as Needle draws covers.
        let radius = 28.;
        let mut rounded = square.clone();
        for (x, y, pixel) in rounded.enumerate_pixels_mut() {
            let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
            let cx = fx.clamp(radius, COVER as f32 - radius);
            let cy = fy.clamp(radius, COVER as f32 - radius);
            let outside = ((fx - cx).powi(2) + (fy - cy).powi(2)).sqrt() - radius;
            if outside > 0. {
                pixel.0[3] = (255. * (1. - outside).clamp(0., 1.)) as u8;
            }
        }
        imageops::overlay(&mut card, &rounded, MARGIN as i64, MARGIN as i64);
    }
    let rows = wrap(line.trim(), 20);
    let size = if rows.len() > 4 { 64 } else { 78 };
    let top = MARGIN + COVER + 150;
    let mut text = String::new();
    for (i, row) in rows.iter().take(7).enumerate() {
        text.push_str(&format!(
            r##"<text x="{MARGIN}" y="{}" font-family="'Fraunces 72pt Soft', 'Fraunces 72pt Soft SemiBold', Fraunces, Inter" font-weight="600" font-size="{size}" fill="#ffffff">{}</text>"##,
            top + i as u32 * (size as u32 + 18),
            escape(row)
        ));
    }
    let who = [track.title.as_str(), track.display_artist()]
        .iter()
        .filter(|s| !s.is_empty())
        .cloned()
        .collect::<Vec<_>>()
        .join(" · ");
    let svg = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{WIDTH}" height="{HEIGHT}">{text}
<text x="{MARGIN}" y="{}" font-family="Inter" font-weight="600" font-size="34" fill="#ffffff" fill-opacity="0.85">{}</text>
<circle cx="{}" cy="{}" r="11" fill="#e2b46c"/>
<text x="{}" y="{}" font-family="Inter" font-weight="600" font-size="30" fill="#e2b46c">Needle</text>
</svg>"##,
        HEIGHT - MARGIN - 70,
        escape(&who),
        MARGIN + 11,
        HEIGHT - MARGIN - 10,
        MARGIN + 34,
        HEIGHT - MARGIN,
    );
    let mut options = usvg::Options::default();
    options.fontdb = std::sync::Arc::new(fonts());
    let tree = usvg::Tree::from_str(&svg, &options).context("Drawing the words")?;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(WIDTH, HEIGHT).context("No room to draw")?;
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::default(),
        &mut pixmap.as_mut(),
    );
    // The words over the background (tiny-skia keeps colours premultiplied).
    let words = RgbaImage::from_fn(WIDTH, HEIGHT, |x, y| {
        let c = pixmap
            .pixel(x, y)
            .map(|p| p.demultiply())
            .unwrap_or_else(|| resvg::tiny_skia::PremultipliedColorU8::TRANSPARENT.demultiply());
        Rgba([c.red(), c.green(), c.blue(), c.alpha()])
    });
    imageops::overlay(&mut card, &words, 0, 0);
    Ok(card)
}

/// Pictures › Needle, made when needed.
fn folder() -> PathBuf {
    let pictures = std::env::var_os("USERPROFILE")
        .map(|home| Path::new(&home).join("Pictures"))
        .or_else(|| std::env::var_os("HOME").map(|home| Path::new(&home).join("Pictures")))
        .unwrap_or_else(std::env::temp_dir);
    pictures.join("Needle")
}

impl AppView {
    /// Make a card of this line of the playing song, in the background, and say where it went.
    pub(super) fn share_lyric(&mut self, track: Track, line: String) {
        let sender = self.sender.clone();
        self.notify("Making a picture of that line…");
        std::thread::spawn(move || {
            let result = (|| -> Result<PathBuf> {
                let card = draw(&track, &line)?;
                let dir = folder();
                std::fs::create_dir_all(&dir)?;
                let name: String = format!("{} - lyric", track.title)
                    .chars()
                    .map(|c| {
                        if "<>:\"/\\|?*".contains(c) || c.is_control() {
                            '_'
                        } else {
                            c
                        }
                    })
                    .collect();
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
                Err(e) => Event::Error(format!("The lyric picture: {e:#}")),
            });
        });
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn lines_wrap_at_spaces() {
        assert_eq!(
            super::wrap("I keep a lamp burning for you", 12),
            ["I keep a", "lamp burning", "for you"]
        );
        assert!(super::wrap("   ", 10).is_empty());
    }

    #[test]
    fn a_card_is_drawn_without_a_cover() {
        let track = needle_core::model::Track {
            title: "Undertow".into(),
            artist: "Mara Quinn".into(),
            ..Default::default()
        };
        let card = super::draw(&track, "I keep a lamp burning for you").unwrap();
        assert_eq!(card.dimensions(), (super::WIDTH, super::HEIGHT));
        // NEEDLE_CARD_COVER and NEEDLE_CARD_OUT: draw one with that cover, to look at.
        if let (Ok(cover), Ok(out)) = (
            std::env::var("NEEDLE_CARD_COVER"),
            std::env::var("NEEDLE_CARD_OUT"),
        ) {
            let track = needle_core::model::Track {
                artwork: Some(cover),
                ..track
            };
            super::draw(
                &track,
                "Harbor lights are coming on, the tide is turning slow",
            )
            .unwrap()
            .save(out)
            .unwrap();
        }
        // Words were written: some pixels are near white.
        assert!(
            card.pixels()
                .any(|p| p.0[0] > 230 && p.0[1] > 230 && p.0[2] > 230)
        );
    }
}
