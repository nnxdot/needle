//! Motion: short, purposeful animations that can be switched off.
//!
//! Every animation goes through [`animate`] (or [`fade`]), which skips straight to the final
//! state when motion is reduced (the system's animations are off, or Needle's own setting)
//! and when a key started it. With motion reduced, [`fade`] keeps a short fade.
use gpui::{prelude::*, *};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::sync::{
    OnceLock,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};

/// Needle's own Reduce motion switch.
static APP_STILL: AtomicBool = AtomicBool::new(false);
/// The system asks apps not to animate (read at start and each time the window comes to the
/// front, so a change made while Needle runs is followed).
static SYSTEM_STILL: AtomicBool = AtomicBool::new(false);

/// Whether things may move. With motion reduced, only short fades stay (see [`fade`]).
pub fn enabled(_cx: &App) -> bool {
    !APP_STILL.load(Ordering::Relaxed) && !SYSTEM_STILL.load(Ordering::Relaxed)
}

/// Needle's own Reduce motion switch changed (or was read at start).
pub fn set_reduce(on: bool) {
    APP_STILL.store(on, Ordering::Relaxed);
    tell_toolkit();
}

/// The toolkit's dialogs and switches keep still with Needle.
fn tell_toolkit() {
    gpui_component::animation::set_reduce_motion(
        APP_STILL.load(Ordering::Relaxed) || SYSTEM_STILL.load(Ordering::Relaxed),
    );
}

/// Whether the system asks apps to animate, as last read by [`refresh_system`].
pub fn system_allows_animation() -> bool {
    !SYSTEM_STILL.load(Ordering::Relaxed)
}

/// Read the system's animation setting again: Windows' "Animation effects" (Settings ›
/// Accessibility › Visual effects); on Linux, GNOME's "enable-animations" and KDE's animation
/// speed. Linux asks over D-Bus, so it reads on another thread.
pub fn refresh_system() {
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
        SYSTEM_STILL.store(ok != 0 && on == 0, Ordering::Relaxed);
        tell_toolkit();
    }
    #[cfg(target_os = "linux")]
    {
        static READING: AtomicBool = AtomicBool::new(false);
        if READING.swap(true, Ordering::Relaxed) {
            return;
        }
        std::thread::spawn(|| {
            let still = linux::gnome_still().unwrap_or(false) || linux::kde_still();
            SYSTEM_STILL.store(still, Ordering::Relaxed);
            tell_toolkit();
            READING.store(false, Ordering::Relaxed);
        });
    }
}

#[cfg(target_os = "linux")]
mod linux {
    /// GNOME (and desktops that answer for its settings): "enable-animations" off. Asked
    /// through the desktop's settings portal, else through `gsettings`.
    pub fn gnome_still() -> Option<bool> {
        portal("org.gnome.desktop.interface", "enable-animations")
            .or_else(|| {
                let out = std::process::Command::new("gsettings")
                    .args(["get", "org.gnome.desktop.interface", "enable-animations"])
                    .output()
                    .ok()?;
                match String::from_utf8_lossy(&out.stdout).trim() {
                    "true" => Some(true),
                    "false" => Some(false),
                    _ => None,
                }
            })
            .map(|on| !on)
    }

    fn portal(namespace: &str, key: &str) -> Option<bool> {
        use zbus::zvariant::{OwnedValue, Value};
        let bus = zbus::blocking::Connection::session().ok()?;
        let reply = bus
            .call_method(
                Some("org.freedesktop.portal.Desktop"),
                "/org/freedesktop/portal/desktop",
                Some("org.freedesktop.portal.Settings"),
                "Read",
                &(namespace, key),
            )
            .ok()?;
        let body = reply.body();
        let value: OwnedValue = body.deserialize().ok()?;
        // The old Read call wraps the value in a variant inside a variant.
        let mut value: Value = value.into();
        while let Value::Value(inner) = value {
            value = *inner;
        }
        match value {
            Value::Bool(on) => Some(on),
            _ => None,
        }
    }

    /// KDE Plasma: "Animation speed" set to instant (`AnimationDurationFactor=0` in
    /// kdeglobals).
    pub fn kde_still() -> bool {
        let dir = std::env::var_os("XDG_CONFIG_HOME")
            .map(std::path::PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| std::path::Path::new(&h).join(".config")));
        let Some(text) = dir.and_then(|d| std::fs::read_to_string(d.join("kdeglobals")).ok())
        else {
            return false;
        };
        let mut in_kde = false;
        for line in text.lines() {
            let line = line.trim();
            if line.starts_with('[') {
                in_kde = line == "[KDE]";
            } else if in_kde && let Some(value) = line.strip_prefix("AnimationDurationFactor=") {
                return value.trim().parse::<f32>().is_ok_and(|f| f <= 0.);
            }
        }
        false
    }
}

