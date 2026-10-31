//! Lyric cards: one line of a song's lyrics as a picture to share, in the colours of its cover,
//! with the cover, the song's name, and Needle's mark. The card is as tall as its words need
//! (a square at least); short lines are written larger. Saved to Pictures › Needle, and
//! copied, to paste anywhere.
use super::{AppView, Event};
use anyhow::{Context as _, Result};
use image::{Rgba, RgbaImage, imageops};
use needle_core::model::Track;
use std::path::{Path, PathBuf};

const WIDTH: u32 = 1080;
const COVER: u32 = 300;
const MARGIN: u32 = 84;
/// The most rows of words; a longer line is written smaller.
const MOST_ROWS: usize = 12;

/// The fonts the card is written in: Needle's own, so it looks the same everywhere, and this
/// computer's fonts after them, for the letters Needle's lack (Japanese, Korean, Chinese, …).
pub(super) fn fonts() -> usvg::fontdb::Database {
    let mut db = usvg::fontdb::Database::new();
    db.load_system_fonts();
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
pub(super) fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// How wide a letter is, in narrow letters: Japanese, Korean, and Chinese ones take two.
fn letter_width(c: char) -> usize {
    match c as u32 {
        0x1100..=0x115F
        | 0x2E80..=0xA4CF
        | 0xAC00..=0xD7A3
        | 0xF900..=0xFAFF
        | 0xFE30..=0xFE4F
        | 0xFF00..=0xFF60
        | 0xFFE0..=0xFFE6
        | 0x20000..=0x3FFFD => 2,
        _ => 1,
    }
}

fn text_width(text: &str) -> usize {
    text.chars().map(letter_width).sum()
}

/// The line cut into rows at most `width` narrow letters wide: at spaces, and between wide
/// letters (which need no spaces), and inside a word too long for a row.
fn wrap(text: &str, width: usize) -> Vec<String> {
    let width = width.max(2);
    // The pieces a row may break between: words, and each wide letter on its own.
    let mut pieces: Vec<(String, bool)> = vec![];
    for word in text.split_whitespace() {
        let mut spaced = true;
        let mut part = String::new();
        for c in word.chars() {
            if letter_width(c) == 2 {
                if !part.is_empty() {
                    pieces.push((std::mem::take(&mut part), spaced));
                    spaced = false;
                }
                pieces.push((c.to_string(), spaced));
                spaced = false;
            } else {
                part.push(c);
            }
        }
        if !part.is_empty() {
            pieces.push((part, spaced));
        }
    }
    let mut rows: Vec<String> = vec![];
    for (piece, spaced) in pieces {
        let gap = usize::from(spaced);
        match rows.last_mut() {
            Some(row) if text_width(row) + gap + text_width(&piece) <= width => {
                if spaced {
                    row.push(' ');
                }
                row.push_str(&piece);
            }
            _ => {
                // A word wider than a row is cut.
                let mut row = String::new();
                for c in piece.chars() {
                    if !row.is_empty() && text_width(&row) + letter_width(c) > width {
                        rows.push(std::mem::take(&mut row));
                    }
                    row.push(c);
                }
                rows.push(row);
            }
        }
    }
    rows
}

/// The font for a row with wide letters, from this computer (Needle's fonts have none). The
/// whole row is written in it: one font per row, as the drawing takes the row's first font.
fn wide_fonts(row: &str) -> Option<&'static str> {
    let has = |from: u32, to: u32| row.chars().any(|c| (from..=to).contains(&(c as u32)));
    if !row.chars().any(|c| letter_width(c) == 2) {
        None
    } else if has(0xAC00, 0xD7A3) || has(0x1100, 0x11FF) || has(0x3130, 0x318F) {
        Some(
            "'Malgun Gothic', 'Apple SD Gothic Neo', 'Noto Sans CJK KR', 'Noto Sans KR', sans-serif",
        )
    } else if has(0x3040, 0x30FF) {
        Some(
            "'Yu Gothic', 'Hiragino Sans', 'Noto Sans CJK JP', 'Noto Sans JP', 'Meiryo', sans-serif",
        )
    } else {
        Some(
            "'Microsoft YaHei', 'PingFang SC', 'Noto Sans CJK SC', 'Noto Sans SC', 'Yu Gothic', sans-serif",
        )
    }
}

