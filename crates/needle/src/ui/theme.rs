use gpui::{App, Global, Hsla, Pixels, Window, hsla, px, rgb};
use gpui_component::{Theme, ThemeMode};

/// Colours the component theme has no slot for. Read with `pal(cx)`.
/// The base looks. Colour from the music tints whichever one is chosen; the Ambient setting
/// lets it fill the whole background of any of them.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Base {
    /// Warm charcoal.
    Night,
    /// Deep blue-black, darkest.
    Midnight,
    /// Light.
    Day,
}
impl Base {
    /// Settings value, the name people see, and a few words about it.
    pub const ALL: [(&'static str, &'static str, &'static str); 3] = [
        ("dark", "Night", "Warm charcoal"),
        ("midnight", "Midnight", "True black"),
        ("light", "Day", "Light and airy"),
    ];
    pub fn from_name(name: &str) -> Self {
        match name {
            "light" => Self::Day,
            "midnight" => Self::Midnight,
            _ => Self::Night,
        }
    }
}

/// Colours the component theme has no slot for. Read with `pal(cx)`.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Palette {
    pub dark: bool,
    /// Sidebar, title bar and player: the back layer.
    pub chrome: Hsla,
    /// The back layer as painted: `chrome`, or see-through when window glass is on.
    pub back: Hsla,
    /// Main content sheet.
    pub canvas: Hsla,
    /// Raised elements on the canvas (hover rows, fields, cards).
    pub raised: Hsla,
    pub raised_hover: Hsla,
    pub line: Hsla,
    pub line_soft: Hsla,
    pub ink: Hsla,
    pub ink_2: Hsla,
    pub ink_3: Hsla,
    pub accent: Hsla,
    pub accent_ink: Hsla,
    pub accent_soft: Hsla,
    pub selection: Hsla,
    pub danger: Hsla,
    pub success: Hsla,
    /// The music's colour at full strength, for glows and backdrops (never for text).
    pub glow: Hsla,
}
impl Global for Palette {}

pub fn pal(cx: &App) -> Palette {
    *cx.global::<Palette>()
}

fn c(value: u32) -> Hsla {
    rgb(value).into()
}

/// Colors to choose from when colors from the music are off ("" is Needle's amber).
pub const ACCENTS: [(&str, &str); 8] = [
    ("", "Needle"),
    ("#e5484d", "Red"),
    ("#e93d82", "Pink"),
    ("#8e4ec6", "Purple"),
    ("#3e63dd", "Blue"),
    ("#12a594", "Teal"),
    ("#46a758", "Green"),
    ("#f76b15", "Orange"),
];

/// A "#rrggbb" color, if it is one.
pub fn parse_hex(hex: &str) -> Option<Hsla> {
    let hex = hex.trim().trim_start_matches('#');
    (hex.len() == 6)
        .then(|| u32::from_str_radix(hex, 16).ok())
        .flatten()
        .map(|v| rgb(v).into())
}

/// Fonts for page titles and big names: settings value, name people see, font family.
pub const DISPLAY_FONTS: [(&str, &str, &str); 3] = [
    ("system", "Segoe UI", "Segoe UI Variable Display"),
    ("bahnschrift", "Bahnschrift", "Bahnschrift"),
    ("fraunces", "Fraunces (Nick's font)", "Fraunces 72pt Soft"),
];

static DISPLAY_FONT: std::sync::RwLock<&'static str> =
    std::sync::RwLock::new("Segoe UI Variable Display");

/// The font family for page titles and big names.
pub fn display_font() -> &'static str {
    *DISPLAY_FONT.read().unwrap_or_else(|e| e.into_inner())
}

/// Use the title font with this settings value (unknown values fall back to the first).
pub fn set_display_font(key: &str) {
    let family = DISPLAY_FONTS
        .iter()
        .find(|(k, _, _)| *k == key)
        .unwrap_or(&DISPLAY_FONTS[0])
        .2;
    *DISPLAY_FONT.write().unwrap_or_else(|e| e.into_inner()) = family;
}

