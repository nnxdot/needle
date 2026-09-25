//! Colour from the music. Each cover gives the app its strongest colour and a soft blurred copy
//! of itself; the palette fades to the playing cover's colour, and pages glow with it.
use super::{
    AppView, Event, motion,
    theme::{self, Palette},
};
use gpui::{prelude::*, *};
use needle_core::audio::QueueItem;
use std::{
    collections::HashMap,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
    time::Instant,
};

/// What a cover brings to the interface.
#[derive(Clone, Debug, PartialEq)]
pub struct Look {
    /// The cover's most striking colour.
    pub vivid: Hsla,
    /// A small, heavily blurred copy of the cover for backdrops.
    pub blur: Option<PathBuf>,
    /// How bright the cover is overall, 0 (black) to 1 (white).
    pub luma: f32,
}

impl Look {
    /// How strongly the blurred cover may show: a dark cover on the light look (or a bright
    /// one on a dark look) would lay a heavy band over the page, so it is toned down.
    pub fn strength(&self, dark: bool) -> f32 {
        if dark {
            (1.35 - self.luma).clamp(0.35, 1.)
        } else {
            (self.luma * 1.5).clamp(0.12, 1.)
        }
    }
}

/// Looks by artwork path. `None` while measuring, or when the image could not be read.
#[derive(Default)]
pub struct Looks {
    map: HashMap<String, Option<Look>>,
}

/// The palette fading from one cover's colours to the next.
pub struct Fade {
    from: Palette,
    to: Palette,
    started: Instant,
}
impl Fade {
    pub fn new(p: Palette) -> Self {
        Self {
            from: p,
            to: p,
            started: Instant::now(),
        }
    }
}

const FADE_MS: f32 = 900.;

/// The strongest colour in an image: the most colourful band of hues, weighted by how
/// saturated and how clearly lit each pixel is, so a small red label beats a grey sky.
pub fn vivid_color(image: &image::RgbImage) -> Option<Hsla> {
    const BINS: usize = 24;
    let mut weight = [0f32; BINS];
    let mut sums = [(0f32, 0f32, 0f32, 0f32); BINS]; // hue x, hue y, saturation, lightness
    let (mut r, mut g, mut b, mut n) = (0f32, 0f32, 0f32, 0f32);
    for pixel in image.pixels() {
        let rgba = Rgba {
            r: pixel[0] as f32 / 255.,
            g: pixel[1] as f32 / 255.,
            b: pixel[2] as f32 / 255.,
            a: 1.,
        };
        (r, g, b, n) = (r + rgba.r, g + rgba.g, b + rgba.b, n + 1.);
        let color: Hsla = rgba.into();
        if color.l < 0.1 || color.l > 0.93 || color.s < 0.12 {
            continue;
        }
        let w = color.s * color.s * (1. - (color.l - 0.5).abs() * 1.6).max(0.05);
        let bin = ((color.h * BINS as f32) as usize).min(BINS - 1);
        let angle = color.h * std::f32::consts::TAU;
        weight[bin] += w;
        let s = &mut sums[bin];
        *s = (
            s.0 + angle.cos() * w,
            s.1 + angle.sin() * w,
            s.2 + color.s * w,
            s.3 + color.l * w,
        );
    }
    if n == 0. {
        return None;
    }
    // Neighbouring bins count too, so a hue split across a boundary is not undercounted.
    let score =
        |i: usize| weight[i] + 0.5 * (weight[(i + BINS - 1) % BINS] + weight[(i + 1) % BINS]);
    let best = (0..BINS).max_by(|a, b| score(*a).total_cmp(&score(*b)))?;
    if weight[best] < n * 0.004 {
        // Hardly any colour at all: return the average, which the palette treats as grey.
        return Some(
            Rgba {
                r: r / n,
                g: g / n,
                b: b / n,
                a: 1.,
            }
            .into(),
        );
    }
    let (x, y, s, l) = sums[best];
    let w = weight[best];
    let hue = y.atan2(x).rem_euclid(std::f32::consts::TAU) / std::f32::consts::TAU;
    Some(hsla(hue, (s / w).clamp(0., 1.), (l / w).clamp(0., 1.), 1.))
}

