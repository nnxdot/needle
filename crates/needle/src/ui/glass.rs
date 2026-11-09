//! Window materials: Mica, Acrylic, or clear glass behind Needle's back layer on Windows;
//! Liquid Glass on macOS.
//!
//! GPUI draws with premultiplied alpha into a DirectComposition surface, so any pixel Needle
//! leaves see-through shows whatever Windows draws behind the window: the Mica or Acrylic
//! system backdrop, or the desktop itself.
//!
//! On macOS the window's back is a translucent system backdrop, and the sidebar and the
//! player float on the system's glass (`probe`): Liquid Glass on macOS 26 and newer, the
//! sidebar's vibrancy before. AppKit draws these under GPUI's layer, so Needle leaves them
//! see-through.
//!
//! On Windows and Linux the glass look is Needle's own: the sidebar and the page float as
//! panes over the back layer, and the playing cover glows behind it (`ambient_layer`).
//! Linux gets the desktop's blur on KDE with Wayland. GPUI cannot blur Needle's own content,
//! so panes over the page stay about 80% solid.
use gpui::{App, Hsla, IntoElement, Styled, Window, canvas, point, px};

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Material {
    Solid,
    Mica,
    Acrylic,
    Clear,
    /// macOS: the system backdrop, with the sidebar and the player on glass.
    Liquid,
    /// Linux: Needle's own glass, the cover's glow behind panes in an opaque window. Safe on
    /// every desktop; the default there.
    Painted,
}

impl Material {
    /// Linux: the settings value, name, and line of each choice. No Mica there: its value,
    /// the default, is the cover's glow, which every desktop can show.
    pub const LINUX: [(&'static str, &'static str, &'static str); 4] = [
        (
            "mica",
            "Cover glow",
            "Panes of glass over the playing cover's colors.",
        ),
        (
            "acrylic",
            "Blur",
            "Frosted glass: a blur of what is behind the window.",
        ),
        ("clear", "Clear", "See straight through, with no blur."),
        ("solid", "Solid", "No glass."),
    ];
    /// The choices this system offers.
    pub fn choices() -> &'static [(&'static str, &'static str, &'static str)] {
        if cfg!(target_os = "linux") {
            &Self::LINUX
        } else {
            &Self::ALL
        }
    }
    /// Settings value, the name people see, and what it does.
    pub const ALL: [(&'static str, &'static str, &'static str); 4] = [
        (
            "mica",
            "Mica",
            "Soft color from your wallpaper. Calm and smooth.",
        ),
        (
            "acrylic",
            "Acrylic",
            "Frosted glass: a blur of what is behind the window.",
        ),
        ("clear", "Clear", "See straight through, with no blur."),
        ("solid", "Solid", "No glass."),
    ];
    pub fn from_name(name: &str) -> Self {
        // macOS has one kind of glass: on unless turned off.
        if cfg!(target_os = "macos") {
            return if name == "solid" {
                Self::Solid
            } else {
                Self::Liquid
            };
        }
        match name {
            "solid" => Self::Solid,
            "acrylic" => Self::Acrylic,
            "clear" => Self::Clear,
            // Linux has no Mica: the default "mica" is the cover's glow, in a solid window,
            // so a profile that never chose stays opaque.
            _ if cfg!(target_os = "linux") => Self::Painted,
            _ => Self::Mica,
        }
    }
}

/// Windows Settings › Personalization › Colors › Transparency effects.
pub fn system_allows_transparency() -> bool {
    #[cfg(windows)]
    {
        read_dword(
            windows_sys::Win32::System::Registry::HKEY_CURRENT_USER,
            "Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize",
            "EnableTransparency",
        )
        .is_none_or(|v| v != 0)
    }
    // macOS makes its glass opaque by itself under Reduce transparency. Linux has no such
    // setting: where the desktop cannot blur, the window stays solid anyway (`apply`).
    #[cfg(not(windows))]
    true
}

