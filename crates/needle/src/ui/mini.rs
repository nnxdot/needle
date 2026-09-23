use super::{
    AppView, pal,
    widgets::{artwork, faint, glyph, icon_button, meta},
};
use gpui::{prelude::*, *};
use gpui_component::{
    Root, Sizable,
    slider::{Slider, SliderEvent, SliderState},
};
use needle_core::{audio::Command, model::format_duration};

pub const COMPACT: Size<Pixels> = Size {
    width: px(360.),
    height: px(156.),
};
pub const EXPANDED: Size<Pixels> = Size {
    width: px(360.),
    height: px(640.),
};

#[derive(Clone, Copy, PartialEq)]
enum Tab {
    Next,
    Lyrics,
    History,
}

/// A small always-available window with the essentials, like Apple Music's mini player.
pub struct MiniView {
    app: WeakEntity<AppView>,
    main: AnyWindowHandle,
    expanded: bool,
    tab: Tab,
    pinned: bool,
    glass_applied: Option<(super::glass::Material, bool)>,
    /// The mini player's own sliders. Sharing the main window's would mix up their sizes, so
    /// the thumb and the filled part drift apart when both windows are open.
    seek: Entity<SliderState>,
    volume: Entity<SliderState>,
    _observe: Option<Subscription>,
    _sliders: Vec<Subscription>,
}

impl AppView {
    pub(super) fn open_mini(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(handle) = self.mini
            && handle
                .update(cx, |_, window, _| window.activate_window())
                .is_ok()
        {
            window.minimize_window();
            return;
        }
        let app = cx.entity();
        let main = window.window_handle();
        // Open after this update finishes: the new window's first frame reads this view.
        cx.defer(move |cx| {
            let bounds = Bounds::centered(None, COMPACT, cx);
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("Needle mini player".into()),
                    appears_transparent: true,
                    ..Default::default()
                }),
                window_min_size: Some(size(px(300.), px(140.))),
                // The mini player sizes itself: compact, or expanded with a list.
                is_resizable: false,
                ..Default::default()
            };
            let weak = app.downgrade();
            let opened = cx.open_window(options, |window, cx| {
                window.set_window_title("Needle mini player");
                let view = cx.new(|cx| {
                    let seek = cx.new(|_| SliderState::new().min(0.).max(1000.).step(1.));
                    let volume = cx.new(|_| SliderState::new().min(0.).max(1.).step(0.01));
                    let sliders = vec![
                        cx.subscribe(&seek, |this: &mut MiniView, _, event: &SliderEvent, cx| {
                            let SliderEvent::Change(value) = event;
                            let Some(app) = this.app.upgrade() else {
                                return;
                            };
                            let a = app.read(cx);
                            if let Some(item) = &a.playback.current {
                                a.player.send(Command::Seek(
                                    value.start() as f64 / 1000.0 * item.track.duration,
                                ));
                            }
                        }),
                        cx.subscribe(
                            &volume,
                            |this: &mut MiniView, _, event: &SliderEvent, cx| {
                                let SliderEvent::Change(value) = event;
                                let volume = value.start();
                                if let Some(app) = this.app.upgrade() {
                                    app.read(cx).player.send(Command::Volume(volume));
                                }
                            },
                        ),
                    ];
                    MiniView {
                        seek,
                        volume,
                        _sliders: sliders,
                        _observe: Some(cx.observe(&app, |_, _, cx| cx.notify())),
                        app: weak.clone(),
                        main,
                        expanded: false,
                        tab: Tab::Next,
                        pinned: false,
                        glass_applied: None,
                    }
                });
                cx.new(|cx| Root::new(view, window, cx))
            });
            match opened {
                Ok(handle) => {
                    app.update(cx, |this, _| this.mini = Some(handle.into()));
                    let _ = main.update(cx, |_, window, _| window.minimize_window());
                }
                Err(e) => app.update(cx, |this, cx| {
                    this.fail(format!("The mini player could not open: {e:#}"));
                    cx.notify();
                }),
            }
        });
    }
}

