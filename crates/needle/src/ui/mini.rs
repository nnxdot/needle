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

/// The window size for `inside`: on Linux, where Needle draws its own frame, the window also
/// holds the frame's shadow around it.
fn window_size(inside: Size<Pixels>, window: Option<&Window>) -> Size<Pixels> {
    let edges = match window {
        Some(window) => gpui_component::window_paddings(window),
        // Before the window exists: the shadow gpui-component draws on Linux.
        None if cfg!(target_os = "linux") => Edges::all(px(12.)),
        None => Edges::all(px(0.)),
    };
    size(
        inside.width + edges.left + edges.right,
        inside.height + edges.top + edges.bottom,
    )
}

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
    /// When the seek bar was last moved by hand: it is not pulled back while the jump lands.
    seek_moved: Option<std::time::Instant>,
    /// The seek bar is held: silent, and where to jump when it is let go.
    seek_held: bool,
    /// Where a held seek bar was left, and for which song.
    seek_to: Option<(String, f64)>,
    /// Linux: the title strip was pressed; moving the pointer now starts a window move.
    drag_armed: bool,
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
            let bounds = Bounds::centered(None, window_size(COMPACT, None), cx);
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
                window_decorations: cfg!(target_os = "linux").then_some(WindowDecorations::Client),
                app_id: Some(super::APP_ID.into()),
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
                            this.seek_moved = Some(std::time::Instant::now());
                            let Some(app) = this.app.upgrade() else {
                                return;
                            };
                            let a = app.read(cx);
                            if let Some(item) = &a.playback.current {
                                let to = value.start() as f64 / 1000.0 * item.track.duration;
                                // Held: the jump waits for the bar to be let go.
                                if this.seek_held {
                                    this.seek_to = Some((item.track.id.clone(), to));
                                } else {
                                    a.player.send(Command::Seek(to));
                                }
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
                        drag_armed: false,
                        seek_moved: None,
                        seek_held: false,
                        seek_to: None,
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

/// Whether this system lets Needle keep a window above others: Windows, and X11 on Linux
/// (Wayland has no common way for an app to ask).
fn can_stay_on_top(window: &Window) -> bool {
    #[cfg(target_os = "linux")]
    {
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
        HasWindowHandle::window_handle(window).is_ok_and(|handle| {
            matches!(
                handle.as_raw(),
                RawWindowHandle::Xcb(_) | RawWindowHandle::Xlib(_)
            )
        })
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = window;
        cfg!(windows)
    }
}

/// Keep a window above others (Windows, and X11 on Linux).
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
    #[cfg(target_os = "linux")]
    {
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
        let id = match HasWindowHandle::window_handle(window).map(|h| h.as_raw()) {
            Ok(RawWindowHandle::Xcb(h)) => h.window.get(),
            Ok(RawWindowHandle::Xlib(h)) => h.window as u32,
            _ => return,
        };
        let _ = x11_above(id, on);
    }
    #[cfg(not(any(windows, target_os = "linux")))]
    let _ = (window, on);
}