/// Whether this Mac has Liquid Glass (macOS 26 and newer); older Macs get the sidebar's
/// frosted vibrancy instead.
pub fn liquid_glass() -> bool {
    #[cfg(target_os = "macos")]
    {
        static FOUND: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        *FOUND.get_or_init(|| {
            std::process::Command::new("sw_vers")
                .arg("-productVersion")
                .output()
                .ok()
                .and_then(|o| {
                    String::from_utf8_lossy(&o.stdout)
                        .trim()
                        .split('.')
                        .next()?
                        .parse::<u32>()
                        .ok()
                })
                .is_some_and(|major| major >= 26)
        })
    }
    #[cfg(not(target_os = "macos"))]
    false
}

/// An invisible element that puts the system's glass under its parent (macOS), inset from its
/// edges by `inset` (left, top, right, bottom), with round corners and, when `shadow`, a soft
/// shadow so it floats. With `to_top`, the glass reaches up to the window's top, past the
/// title bar, as macOS's own sidebars do.
pub fn probe(
    name: &'static str,
    inset: (f32, f32, f32, f32),
    to_top: bool,
    radius: f32,
    shadow: bool,
) -> impl IntoElement {
    canvas(
        move |bounds, window, _| {
            let top = if to_top {
                px(inset.1)
            } else {
                bounds.top() + px(inset.1)
            };
            let corners = gpui::Bounds::from_corners(
                point(bounds.left() + px(inset.0), top),
                point(bounds.right() - px(inset.2), bounds.bottom() - px(inset.3)),
            );
            window.set_glass_region(
                name,
                Some(gpui::GlassRegion {
                    bounds: corners,
                    corner_radius: px(radius),
                    shadow,
                }),
            );
        },
        |_, _, _, _| {},
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
}

/// Windows 11 is build 22000 and later. Read once.
pub fn windows_11() -> bool {
    #[cfg(windows)]
    {
        static FOUND: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        *FOUND.get_or_init(|| {
            read_string(
                windows_sys::Win32::System::Registry::HKEY_LOCAL_MACHINE,
                "SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion",
                "CurrentBuildNumber",
            )
            .and_then(|b| b.trim().parse::<u32>().ok())
            .is_some_and(|b| b >= 22000)
        })
    }
    #[cfg(not(windows))]
    false
}

#[cfg(windows)]
fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain([0]).collect()
}

#[cfg(windows)]
fn read_dword(
    root: windows_sys::Win32::System::Registry::HKEY,
    key: &str,
    value: &str,
) -> Option<u32> {
    use windows_sys::Win32::System::Registry::{RRF_RT_REG_DWORD, RegGetValueW};
    let (key, value) = (wide(key), wide(value));
    let mut data = 0u32;
    let mut size = 4u32;
    let status = unsafe {
        RegGetValueW(
            root,
            key.as_ptr(),
            value.as_ptr(),
            RRF_RT_REG_DWORD,
            std::ptr::null_mut(),
            &mut data as *mut u32 as *mut _,
            &mut size,
        )
    };
    (status == 0).then_some(data)
}

#[cfg(windows)]
fn read_string(
    root: windows_sys::Win32::System::Registry::HKEY,
    key: &str,
    value: &str,
) -> Option<String> {
    use windows_sys::Win32::System::Registry::{RRF_RT_REG_SZ, RegGetValueW};
    let (key, value) = (wide(key), wide(value));
    let mut data = [0u16; 64];
    let mut size = (data.len() * 2) as u32;
    let status = unsafe {
        RegGetValueW(
            root,
            key.as_ptr(),
            value.as_ptr(),
            RRF_RT_REG_SZ,
            std::ptr::null_mut(),
            data.as_mut_ptr() as *mut _,
            &mut size,
        )
    };
    if status != 0 {
        return None;
    }
    let len = data.iter().position(|c| *c == 0).unwrap_or(data.len());
    Some(String::from_utf16_lossy(&data[..len]))
}