pub fn ease_out(t: f32) -> f32 {
    1. - (1. - t).powi(4)
}

// Keys never start an animation: a key's result shows at once. The action handlers call
// [`key_started`]; the window's mouse-down clears it; and it lasts for the frame drawn next.
thread_local! {
    /// 0: no key; 1: a key action ran, the next frame is its; 2: this frame is a key's.
    static BY_KEY: Cell<u8> = const { Cell::new(0) };
    /// Animations drawn last frame and this frame, and whether a key drew them still.
    static SEEN: RefCell<(HashMap<ElementId, bool>, HashMap<ElementId, bool>)> =
        RefCell::new(Default::default());
}

/// A keyboard shortcut ran: what it changes shows at once.
pub fn key_started() {
    BY_KEY.with(|k| k.set(1));
}

/// A mouse button went down: what it starts may move again.
pub fn mouse_started() {
    BY_KEY.with(|k| k.set(0));
}

/// Whether the change being made (or drawn) came from a key.
pub fn by_key() -> bool {
    BY_KEY.with(|k| k.get() != 0)
}

/// Called once at the start of each frame of the main window, before anything is drawn.
pub fn begin_frame() {
    BY_KEY.with(|k| {
        k.set(match k.get() {
            1 => 2,
            _ => 0,
        })
    });
    SEEN.with(|s| {
        let mut s = s.borrow_mut();
        let (last, this) = &mut *s;
        std::mem::swap(last, this);
        this.clear();
    });
}

/// Whether the animation `id` is drawn at its end because a key started it. An animation
/// already running when a key comes goes on; a new one started by a key does not run, and
/// stays at its end for as long as it is drawn (GPUI forgets an animation once it is not
/// drawn, and so does this).
fn still_by_key(id: &ElementId) -> bool {
    SEEN.with(|s| {
        let mut s = s.borrow_mut();
        let (last, this) = &mut *s;
        let still = match last.get(id) {
            Some(&still) => still,
            None => by_key(),
        };
        this.insert(id.clone(), still);
        still
    })
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
    let id = id.into();
    if !enabled(cx) || still_by_key(&id) {
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

/// The longest fade kept with motion reduced.
const STILL_FADE_MS: u64 = 150;

/// Like [`animate`], for something that fades and moves: `apply` gets the fade (0 → 1) and
/// the move (0 → 1) apart. With motion reduced the move is at its end at once and only a
/// short fade stays (at most 150 ms); started by a key, both are at their end.
pub fn fade<E: IntoElement + 'static>(
    element: E,
    id: impl Into<ElementId>,
    ms: u64,
    cx: &App,
    apply: impl Fn(E, f32, f32) -> E + 'static,
) -> AnyElement {
    let id = id.into();
    if still_by_key(&id) {
        return apply(element, 1., 1.).into_any_element();
    }
    if !enabled(cx) {
        return element
            .with_animation(
                id,
                Animation::new(Duration::from_millis(ms.min(STILL_FADE_MS))).with_easing(ease_out),
                move |el, t| apply(el, t, 1.),
            )
            .into_any_element();
    }
    element
        .with_animation(
            id,
            Animation::new(Duration::from_millis(ms)).with_easing(ease_out),
            move |el, t| apply(el, t, t),
        )
        .into_any_element()
}

/// A dialog opening: it fades in and rises 12 px into place over 220 ms, once each time it
/// appears (GPUI forgets an animation's state when its element is not drawn, so `id` can stay
/// the same). The toolkit's own dialogs move the same way.
pub fn dialog_in<E: IntoElement + Styled + 'static>(
    card: E,
    id: &'static str,
    cx: &App,
) -> AnyElement {
    fade(card, id, 220, cx, |el, f, m| {
        el.opacity(f).mt(px(12. * (1. - m)))
    })
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

/// A heart (or any icon) that swells a little and settles once when `id` changes (not when a
/// key changed it).
pub fn pop(icon: Svg, id: impl Into<ElementId>, cx: &App) -> AnyElement {
    animate(icon, id, 300, cx, |el, t| {
        let s = 1. + 0.2 * (t * std::f32::consts::PI).sin();
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
