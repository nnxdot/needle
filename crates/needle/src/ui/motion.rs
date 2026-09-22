//! Motion: short, purposeful animations that can be switched off.
//!
//! Every animation goes through [`animate`], which skips straight to the final state when
//! motion is reduced (Windows' "Show animations in Windows" is off, or Needle's own setting).
use gpui::{prelude::*, *};
use std::time::Duration;

#[derive(Clone, Copy)]
pub struct Motion {
    pub enabled: bool,
}
impl Global for Motion {}

pub fn enabled(cx: &App) -> bool {
    cx.try_global::<Motion>().is_none_or(|m| m.enabled)
}

/// Whether Windows asks apps to animate (Settings › Accessibility › Visual effects).
pub fn system_allows_animation() -> bool {
    #[cfg(windows)]
    {
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            SPI_GETCLIENTAREAANIMATION, SystemParametersInfoW,
        };
        let mut on: i32 = 1;
        let ok = unsafe {
            SystemParametersInfoW(
                SPI_GETCLIENTAREAANIMATION,
                0,
                &mut on as *mut i32 as *mut _,
                0,
            )
        };
        if ok != 0 {
            return on != 0;
        }
    }
    true
}

pub fn ease_out(t: f32) -> f32 {
    1. - (1. - t).powi(4)
}

/// Run `apply` from 0 to 1 over `ms`, once per distinct `id`. With motion reduced the element
/// is drawn at 1 immediately.
pub fn animate<E: IntoElement + 'static>(
    element: E,
    id: impl Into<ElementId>,
    ms: u64,
    cx: &App,
    apply: impl Fn(E, f32) -> E + 'static,
) -> AnyElement {
    if !enabled(cx) {
        return apply(element, 1.).into_any_element();
    }
    element
        .with_animation(
            id,
            Animation::new(Duration::from_millis(ms)).with_easing(ease_out),
            apply,
        )
        .into_any_element()
}

/// A looping animation (0 → 1 → 0 …); drawn still at `rest` when motion is reduced.
pub fn repeat<E: IntoElement + 'static>(
    element: E,
    id: impl Into<ElementId>,
    ms: u64,
    rest: f32,
    cx: &App,
    apply: impl Fn(E, f32) -> E + 'static,
) -> AnyElement {
    if !enabled(cx) {
        return apply(element, rest).into_any_element();
    }
    element
        .with_animation(
            id,
            Animation::new(Duration::from_millis(ms))
                .repeat()
                .with_easing(pulsating_between(0., 1.)),
            apply,
        )
        .into_any_element()
}

/// Mix two colours; `t` = 0 gives `a`, 1 gives `b`.
pub fn mix(a: Hsla, b: Hsla, t: f32) -> Hsla {
    let (a, b) = (a.to_rgb(), b.to_rgb());
    Rgba {
        r: a.r + (b.r - a.r) * t,
        g: a.g + (b.g - a.g) * t,
        b: a.b + (b.b - a.b) * t,
        a: a.a + (b.a - a.a) * t,
    }
    .into()
}

/// A heart (or any icon) that swells and settles once when `id` changes.
pub fn pop(icon: Svg, id: impl Into<ElementId>, cx: &App) -> AnyElement {
    animate(icon, id, 420, cx, |el, t| {
        let s = 1. + 0.38 * (t * std::f32::consts::PI).sin();
        el.with_transformation(Transformation::scale(size(s, s)))
    })
}

/// Three little bars that bounce while a song plays and rest when it is paused.
pub fn equalizer(
    id: impl Into<SharedString>,
    color: Hsla,
    moving: bool,
    cx: &App,
) -> impl IntoElement {
    let id: SharedString = id.into();
    let moving = moving && enabled(cx);
    div().h(px(14.)).flex().items_end().gap(px(2.)).children(
        [(620u64, 0.55), (820, 0.9), (540, 0.4)]
            .into_iter()
            .enumerate()
            .map(move |(i, (ms, rest))| {
                let bar = div().w(px(3.)).rounded(px(1.)).bg(color);
                if moving {
                    bar.with_animation(
                        ElementId::NamedInteger(id.clone(), i as u64),
                        Animation::new(Duration::from_millis(ms))
                            .repeat()
                            .with_easing(pulsating_between(0., 1.)),
                        |el, t| el.h(px(3. + 11. * t)),
                    )
                    .into_any_element()
                } else {
                    bar.h(px(3. + 11. * rest * 0.5)).into_any_element()
                }
            }),
    )
}