/// Ask Windows to draw `material` behind the window, in dark or light.
pub fn apply(
    window: &mut Window,
    material: Material,
    #[cfg_attr(not(windows), allow(unused_variables))] dark: bool,
) {
    use gpui::WindowBackgroundAppearance as A;
    // Acrylic and clear glass go through GPUI; Mica (and Acrylic on Windows 11) through DWM.
    let win11 = windows_11();
    // macOS: the system's own controls and glass follow Needle's dark or light theme.
    #[cfg(target_os = "macos")]
    window.set_dark_appearance(Some(dark));
    window.set_background_appearance(match material {
        Material::Clear | Material::Liquid => A::Transparent,
        // Windows 10's Acrylic, and Linux's blur where the compositor offers it (`material`
        // falls back to Painted elsewhere).
        Material::Acrylic if !win11 && system_blur() => A::Blurred,
        _ => A::Opaque,
    });
    #[cfg(windows)]
    {
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
        use windows_sys::Win32::Graphics::Dwm::{
            DwmExtendFrameIntoClientArea, DwmSetWindowAttribute,
        };
        use windows_sys::Win32::UI::Controls::MARGINS;
        let Ok(handle) = HasWindowHandle::window_handle(window) else {
            return;
        };
        let RawWindowHandle::Win32(win32) = handle.as_raw() else {
            return;
        };
        let hwnd = win32.hwnd.get() as _;
        // DWMWA_USE_IMMERSIVE_DARK_MODE = 20, DWMWA_SYSTEMBACKDROP_TYPE = 38.
        // Backdrop types: 1 none, 2 Mica, 3 Acrylic.
        let dark_mode: i32 = dark as i32;
        let backdrop: i32 = match material {
            Material::Mica => 2,
            Material::Acrylic if win11 => 3,
            _ => 1,
        };
        let edge = 0;
        let margins = MARGINS {
            cxLeftWidth: edge,
            cxRightWidth: edge,
            cyTopHeight: edge,
            cyBottomHeight: edge,
        };
        unsafe {
            DwmSetWindowAttribute(hwnd, 20, &dark_mode as *const i32 as *const _, 4);
            if win11 {
                DwmSetWindowAttribute(hwnd, 38, &backdrop as *const i32 as *const _, 4);
            }
            DwmExtendFrameIntoClientArea(hwnd, &margins);
        }
    }
}

/// Liquid Glass is on (macOS): the bars' colour is then fully see-through.
pub fn liquid_on(cx: &App) -> bool {
    cfg!(target_os = "macos") && super::theme::pal(cx).back.a < 0.01
}

/// Linux: what the desktop can do, asked of the compositor when the window first applies
/// its glass: (blurs what is behind a window, shows see-through parts at all).
#[cfg(target_os = "linux")]
static DESKTOP: std::sync::OnceLock<(bool, bool)> = std::sync::OnceLock::new();

/// Linux: ask the compositor, once, whether it blurs (KWin's `org_kde_kwin_blur`, which the
/// patched GPUI asks for with `Blurred`) and whether it composites see-through windows (always
/// on Wayland; on X11 only with a compositing manager). Elsewhere nothing to ask.
pub fn probe_desktop(
    #[cfg_attr(not(target_os = "linux"), allow(unused_variables))] window: &Window,
) {
    #[cfg(target_os = "linux")]
    DESKTOP.get_or_init(|| {
        (
            window.system_blur_available(),
            window.transparency_available(),
        )
    });
}

/// The desktop blurs what is behind the window. On Linux, only where the compositor said so
/// (until it is asked, no); Windows and macOS always can.
pub fn system_blur() -> bool {
    #[cfg(target_os = "linux")]
    {
        DESKTOP.get().is_some_and(|d| d.0)
    }
    #[cfg(not(target_os = "linux"))]
    true
}

/// See-through parts of the window show the desktop. On Linux, only with a compositor (else
/// they would be black); Windows and macOS always.
pub fn see_through() -> bool {
    #[cfg(target_os = "linux")]
    {
        DESKTOP.get().is_some_and(|d| d.1)
    }
    #[cfg(not(target_os = "linux"))]
    true
}

// Windows and Linux glass. The macOS paths above and in `pane` stay as they are.

/// The three corner sizes off the Mac: rows and small chips, panes (menus, dialogs, tooltips,
/// the page), and the big floating parts (the sidebar, the side panel).
pub const SMALL: f32 = 8.;
pub const MEDIUM: f32 = 14.;
pub const LARGE: f32 = 20.;