/// Measure a cover and write its blurred copy into `cache`.
pub fn measure(path: &str, cache: &Path) -> Option<Look> {
    // Read the type from the file's contents: saved covers use a neutral ".img" ending.
    let image = image::ImageReader::open(path)
        .ok()?
        .with_guessed_format()
        .ok()?
        .decode()
        .ok()?;
    let small = image.thumbnail(48, 48).to_rgb8();
    let vivid = vivid_color(&small)?;
    let luma = small
        .pixels()
        .map(|p| (0.2126 * p[0] as f32 + 0.7152 * p[1] as f32 + 0.0722 * p[2] as f32) / 255.)
        .sum::<f32>()
        / (small.width() * small.height()).max(1) as f32;
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    path.hash(&mut hasher);
    std::fs::metadata(path)
        .ok()
        .and_then(|m| m.modified().ok())
        .hash(&mut hasher);
    // "-v2": blurs are now stored large (see below); older small ones are made again.
    let out = cache.join(format!("{:016x}-v2.png", hasher.finish()));
    let blur = if out.exists() {
        Some(out)
    } else {
        let base = image.thumbnail_exact(72, 72).to_rgb8();
        // GPUI stretches images without smoothing, so a small blur shows as soft blocks
        // across a whole window. Blur small (cheap), then enlarge smoothly.
        let blurred = image::imageops::resize(
            &image::imageops::blur(&base, 7.),
            512,
            512,
            image::imageops::FilterType::Triangle,
        );
        std::fs::create_dir_all(cache).ok();
        blurred.save(&out).ok().map(|_| out)
    };
    Some(Look { vivid, blur, luma })
}

impl AppView {
    /// The look for an artwork file, measuring it in the background the first time.
    pub(super) fn look(&mut self, path: &str) -> Option<Look> {
        if let Some(look) = self.looks.map.get(path) {
            return look.clone();
        }
        self.looks.map.insert(path.to_string(), None);
        let sender = self.sender.clone();
        let cache = self.library.directory.join("artwork").join("ambient");
        let path = path.to_string();
        std::thread::spawn(move || {
            let look = measure(&path, &cache);
            let _ = sender.send(Event::Look(path, look));
        });
        None
    }
    /// A look that is already measured, without starting a measurement.
    pub(super) fn cached_look(&self, path: &str) -> Option<Look> {
        self.looks.map.get(path).cloned().flatten()
    }
    pub(super) fn set_look(&mut self, path: String, look: Option<Look>) {
        self.looks.map.insert(path, look);
    }

    /// The picture that belongs to the current page: an album's cover or an artist's photo.
    pub(super) fn page_art(&self) -> Option<String> {
        match &self.page {
            super::Page::Album { .. } => self.tracks.first().and_then(|t| t.artwork.clone()),
            super::Page::Artist(name) => self.artist_images.get(name).cloned().flatten(),
            _ => None,
        }
    }

    /// The song the interface shows: the one playing, or for a moment after playback
    /// briefly has none (switching to stems reloads the song), the one before. Without this
    /// the colours and backgrounds would blink.
    pub(super) fn shown_item(&mut self) -> Option<QueueItem> {
        match &self.playback.current {
            Some(item) => {
                let same = self
                    .last_item
                    .as_ref()
                    .is_some_and(|(last, _)| last.track.id == item.track.id);
                if same {
                    if let Some((_, at)) = self.last_item.as_mut() {
                        *at = Instant::now();
                    }
                } else {
                    self.last_item = Some((item.clone(), Instant::now()));
                }
                Some(item.clone())
            }
            None => self
                .last_item
                .as_ref()
                .filter(|(_, at)| at.elapsed() < std::time::Duration::from_millis(1500))
                .map(|(item, _)| item.clone()),
        }
    }

    /// The playing song's look.
    pub(super) fn now_look(&mut self) -> Option<Look> {
        let path = self.shown_item()?.track.artwork?;
        self.look(&path)
    }

    /// The window material in use now, after Windows' own settings.
    pub(super) fn material(&self) -> super::glass::Material {
        use super::glass::Material;
        let (allowed, win11) = self.glass_system;
        match Material::from_name(&self.settings.window_material) {
            // Ambient covers the whole window with its own background.
            _ if !allowed || self.ambient_look() => Material::Solid,
            Material::Mica if !win11 => Material::Acrylic,
            m => m,
        }
    }

    /// Tell Windows which material to draw, when it or dark/light changed.
    pub(super) fn update_glass(&mut self, window: &mut Window, cx: &App) {
        let key = (self.material(), theme::pal(cx).dark);
        if self.glass_applied != Some(key) {
            super::glass::apply(window, key.0, key.1);
            self.glass_applied = Some(key);
        }
    }

