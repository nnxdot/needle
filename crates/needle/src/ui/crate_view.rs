//! The record crate: the albums as records in a box. The front one stands up to play; the
//! ones behind lean back, smaller and darker. The wheel, the arrow buttons, or a click on a
//! record behind flips through them.
use super::{
    AppView, pal,
    widgets::{cover, display, icon, icon_button, meta},
};
use gpui::{prelude::*, *};
use gpui_component::button::{Button, ButtonVariants};

/// Records shown behind the front one.
const BEHIND: usize = 6;

impl AppView {
    pub(super) fn crate_view(&self, width: f32, cx: &mut Context<Self>) -> AnyElement {
        let p = pal(cx);
        let count = self.groups.len();
        if count == 0 {
            return div().into_any_element();
        }
        let front = self.crate_index.min(count - 1);
        let side = (width * 0.34).clamp(220., 380.);
        let group = &self.groups[front];
        let page = group.page.clone();
        // Back to front, so the front record is drawn last.
        let behind: Vec<usize> = (front + 1..(front + 1 + BEHIND).min(count)).rev().collect();
        let stack = div()
            .relative()
            .w(px(side + 40.))
            .h(px(side + 18. * BEHIND as f32 + 30.))
            .children(behind.into_iter().map(|i| {
                let depth = (i - front) as f32;
                let size = side * (1. - 0.06 * depth);
                let g = &self.groups[i];
                div()
                    .id(("crate-behind", i))
                    .absolute()
                    .left(px(20. + (side - size) / 2.))
                    .top(px(30. + 18. * (BEHIND as f32 - depth)))
                    .rounded(px(6.))
                    .shadow_lg()
                    .cursor_pointer()
                    .child(cover(g.artwork.as_deref(), &g.seed, size, cx))
                    // Darker the further back it is.
                    .child(
                        div()
                            .absolute()
                            .inset_0()
                            .rounded(px(6.))
                            .bg(gpui::black().opacity(0.12 + 0.1 * depth)),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.crate_index = i;
                        cx.notify();
                    }))
            }))
            .child(
                div()
                    .id("crate-front")
                    .absolute()
                    .left(px(20.))
                    .top(px(18. * BEHIND as f32 + 30.))
                    .rounded(px(8.))
                    .shadow(vec![BoxShadow {
                        color: gpui::black().opacity(0.55),
                        offset: point(px(0.), px(22.)),
                        blur_radius: px(50.),
                        spread_radius: px(-6.),
                    }])
                    .cursor_pointer()
                    .child(cover(group.artwork.as_deref(), &group.seed, side, cx))
                    .on_click({
                        let page = page.clone();
                        cx.listener(move |this, _, window, cx| {
                            this.navigate(page.clone(), window, cx)
                        })
                    }),
            );
        div()
            .id("crate")
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_5()
            .pb(self.foot_room())
            // The wheel flips through the records.
            .on_scroll_wheel(cx.listener(move |this, event: &ScrollWheelEvent, _, cx| {
                let delta = match event.delta {
                    ScrollDelta::Lines(lines) => lines.y,
                    ScrollDelta::Pixels(pixels) => f32::from(pixels.y) / 60.,
                };
                this.crate_scroll += delta;
                while this.crate_scroll >= 1. {
                    this.crate_scroll -= 1.;
                    this.crate_index = this.crate_index.saturating_sub(1);
                }
                while this.crate_scroll <= -1. {
                    this.crate_scroll += 1.;
                    this.crate_index = (this.crate_index + 1).min(count - 1);
                }
                cx.notify();
            }))
            .child(stack)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap_1()
                    .child(
                        display(group.title.clone(), 26.)
                            .max_w(px(side + 120.))
                            .truncate(),
                    )
                    .child(meta(group.subtitle.clone(), cx)),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(
                        icon_button("crate-back", "chevron-left", "The record in front of it")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.crate_index = this.crate_index.saturating_sub(1);
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("crate-play")
                            .primary()
                            .icon(icon("play"))
                            .label("Play")
                            .on_click(cx.listener(move |this, _, _, _| {
                                let reason = match &page {
                                    super::Page::Album { album, .. } => {
                                        format!("From the album {album}")
                                    }
                                    _ => "From your library".into(),
                                };
                                this.play_rule(page.base(), reason);
                            })),
                    )
                    .child(
                        icon_button("crate-next", "chevron-right", "The record behind it")
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.crate_index = (this.crate_index + 1).min(count - 1);
                                cx.notify();
                            })),
                    ),
            )
            .child(div().text_size(px(11.5)).text_color(p.ink_3).child(format!(
                "{} of {}",
                front + 1,
                count
            )))
            .into_any_element()
    }
}