fn luminance(color: Hsla) -> f32 {
    let rgba = color.to_rgb();
    let channel = |c: f32| {
        if c <= 0.03928 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * channel(rgba.r) + 0.7152 * channel(rgba.g) + 0.0722 * channel(rgba.b)
}
pub fn contrast(a: Hsla, b: Hsla) -> f32 {
    let (a, b) = (luminance(a), luminance(b));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

impl Palette {
    /// A palette for `base`, tinted toward `tint` (a cover's colour) when there is one.
    /// `ambient` tints the surfaces much more strongly, for the full-window background.
    pub fn build(base: Base, tint: Option<Hsla>, ambient: bool) -> Self {
        // Covers that are nearly grey keep the default accent.
        let tint = tint.filter(|t| t.s > 0.14 && t.l > 0.06 && t.l < 0.96);
        let (hue, sat): (f32, f32) = match (tint, base) {
            (Some(t), Base::Day) if ambient => (t.h, 0.3),
            (Some(t), Base::Day) => (t.h, 0.16),
            (Some(t), _) if ambient => (t.h, 0.34),
            // Midnight stays neutral black; only the accent takes the colour.
            (Some(t), Base::Midnight) => (t.h, 0.0),
            (Some(t), _) => (t.h, 0.13),
            (None, _) if ambient => (0.083, 0.2),
            (None, Base::Midnight) => (0.0, 0.0),
            (None, _) => (0.083, 0.05),
        };
        let n = |l: f32| hsla(hue, sat, l, 1.);
        let dark = base != Base::Day;
        let (chrome, canvas, raised, raised_hover, line, line_soft) = match base {
            Base::Night => (n(0.05), n(0.082), n(0.118), n(0.152), n(0.165), n(0.125)),
            Base::Midnight => (n(0.0), n(0.03), n(0.07), n(0.105), n(0.125), n(0.085)),
            Base::Day => (n(0.935), n(0.985), n(0.93), n(0.9), n(0.865), n(0.915)),
        };
        let ink_sat = sat.min(0.1);
        let (ink, ink_2, ink_3) = if dark {
            (
                hsla(hue, ink_sat, 0.93, 1.),
                hsla(hue, ink_sat, 0.72, 1.),
                hsla(hue, ink_sat, 0.6, 1.),
            )
        } else {
            (
                hsla(hue, ink_sat, 0.1, 1.),
                hsla(hue, ink_sat, 0.31, 1.),
                hsla(hue, ink_sat, 0.37, 1.),
            )
        };
        let surfaces = [chrome, canvas, raised];
        let readable = |color: Hsla| surfaces.iter().all(|s| contrast(color, *s) >= 4.6);
        let accent = match tint {
            None if dark => c(0xe2b46c),
            None => c(0x8c5a12),
            Some(t) => {
                // Keep the cover's hue, give it enough colour, then move the lightness until
                // it reads on every surface.
                let s = t.s.clamp(0.5, 0.85);
                let mut l = if dark { 0.64 } else { 0.4 };
                let mut color = hsla(t.h, s, l, 1.);
                while !readable(color) && (0.05..0.95).contains(&l) {
                    l += if dark { 0.02 } else { -0.02 };
                    color = hsla(t.h, s, l, 1.);
                }
                color
            }
        };
        let dark_ink = hsla(accent.h, 0.5, 0.09, 1.);
        let accent_ink = if contrast(dark_ink, accent) >= 4.5 {
            dark_ink
        } else {
            c(0xffffff)
        };
        let glow = match tint {
            Some(t) => hsla(t.h, t.s.clamp(0.4, 0.9), if dark { 0.5 } else { 0.62 }, 1.),
            None => hsla(accent.h, accent.s, if dark { 0.45 } else { 0.7 }, 1.),
        };
        Self {
            dark,
            chrome,
            back: chrome,
            canvas,
            raised,
            raised_hover,
            line,
            line_soft,
            ink,
            ink_2,
            ink_3,
            accent,
            accent_ink,
            accent_soft: accent.opacity(if dark { 0.15 } else { 0.11 }),
            selection: accent.opacity(if dark { 0.12 } else { 0.1 }),
            danger: if dark { c(0xee8479) } else { c(0xb4382c) },
            success: if dark { c(0x8cc79a) } else { c(0x2f7a45) },
            glow,
        }
    }

    /// Let window glass show through: the back layer by `amount` (0–1), and the page a little
    /// too when `page` is set. Text keeps its colours; surfaces only lose opacity. `reach` is
    /// how far the back layer may go: blurred materials can go further than clear glass.
    pub fn glass(mut self, amount: f32, page: bool, reach: f32) -> Self {
        let amount = amount.clamp(0., 1.);
        self.back = self.chrome.opacity(1. - amount * reach);
        if page {
            self.canvas = self.canvas.opacity(1. - amount * 0.3);
        }
        self
    }

    /// Part way from `a` to `b`, for fading between covers.
    pub fn mix(a: &Self, b: &Self, t: f32) -> Self {
        let m = |x: Hsla, y: Hsla| super::motion::mix(x, y, t);
        Self {
            dark: b.dark,
            chrome: m(a.chrome, b.chrome),
            back: m(a.back, b.back),
            canvas: m(a.canvas, b.canvas),
            raised: m(a.raised, b.raised),
            raised_hover: m(a.raised_hover, b.raised_hover),
            line: m(a.line, b.line),
            line_soft: m(a.line_soft, b.line_soft),
            ink: m(a.ink, b.ink),
            ink_2: m(a.ink_2, b.ink_2),
            ink_3: m(a.ink_3, b.ink_3),
            accent: m(a.accent, b.accent),
            accent_ink: m(a.accent_ink, b.accent_ink),
            accent_soft: m(a.accent_soft, b.accent_soft),
            selection: m(a.selection, b.selection),
            danger: m(a.danger, b.danger),
            success: m(a.success, b.success),
            glow: m(a.glow, b.glow),
        }
    }
}

pub const RADIUS: Pixels = px(6.);

pub fn set_theme(mode: &str, window: Option<&mut Window>, cx: &mut App) {
    let p = Palette::build(Base::from_name(mode), None, false);
    Theme::change(
        if p.dark {
            ThemeMode::Dark
        } else {
            ThemeMode::Light
        },
        window,
        cx,
    );
    let theme = Theme::global_mut(cx);
    theme.font_family = "Segoe UI Variable Text".into();
    theme.font_size = px(14.);
    theme.radius = RADIUS;
    theme.radius_lg = px(10.);
    theme.shadow = false;
    apply(p, cx);
}

/// Make `p` the palette for the whole app, including the component theme.
pub fn apply(p: Palette, cx: &mut App) {
    cx.set_global(p);
    let dark = p.dark;
    let t = &mut Theme::global_mut(cx).colors;
    // With glass on, nothing may paint an opaque layer under the whole window.
    t.background = if p.back.a < 1. {
        gpui::transparent_black()
    } else {
        p.canvas
    };
    t.foreground = p.ink;
    t.muted = p.raised;
    t.muted_foreground = p.ink_2;
    t.border = p.line;
    t.input = p.line;
    t.ring = p.accent.opacity(0.7);
    t.caret = p.accent;
    t.selection = p.accent.opacity(0.28);
    t.primary = p.accent;
    t.primary_foreground = p.accent_ink;
    t.primary_hover = p.accent.opacity(0.88);
    t.primary_active = p.accent.opacity(0.78);
    t.secondary = p.raised;
    t.secondary_foreground = p.ink;
    t.secondary_hover = p.raised_hover;
    t.secondary_active = p.line;
    t.accent = p.accent_soft;
    t.accent_foreground = p.ink;
    t.popover = if dark {
        super::motion::mix(p.raised, p.raised_hover, 0.5)
    } else {
        c(0xffffff)
    };
    t.popover_foreground = p.ink;
    t.list_hover = p.raised;
    t.list_active = p.accent_soft;
    t.list_active_border = p.accent.opacity(0.0);
    t.slider_bar = p.accent;
    t.slider_thumb = p.ink;
    t.switch = p.line;
    t.switch_thumb = if dark { c(0xe8e6e2) } else { c(0xffffff) };
    t.progress_bar = p.accent;
    t.scrollbar = gpui::transparent_black();
    t.scrollbar_thumb = p.ink_3.opacity(0.35);
    t.scrollbar_thumb_hover = p.ink_3.opacity(0.6);
    t.title_bar = p.back;
    t.title_bar_border = p.back;
    t.sidebar = p.back;
    t.sidebar_border = p.line_soft;
    t.tab_bar = p.chrome;
    t.tab = p.chrome;
    t.tab_active = p.canvas;
    t.tab_active_foreground = p.ink;
    t.tab_foreground = p.ink_2;
    t.tab_bar_segmented = p.raised;
    t.danger = p.danger;
    t.danger_foreground = if dark { c(0x1a0b09) } else { c(0xffffff) };
    t.danger_hover = p.danger.opacity(0.88);
    t.danger_active = p.danger.opacity(0.78);
    t.success = p.success;
    t.skeleton = p.raised;
    t.overlay = c(0x000000).opacity(if dark { 0.55 } else { 0.25 });
    t.window_border = p.line;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every text colour must stay readable (WCAG AA, 4.5:1) on every surface it is drawn on,
    /// in every base look and whatever colour the cover brings.
    #[test]
    fn text_meets_wcag_aa_on_every_surface() {
        let tints = [None]
            .into_iter()
            .chain((0..24).map(|i| Some(hsla(i as f32 / 24., 0.8, 0.5, 1.))))
            .chain([
                Some(hsla(0.6, 0.3, 0.2, 1.)),
                Some(hsla(0.15, 0.95, 0.85, 1.)),
                Some(hsla(0.0, 0.0, 0.5, 1.)),
            ]);
        for tint in tints {
            for (base, ambient) in [Base::Night, Base::Midnight, Base::Day]
                .into_iter()
                .flat_map(|b| [(b, false), (b, true)])
            {
                let p = Palette::build(base, tint, ambient);
                for (surface_name, surface) in [
                    ("chrome", p.chrome),
                    ("canvas", p.canvas),
                    ("raised", p.raised),
                ] {
                    for (text_name, text) in [
                        ("ink", p.ink),
                        ("ink_2", p.ink_2),
                        ("ink_3", p.ink_3),
                        ("accent", p.accent),
                        ("danger", p.danger),
                    ] {
                        let ratio = contrast(text, surface);
                        assert!(
                            ratio >= 4.5,
                            "{text_name} on {surface_name} ({base:?}, ambient {ambient}, tint {tint:?}) is {ratio:.2}:1"
                        );
                    }
                }
                assert!(
                    contrast(p.accent_ink, p.accent) >= 4.5,
                    "button label on accent ({base:?}, ambient {ambient}, tint {tint:?})"
                );
            }
        }
    }

    /// Glass only thins the back layer (and the page when asked), never text colours.
    #[test]
    fn glass_thins_surfaces_only() {
        let p = Palette::build(Base::Night, None, false);
        let solid = p.glass(0., false, 0.8);
        assert_eq!(solid.back, p.chrome);
        let glass = p.glass(1., false, 0.8);
        assert!((glass.back.a - 0.2).abs() < 1e-5);
        assert_eq!(
            (glass.canvas, glass.ink, glass.accent),
            (p.canvas, p.ink, p.accent)
        );
        let page = p.glass(1., true, 0.55);
        assert!((page.back.a - 0.45).abs() < 1e-5);
        assert!(page.canvas.a >= 0.7 && page.canvas.a < 1.);
    }
}