    /// Move the app palette toward the playing cover's colours. Called once per frame.
    pub(super) fn update_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // An album or artist page takes that album's or artist's colour; elsewhere the
        // playing song's cover leads.
        let tint = if self.settings.music_colors {
            match self.page_art() {
                Some(path) => self.look(&path).map(|l| l.vivid),
                None => match self.shown_item().map(|c| c.track) {
                    // No cover at all: use the colour of the made-up one.
                    Some(track) if track.artwork.is_none() => Some(super::widgets::seed_color(
                        &super::widgets::track_seed(&track),
                    )),
                    _ => self.now_look().map(|l| l.vivid),
                },
            }
        } else {
            theme::parse_hex(&self.settings.accent_color)
        };
        let mut target = theme::look(&self.settings.theme, tint, self.ambient_look(), cx);
        match self.material() {
            super::glass::Material::Solid => {}
            // Clear glass shows the desktop unblurred, so it keeps more of the surface.
            super::glass::Material::Clear => {
                target = target.glass(self.settings.glass_amount, self.settings.glass_page, 0.55)
            }
            // Light glass washes text out sooner, so it keeps more of the surface too.
            _ if !target.dark => {
                target = target.glass(self.settings.glass_amount, self.settings.glass_page, 0.6)
            }
            _ => target = target.glass(self.settings.glass_amount, self.settings.glass_page, 0.8),
        }
        // Ambient: the bars and the page float over the full-window cover, part see-through.
        if self.ambient_look() {
            target.back = target.chrome.opacity(if target.dark { 0.5 } else { 0.55 });
            target.canvas = target.canvas.opacity(if target.dark { 0.62 } else { 0.7 });
        }
        let shown = theme::pal(cx);
        if target != self.fade.to {
            // Switching dark and light snaps; covers fade.
            let instant = target.dark != shown.dark || !motion::enabled(cx);
            self.fade = Fade {
                from: if instant { target } else { shown },
                to: target,
                started: Instant::now(),
            };
        }
        let t = (self.fade.started.elapsed().as_secs_f32() * 1000. / FADE_MS).min(1.);
        let now = if t >= 1. {
            self.fade.to
        } else {
            Palette::mix(&self.fade.from, &self.fade.to, motion::ease_out(t))
        };
        if now != shown {
            theme::apply(now, cx);
        }
        if t < 1. {
            window.request_animation_frame();
        }
    }

    /// A soft glow of colour across the top of a page: the blurred cover when there is one,
    /// otherwise the palette's glow colour. `strength` is 0–1.
    pub(super) fn backdrop(
        &mut self,
        path: Option<&str>,
        height: f32,
        strength: f32,
        cx: &App,
    ) -> AnyElement {
        let p = theme::pal(cx);
        let look = path.and_then(|path| self.look(path));
        let fade_to = p.canvas;
        let fit = look.as_ref().map_or(1., |l| l.strength(p.dark));
        let base_alpha = (if p.dark { 0.42 } else { 0.34 } * strength).min(0.66) * fit;
        let glow = div()
            .absolute()
            .top_0()
            .left_0()
            .right_0()
            .h(px(height))
            .overflow_hidden();
        let glow = match look.and_then(|l| l.blur) {
            Some(blur) => glow.child(
                img(blur)
                    .absolute()
                    .top(px(-height * 0.25))
                    .left_0()
                    .w_full()
                    .h(px(height * 1.5))
                    .object_fit(ObjectFit::Cover)
                    .opacity(base_alpha),
            ),
            None => glow.bg(linear_gradient(
                160.,
                linear_color_stop(p.glow.opacity((0.28 * strength).min(0.4)), 0.),
                linear_color_stop(p.glow.opacity(0.), 0.8),
            )),
        };
        // Fade the glow into the page so text below it sits on the plain surface.
        glow.child(div().absolute().inset_0().bg(linear_gradient(
            180.,
            linear_color_stop(fade_to.opacity(0.), 0.),
            linear_color_stop(fade_to, 1.),
        )))
        .into_any_element()
    }
}

/// Side of one grain tile, in pixels.
const GRAIN: u32 = 384;

/// Write a tile of fine film grain: light and dark specks that average out to nothing.
fn make_grain(path: &Path) -> Option<PathBuf> {
    if path.exists() {
        return Some(path.to_path_buf());
    }
    let mut seed = 0x9E37_79B9u32;
    let mut next = move || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        seed
    };
    let image = image::RgbaImage::from_fn(GRAIN, GRAIN, |_, _| {
        let r = next();
        let v = if r & 1 == 0 { 255 } else { 0 };
        image::Rgba([v, v, v, ((r >> 8) % 160) as u8])
    });
    std::fs::create_dir_all(path.parent()?).ok()?;
    image.save(path).ok()?;
    Some(path.to_path_buf())
}

impl AppView {
    /// Whether the cover fills the whole background ("ambient" was once a look of its own).
    pub(super) fn ambient_look(&self) -> bool {
        self.settings.ambient || self.settings.theme == "ambient"
    }

