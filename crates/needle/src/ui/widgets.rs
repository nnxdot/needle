use super::theme::{Palette, display_font, pal};
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
    display(text, 34.).truncate()
}
/// Big names in the display face: page titles, album and artist names, the big player.
pub fn display(text: impl Into<SharedString>, size: f32) -> Div {
    div()
        .font_family(display_font())
        .text_size(px(size))
        .line_height(px(size * 1.18))
        .font_weight(FontWeight::SEMIBOLD)
        .child(text.into())
}

fn seed_hash(seed: &str) -> u32 {
    seed.to_lowercase()
        .bytes()
        .fold(2166136261u32, |h, b| (h ^ b as u32).wrapping_mul(16777619))
}

/// The main colour of the made-up cover for `seed`, so it can tint the app like a real one.
pub fn seed_color(seed: &str) -> Hsla {
    hsla((seed_hash(seed) % 360) as f32 / 360., 0.5, 0.4, 1.)
}
/// The seed a track's made-up cover is drawn from.
pub fn track_seed(track: &Track) -> String {
    format!("{}{}", track.album, track.album_artist)
}

/// A made-up cover for music without artwork: a two-colour gradient and a large, cropped
/// first letter, different for every album and the same every time.
pub fn generated_cover(seed: &str, size: f32, round: bool, p: &Palette) -> Div {
    let hash = seed_hash(seed);
    let h1 = (hash % 360) as f32 / 360.;
    let h2 = (h1 + 0.06 + ((hash >> 9) % 12) as f32 / 100.) % 1.;
    let angle = 110. + ((hash >> 13) % 120) as f32;
    let (a, b) = if p.dark {
        (hsla(h1, 0.42, 0.34, 1.), hsla(h2, 0.5, 0.17, 1.))
    } else {
        (hsla(h1, 0.5, 0.8, 1.), hsla(h2, 0.42, 0.62, 1.))
    };
    // Round (artist) covers show up to two initials; square ones a single letter.
    let letter: String = seed
        .chars()
        .filter(|c| c.is_alphanumeric())
        .take(if round { 2 } else { 1 })
        .flat_map(|c| c.to_uppercase())
        .collect();
    let letter = if letter.is_empty() {
        "♪".to_string()
    } else {
        letter
    };
    let el = div()
        .size(px(size))
        .flex_shrink_0()
        .relative()
        .overflow_hidden()
        .bg(linear_gradient(
            angle,
            linear_color_stop(a, 0.),
            linear_color_stop(b, 1.),
        ));
    let el = if round {
        el.rounded_full()
    } else {
        el.rounded(px((size * 0.06).clamp(3., 8.)))
    };
    el.when(size >= 28., |el| {
        let ink = if p.dark {
            gpui::white().opacity(0.2)
        } else {
            gpui::black().opacity(0.16)
        };
        if round {
            el.flex().items_center().justify_center().child(
                div()
                    .font_family(display_font())
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_size(px(size * 0.42))
                    .text_color(ink.opacity(if p.dark { 0.75 } else { 0.6 }))
                    .child(letter),
            )
        } else {
            el.child(
                div()
                    .absolute()
                    .left(px(size * 0.08))
                    .bottom(px(-size * 0.2))
                    .font_family(display_font())
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_size(px(size * 0.78))
                    .line_height(px(size * 0.9))
                    .text_color(ink)
                    .child(letter),
            )
        }
    })
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
    let _ = radius;
    generated_cover(seed, size, false, &p).into_any_element()
}
pub fn artwork(track: Option<&Track>, size: f32, cx: &App) -> AnyElement {
    let seed = track.map(track_seed).unwrap_or_default();
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
                    el.child(
                        meta(description.to_string(), cx)
                            .w_full()
                            .line_height(relative(1.45)),
                    )
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