/// The nearest of the three corner sizes to `radius`.
pub fn corner(radius: f32) -> f32 {
    if radius < 9. {
        SMALL
    } else if radius <= 16. {
        MEDIUM
    } else {
        LARGE
    }
}

/// The edge light's colours for `p`: a bright line along the top edge and a faint outline.
/// Light glass washes out sooner, so light looks keep a darker outline and a white line.
pub fn edge_colors(p: &super::theme::Palette) -> (Hsla, Hsla) {
    let white = |a: f32| gpui::hsla(0., 0., 1., a);
    match (p.dark, p.glass) {
        (true, true) => (white(0.2), white(0.08)),
        (true, false) => (white(0.12), white(0.07)),
        // Light glass panes are a touch grey (`tinted`), so the white line still reads.
        (false, true) => (white(1.), gpui::hsla(0., 0., 0., 0.09)),
        (false, false) => (white(0.9), gpui::hsla(0., 0., 0., 0.1)),
    }
}

/// How far a part floats: panes over the page float highest. The toolkit's own popovers use
/// the same helper, so every glass part shares one edge.
pub use gpui_component::EdgeLift as Lift;

/// The edge light, for every glass part off the Mac: a faint outline, a 1 px bright line on
/// the top edge (part of the pane, thinning out along the round corners), and a soft shadow
/// under it. It sets the part's whole shadow list.
pub fn edge<E: Styled>(el: E, lift: Lift, cx: &App) -> E {
    let p = super::theme::pal(cx);
    let (top, outline) = edge_colors(&p);
    gpui_component::glass_edge(el, lift, top, outline, p.dark)
}

/// A pane's fill off the Mac: the raised surface tinted a little with the music's colour, at
/// least `alpha` solid. Over the page it must stay about 80% solid: GPUI cannot blur Needle's
/// own content under it, so only the tint, not a blur, ties it to the glass.
pub fn tinted(p: &super::theme::Palette, alpha: f32) -> Hsla {
    let mix = super::motion::mix;
    // Dark glass starts from the raised colour itself: the tint lifts it, and faint text
    // must still read on it.
    let base = match (p.dark, p.glass) {
        (true, true) => p.raised,
        (true, false) => mix(p.raised, p.raised_hover, 0.5),
        // Light glass: a touch grey, so the white edge light reads on it and the pane does
        // not wash out over a pale page (GLASS-POLISH 11).
        (false, true) => gpui::hsla(0., 0., 0.965, 1.),
        (false, false) => gpui::hsla(0., 0., 1., 1.),
    };
    let tint = if p.glass { 0.08 } else { 0.035 };
    let floor = if p.dark { 0.8 } else { 0.86 };
    mix(base, p.glow, if p.dark { tint } else { tint * 0.6 }).opacity(alpha.max(floor).min(1.))
}

/// The floating player, the side panel, and their like off the Mac: tinted glass with the
/// cover's colour coming in from the left, the edge light, and a shadow.
pub fn floating_pane<E: Styled>(el: E, radius: f32, cx: &App) -> E {
    let p = super::theme::pal(cx);
    let fill = tinted(&p, if p.glass { 0.9 } else { 0.96 });
    let from = super::motion::mix(fill.opacity(1.), p.glow, if p.dark { 0.22 } else { 0.14 })
        .opacity(fill.a);
    edge(
        el.rounded(px(radius)).bg(gpui::linear_gradient(
            90.,
            gpui::linear_color_stop(from, 0.),
            gpui::linear_color_stop(fill, 0.5),
        )),
        Lift::Mid,
        cx,
    )
}

/// The floating sidebar's fill off the Mac: a little more solid and lighter than the back
/// layer around it, so it reads as a pane of glass on the glass.
pub fn sidebar_fill(p: &super::theme::Palette) -> Hsla {
    let mix = super::motion::mix;
    let base = if p.dark {
        mix(p.chrome, p.raised, 0.45)
    } else {
        mix(p.chrome, gpui::hsla(0., 0., 1., 1.), 0.5)
    };
    base.opacity((p.back.a + 0.22).clamp(0.45, 0.94))
}

