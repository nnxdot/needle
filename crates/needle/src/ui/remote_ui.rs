//! Settings › Playback › Phone remote: turn it on, and show its address as a QR code. And AI
//! assistants (needle_core::mcp): turn it on, and show the address to give them.
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
                self.persist_settings();
                self.fail(format!("The phone remote could not start: {error:#}"));
            }
        }
    }

    /// Start or stop Needle for AI assistants to match the settings.
    pub(super) fn apply_mcp(&mut self) {
        if !self.settings.mcp {
            self.mcp = None;
            return;
        }
        if self.settings.mcp_key.len() < 16 {
            self.settings.mcp_key = remote::new_key();
            self.persist_settings();
        }
        if self.mcp.is_some() {
            return;
        }
        // An assistant made a playlist or favorited a song: the pages show it.
        let sender = std::sync::Mutex::new(self.sender.clone());
        let changed: needle_core::mcp::Changed = std::sync::Arc::new(move || {
            if let Ok(sender) = sender.lock() {
                let _ = sender.send(super::Event::Notice(String::new()));
            }
        });
        match needle_core::mcp::start(
            self.player.clone(),
            self.library.clone(),
            self.settings.mcp_key.clone(),
            changed,
        ) {
            Ok(server) => self.mcp = Some(server),
            Err(error) => {
                needle_core::logfile::error(format!("AI assistants: {error:#}"));
                self.settings.mcp = false;
                self.persist_settings();
                self.fail(format!(
                    "Needle for AI assistants could not start: {error:#}"
                ));
            }
        }
    }

    /// Settings › Playback › AI assistants: the switch, and with it on, the address.
    fn mcp_settings(&self, cx: &mut Context<Self>) -> Div {
        let p = pal(cx);
        let address = self.mcp.as_ref().map(|m| m.address());
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(setting_row(
                "AI assistants",
                "Let an AI app on this computer, such as Claude, play music, search your library, and make playlists (MCP).",
                Switch::new("mcp-on").checked(self.settings.mcp).on_click(cx.listener(|this, checked: &bool, _, cx| {
                    this.settings.mcp = *checked;
                    this.persist_settings();
                    this.apply_mcp();
                    cx.notify();
                })),
                cx,
            ))
            .when_some(address, |el, address| {
                let command = format!("claude mcp add --transport http needle {address}");
                el.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(meta("Give your AI app this address (an MCP server, Streamable HTTP). For Claude Code, run the command below.", cx))
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
                                .child(small_button("mcp-copy", "Copy address").on_click({
                                    let address = address.clone();
                                    cx.listener(move |this, _, _, cx| {
                                        cx.write_to_clipboard(ClipboardItem::new_string(address.clone()));
                                        this.notify("Address copied.");
                                    })
                                }))
                                .child(small_button("mcp-claude", "Copy for Claude Code").on_click(cx.listener(move |this, _, _, cx| {
                                    cx.write_to_clipboard(ClipboardItem::new_string(command.clone()));
                                    this.notify("Command copied. Run it in a terminal.");
                                })))
                                .child(small_button("mcp-new", "New address").ghost().on_click(cx.listener(|this, _, _, cx| {
                                    // A new key: apps with the old address can no longer use Needle.
                                    this.settings.mcp_key = remote::new_key();
                                    this.persist_settings();
                                    this.mcp = None;
                                    this.apply_mcp();
                                    this.notify("New address. Apps with the old one no longer work.");
                                    cx.notify();
                                }))),
                        )
                        .child(faint("Only apps on this computer can reach it, and only with this address. Needle must be open for them to use it.", cx)),
                )
            })
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
                if cfg!(target_os = "macos") { "Keep in the menu bar" } else { "Hide to tray" },
                if cfg!(target_os = "macos") {
                    "Closing the window keeps the music playing, from the menu bar."
                } else {
                    "Closing the window keeps the music playing, from the notification area."
                },
                Switch::new("tray-on").checked(self.settings.tray).on_click(cx.listener(|this, checked: &bool, _, cx| {
                    this.settings.tray = *checked;
                    this.persist_settings();
                    this.apply_tray();
                    cx.notify();
                })),
                cx,
            ))
            .child(setting_row(
                "Phone remote",
                "Control Needle from a phone's browser on the same Wi-Fi.",
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
                                .child(faint("Anyone who has the address and can reach this computer on a private network (your Wi-Fi, or a VPN) can control Needle. Choose New address to lock them out. If Windows or a firewall asks, allow Needle on private networks.", cx)),
                        ),
                )
            })
            .child(self.mcp_settings(cx))
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
