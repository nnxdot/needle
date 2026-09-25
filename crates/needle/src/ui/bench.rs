//! A benchmark that walks Needle through its pages by itself, for measuring stutters without
//! anyone at the keyboard. Runs when `NEEDLE_BENCH` is set, together with `NEEDLE_FRAME_LOG`
//! (the frame timing file, a GPUI patch), and closes Needle when it is done. Use a copy of a
//! data folder (`--data-dir`), never the real one: it changes pages and settings.

use super::{AppView, Page, Panel};
use gpui::{
    Context, Modifiers, MouseButton, MouseDownEvent, NavigationDirection, Pixels, PlatformInput,
    Window, point, px,
};
use std::time::Duration;

type Start = fn(&mut AppView, &mut Window, &mut Context<AppView>);
type Tick = fn(&mut AppView, u32, &mut Window, &mut Context<AppView>);

/// One step: its name in the log, what it does first, and what it does on each tick.
struct Step {
    name: &'static str,
    start: Start,
    tick: Option<Tick>,
}

const STEP: Duration = Duration::from_secs(4);
const TICK: Duration = Duration::from_millis(16);

/// Opens `page` as a click in the sidebar does, so Back and Forward have a history.
fn open(view: &mut AppView, page: Page, window: &mut Window, cx: &mut Context<AppView>) {
    view.navigate(page, window, cx);
}

fn scroll_list(view: &mut AppView, tick: u32, cx: &mut Context<AppView>) {
    let y: Pixels = px(-(tick as f32) * 24.);
    view.list_scroll
        .0
        .borrow()
        .base_handle
        .set_offset(point(px(0.), y));
    cx.notify();
}

fn scroll_grid(view: &mut AppView, tick: u32, cx: &mut Context<AppView>) {
    let y: Pixels = px(-(tick as f32) * 24.);
    view.grid_scroll
        .0
        .borrow()
        .base_handle
        .set_offset(point(px(0.), y));
    cx.notify();
}

fn type_search(view: &mut AppView, tick: u32, window: &mut Window, cx: &mut Context<AppView>) {
    // A letter every 150 ms, as a quick typist would, then start again.
    if !tick.is_multiple_of(9) {
        return;
    }
    let text = &"the love song"[..((tick / 9) as usize % 13) + 1];
    view.search
        .update(cx, |s, cx| s.set_value(text, window, cx));
    view.page_offset = 0;
    view.refresh(cx);
    view.update_suggestions(cx);
}

fn steps() -> Vec<Step> {
    vec![
        Step {
            name: "home",
            start: |v, w, cx| open(v, Page::Home, w, cx),
            tick: None,
        },
        Step {
            name: "songs",
            start: |v, w, cx| open(v, Page::Songs, w, cx),
            tick: None,
        },
        Step {
            name: "songs scroll",
            start: |_, _, _| {},
            tick: Some(|v, t, _, cx| scroll_list(v, t, cx)),
        },
        Step {
            name: "albums",
            start: |v, w, cx| open(v, Page::Albums, w, cx),
            tick: None,
        },
        Step {
            name: "albums scroll",
            start: |_, _, _| {},
            tick: Some(|v, t, _, cx| scroll_grid(v, t, cx)),
        },
        Step {
            name: "artists",
            start: |v, w, cx| open(v, Page::Artists, w, cx),
            tick: None,
        },
        Step {
            name: "artists scroll",
            start: |_, _, _| {},
            tick: Some(|v, t, _, cx| scroll_grid(v, t, cx)),
        },
        Step {
            name: "history",
            start: |v, w, cx| open(v, Page::History, w, cx),
            tick: None,
        },
        Step {
            name: "settings",
            start: |v, w, cx| open(v, Page::Settings, w, cx),
            tick: None,
        },
        Step {
            name: "search typing",
            start: |v, w, cx| open(v, Page::Songs, w, cx),
            tick: Some(type_search),
        },
        Step {
            name: "queue panel",
            start: |v, w, cx| {
                v.search.update(cx, |s, cx| s.set_value("", w, cx));
                v.refresh(cx);
                v.settings.show_inspector = true;
                v.panel = Panel::Queue;
            },
            tick: None,
        },
        Step {
            name: "lyrics panel",
            start: |v, _, _| v.panel = Panel::Lyrics,
            tick: None,
        },
        Step {
            name: "big player",
            start: |v, _, _| {
                v.settings.show_inspector = false;
                v.big = true;
            },
            tick: None,
        },
        Step {
            name: "back to songs",
            start: |v, w, cx| {
                v.big = false;
                open(v, Page::Songs, w, cx);
            },
            tick: None,
        },
    ]
}

impl AppView {
    /// Starts the benchmark when `NEEDLE_BENCH` is set.
    pub(super) fn maybe_bench(&self, window: &mut Window, cx: &mut Context<Self>) {
        if std::env::var_os("NEEDLE_BENCH").is_none() {
            return;
        }
        cx.spawn_in(window, async move |view, cx| {
            gpui::frame_log_phase("startup");
            cx.background_executor().timer(Duration::from_secs(8)).await;
            for step in steps() {
                gpui::frame_log_phase(step.name);
                if view
                    .update_in(cx, |v, window, cx| {
                        (step.start)(v, window, cx);
                        cx.notify();
                    })
                    .is_err()
                {
                    return;
                }
                let ticks = (STEP.as_millis() / TICK.as_millis()) as u32;
                for tick in 0..ticks {
                    cx.background_executor().timer(TICK).await;
                    if let Some(on_tick) = step.tick {
                        let _ = view.update_in(cx, |v, window, cx| on_tick(v, tick, window, cx));
                    }
                }
            }
            // The mouse's side buttons: back and forward again, with the pages noted.
            let page = |cx: &mut gpui::AsyncWindowContext| {
                view.update(cx, |v, _| format!("{:?}", v.page))
                    .unwrap_or_default()
            };
            let before = page(cx);
            for direction in [NavigationDirection::Back, NavigationDirection::Forward] {
                let _ = cx.update(|window, cx| {
                    window.dispatch_event(
                        PlatformInput::MouseDown(MouseDownEvent {
                            button: MouseButton::Navigate(direction),
                            position: point(px(400.), px(300.)),
                            modifiers: Modifiers::default(),
                            click_count: 1,
                            first_mouse: false,
                        }),
                        cx,
                    );
                });
                cx.background_executor().timer(STEP / 4).await;
                let now = page(cx);
                gpui::frame_log_note(|| format!("mouse {direction:?}: {before} -> {now}"));
            }
            gpui::frame_log_phase("end");
            let _ = cx.update(|_, cx| cx.quit());
        })
        .detach();
    }
}
