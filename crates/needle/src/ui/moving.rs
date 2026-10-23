//! Moving covers (animated WebP and GIF) for the big player, ready to draw. GPUI keeps every
//! frame of an animation as pixels, so a large one is made smaller first, on another thread:
//! a 600 px moving cover of 337 frames is about 485 MB as it comes, too much to keep.

use gpui::RenderImage;
use image::{AnimationDecoder, Frame, RgbaImage, imageops::FilterType};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, LazyLock, Mutex},
};

/// The most memory one moving cover's frames may take, in bytes.
const BUDGET: f64 = 160.0 * 1024.0 * 1024.0;
/// Frames are never made smaller than this, in pixels across.
const SMALLEST: u32 = 240;

/// Each moving cover as prepared: `None` while it is being made, or when it cannot be.
static READY: LazyLock<Mutex<HashMap<PathBuf, Option<Arc<RenderImage>>>>> =
    LazyLock::new(Default::default);

/// The moving cover at `path`, ready to draw, or `None` until it is (the first time it is
/// made on another thread; the still cover shows until then).
pub fn image(path: &Path) -> Option<Arc<RenderImage>> {
    let mut ready = READY.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(found) = ready.get(path) {
        return found.clone();
    }
    // Only the last few: the song playing now and the ones just before it.
    if ready.len() >= 4 {
        ready.clear();
    }
    ready.insert(path.to_path_buf(), None);
    let path = path.to_path_buf();
    std::thread::spawn(move || {
        let made = prepare(&path).map(Arc::new);
        READY
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(path, made);
    });
    None
}

/// Read an animated WebP or GIF, make its frames small enough for `BUDGET`, and turn them to
/// the BGRA pixels GPUI draws.
fn prepare(path: &Path) -> Option<RenderImage> {
    use image::ImageDecoder;
    let bytes = std::fs::read(path).ok()?;
    let cursor = || std::io::Cursor::new(&bytes);
    // The size and the number of frames first, so each frame is made smaller as it is read
    // (reading them all at full size first would take that memory for a moment).
    let (frames, (w, h)): (
        Box<dyn Iterator<Item = image::ImageResult<Frame>>>,
        (u32, u32),
    ) = match image::guess_format(&bytes).ok()? {
        image::ImageFormat::Gif => {
            let decoder = image::codecs::gif::GifDecoder::new(cursor()).ok()?;
            let size = decoder.dimensions();
            (Box::new(decoder.into_frames()), size)
        }
        image::ImageFormat::WebP => {
            let mut decoder = image::codecs::webp::WebPDecoder::new(cursor()).ok()?;
            if !decoder.has_animation() {
                return None;
            }
            let _ = decoder.set_background_color(image::Rgba([0, 0, 0, 0]));
            let size = decoder.dimensions();
            (Box::new(decoder.into_frames()), size)
        }
        _ => return None,
    };
    let count = frame_count(&bytes).max(1);
    let total = w as f64 * h as f64 * 4.0 * count as f64;
    let scale = (BUDGET / total).sqrt().min(1.0);
    let target = ((w as f64 * scale) as u32).max(SMALLEST.min(w));
    let shrink = target < w;
    let mut out = Vec::with_capacity(count);
    for frame in frames {
        let frame = frame.ok()?;
        let delay = frame.delay();
        let (left, top) = (frame.left(), frame.top());
        let mut buffer: RgbaImage = frame.into_buffer();
        if shrink {
            let height = (buffer.height() as u64 * target as u64 / w as u64).max(1) as u32;
            let width = (buffer.width() as u64 * target as u64 / w as u64).max(1) as u32;
            buffer = image::imageops::resize(&buffer, width, height, FilterType::Triangle);
        }
        // GPUI draws BGRA.
        for pixel in buffer.as_chunks_mut::<4>().0 {
            pixel.swap(0, 2);
        }
        let (left, top) = if shrink {
            (left * target / w, top * target / w)
        } else {
            (left, top)
        };
        out.push(Frame::from_parts(buffer, left, top, delay));
    }
    (out.len() > 1).then(|| RenderImage::new(out))
}

/// How many frames an animated WebP (ANMF chunks) or GIF (graphic control blocks) has.
fn frame_count(bytes: &[u8]) -> usize {
    if bytes.starts_with(b"GIF8") {
        return bytes
            .windows(3)
            .filter(|w| *w == [0x21, 0xF9, 0x04])
            .count();
    }
    let mut at = 12;
    let mut count = 0;
    while at + 8 <= bytes.len() {
        let size = u32::from_le_bytes([bytes[at + 4], bytes[at + 5], bytes[at + 6], bytes[at + 7]])
            as usize;
        if &bytes[at..at + 4] == b"ANMF" {
            count += 1;
        }
        at += 8 + size + (size & 1);
    }
    count
}