/// A selected or hovered row on glass: a light capsule, as the Mac's sidebar has, not the
/// accent fill. On the light look a white capsule vanishes on the pale sidebar, so it is a
/// soft grey there, as on the Mac.
pub fn capsule(p: &super::theme::Palette, hover: bool) -> Hsla {
    match (p.dark, hover) {
        (true, false) => gpui::hsla(0., 0., 1., 0.13),
        (true, true) => gpui::hsla(0., 0., 1., 0.07),
        (false, false) => gpui::hsla(0., 0., 0., 0.08),
        (false, true) => gpui::hsla(0., 0., 0., 0.045),
    }
}

/// A row's hover fill on glass, `capsule(p, true)`, or the plain look's raised fill.
pub fn hover_fill(p: &super::theme::Palette) -> Hsla {
    if p.glass {
        capsule(p, true)
    } else {
        p.raised_hover
    }
}

/// A chosen (or keyboard-marked) row's fill: the glass capsule, or the plain look's raised
/// fill.
pub fn active_fill(p: &super::theme::Palette) -> Hsla {
    if p.glass {
        capsule(p, false)
    } else {
        p.raised_hover
    }
}

/// The corners of a row in a pane with 4 px of padding (menus, the palette, lists): the
/// pane's corners less its padding, so the two curves run side by side.
pub const ROW_IN_PANE: f32 = MEDIUM - 4.;

/// Give a row its hover fill (`hover_fill`) as a capsule that fades in and out. Use it before
/// the row's own children, in place of `.hover(|s| s.bg(..))`.
pub fn hover_row<E: gpui::ParentElement + Styled>(el: E, radius: f32, cx: &App) -> E {
    let p = super::theme::pal(cx);
    el.relative()
        .child(hover_capsule(hover_fill(&p), radius, cx))
}

/// How long a hover capsule takes to fade in or out.
const HOVER_MS: f32 = 120.;

/// A row's hover capsule that fades in under the pointer and out when it leaves, instead of
/// snapping. Put it first in a `relative` row: it fills the row, under its content, with
/// `radius` corners (a capsule when `radius` is half the row's height or more). It asks for
/// frames only while it fades; with Reduce motion it shows and goes at once.
pub fn hover_capsule(color: Hsla, radius: f32, cx: &App) -> HoverCapsule {
    HoverCapsule {
        color,
        radius,
        fade: super::motion::enabled(cx),
    }
}

pub struct HoverCapsule {
    color: Hsla,
    radius: f32,
    fade: bool,
}

/// Where a hover capsule's fade stands: the way it is going, and from how strong, since when.
struct HoverFade {
    hovered: bool,
    from: f32,
    since: std::time::Instant,
}

impl HoverFade {
    fn strength(&self, fade: bool) -> f32 {
        let target = if self.hovered { 1. } else { 0. };
        if !fade {
            return target;
        }
        let t = (self.since.elapsed().as_secs_f32() * 1000. / HOVER_MS).min(1.);
        self.from + (target - self.from) * super::motion::ease_out(t)
    }
}

impl IntoElement for HoverCapsule {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

impl gpui::Element for HoverCapsule {
    type RequestLayoutState = ();
    type PrepaintState = gpui::Hitbox;

