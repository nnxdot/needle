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
        use windows_sys::Win32::UI::WindowsAndMessaging::{SPI_GETCLIENTAREAANIMATION, SystemParametersInfoW};
        let mut on: i32 = 1;
        let ok = unsafe { SystemParametersInfoW(SPI_GETCLIENTAREAANIMATION, 0, &mut on as *mut i32 as *mut _, 0) };
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
        .with_animation(id, Animation::new(Duration::from_millis(ms)).with_easing(ease_out), apply)
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
            Animation::new(Duration::from_millis(ms)).repeat().with_easing(pulsating_between(0., 1.)),
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
