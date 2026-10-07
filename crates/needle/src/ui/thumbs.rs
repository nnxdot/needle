//! Small copies of covers for rows and tiles. GPUI decodes and uploads an image at its full
//! size however small it is drawn, and covers can be 4000 pixels wide (a 68 MB texture), so a
//! tile draws a copy made once, in the background, and kept in `artwork/thumbs`.

use std::{
    collections::HashMap,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
    sync::{
        Mutex, OnceLock,
        atomic::{AtomicU64, Ordering},
        mpsc::{Sender, channel},
    },
};

/// A cover file up to this size is drawn as it is: a copy would save little.
const SMALL_FILE: u64 = 400 * 1024;
/// The edges of the copies, in pixels; a cover is drawn from the smallest that covers twice
/// its size on screen (sharp on high-density screens). Larger covers draw the original.
const EDGES: [u32; 2] = [256, 640];

enum Thumb {
    /// Draw this file.
    Ready(PathBuf),
    /// Being made; draw the stand-in until then.
    Making,
    Failed(std::time::Instant),
}

type Key = (String, u32, (u64, Option<std::time::SystemTime>));
fn stamp(path: &str) -> Option<(u64, Option<std::time::SystemTime>)> {
    let metadata = std::fs::metadata(path).ok()?;
    Some((metadata.len(), metadata.modified().ok()))
}

struct Thumbs {
    folder: PathBuf,
    known: Mutex<HashMap<Key, Thumb>>,
    jobs: Mutex<Sender<(Key, PathBuf)>>,
}

static THUMBS: OnceLock<Thumbs> = OnceLock::new();
/// Counts copies made, so the window knows to draw again.
static MADE: AtomicU64 = AtomicU64::new(0);

/// Where copies are kept; call once at start.
pub fn start(folder: PathBuf) {
    let (tx, rx) = channel::<(Key, PathBuf)>();
    // One thread: decoding a large cover is heavy, and a screenful of them at once would
    // take every core.
    std::thread::spawn(move || {
        for (key, out) in rx {
            let (path, edge, version) = &key;
            let made = make(path, *edge, &out);
            if let Some(thumbs) = THUMBS.get() {
                let mut known = thumbs.known.lock().unwrap_or_else(|p| p.into_inner());
                // A cover that cannot be read is drawn as it is (and fails there as before).
                if stamp(path).as_ref() != Some(version) {
                    known.remove(&key);
                    let _ = std::fs::remove_file(&out);
                } else if known.contains_key(&key) {
                    known.insert(
                        key,
                        if made {
                            Thumb::Ready(out)
                        } else {
                            Thumb::Failed(std::time::Instant::now())
                        },
                    );
                }
            }
            MADE.fetch_add(1, Ordering::Relaxed);
        }
    });
    let _ = THUMBS.set(Thumbs {
        folder,
        known: Mutex::default(),
        jobs: Mutex::new(tx),
    });
}

/// How many copies have been made so far.
pub fn made() -> u64 {
    MADE.load(Ordering::Relaxed)
}

/// The file to draw for the cover at `path`, shown `size` points wide: a small copy when the
/// cover is large, or the original. `None` while the copy is being made.
pub fn for_size(path: &str, size: f32) -> Option<PathBuf> {
    let Some(thumbs) = THUMBS.get() else {
        return Some(PathBuf::from(path));
    };
    let wanted = (size * 2.).ceil() as u32;
    let Some(&edge) = EDGES.iter().find(|&&e| e >= wanted) else {
        return Some(PathBuf::from(path));
    };
    let mut known = thumbs.known.lock().unwrap_or_else(|p| p.into_inner());
    let Some(version) = stamp(path) else {
        return Some(PathBuf::from(path));
    };
    let key = (path.to_string(), edge, version);
    known.retain(|k, _| k.0 != path || k.1 != edge || *k == key);
    match known.get(&key) {
        Some(Thumb::Ready(file)) if file.is_file() => return Some(file.clone()),
        Some(Thumb::Making) => return None,
        Some(Thumb::Failed(at)) if at.elapsed().as_secs() < 1 => return Some(PathBuf::from(path)),
        _ => {}
    }
    if version.0 <= SMALL_FILE {
        known.insert(key, Thumb::Ready(PathBuf::from(path)));
        return Some(PathBuf::from(path));
    }
    // Named by the cover's path, size, and time, so a changed cover gets a new copy.
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    path.hash(&mut hasher);
    version.hash(&mut hasher);
    let out = thumbs
        .folder
        .join(format!("{:016x}-{edge}.jpg", hasher.finish()));
    if out.is_file() {
        known.insert(key, Thumb::Ready(out.clone()));
        return Some(out);
    }
    known.insert(key.clone(), Thumb::Making);
    if thumbs
        .jobs
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .send((key.clone(), out))
        .is_err()
    {
        known.insert(key, Thumb::Failed(std::time::Instant::now()));
        return Some(PathBuf::from(path));
    }
    None
}