    /// The Ambient look's background: the page's or the playing song's cover, blurred, over
    /// the whole window (or the chosen colour as a soft gradient), with a dark wash on top so
    /// text stays readable on any cover.
    pub(super) fn ambient_layer(&mut self, cx: &App) -> Option<AnyElement> {
        if !self.ambient_look() {
            return None;
        }
        let p = theme::pal(cx);
        let shown = self.shown_item();
        let path = self
            .settings
            .music_colors
            .then(|| {
                self.page_art()
                    .or_else(|| shown.and_then(|c| c.track.artwork))
            })
            .flatten();
        let blur = path.and_then(|path| self.look(&path)).and_then(|look| {
            look.blur.clone().map(|b| {
                (
                    b,
                    look.strength(p.dark).max(if p.dark { 0.6 } else { 0.35 }),
                )
            })
        });
        let layer = div().absolute().inset_0().bg(p.chrome);
        let layer = match blur {
            Some((blur, fit)) => layer.child(
                img(blur)
                    .absolute()
                    .inset_0()
                    .size_full()
                    .object_fit(ObjectFit::Cover)
                    .opacity(0.8 * fit),
            ),
            None => layer.child(div().absolute().inset_0().bg(linear_gradient(
                150.,
                linear_color_stop(p.glow.opacity(0.55), 0.),
                linear_color_stop(p.glow.opacity(0.08), 1.),
            ))),
        };
        Some(
            layer
                // On the light look a busy mid-tone background makes grey text hard to read,
                // so the wash is lighter and more even there.
                .child(div().absolute().inset_0().bg(linear_gradient(
                    180.,
                    linear_color_stop(p.chrome.opacity(if p.dark { 0.3 } else { 0.55 }), 0.),
                    linear_color_stop(p.chrome.opacity(if p.dark { 0.65 } else { 0.8 }), 1.),
                )))
                .into_any_element(),
        )
    }

    /// The glow behind the current page: an album's or artist's own picture on their pages,
    /// otherwise the playing song's cover.
    pub(super) fn page_backdrop(&mut self, cx: &App) -> AnyElement {
        let shown = self.shown_item();
        let playing = self
            .settings
            .music_colors
            .then(|| shown.and_then(|c| c.track.artwork))
            .flatten();
        let (path, height, strength) = match &self.page {
            super::Page::Album { .. } | super::Page::Artist(_) if self.page_art().is_some() => {
                (self.page_art(), 460., 1.45)
            }
            super::Page::Artist(_) => (playing, 380., 1.),
            super::Page::Home => (playing, 440., 1.),
            _ => (playing, 300., 0.55),
        };
        // Ambient paints the whole window instead; no band on top of it.
        if self.ambient_look() || (path.is_none() && !self.settings.music_colors) {
            return div().into_any_element();
        }
        self.backdrop(path.as_deref(), height, strength, cx)
    }

    /// Film grain over the whole window, so large surfaces do not look like flat paint.
    pub(super) fn grain(&mut self, window: &Window, cx: &App) -> Option<AnyElement> {
        if self.settings.grain <= 0.001 {
            return None;
        }
        if self.grain_file.is_none() {
            let path = self
                .library
                .directory
                .join("artwork")
                .join("ambient")
                .join("grain.png");
            self.grain_file = Some(make_grain(&path));
        }
        let path = self.grain_file.clone().flatten()?;
        let size = super::widgets::content_size(window);
        let (cols, rows) = (
            (f32::from(size.width) / GRAIN as f32).ceil() as u32,
            (f32::from(size.height) / GRAIN as f32).ceil() as u32,
        );
        // Even at full strength the grain stays faint.
        let strength =
            self.settings.grain.clamp(0., 1.) * if theme::pal(cx).dark { 0.05 } else { 0.035 };
        Some(
            div()
                .absolute()
                .inset_0()
                .overflow_hidden()
                .opacity(strength)
                .children((0..rows).flat_map(|row| {
                    let path = path.clone();
                    (0..cols).map(move |col| {
                        img(path.clone())
                            .absolute()
                            .left(px((col * GRAIN) as f32))
                            .top(px((row * GRAIN) as f32))
                            .size(px(GRAIN as f32))
                    })
                }))
                .into_any_element(),
        )
    }
}

#[cfg(test)]
mod tests {
    // Not `super::*`: that would bring in GPUI's own `test` attribute in place of Rust's.
    use super::measure;

    /// Saved covers end in ".img"; their colour must still be read.
    #[test]
    fn measures_covers_saved_without_an_image_extension() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cover.img");
        image::RgbImage::from_pixel(32, 32, image::Rgb([40, 150, 60]))
            .save_with_format(&path, image::ImageFormat::Jpeg)
            .unwrap();
        let look = measure(path.to_str().unwrap(), &dir.path().join("cache")).expect("measured");
        assert!(
            (look.vivid.h - 0.37).abs() < 0.05,
            "green hue, got {:?}",
            look.vivid
        );
        assert!(look.blur.is_some_and(|b| b.exists()));
    }
}
