//! The small floating player: a card that stays above other windows, with the cover, the song,
//! and the artist; the buttons show under the pointer, a thin line along its foot shows where
//! the song is. Drag it anywhere; double-click opens Needle. Staying above others works on
//! Windows and on Linux with X11; Wayland lets no app ask for it.
use super::{
    AppView, pal,
    widgets::{artwork, glyph},
};
use gpui::{prelude::*, *};
use gpui_component::Root;
use needle_core::audio::Command;

const SIZE: Size<Pixels> = Size {
    width: px(330.),
    height: px(96.),
};

pub struct FloatView {
    app: WeakEntity<AppView>,
    main: AnyWindowHandle,
    hovered: bool,
    /// Pressed and not yet moved: moving now moves the window; letting go is a click.
    pressed: Option<Point<Pixels>>,
    _observe: Subscription,
}

impl AppView {
    /// Open the small floating player, or close it when it is open.
    pub(super) fn toggle_float(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(handle) = self.float.take() {
            let _ = handle.update(cx, |_, window, _| window.remove_window());
            return;
        }
        let app = cx.entity();
        let main = window.window_handle();
        // Open after this update finishes: the new window's first frame reads this view.
        cx.defer(move |cx| {
            let bounds = bottom_right(cx);
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("Needle".into()),
                    appears_transparent: true,
                    ..Default::default()
                }),
                kind: WindowKind::PopUp,
                is_resizable: false,
                is_minimizable: false,
                focus: false,
                window_decorations: cfg!(target_os = "linux").then_some(WindowDecorations::Client),
                app_id: Some(super::APP_ID.into()),
                ..Default::default()
            };
            let weak = app.downgrade();
            let opened = cx.open_window(options, |window, cx| {
                window.set_window_title("Needle");
                let view = cx.new(|cx| FloatView {
                    app: weak.clone(),
                    main,
                    hovered: false,
                    pressed: None,
                    _observe: cx.observe(&app, |_, _, cx| cx.notify()),
                });
                cx.new(|cx| Root::new(view, window, cx))
            });
            match opened {
                Ok(handle) => {
                    let _ = handle.update(cx, |_, window, _| {
                        round_corners(window);
                        super::mini::set_topmost(window, true);
                    });
                    app.update(cx, |this, _| this.float = Some(handle.into()));
                }
                Err(e) => app.update(cx, |this, cx| {
                    this.fail(format!("The small player could not open: {e:#}"));
                    cx.notify();
                }),
            }
        });
    }

    /// Favorite the song playing, or take it back out of favorites.
    pub(super) fn toggle_favorite_playing(&mut self) {
        let Some(track) = self.playback.current.as_ref().map(|i| i.track.clone()) else {
            return;
        };
        let favorite = self
            .tracks
            .iter()
            .find(|t| t.id == track.id)
            .map_or(track.rating, |t| t.rating)
            >= 4;
        let rating = if favorite { 0 } else { 5 };
        self.set_rating(std::slice::from_ref(&track.id), rating);
        if let Some(item) = self.playback.current.as_mut() {
            item.track.rating = rating;
        }
    }
}

/// The lower right of the screen, above the taskbar.
fn bottom_right(cx: &App) -> Bounds<Pixels> {
    let screen = cx
        .primary_display()
        .map(|d| d.bounds())
        .unwrap_or(Bounds::new(
            point(px(0.), px(0.)),
            size(px(1280.), px(800.)),
        ));
    let margin = px(24.);
    Bounds::new(
        point(
            screen.origin.x + screen.size.width - SIZE.width - margin,
            screen.origin.y + screen.size.height - SIZE.height - px(72.),
        ),
        SIZE,
    )
}

/// Windows 11 rounds a pop-up window's corners only when asked.
fn round_corners(window: &Window) {
    #[cfg(windows)]
    {
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
        use windows_sys::Win32::Graphics::Dwm::{
            DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND, DwmSetWindowAttribute,
        };
        if let Ok(handle) = HasWindowHandle::window_handle(window)
            && let RawWindowHandle::Win32(win32) = handle.as_raw()
        {
            let round = DWMWCP_ROUND;
            unsafe {
                DwmSetWindowAttribute(
                    win32.hwnd.get() as _,
                    DWMWA_WINDOW_CORNER_PREFERENCE as u32,
                    &round as *const _ as *const _,
                    std::mem::size_of_val(&round) as u32,
                );
            }
        }
    }
    #[cfg(not(windows))]
    let _ = window;
}

/// Move the window with the pointer, as dragging a title bar does.
fn move_window(window: &Window) {
    #[cfg(windows)]
    {
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
        use windows_sys::Win32::UI::{
            Input::KeyboardAndMouse::ReleaseCapture,
            WindowsAndMessaging::{HTCAPTION, SendMessageW, WM_NCLBUTTONDOWN},
        };
        if let Ok(handle) = HasWindowHandle::window_handle(window)
            && let RawWindowHandle::Win32(win32) = handle.as_raw()
        {
            unsafe {
                ReleaseCapture();
                SendMessageW(
                    win32.hwnd.get() as _,
                    WM_NCLBUTTONDOWN,
                    HTCAPTION as usize,
                    0,
                );
            }
        }
    }
    #[cfg(not(windows))]
    window.start_window_move();
}