/// Keep a window above others (Windows only).
fn set_topmost(window: &Window, on: bool) {
    #[cfg(windows)]
    {
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            HWND_NOTOPMOST, HWND_TOPMOST, SWP_NOMOVE, SWP_NOSIZE, SetWindowPos,
        };
        if let Ok(handle) = HasWindowHandle::window_handle(window)
            && let RawWindowHandle::Win32(win32) = handle.as_raw()
        {
            unsafe {
                SetWindowPos(
                    win32.hwnd.get() as _,
                    if on { HWND_TOPMOST } else { HWND_NOTOPMOST },
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE,
                );
            }
        }
    }
    #[cfg(not(windows))]
    let _ = (window, on);
}

/// After growing, move the window up so it stays inside the screen's work area (Windows only).
fn keep_on_screen(window: &Window, grow: f32) {
    #[cfg(windows)]
    {
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
        use windows_sys::Win32::{
            Foundation::RECT,
            Graphics::Gdi::{
                GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow,
            },
            UI::WindowsAndMessaging::{GetWindowRect, SWP_NOSIZE, SWP_NOZORDER, SetWindowPos},
        };
        if let Ok(handle) = HasWindowHandle::window_handle(window)
            && let RawWindowHandle::Win32(win32) = handle.as_raw()
        {
            let hwnd = win32.hwnd.get() as _;
            unsafe {
                let mut rect: RECT = std::mem::zeroed();
                let mut info: MONITORINFO = std::mem::zeroed();
                info.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
                if GetWindowRect(hwnd, &mut rect) == 0
                    || GetMonitorInfoW(MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST), &mut info)
                        == 0
                {
                    return;
                }
                // The resize has not happened yet, so measure with the height it is growing by.
                let grow = (grow * window.scale_factor()) as i32;
                let overflow = rect.bottom + grow - info.rcWork.bottom;
                if overflow > 0 {
                    let top = (rect.top - overflow).max(info.rcWork.top);
                    SetWindowPos(
                        hwnd,
                        0 as _,
                        rect.left,
                        top,
                        0,
                        0,
                        SWP_NOSIZE | SWP_NOZORDER,
                    );
                }
            }
        }
    }
    #[cfg(not(windows))]
    let _ = (window, grow);
}

impl MiniView {
    fn back_to_main(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let _ = self.main.update(cx, |_, main, _| main.activate_window());
        if let Some(app) = self.app.upgrade() {
            app.update(cx, |this, _| this.mini = None);
        }
        window.remove_window();
    }
    fn send(&self, command: Command, cx: &mut Context<Self>) {
        if let Some(app) = self.app.upgrade() {
            app.read(cx).player.send(command);
        }
    }
}

