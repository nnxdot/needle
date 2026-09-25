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
const STALL_TICK: Duration = Duration::from_millis(50);
const STALL: Duration = Duration::from_millis(30);

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
            name: "album page",
            start: |v, w, cx| {
                let track = v
                    .library
                    .search_page("", 0, 1)
                    .ok()
                    .and_then(|p| p.tracks.into_iter().next());
                if let Some(track) = track {
                    open(v, super::album_page(&track), w, cx);
                }
            },
            tick: None,
        },
        Step {
            name: "artist page",
            start: |v, w, cx| {
                let track = v
                    .library
                    .search_page("", 0, 1)
                    .ok()
                    .and_then(|p| p.tracks.into_iter().next());
                if let Some(track) = track {
                    open(v, Page::Artist(track.artist), w, cx);
                }
            },
            tick: None,
        },
        Step {
            name: "songs sorted",
            start: |v, w, cx| {
                open(v, Page::Songs, w, cx);
                v.sort = super::Sort::Asc("title");
                v.page_offset = 0;
                v.refresh(cx);
            },
            tick: None,
        },
        Step {
            // Blocks the UI thread for 80 ms on purpose: the stall watcher must note it.
            name: "stall self-test",
            start: |_, _, _| std::thread::sleep(Duration::from_millis(80)),
            tick: None,
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
            name: "full screen",
            start: |v, w, cx| {
                v.big = false;
                v.set_immersive(true, w, cx);
            },
            tick: None,
        },
        Step {
            // The playing song's album: its row shows the bouncing bars.
            name: "playing album",
            start: |v, w, cx| {
                v.set_immersive(false, w, cx);
                if let Some(item) = v.playback.current.clone() {
                    open(v, super::album_page(&item.track), w, cx);
                }
            },
            tick: None,
        },
        Step {
            name: "player on top",
            start: |v, w, cx| {
                open(v, Page::Songs, w, cx);
                v.settings.layout.player_on_top = true;
            },
            tick: Some(|v, t, _, cx| scroll_list(v, t, cx)),
        },
        Step {
            // No search field: Ctrl+F opens the command palette instead.
            name: "search hidden",
            start: |v, w, cx| {
                v.settings.layout.search_hidden = true;
                w.dispatch_action(Box::new(super::FocusSearch), cx);
            },
            tick: None,
        },
        Step {
            name: "folded sidebar",
            start: |v, w, cx| {
                gpui::frame_log_note(|| format!("palette open after Ctrl+F: {}", v.palette.open));
                v.palette.open = false;
                v.settings.layout.search_hidden = false;
                v.settings.layout.player_on_top = false;
                open(v, Page::Songs, w, cx);
                v.toggle_sidebar();
            },
            tick: Some(|v, t, _, cx| scroll_list(v, t, cx)),
        },
        Step {
            // Folded with the compact sidebar off: no sidebar at all.
            name: "sidebar hidden",
            start: |v, _, _| v.settings.layout.compact_sidebar = false,
            tick: None,
        },
        Step {
            name: "back to songs",
            start: |v, w, cx| {
                v.settings.layout.compact_sidebar = true;
                v.toggle_sidebar();
                open(v, Page::Songs, w, cx);
            },
            tick: None,
        },
    ]
}

impl AppView {
    /// Starts the benchmark when `NEEDLE_BENCH` is set.
    pub(super) fn maybe_bench(&self, window: &mut Window, cx: &mut Context<Self>) {
        if std::env::var_os("NEEDLE_FRAME_LOG").is_some() {
            // Stalls: a timer on the UI thread that should wake every 50 ms notes each time it
            // woke more than 30 ms late, as the thread was busy (drawing, or other work).
            cx.spawn(async move |_, cx| {
                loop {
                    let asked = std::time::Instant::now();
                    cx.background_executor().timer(STALL_TICK).await;
                    let late = asked.elapsed().saturating_sub(STALL_TICK);
                    if late > STALL {
                        gpui::frame_log_note(|| format!("UI thread busy {late:.0?}"));
                    }
                }
            })
            .detach();
        }
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

/// Notes in the frame log (`NEEDLE_FRAME_LOG`) when the work it guards took over 8 ms of the
/// UI thread: `let _slow = Slow::new("open");` at the top of a function.
pub(super) struct Slow(&'static str, std::time::Instant);

impl Slow {
    pub(super) fn new(name: &'static str) -> Self {
        Self(name, std::time::Instant::now())
    }
}

impl Drop for Slow {
    fn drop(&mut self) {
        let took = self.1.elapsed();
        if took > Duration::from_millis(8) {
            gpui::frame_log_note(|| format!("slow {}: {took:.1?}", self.0));
        }
    }
}
