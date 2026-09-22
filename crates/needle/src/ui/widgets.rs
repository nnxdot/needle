use super::theme::{Palette, pal};
use gpui::{prelude::*, *};
use gpui_component::{
    Icon, Sizable,
    button::{Button, ButtonVariants},
};
use needle_core::model::Track;
use std::path::PathBuf;

pub fn glyph(name: &str) -> Svg {
    svg()
        .path(SharedString::from(format!("needle/{name}.svg")))
        .size(px(18.))
        .flex_shrink_0()
}
pub fn icon(name: &str) -> Icon {
    Icon::empty().path(SharedString::from(format!("needle/{name}.svg")))
}

/// Square ghost button carrying one of Needle's icons.
pub fn icon_button(
    id: impl Into<ElementId>,
    name: &str,
    tooltip: impl Into<SharedString>,
) -> Button {
    Button::new(id).ghost().icon(icon(name)).tooltip(tooltip)
}

pub fn meta(text: impl Into<SharedString>, cx: &App) -> Div {
    div()
        .text_size(px(12.))
        .text_color(pal(cx).ink_2)
        .child(text.into())
}
pub fn faint(text: impl Into<SharedString>, cx: &App) -> Div {
    div()
        .text_size(px(12.))
        .text_color(pal(cx).ink_3)
        .child(text.into())
}
pub fn strong(text: impl Into<SharedString>) -> Div {
    div()
        .text_size(px(13.))
        .font_weight(FontWeight::MEDIUM)
        .child(text.into())
}
pub fn heading(text: impl Into<SharedString>) -> Div {
    div()
        .text_size(px(16.))
        .font_weight(FontWeight::SEMIBOLD)
        .child(text.into())
}
pub fn page_title(text: impl Into<SharedString>) -> Div {
    div()
        .text_size(px(26.))
        .line_height(px(32.))
        .font_weight(FontWeight::SEMIBOLD)
        .truncate()
        .child(text.into())
}

/// Stable, quiet hue per album so missing artwork still tells albums apart.
fn tint(seed: &str, p: &Palette) -> Hsla {
    let hash = seed
        .bytes()
        .fold(2166136261u32, |h, b| (h ^ b as u32).wrapping_mul(16777619));
    let hue = (hash % 360) as f32 / 360.;
    if p.dark {
        hsla(hue, 0.16, 0.19, 1.)
    } else {
        hsla(hue, 0.22, 0.88, 1.)
    }
}

pub fn cover(path: Option<&str>, seed: &str, size: f32, cx: &App) -> AnyElement {
    let p = pal(cx);
    let radius = px((size * 0.06).clamp(3., 8.));
    if let Some(path) = path {
        return img(PathBuf::from(path))
            .size(px(size))
            .flex_shrink_0()
            .object_fit(ObjectFit::Cover)
            .rounded(radius)
            .into_any_element();
    }
    div()
        .size(px(size))
        .flex_shrink_0()
        .rounded(radius)
        .bg(tint(seed, &p))
        .flex()
        .items_center()
        .justify_center()
        .child(
            glyph("albums")
                .size(px((size * 0.38).max(12.)))
                .text_color(p.ink.opacity(0.28)),
        )
        .into_any_element()
}
pub fn artwork(track: Option<&Track>, size: f32, cx: &App) -> AnyElement {
    let seed = track
        .map(|t| format!("{}{}", t.album, t.album_artist))
        .unwrap_or_default();
    cover(track.and_then(|t| t.artwork.as_deref()), &seed, size, cx)
}

/// "FLAC 24/96", "MP3 320" — what the file is, in the vocabulary of people who care.
pub fn quality(track: &Track) -> String {
    let khz = track.sample_rate as f64 / 1000.;
    let rate = if khz.fract() == 0. {
        format!("{khz:.0}")
    } else {
        format!("{khz:.1}")
    };
    if track.bit_depth > 0 {
        format!("{} {}/{}", track.format, track.bit_depth, rate)
    } else if track.bitrate > 0 {
        format!("{} {}", track.format, track.bitrate)
    } else {
        track.format.clone()
    }
}

/// A row of mutually exclusive choices. `selected` is the index shown as active.
pub fn segmented(
    id: &'static str,
    options: &[&'static str],
    selected: usize,
    cx: &App,
    on_select: impl Fn(usize, &mut Window, &mut App) + 'static,
) -> Div {
    let p = pal(cx);
    let on_select = std::rc::Rc::new(on_select);
    div()
        .flex()
        .p(px(3.))
        .gap(px(2.))
        .rounded(px(8.))
        .bg(p.raised)
        .children(options.iter().enumerate().map(|(index, option)| {
            let on_select = on_select.clone();
            let active = index == selected;
            div()
                .id((id, index))
                .px_3()
                .py(px(5.))
                .rounded(px(6.))
                .text_size(px(13.))
                .cursor_pointer()
                .when(active, |el| {
                    el.bg(p.canvas)
                        .text_color(p.ink)
                        .font_weight(FontWeight::MEDIUM)
                })
                .when(!active, |el| {
                    el.text_color(p.ink_2).hover(|s| s.text_color(p.ink))
                })
                .child(*option)
                .on_click(move |_, window, cx| on_select(index, window, cx))
        }))
}

/// One settings row: label and explanation on the left, control on the right.
pub fn setting_row(title: &str, description: &str, control: impl IntoElement, cx: &App) -> Div {
    div()
        .py_4()
        .flex()
        .items_center()
        .gap_6()
        .border_b_1()
        .border_color(pal(cx).line_soft)
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap_1()
                .child(strong(title.to_string()))
                .when(!description.is_empty(), |el| {
                    el.child(meta(description.to_string(), cx).line_height(relative(1.45)))
                }),
        )
        .child(div().flex_shrink_0().child(control))
}

pub fn small_button(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Button {
    Button::new(id).small().label(label)
}

/// 500000 → "500,000".
pub fn count(n: usize) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}