impl Render for MiniView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = pal(cx);
        let Some(app) = self.app.upgrade() else {
            window.remove_window();
            return div().into_any_element();
        };
        let (current, playing, position, volume, seek, volume_state, blur, material) = {
            let a = app.read(cx);
            let material = a.material();
            let blur = a
                .settings
                .music_colors
                .then(|| {
                    a.playback
                        .current
                        .as_ref()
                        .and_then(|c| c.track.artwork.as_ref())
                })
                .flatten()
                .and_then(|art| a.cached_look(art))
                .and_then(|look| look.blur);
            (
                a.playback.current.clone(),
                a.playback.playing,
                a.playback.position,
                a.playback.volume,
                self.seek.clone(),
                self.volume.clone(),
                blur,
                material,
            )
        };
        // Follow playback, changing the sliders only when they are off, so a frame is not
        // redrawn for nothing.
        let seek_value = match &current {
            Some(item) if item.track.duration > 0.0 => {
                (position / item.track.duration * 1000.0) as f32
            }
            _ => 0.,
        };
        if (self.seek.read(cx).value().start() - seek_value).abs() > 0.5 {
            self.seek
                .update(cx, |s, cx| s.set_value(seek_value, window, cx));
        }
        if (self.volume.read(cx).value().start() - volume).abs() > 0.001 {
            self.volume
                .update(cx, |s, cx| s.set_value(volume, window, cx));
        }
        if self.glass_applied != Some((material, p.dark)) {
            super::glass::apply(window, material, p.dark);
            self.glass_applied = Some((material, p.dark));
        }
        let control = |id: &'static str, name: &'static str, tip: &'static str| {
            icon_button(id, name, tip).xsmall()
        };
        let tab = self.tab;
        let tab_button =
            |id: &'static str, label: &'static str, which: Tab, cx: &mut Context<Self>| {
                let active = tab == which;
                div()
                    .id(id)
                    .flex_1()
                    .py(px(5.))
                    .rounded(px(6.))
                    // Centre with layout: a hover style resets text alignment in GPUI.
                    .flex()
                    .justify_center()
                    .text_size(px(12.5))
                    .cursor_pointer()
                    .when(active, |el| {
                        el.bg(p.canvas)
                            .text_color(p.ink)
                            .font_weight(FontWeight::MEDIUM)
                    })
                    .when(!active, |el| {
                        el.text_color(p.ink_2).hover(|s| s.text_color(p.ink))
                    })
                    .child(label)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.tab = which;
                        cx.notify();
                    }))
            };
        div()
            .size_full()
            .relative()
            .overflow_hidden()
            .bg(p.back)
            .text_color(p.ink)
            .flex()
            .flex_col()
            // The cover, blurred, glowing behind everything.
            .when_some(blur, |el, blur| {
                el.child(
                    img(blur)
                        .absolute()
                        .inset_0()
                        .size_full()
                        .object_fit(ObjectFit::Cover)
                        .opacity(if p.dark { 0.55 } else { 0.4 } * (0.4 + 0.6 * p.back.a)),
                )
                .child(div().absolute().inset_0().bg(linear_gradient(
                    180.,
                    linear_color_stop(p.chrome.opacity(0.25 * p.back.a), 0.),
                    linear_color_stop(p.chrome.opacity(0.75 * p.back.a), 1.),
                )))
            })
            // Title strip: drag anywhere, window controls on the right.
            .child(
                div()
                    .id("mini-title")
                    .h(px(30.))
                    .flex_shrink_0()
                    .pl_3()
                    .pr_1()
                    .flex()
                    .items_center()
                    .window_control_area(WindowControlArea::Drag)
                    .child(glyph("logo").size(px(14.)).text_color(p.accent))
                    .child(div().flex_1())
                    .child(
                        control(
                            "mini-pin",
                            if self.pinned { "pin-fill" } else { "pin" },
                            "Keep on top",
                        )
                        .when(self.pinned, |b| b.text_color(p.accent))
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.pinned = !this.pinned;
                            set_topmost(window, this.pinned);
                            cx.notify();
                        })),
                    )
                    .child(
                        control("mini-full", "expand", "Back to the full window").on_click(
                            cx.listener(|this, _, window, cx| this.back_to_main(window, cx)),
                        ),
                    )
                    .child(
                        control("mini-min", "minus", "Minimize")
                            .on_click(|_, window, _| window.minimize_window()),
                    )
                    .child(
                        control("mini-close", "close", "Close the mini player").on_click(
                            cx.listener(|this, _, window, cx| this.back_to_main(window, cx)),
                        ),
                    ),
            )
            // Now playing
            .child(
                div()
                    .px_3()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(
                        div()
                            .id("mini-art")
                            .cursor_pointer()
                            .child(artwork(current.as_ref().map(|c| &c.track), 52., cx))
                            .on_click(cx.listener(|this, _, window, cx| {
                                if let Some(app) = this.app.upgrade() {
                                    app.update(cx, |a, _| a.big = true);
                                }
                                this.back_to_main(window, cx);
                            })),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap(px(2.))
                            .child(
                                div()
                                    .text_size(px(13.5))
                                    .font_weight(FontWeight::MEDIUM)
                                    .truncate()
                                    .child(
                                        current
                                            .as_ref()
                                            .map(|c| c.track.title.clone())
                                            .unwrap_or_else(|| "Nothing playing".into()),
                                    ),
                            )
                            .child(
                                meta(
                                    current
                                        .as_ref()
                                        .map(|c| c.track.display_artist().to_string())
                                        .unwrap_or_default(),
                                    cx,
                                )
                                .truncate(),
                            ),
                    ),
            )
            .child(
                div()
                    .px_3()
                    .pt_1()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        faint(
                            format_duration(if current.is_some() { position } else { 0. }),
                            cx,
                        )
                        .w(px(32.)),
                    )
                    .child(Slider::new(&seek).flex_1().disabled(current.is_none()))
                    .child(
                        faint(
                            current
                                .as_ref()
                                .map(|c| format_duration(c.track.duration))
                                .unwrap_or_default(),
                            cx,
                        )
                        .w(px(32.))
                        .text_right(),
                    ),
            )
            .child(
                div()
                    .h(px(40.))
                    .px_2()
                    .flex()
                    .items_center()
                    .child(
                        div()
                            .w(px(92.))
                            .flex()
                            .items_center()
                            .gap_1()
                            .child(
                                glyph(if volume <= 0.001 {
                                    "volume-off"
                                } else {
                                    "volume-low"
                                })
                                .size(px(14.))
                                .text_color(p.ink_2),
                            )
                            .child(Slider::new(&volume_state).flex_1()),
                    )
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .justify_center()
                            .items_center()
                            .gap_1()
                            .child(
                                control("mini-prev", "previous", "Previous")
                                    .small()
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.send(Command::Previous, cx)
                                    })),
                            )
                            .child(
                                div()
                                    .id("mini-play")
                                    .size(px(32.))
                                    .rounded_full()
                                    .bg(p.ink)
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .cursor_pointer()
                                    .hover(|s| s.opacity(0.88))
                                    .active(|s| s.size(px(29.)).m(px(1.5)).opacity(0.8))
                                    .child(
                                        glyph(if playing { "pause" } else { "play" })
                                            .size(px(15.))
                                            .text_color(p.canvas),
                                    )
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        if let Some(app) = this.app.upgrade() {
                                            app.update(cx, |a, cx| a.toggle_playback(cx));
                                        }
                                    })),
                            )
                            .child(control("mini-next", "next", "Next").small().on_click(
                                cx.listener(|this, _, _, cx| this.send(Command::Next, cx)),
                            )),
                    )
                    .child(
                        div()
                            .w(px(92.))
                            .flex()
                            .justify_end()
                            .child(
                                control("mini-lyrics", "lyrics", "Lyrics")
                                    .when(self.expanded && tab == Tab::Lyrics, |b| {
                                        b.text_color(p.accent)
                                    })
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.toggle(Tab::Lyrics, window);
                                        cx.notify();
                                    })),
                            )
                            .child(
                                control("mini-queue", "queue", "Playing next and history")
                                    .when(self.expanded && tab != Tab::Lyrics, |b| {
                                        b.text_color(p.accent)
                                    })
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.toggle(Tab::Next, window);
                                        cx.notify();
                                    })),
                            ),
                    ),
            )
            .when(self.expanded, |el| {
                let body = match tab {
                    Tab::Lyrics => {
                        app.update(cx, |a, cx| a.lyrics_view(false, cx).into_any_element())
                    }
                    Tab::Next => app.update(cx, |a, cx| a.up_next(cx).into_any_element()),
                    Tab::History => app.update(cx, |a, cx| a.recent_listens(cx).into_any_element()),
                };
                el.child(
                    div()
                        .flex_1()
                        .min_h_0()
                        .border_t_1()
                        .border_color(p.line_soft)
                        .px_3()
                        .pt_3()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .when(tab != Tab::Lyrics, |el| {
                            el.child(
                                div()
                                    .flex()
                                    .p(px(3.))
                                    .gap(px(2.))
                                    .rounded(px(8.))
                                    .bg(p.raised)
                                    .child(tab_button(
                                        "mini-tab-next",
                                        "Playing next",
                                        Tab::Next,
                                        cx,
                                    ))
                                    .child(tab_button(
                                        "mini-tab-history",
                                        "History",
                                        Tab::History,
                                        cx,
                                    )),
                            )
                        })
                        .child(body),
                )
            })
            .into_any_element()
    }
}

impl MiniView {
    /// Open the lower panel on `tab`, or fold it away when it is already showing.
    fn toggle(&mut self, tab: Tab, window: &mut Window) {
        let same =
            self.expanded && (self.tab == tab || (tab == Tab::Next && self.tab == Tab::History));
        if same {
            self.expanded = false;
            window.resize(COMPACT);
        } else {
            self.expanded = true;
            self.tab = tab;
            keep_on_screen(
                window,
                f32::from(EXPANDED.height - window.bounds().size.height),
            );
            window.resize(EXPANDED);
        }
    }
}
