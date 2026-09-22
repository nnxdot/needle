//! Colour from the music. Each cover gives the app its strongest colour and a soft blurred copy
//! of itself; the palette fades to the playing cover's colour, and pages glow with it.
use super::{
    AppView, Event, motion,
    theme::{self, Base, Palette},
};
use gpui::{prelude::*, *};
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
    let image = image::open(path).ok()?;
    let small = image.thumbnail(48, 48).to_rgb8();
    let vivid = vivid_color(&small)?;
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    path.hash(&mut hasher);
    std::fs::metadata(path)
        .ok()
        .and_then(|m| m.modified().ok())
        .hash(&mut hasher);
    let out = cache.join(format!("{:016x}.png", hasher.finish()));
    let blur = if out.exists() {
        Some(out)
    } else {
        let base = image.thumbnail_exact(72, 72).to_rgb8();
        let blurred = image::imageops::blur(&base, 7.);
        std::fs::create_dir_all(cache).ok();
        blurred.save(&out).ok().map(|_| out)
    };
    Some(Look { vivid, blur })
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

    /// The playing song's look.
    pub(super) fn now_look(&mut self) -> Option<Look> {
        let path = self.playback.current.as_ref()?.track.artwork.clone()?;
        self.look(&path)
    }

    /// The window material in use now, after Windows' own settings.
    pub(super) fn material(&self) -> super::glass::Material {
        use super::glass::Material;
        let (allowed, win11) = self.glass_system;
        match Material::from_name(&self.settings.window_material) {
            _ if !allowed => Material::Solid,
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
                None => match self.playback.current.as_ref().map(|c| c.track.clone()) {
                    // No cover at all: use the colour of the made-up one.
                    Some(track) if track.artwork.is_none() => Some(super::widgets::seed_color(
                        &super::widgets::track_seed(&track),
                    )),
                    _ => self.now_look().map(|l| l.vivid),
                },
            }
        } else {
            None
        };
        let mut target = Palette::build(Base::from_name(&self.settings.theme), tint);
        match self.material() {
            super::glass::Material::Solid => {}
            // Clear glass shows the desktop unblurred, so it keeps more of the surface.
            super::glass::Material::Clear => {
                target = target.glass(self.settings.glass_amount, self.settings.glass_page, 0.55)
            }
            _ => target = target.glass(self.settings.glass_amount, self.settings.glass_page, 0.8),
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
        let base_alpha = (if p.dark { 0.42 } else { 0.34 } * strength).min(0.66);
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
    /// The glow behind the current page: an album's or artist's own picture on their pages,
    /// otherwise the playing song's cover.
    pub(super) fn page_backdrop(&mut self, cx: &App) -> AnyElement {
        let playing = self
            .settings
            .music_colors
            .then(|| {
                self.playback
                    .current
                    .as_ref()
                    .and_then(|c| c.track.artwork.clone())
            })
            .flatten();
        let (path, height, strength) = match &self.page {
            super::Page::Album { .. } | super::Page::Artist(_) if self.page_art().is_some() => {
                (self.page_art(), 460., 1.45)
            }
            super::Page::Artist(_) => (playing, 380., 1.),
            super::Page::Home => (playing, 440., 1.),
            _ => (playing, 300., 0.55),
        };
        if path.is_none() && !self.settings.music_colors {
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
        let size = window.viewport_size();
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