    fn id(&self) -> Option<gpui::ElementId> {
        Some("hover-capsule".into())
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&gpui::GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (gpui::LayoutId, ()) {
        let mut style = gpui::Style {
            position: gpui::Position::Absolute,
            ..Default::default()
        };
        style.inset = gpui::Edges::all(px(0.).into());
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&gpui::GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        bounds: gpui::Bounds<gpui::Pixels>,
        _: &mut (),
        window: &mut Window,
        _: &mut App,
    ) -> gpui::Hitbox {
        window.insert_hitbox(bounds, gpui::HitboxBehavior::Normal)
    }

    fn paint(
        &mut self,
        id: Option<&gpui::GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        bounds: gpui::Bounds<gpui::Pixels>,
        _: &mut (),
        hitbox: &mut gpui::Hitbox,
        window: &mut Window,
        _: &mut App,
    ) {
        let hovered = hitbox.is_hovered(window);
        let fade = self.fade;
        let Some(id) = id else {
            return;
        };
        let strength = window.with_element_state::<HoverFade, _>(id, |state, _| {
            let mut state = state.unwrap_or(HoverFade {
                hovered: false,
                from: 0.,
                since: std::time::Instant::now(),
            });
            if state.hovered != hovered {
                state = HoverFade {
                    from: state.strength(fade),
                    hovered,
                    since: std::time::Instant::now(),
                };
            }
            (state.strength(fade), state)
        });
        let target = if hovered { 1. } else { 0. };
        if (strength - target).abs() > 0.001 {
            window.request_animation_frame();
        }
        if strength > 0.001 {
            let radius = self.radius.min(f32::from(bounds.size.height) / 2.);
            window.paint_quad(
                gpui::fill(bounds, self.color.opacity(self.color.a * strength))
                    .corner_radii(px(radius)),
            );
        }
        // Draw again when the pointer comes or goes.
        let hitbox = hitbox.clone();
        window.on_mouse_event(move |_: &gpui::MouseMoveEvent, phase, window, _| {
            if phase == gpui::DispatchPhase::Capture && hitbox.is_hovered(window) != hovered {
                window.refresh();
            }
        });
    }
}

/// A menu, the command palette, a dialog, or a message, over the page: with Liquid Glass, a
/// pane of glass (the page under it is cleared, so the system's glass shows), its corners
/// rounder, as macOS's own; otherwise `fill`, with a thin line and a shadow. On Windows and
/// Linux the corners are one of the three sizes, and the line and shadow are the edge light.
pub fn pane<E: Styled>(el: E, radius: f32, glass_radius: f32, fill: Hsla, cx: &App) -> E {
    if cfg!(target_os = "macos") {
        let el = if liquid_on(cx) {
            el.rounded(px(glass_radius)).glass()
        } else {
            el.rounded(px(radius)).bg(fill)
        };
        el.border_1()
            .border_color(super::theme::pal(cx).line)
            .shadow_lg()
    } else {
        edge(el.rounded(px(corner(radius))).bg(fill), Lift::High, cx)
    }
}

#[cfg(test)]
mod tests {
    use super::{corner, edge_colors, tinted};
    use crate::ui::theme::{Base, Palette, contrast};
    use gpui::hsla;

    /// Off the Mac, every pane takes one of three corner sizes.
    #[test]
    fn three_corner_sizes() {
        for r in [6., 8., 9., 10., 12., 14., 16., 18., 22., 24., 26.] {
            assert!(
                [8., 14., 20.].contains(&corner(r)),
                "{r} gives {}",
                corner(r)
            );
        }
    }

    /// Text stays readable (4.5:1) on a tinted pane over the page, in every look and with
    /// any cover, glass on or off; and panes stay at least 80% solid.
    #[test]
    fn tinted_panes_keep_text_readable() {
        let tints = [None]
            .into_iter()
            .chain((0..12).map(|i| Some(hsla(i as f32 / 12., 0.8, 0.5, 1.))));
        for tint in tints {
            for base in [Base::Night, Base::Midnight, Base::Day] {
                for (glass, alpha) in [(false, 1.), (true, 0.84), (true, 0.88)] {
                    let mut p = Palette::build(base, tint, false);
                    p.glass = glass;
                    let fill = tinted(&p, alpha);
                    assert!(fill.a >= 0.8);
                    let seen = crate::ui::motion::mix(p.canvas, fill.opacity(1.), fill.a);
                    for (name, ink) in [("ink", p.ink), ("ink_2", p.ink_2), ("ink_3", p.ink_3)] {
                        let ratio = contrast(ink, seen);
                        assert!(
                            ratio >= 4.5,
                            "{name} on a pane ({base:?}, glass {glass}, tint {tint:?}) is {ratio:.2}:1"
                        );
                    }
                    // The edge light is a faint line, never a heavy one.
                    let (_, outline) = edge_colors(&p);
                    assert!(outline.a <= 0.12);
                }
            }
        }
    }
}
