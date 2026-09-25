// A check for the 1.5.0 crash: truncated text with multi-byte characters (’, セ, 虚) in a
// Songs-like row, measured more than once per layout, at several window widths. Run with
// `cargo run -p needle --example text_truncation_check`; it prints OK when every frame drew.
use gpui::{prelude::*, *};

struct Rows;
impl Render for Rows {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let names = [
            "I’m Here",
            "It’s a Song’s Title",
            "セ",
            "虚しい夜",
            "ホセ’s",
            "Don’t Stop",
            "キ’ラ",
        ];
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(white())
            .text_color(black())
            .text_size(px(13.))
            .children(names.iter().enumerate().map(|(i, name)| {
                div()
                    .id(i)
                    .h(px(46.))
                    .w_full()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(div().w(px(24.)).flex_shrink_0().child("1"))
                    .child(div().size(px(36.)).flex_shrink_0().bg(black()))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .min_w_0()
                                    .child(div().min_w_0().truncate().child(name.to_string())),
                            )
                            .child(
                                div()
                                    .min_w_0()
                                    .flex_shrink()
                                    .truncate()
                                    .text_size(px(12.))
                                    .child(format!("{name} · {name}")),
                            ),
                    )
                    .child(
                        div()
                            .w(px(180.))
                            .flex_shrink_0()
                            .truncate()
                            .child(name.to_string()),
                    )
                    .child(div().w(px(90.)).flex_shrink_0().child("M4A 768"))
                    .child(div().w(px(50.)).flex_shrink_0().child("3:12"))
            }))
    }
}

fn main() {
    Application::new().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(520.), px(420.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |_, cx| cx.new(|_| Rows),
        )
        .unwrap();
        cx.spawn(async move |cx| {
            // Draw a few frames at two widths, then quit.
            for width in [520., 380., 700., 300., 900.] {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(250))
                    .await;
                let _ = cx.update(|cx| {
                    if let Some(window) = cx.windows().first() {
                        let _ = window
                            .update(cx, |_, window, _| window.resize(size(px(width), px(420.))));
                    }
                });
            }
            cx.background_executor()
                .timer(std::time::Duration::from_millis(400))
                .await;
            println!("OK: drew every frame without crashing");
            let _ = cx.update(|cx| cx.quit());
        })
        .detach();
    });
}