/// Ask the X11 window manager to keep window `id` above others, the way `wmctrl -b add,above`
/// does: a _NET_WM_STATE message to the root window.
#[cfg(target_os = "linux")]
fn x11_above(id: u32, on: bool) -> anyhow::Result<()> {
    use x11rb::{
        connection::Connection,
        protocol::xproto::{ClientMessageEvent, ConnectionExt, EventMask},
    };
    let (connection, screen) = x11rb::connect(None)?;
    let root = connection.setup().roots[screen].root;
    let atom = |name: &[u8]| -> anyhow::Result<u32> {
        Ok(connection.intern_atom(false, name)?.reply()?.atom)
    };
    let (state, above) = (atom(b"_NET_WM_STATE")?, atom(b"_NET_WM_STATE_ABOVE")?);
    // 1 adds the state, 0 removes it; 1 again says a normal application asks.
    let event = ClientMessageEvent::new(32, id, state, [u32::from(on), above, 0, 1, 0]);
    connection.send_event(
        false,
        root,
        EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY,
        event,
    )?;
    connection.flush()?;
    Ok(())
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
            let ambient = a.ambient_look();
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
                .and_then(|look| {
                    let fit = look.strength(p.dark);
                    look.blur.map(|b| (b, fit))
                });
            (
                a.playback.current.clone(),
                a.playback.playing,
                a.playback.position,
                a.playback.volume,
                self.seek.clone(),
                self.volume.clone(),
                blur,
                (material, ambient),
            )
        };
        let (material, ambient) = material;
        // Follow playback, changing the sliders only when they are off, so a frame is not
        // redrawn for nothing.
        let seek_value = match &current {
            Some(item) if item.track.duration > 0.0 => {
                (position / item.track.duration * 1000.0) as f32
            }
            _ => 0.,
        };
        let dragging = self.seek_held
            || self
                .seek_moved
                .is_some_and(|at| at.elapsed() < std::time::Duration::from_millis(600));
        if !dragging && (self.seek.read(cx).value().start() - seek_value).abs() > 0.5 {
            self.seek
                .update(cx, |s, cx| s.set_value(seek_value, window, cx));
        }
        if (self.volume.read(cx).value().start() - volume).abs() > 0.001 {
            self.volume
                .update(cx, |s, cx| s.set_value(volume, window, cx));
        }
        if self.expanded
            && self.tab == Tab::Lyrics
            && app.update(cx, |a, cx| a.glide_mini_lyrics(cx))
        {
            window.request_animation_frame();
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
            // A held seek bar is let go wherever the button comes up.
            .capture_any_mouse_up(cx.listener(|this, event: &MouseUpEvent, _, cx| {
                if event.button == MouseButton::Left {
                    this.release_seek(cx)
                }
            }))
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| this.release_seek(cx)),
            )
            // Ambient has no desktop glass behind the mini player: keep a solid base.
            .bg(if ambient { p.chrome } else { p.back })
            .text_color(p.ink)
            .flex()
            .flex_col()
            // The cover, blurred, glowing behind everything.
            .when_some(blur, |el, (blur, fit)| {
                el.child(
                    img(blur)
                        .absolute()
                        .inset_0()
                        .size_full()
                        .object_fit(ObjectFit::Cover)
                        .opacity(
                            if ambient {
                                0.8
                            } else if p.dark {
                                0.55
                            } else {
                                0.4
                            } * (0.4 + 0.6 * p.back.a)
                                * fit,
                        ),
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
                    // Linux has no drag area: press, then move, to move the window (so the
                    // buttons here still take clicks).
                    .when(cfg!(target_os = "linux"), |el| {
                        el.on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, _, _, _| this.drag_armed = true),
                        )
                        .on_mouse_up(
                            MouseButton::Left,
                            cx.listener(|this, _, _, _| this.drag_armed = false),
                        )
                        .on_mouse_move(cx.listener(
                            |this, _, window, _| {
                                if this.drag_armed {
                                    this.drag_armed = false;
                                    window.start_window_move();
                                }
                            },
                        ))
                    })
                    .child(glyph("logo").size(px(14.)).text_color(p.accent))
                    .child(div().flex_1())
                    // Keeping a window on top: Windows, and X11 on Linux.
                    .when(can_stay_on_top(window), |el| {
                        el.child(
                            control(
                                "mini-pin",
                                if self.pinned { "pin-fill" } else { "pin" },
                                "Keep on top",
                            )
                            .when(self.pinned, |b| b.text_color(p.accent))
                            .on_click(cx.listener(
                                |this, _, window, cx| {
                                    this.pinned = !this.pinned;
                                    set_topmost(window, this.pinned);
                                    cx.notify();
                                },
                            )),
                        )
                    })
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
                    .child(self.seek_bar(&seek, current.is_none(), cx))
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
                    Tab::Lyrics => app.update(cx, |a, cx| {
                        a.lyrics_view(super::lyrics::LyricsKind::Mini, cx)
                            .into_any_element()
                    }),
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
    /// The seek bar: silent while held, one jump when let go (as in the main window).
    fn seek_bar(&self, seek: &Entity<SliderState>, disabled: bool, cx: &mut Context<Self>) -> Div {
        div()
            .flex_1()
            .when(!disabled, |el| {
                el.capture_any_mouse_down(cx.listener(|this, event: &MouseDownEvent, _, cx| {
                    if event.button == MouseButton::Left && !this.seek_held {
                        this.seek_held = true;
                        this.seek_to = None;
                        if let Some(app) = this.app.upgrade() {
                            app.read(cx).player.send(Command::Scrub(true));
                        }
                    }
                }))
            })
            .child(Slider::new(seek).disabled(disabled))
    }

    /// Lets go of a held seek bar: one jump, if the same song still plays, then the sound
    /// comes back.
    fn release_seek(&mut self, cx: &mut Context<Self>) {
        if std::mem::take(&mut self.seek_held) {
            if let Some(app) = self.app.upgrade() {
                let a = app.read(cx);
                if let Some((id, to)) = self.seek_to.take()
                    && a.playback
                        .current
                        .as_ref()
                        .is_some_and(|c| c.track.id == id)
                {
                    a.player.send(Command::Seek(to));
                }
                a.player.send(Command::Scrub(false));
            }
            self.seek_to = None;
            self.seek_moved = Some(std::time::Instant::now());
        }
    }

    /// Open the lower panel on `tab`, or fold it away when it is already showing.
    fn toggle(&mut self, tab: Tab, window: &mut Window) {
        let same =
            self.expanded && (self.tab == tab || (tab == Tab::Next && self.tab == Tab::History));
        if same {
            self.expanded = false;
            window.resize(window_size(COMPACT, Some(window)));
        } else {
            self.expanded = true;
            self.tab = tab;
            keep_on_screen(
                window,
                f32::from(EXPANDED.height - window.bounds().size.height),
            );
            window.resize(window_size(EXPANDED, Some(window)));
        }
    }
}