/// A row's font and weight: Needle's, or the wide letters' one in bold.
fn row_font(row: &str, own: &str, weight: u32) -> String {
    match wide_fonts(row) {
        Some(fonts) => format!(r#"font-family="{fonts}" font-weight="700""#),
        None => format!(r#"font-family="{own}" font-weight="{weight}""#),
    }
}

/// The words' size and rows: as large as the line allows, from 104 down to 52.
fn layout(line: &str) -> (u32, Vec<String>) {
    let room = (WIDTH - 2 * MARGIN) as f32;
    let length = text_width(line);
    let mut size = 104;
    loop {
        // About half the size per narrow letter, in this font.
        let columns = (room / (size as f32 * 0.53)) as usize;
        let rows = wrap(line, columns);
        // Short lines stay large; longer ones step down until they fit in a few rows.
        let fits = rows.len()
            <= match length {
                0..=30 => 3,
                31..=90 => 5,
                _ => MOST_ROWS,
            };
        if fits || size <= 52 {
            return (size, rows.into_iter().take(MOST_ROWS).collect());
        }
        size -= 6;
    }
}

/// The cover, whatever its file is named (covers from a music server are kept as ".img").
fn open_cover(path: &str) -> Option<image::DynamicImage> {
    image::ImageReader::open(path)
        .ok()?
        .with_guessed_format()
        .ok()?
        .decode()
        .ok()
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
    let cover = track.artwork.as_deref().and_then(open_cover);
    let tone = cover.as_ref().map(cover_tone).unwrap_or([46, 38, 30]);
    let (size, rows) = layout(line.trim());
    let leading = size + size / 4;
    // Top to bottom: the cover, the words, then the song and Needle's mark at the foot. The
    // card is a square at least; then the words sit in the middle of the room left.
    let words_top = MARGIN + if cover.is_some() { COVER + 90 } else { 0 };
    let words = rows.len() as u32 * leading;
    let foot = 200;
    let height = (words_top + words + 110 + foot).max(WIDTH);
    let room = height - foot - words_top;
    let first = words_top + (room - words) / 2 + size;
    let mut card = RgbaImage::new(WIDTH, height);
    for (_, y, pixel) in card.enumerate_pixels_mut() {
        let t = y as f32 / height as f32;
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
    let mut text = String::new();
    for (i, row) in rows.iter().enumerate() {
        text.push_str(&format!(
            r##"<text xml:space="preserve" x="{MARGIN}" y="{}" {} font-size="{size}" fill="#ffffff">{}</text>"##,
            first + i as u32 * leading,
            row_font(
                row,
                "'Fraunces 72pt Soft', 'Fraunces 72pt Soft SemiBold', Fraunces, Inter",
                600
            ),
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
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{WIDTH}" height="{height}">{text}
<text xml:space="preserve" x="{MARGIN}" y="{}" {} font-size="34" fill="#ffffff" fill-opacity="0.85">{}</text>
<circle cx="{}" cy="{}" r="11" fill="#e2b46c"/>
<text x="{}" y="{}" font-family="Inter" font-weight="600" font-size="30" fill="#e2b46c">Needle</text>
</svg>"##,
        height - MARGIN - 70,
        row_font(&who, "Inter", 600),
        escape(&who),
        MARGIN + 11,
        height - MARGIN - 10,
        MARGIN + 34,
        height - MARGIN,
    );
    let options = usvg::Options {
        fontdb: std::sync::Arc::new(fonts()),
        ..Default::default()
    };
    let tree = usvg::Tree::from_str(&svg, &options).context("Drawing the words")?;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(WIDTH, height).context("No room to draw")?;
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::default(),
        &mut pixmap.as_mut(),
    );
    // The words over the background (tiny-skia keeps colours premultiplied).
    let words = RgbaImage::from_fn(WIDTH, height, |x, y| {
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
pub(super) fn folder() -> PathBuf {
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
                    Event::LyricCard(path)
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
        // Wide letters break without spaces; a long word is cut.
        assert_eq!(super::wrap("사랑해요 love", 6), ["사랑해", "요", "love"]);
        assert_eq!(super::wrap("abcdefgh", 3), ["abc", "def", "gh"]);
    }

    #[test]
    fn short_lines_are_larger_and_the_card_grows() {
        let (short, rows) = super::layout("Hold on");
        assert_eq!(rows.len(), 1);
        let (long, rows) = super::layout(&"the tide is turning slow ".repeat(8));
        assert!(long < short && rows.len() > 4);
        let track = needle_core::model::Track::default();
        let small = super::draw(&track, "Hold on").unwrap();
        let tall = super::draw(&track, &"the tide is turning slow ".repeat(8)).unwrap();
        assert_eq!(small.dimensions(), (super::WIDTH, super::WIDTH));
        assert!(tall.height() > super::WIDTH);
    }

    #[test]
    fn a_card_is_drawn_without_a_cover() {
        let track = needle_core::model::Track {
            title: "Undertow".into(),
            artist: "Mara Quinn".into(),
            ..Default::default()
        };
        let card = super::draw(&track, "I keep a lamp burning for you").unwrap();
        assert_eq!(card.width(), super::WIDTH);
        // NEEDLE_CARD_COVER and NEEDLE_CARD_OUT: draw one with that cover, to look at.
        if let (Ok(cover), Ok(out)) = (
            std::env::var("NEEDLE_CARD_COVER"),
            std::env::var("NEEDLE_CARD_OUT"),
        ) {
            let track = needle_core::model::Track {
                artwork: Some(cover),
                ..track
            };
            // NEEDLE_CARD_LINE: the line to write.
            let line = std::env::var("NEEDLE_CARD_LINE")
                .unwrap_or_else(|_| "Harbor lights are coming on, the tide is turning slow".into());
            super::draw(&track, &line).unwrap().save(out).unwrap();
        }
        // Words were written: some pixels are near white.
        assert!(
            card.pixels()
                .any(|p| p.0[0] > 230 && p.0[1] > 230 && p.0[2] > 230)
        );
    }
}