/// Writes a copy of the cover at `path` no larger than `edge` pixels to `out`.
fn make(path: &str, edge: u32, out: &Path) -> bool {
    let image = image::ImageReader::open(path)
        .ok()
        .and_then(|r| r.with_guessed_format().ok())
        .and_then(|r| r.decode().ok());
    let Some(image) = image else {
        return false;
    };
    let small = image.thumbnail(edge, edge).to_rgb8();
    let Some(folder) = out.parent() else {
        return false;
    };
    if std::fs::create_dir_all(folder).is_err() {
        return false;
    }
    // Written beside and then renamed, so a copy is never seen half written.
    let part = out.with_extension("part");
    let written = std::fs::File::create(&part).ok().is_some_and(|file| {
        image::codecs::jpeg::JpegEncoder::new_with_quality(std::io::BufWriter::new(file), 85)
            .encode_image(&small)
            .is_ok()
    });
    let completed = written && std::fs::rename(&part, out).is_ok();
    if !completed {
        let _ = std::fs::remove_file(part);
    }
    completed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_large_cover_is_drawn_from_a_small_copy_and_a_small_one_as_it_is() {
        let dir = std::env::temp_dir().join(format!("needle-thumbs-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        // A noisy 1600 px picture: well over the size worth copying.
        let mut seed = 1u32;
        let big = image::RgbImage::from_fn(1600, 1600, |_, _| {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            image::Rgb([(seed >> 24) as u8, (seed >> 16) as u8, (seed >> 8) as u8])
        });
        let big_path = dir.join("big.img");
        big.save_with_format(&big_path, image::ImageFormat::Png)
            .unwrap();
        let small_path = dir.join("small.img");
        image::RgbImage::new(64, 64)
            .save_with_format(&small_path, image::ImageFormat::Png)
            .unwrap();
        start(dir.join("thumbs"));
        let (big_path, small_path) = (big_path.to_string_lossy(), small_path.to_string_lossy());

        assert_eq!(
            for_size(&small_path, 100.),
            Some(PathBuf::from(small_path.as_ref()))
        );
        // Shown big, the original is drawn.
        assert_eq!(
            for_size(&big_path, 500.),
            Some(PathBuf::from(big_path.as_ref()))
        );
        // A tile waits for its copy, then draws it.
        assert_eq!(for_size(&big_path, 100.), None);
        let started = std::time::Instant::now();
        let copy = loop {
            if let Some(copy) = for_size(&big_path, 100.) {
                break copy;
            }
            assert!(started.elapsed().as_secs() < 20, "no copy was made");
            std::thread::sleep(std::time::Duration::from_millis(20));
        };
        assert!(made() >= 1);
        assert_ne!(copy, PathBuf::from(big_path.as_ref()));
        let decoded = image::open(&copy).unwrap();
        assert!(decoded.width() <= 256 && decoded.height() <= 256);
        // Replacing the source at the same path must refresh during this session.
        let replacement = image::RgbImage::from_fn(1600, 1600, |_, _| {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            image::Rgb([(seed >> 24) as u8, (seed >> 16) as u8, (seed >> 8) as u8])
        });
        replacement
            .save_with_format(big_path.as_ref(), image::ImageFormat::Png)
            .unwrap();
        std::fs::File::options()
            .write(true)
            .open(big_path.as_ref())
            .unwrap()
            .set_modified(std::time::SystemTime::now() + std::time::Duration::from_secs(2))
            .unwrap();
        assert_ne!(for_size(&big_path, 100.), Some(copy.clone()));
        let started = std::time::Instant::now();
        let refreshed = loop {
            if let Some(file) = for_size(&big_path, 100.) {
                break file;
            }
            assert!(started.elapsed().as_secs() < 20);
            std::thread::sleep(std::time::Duration::from_millis(20));
        };
        assert_ne!(refreshed, copy);
        assert_ne!(
            std::fs::read(&refreshed).unwrap(),
            std::fs::read(&copy).unwrap()
        );
        let refreshed_image = image::open(&refreshed).unwrap();
        assert!(refreshed_image.width() <= 256 && refreshed_image.height() <= 256);
        // Removing the generated copy also recovers without restarting the app.
        std::fs::remove_file(&refreshed).unwrap();
        assert_eq!(for_size(&big_path, 100.), None);
        let started = std::time::Instant::now();
        while for_size(&big_path, 100.).is_none() {
            assert!(started.elapsed().as_secs() < 20);
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        let _ = std::fs::remove_dir_all(dir);
    }
}
