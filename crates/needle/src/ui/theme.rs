use gpui::{App, Global, Hsla, Pixels, Window, px, rgb};
use gpui_component::{Theme, ThemeMode};

/// Colours the component theme has no slot for. Read with `pal(cx)`.
#[derive(Clone, Copy)]
pub struct Palette {
    pub dark: bool,
    /// Sidebar, title bar and player: the quieter second neutral layer.
    pub chrome: Hsla,
    /// Main content surface.
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
}
impl Global for Palette {}

pub fn pal(cx: &App) -> Palette {
    *cx.global::<Palette>()
}

fn c(value: u32) -> Hsla {
    rgb(value).into()
}

impl Palette {
    fn new(dark: bool) -> Self {
        if dark {
            Self {
                dark,
                chrome: c(0x0f0f0e),
                canvas: c(0x161514),
                raised: c(0x1f1e1c),
                raised_hover: c(0x292725),
                line: c(0x2b2926),
                line_soft: c(0x211f1d),
                ink: c(0xedebe7),
                ink_2: c(0xaaa59e),
                ink_3: c(0x8a857e),
                accent: c(0xe2b46c),
                accent_ink: c(0x1c1509),
                accent_soft: c(0xe2b46c).opacity(0.14),
                selection: c(0xe2b46c).opacity(0.11),
                danger: c(0xee8479),
                success: c(0x8cc79a),
            }
        } else {
            Self {
                dark,
                chrome: c(0xf1f1f0),
                canvas: c(0xfbfbfb),
                raised: c(0xefeeed),
                raised_hover: c(0xe6e5e3),
                line: c(0xdedcd9),
                line_soft: c(0xeceae8),
                ink: c(0x1c1b19),
                ink_2: c(0x5a5650),
                ink_3: c(0x6f6a63),
                accent: c(0x8c5a12),
                accent_ink: c(0xffffff),
                accent_soft: c(0x8c5a12).opacity(0.10),
                selection: c(0x8c5a12).opacity(0.09),
                danger: c(0xb4382c),
                success: c(0x2f7a45),
            }
        }
    }
}

pub const RADIUS: Pixels = px(6.);

pub fn set_theme(mode: &str, window: Option<&mut Window>, cx: &mut App) {
    let dark = mode != "light";
    let p = Palette::new(dark);
    cx.set_global(p);
    Theme::change(
        if dark {
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
    let t = &mut theme.colors;
    t.background = p.canvas;
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
    t.popover = if dark { c(0x1d1c1a) } else { c(0xffffff) };
    t.popover_foreground = p.ink;
    t.list_hover = p.raised;
    t.list_active = p.accent_soft;
    t.list_active_border = p.accent.opacity(0.0);
    t.slider_bar = p.accent;
    t.slider_thumb = p.ink;
    t.switch = p.line;
    t.progress_bar = p.accent;
    t.scrollbar = gpui::transparent_black();
    t.scrollbar_thumb = p.ink_3.opacity(0.35);
    t.scrollbar_thumb_hover = p.ink_3.opacity(0.6);
    t.title_bar = p.chrome;
    t.title_bar_border = p.chrome;
    t.sidebar = p.chrome;
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
