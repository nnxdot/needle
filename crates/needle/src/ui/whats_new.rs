//! What's new: after an update, one large card shows once with what the new version brings.
//! The notes live in `whats-new.md`, one `# version` section each, newest first.
use super::{
    AppView, pal,
    widgets::{display, glyph, meta, small_button, strong},
};
use gpui::{prelude::*, *};
use gpui_component::{ActiveTheme, button::ButtonVariants};

const NOTES: &str = include_str!("../../whats-new.md");

/// One version's notes: a line to open with, then each change's title and text.
pub struct Notes {
    pub version: String,
    pub intro: String,
    pub items: Vec<(String, String)>,
}

/// Every version in `text`, newest first.
fn parse(text: &str) -> Vec<Notes> {
    let mut all: Vec<Notes> = Vec::new();
    for line in text.lines().map(str::trim) {
        if let Some(version) = line.strip_prefix("# ") {
            all.push(Notes {
                version: version.trim().to_string(),
                intro: String::new(),
                items: Vec::new(),
            });
        } else if let Some(notes) = all.last_mut() {
            if let Some(item) = line.strip_prefix("- ") {
                // "**Title.** Text": the bold part is the title.
                let (title, body) = item
                    .strip_prefix("**")
                    .and_then(|rest| rest.split_once("**"))
                    .map(|(title, body)| (title.trim().to_string(), body.trim().to_string()))
                    .unwrap_or_else(|| (String::new(), item.to_string()));
                notes.items.push((title, body));
            } else if !line.is_empty() {
                if !notes.intro.is_empty() {
                    notes.intro.push(' ');
                }
                notes.intro.push_str(line);
            }
        }
    }
    all
}

/// The notes for `version`, or the newest when `version` is `None`.
pub fn notes(version: Option<&str>) -> Option<Notes> {
    let all = parse(NOTES);
    match version {
        Some(v) => all.into_iter().find(|n| n.version == v),
        None => all.into_iter().next(),
    }
}

impl AppView {
    /// After an update, show this version's notes once. A new install gets the welcome guide
    /// instead, so it only remembers the version.
    pub(super) fn maybe_whats_new(&mut self) {
        let version = env!("CARGO_PKG_VERSION");
        if self.settings.whats_new_seen == version {
            return;
        }
        let updated = self.settings.welcomed;
        if updated && let Some(notes) = notes(Some(version)) {
            self.whats_new = Some(notes);
        }
        self.settings.whats_new_seen = version.to_string();
        self.persist_settings();
    }

    /// The card, over everything else, while it is open.
    pub(super) fn whats_new_view(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        let notes = self.whats_new.as_ref()?;
        let p = pal(cx);
        let items = notes.items.iter().enumerate().map(|(i, (title, body))| {
            div()
                .flex()
                .items_start()
                .gap_4()
                .child(
                    div()
                        .flex_none()
                        .size(px(34.))
                        .rounded_full()
                        .bg(p.accent_soft)
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_size(px(14.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(p.accent)
                        .child((i + 1).to_string()),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .when(!title.is_empty(), |el| {
                            el.child(
                                strong(title.trim_end_matches('.').to_string()).text_size(px(15.)),
                            )
                        })
                        .child(
                            meta(body.clone(), cx)
                                .w_full()
                                .text_size(px(13.5))
                                .line_height(relative(1.5)),
                        ),
                )
        });
        let card = div()
            .id("whats-new")
            .occlude()
            .w(px(620.))
            .max_w_full()
            .max_h(relative(0.86))
            .rounded(px(16.))
            .bg(cx.theme().popover)
            .border_1()
            .border_color(p.line)
            .shadow_lg()
            .overflow_hidden()
            .flex()
            .flex_col()
            .child(
                // A band of the accent colour across the top, with the logo.
                div()
                    .px_8()
                    .pt_8()
                    .pb_6()
                    .bg(linear_gradient(
                        180.,
                        linear_color_stop(p.accent.opacity(0.22), 0.),
                        linear_color_stop(p.accent.opacity(0.), 1.),
                    ))
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(glyph("logo").size(px(36.)).text_color(p.accent))
                    .child(display(
                        format!("What's new in Needle {}", notes.version),
                        30.,
                    ))
                    .when(!notes.intro.is_empty(), |el| {
                        el.child(
                            div()
                                .w_full()
                                .text_size(px(15.))
                                .text_color(p.ink_2)
                                .line_height(relative(1.5))
                                .child(notes.intro.clone()),
                        )
                    }),
            )
            .child(
                div()
                    .id("whats-new-list")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .px_8()
                    .pb_2()
                    .flex()
                    .flex_col()
                    .gap_5()
                    .children(items),
            )
            .child(
                div()
                    .px_8()
                    .py_5()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        small_button("whats-new-notes", "Full release notes")
                            .ghost()
                            .on_click({
                                let version = notes.version.clone();
                                move |_, _, cx| {
                                    cx.open_url(&format!(
                                        "https://github.com/nnxdot/needle/releases/tag/v{version}"
                                    ))
                                }
                            }),
                    )
                    .child(div().flex_1())
                    .child(small_button("whats-new-done", "Got it").primary().on_click(
                        cx.listener(|this, _, _, cx| {
                            this.whats_new = None;
                            cx.notify();
                        }),
                    )),
            );
        Some(
            deferred(
                div()
                    .id("whats-new-backdrop")
                    .absolute()
                    .inset_0()
                    .occlude()
                    .bg(gpui::black().opacity(if p.dark { 0.55 } else { 0.3 }))
                    .flex()
                    .justify_center()
                    .items_center()
                    .p_6()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.whats_new = None;
                            cx.notify();
                        }),
                    )
                    .child(card.on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())),
            )
            .with_priority(3),
        )
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn reads_versions_intros_and_items() {
        let all = super::parse(
            "# 2.0.0\nBig news.\n- **One thing.** It does this.\n- Plain line\n\n# 1.9.0\n- **Old.** Before.\n",
        );
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].version, "2.0.0");
        assert_eq!(all[0].intro, "Big news.");
        assert_eq!(
            all[0].items[0],
            ("One thing.".into(), "It does this.".into())
        );
        assert_eq!(all[0].items[1], (String::new(), "Plain line".into()));
        assert_eq!(all[1].items.len(), 1);
    }

    #[test]
    fn the_shipped_notes_read() {
        let newest = super::notes(None).expect("whats-new.md has a version");
        assert!(!newest.items.is_empty());
        assert!(
            newest
                .items
                .iter()
                .all(|(title, body)| !title.is_empty() && !body.is_empty())
        );
    }
}
