//! Motion: short, purposeful animations that can be switched off.
//!
//! Every animation goes through [`animate`], which skips straight to the final state when
//! motion is reduced (Windows' "Show animations in Windows" is off, or Needle's own setting).
use gpui::{prelude::*, *};
use std::sync::{
    OnceLock,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};

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

/// Set while a [`slow_repeat`] is on screen; `poll` then redraws at its own pace.
static SLOW_LOOPS: AtomicBool = AtomicBool::new(false);

/// A looping animation (0 → 1 → 0 …) like [`repeat`], for slow or small movements such as a
/// drifting backdrop or bouncing bars. It moves on Needle's timer (25 times a second while a
/// song plays) instead of on every screen refresh: a GPUI animation redraws the whole window
/// at the screen's rate (up to 180 times a second) for as long as it is on screen.
pub fn slow_repeat<E>(element: E, ms: u64, rest: f32, cx: &App, apply: impl Fn(E, f32) -> E) -> E {
    if !enabled(cx) {
        return apply(element, rest);
    }
    SLOW_LOOPS.store(true, Ordering::Relaxed);
    static CLOCK: OnceLock<Instant> = OnceLock::new();
    let elapsed = CLOCK.get_or_init(Instant::now).elapsed().as_millis() as u64;
    let phase = (elapsed % ms.max(1)) as f32 / ms.max(1) as f32;
    apply(element, pulsating_between(0., 1.)(phase))
}

/// Whether a [`slow_repeat`] was drawn since the last call.
pub fn take_slow_loops() -> bool {
    SLOW_LOOPS.swap(false, Ordering::Relaxed)
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
pub fn equalizer(color: Hsla, moving: bool, cx: &App) -> impl IntoElement {
    let moving = moving && enabled(cx);
    div().h(px(14.)).flex().items_end().gap(px(2.)).children(
        [(620u64, 0.55), (820, 0.9), (540, 0.4)].map(|(ms, rest)| {
            let bar = div().w(px(3.)).rounded(px(1.)).bg(color);
            if moving {
                slow_repeat(bar, ms, rest, cx, |el, t| el.h(px(3. + 11. * t)))
            } else {
                bar.h(px(3. + 11. * rest * 0.5))
            }
        }),
    )
}
