//! Settings › Playback › Phone remote: turn it on, and show its address as a QR code.
use super::{
    AppView, pal,
    widgets::{faint, meta, setting_row, small_button, strong},
};
use gpui::{prelude::*, *};
use gpui_component::{button::ButtonVariants, switch::Switch};
use needle_core::remote;

impl AppView {
    /// Start or stop the remote to match the settings.
    pub(super) fn apply_remote(&mut self) {
        if !self.settings.remote {
            self.remote = None;
            return;
        }
        if self.settings.remote_key.len() < 16 {
            self.settings.remote_key = remote::new_key();
            self.persist_settings();
        }
        if self.remote.is_some() {
            return;
        }
        match remote::start(
            self.player.clone(),
            self.library.clone(),
            self.settings.remote_key.clone(),
        ) {
            Ok(server) => self.remote = Some(server),
            Err(error) => {
                needle_core::logfile::error(format!("Phone remote: {error:#}"));
                self.settings.remote = false;
                self.fail(format!("The phone remote could not start: {error:#}"));
            }
        }
    }

    pub(super) fn remote_settings(&self, cx: &mut Context<Self>) -> Div {
        let p = pal(cx);
        let address = self.remote.as_ref().map(|r| r.address());
        div()
            .mt_4()
            .flex()
            .flex_col()
            .gap_2()
            .child(setting_row(
                "Phone remote",
                "Control Needle from a phone's web browser on the same Wi-Fi. Nothing to install on the phone.",
                Switch::new("remote-on").checked(self.settings.remote).on_click(cx.listener(|this, checked: &bool, _, cx| {
                    this.settings.remote = *checked;
                    this.persist_settings();
                    this.apply_remote();
                    cx.notify();
                })),
                cx,
            ))
            .when_some(address, |el, address| {
                el.child(
                    div()
                        .flex()
                        .gap_5()
                        .items_start()
                        .child(qr(&address, cx))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .flex_col()
                                .gap_2()
                                .child(strong("Point the phone's camera at the code"))
                                .child(meta("Or type this address in the phone's browser. Add it to the home screen to open it like an app.", cx))
                                .child(
                                    div()
                                        .px_3()
                                        .py_2()
                                        .rounded(px(6.))
                                        .bg(p.raised)
                                        .text_size(px(13.))
                                        .child(address.clone()),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .gap_2()
                                        .child(small_button("remote-copy", "Copy address").on_click({
                                            let address = address.clone();
                                            cx.listener(move |this, _, _, cx| {
                                                cx.write_to_clipboard(ClipboardItem::new_string(address.clone()));
                                                this.notify("Address copied.");
                                            })
                                        }))
                                        .child(small_button("remote-new", "New address").ghost().on_click(cx.listener(|this, _, _, cx| {
                                            // A new key: phones with the old address can no longer control Needle.
                                            this.settings.remote_key = remote::new_key();
                                            this.persist_settings();
                                            this.remote = None;
                                            this.apply_remote();
                                            this.notify("The remote has a new address. Phones with the old one no longer work.");
                                            cx.notify();
                                        }))),
                                )
                                .child(faint("Anyone who has the address and is on your Wi-Fi can control Needle. Choose New address to lock them out. If Windows asks, allow Needle on private networks.", cx)),
                        ),
                )
            })
    }
}

/// A QR code for `text`, drawn from squares.
fn qr(text: &str, cx: &App) -> Div {
    let _ = pal(cx);
    let Ok(code) = qrcode::QrCode::new(text.as_bytes()) else {
        return div();
    };
    let width = code.width();
    let colors = code.to_colors();
    let cell = px(4.);
    div()
        .p(px(16.))
        .bg(rgb(0xffffff))
        .rounded(px(8.))
        .flex_none()
        .flex()
        .flex_col()
        .children(colors.chunks(width).map(|row| {
            div().flex().children(row.iter().map(|c| {
                div()
                    .size(cell)
                    .flex_none()
                    .when(*c == qrcode::Color::Dark, |el| el.bg(rgb(0x000000)))
            }))
        }))
}