impl FloatView {
    fn send(&self, command: Command, cx: &mut Context<Self>) {
        if let Some(app) = self.app.upgrade() {
            app.read(cx).player.send(command);
        }
    }
    /// Double-click: Needle's window comes back, and the card goes.
    fn open_needle(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let _ = self.main.update(cx, |_, main, _| main.activate_window());
        if let Some(app) = self.app.upgrade() {
            app.update(cx, |this, _| this.float = None);
        }
        window.remove_window();
    }
}

impl Render for FloatView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = pal(cx);
        let Some(app) = self.app.upgrade() else {
            window.remove_window();
            return div().into_any_element();
        };
        let (current, playing, position, favorite) = {
            let a = app.read(cx);
            let current = a.playback.current.clone();
            let favorite = current.as_ref().is_some_and(|c| {
                a.tracks
                    .iter()
                    .find(|t| t.id == c.track.id)
                    .map_or(c.track.rating, |t| t.rating)
                    >= 4
            });
            (current, a.playback.playing, a.playback.position, favorite)
        };
        let track = current.as_ref().map(|c| &c.track);
        let progress = track
            .filter(|t| t.duration > 0.)
            .map_or(0., |t| (position / t.duration).clamp(0., 1.)) as f32;
        let small = |id: &'static str, name: &'static str, command: fn() -> Command| {
            div()
                .id(id)
                .size(px(26.))
                .rounded_full()
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .hover(|s| s.bg(p.raised_hover))
                .child(glyph(name).size(px(15.)).text_color(p.ink))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.send(command(), cx);
                }))
        };
        let buttons = div()
            .flex()
            .items_center()
            .gap(px(12.))
            .mt(px(6.))
            .child(small("float-previous", "previous", || Command::Previous))
            .child(
                div()
                    .id("float-play")
                    .size(px(28.))
                    .rounded_full()
                    .bg(p.accent)
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .child(
                        glyph(if playing { "pause" } else { "play" })
                            .size(px(14.))
                            .text_color(p.accent_ink),
                    )
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(cx.listener(|this, _, _, cx| {
                        if let Some(app) = this.app.upgrade() {
                            app.update(cx, |a, cx| a.toggle_playback(cx));
                        }
                    })),
            )
            .child(small("float-next", "next", || Command::Next))
            .child(div().flex_1())
            .child(
                div()
                    .id("float-favorite")
                    .size(px(26.))
                    .rounded_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .hover(|s| s.bg(p.raised_hover))
                    .child(
                        glyph(if favorite { "heart-fill" } else { "heart" })
                            .size(px(15.))
                            .text_color(if favorite { p.accent } else { p.ink_2 }),
                    )
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(cx.listener(|this, _, _, cx| {
                        if let Some(app) = this.app.upgrade() {
                            app.update(cx, |a, cx| {
                                a.toggle_favorite_playing();
                                cx.notify();
                            });
                        }
                    })),
            );
        let card = div()
            .id("float")
            .size_full()
            .relative()
            .overflow_hidden()
            .rounded(px(14.))
            .bg(p.raised)
            .border_1()
            .border_color(if p.dark {
                hsla(0., 0., 1., 0.1)
            } else {
                hsla(0., 0., 0., 0.1)
            })
            .text_color(p.ink)
            .flex()
            .items_center()
            .gap(px(12.))
            .p(px(12.))
            .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                this.hovered = *hovered;
                cx.notify();
            }))
            // Press and move to drag it anywhere; a click (or two) stays a click.
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &MouseDownEvent, _, _| {
                    this.pressed = Some(event.position);
                }),
            )
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, event: &MouseUpEvent, window, cx| {
                    this.pressed = None;
                    if event.click_count >= 2 {
                        this.open_needle(window, cx);
                    }
                }),
            )
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, window, _| {
                if let Some(start) = this.pressed
                    && event.pressed_button == Some(MouseButton::Left)
                {
                    let moved = event.position - start;
                    if moved.x.abs() + moved.y.abs() > px(3.) {
                        this.pressed = None;
                        move_window(window);
                    }
                }
            }))
            .child(
                div()
                    .size(px(72.))
                    .flex_shrink_0()
                    .rounded(px(10.))
                    .overflow_hidden()
                    .child(artwork(track, 72., cx)),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .text_size(px(14.))
                            .font_weight(FontWeight::BOLD)
                            .truncate()
                            .child(
                                track.map_or("Nothing playing".to_string(), |t| t.title.clone()),
                            ),
                    )
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(p.ink_2)
                            .truncate()
                            .child(track.map_or(String::new(), |t| t.display_artist().to_string())),
                    )
                    .when(self.hovered && track.is_some(), |el| el.child(buttons)),
            )
            .when(playing, |el| {
                el.child(
                    div()
                        .absolute()
                        .top(px(8.))
                        .right(px(12.))
                        .size(px(7.))
                        .rounded_full()
                        .bg(p.accent),
                )
            })
            .child(
                div()
                    .absolute()
                    .left(px(12.))
                    .right(px(12.))
                    .bottom(px(6.))
                    .h(px(2.))
                    .rounded_full()
                    .bg(p.line)
                    .child(
                        div()
                            .h_full()
                            .w(relative(progress))
                            .rounded_full()
                            .bg(p.accent),
                    ),
            );
        div()
            .size_full()
            .font_family(super::theme::text_font())
            .child(card)
            .into_any_element()
    }
}
