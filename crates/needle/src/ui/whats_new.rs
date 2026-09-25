//! What's new: after an update, one large card shows once with what the new version brings.
//! The notes live in `whats-new.md`, one `# version` section each, newest first. Each change
//! is shown as large as it is:
//!
//! - `! **Title.** Text` a milestone, across the top in its own coloured panel;
//! - `- [icon] **Title.** Text` a feature, with an icon (any glyph name; `[icon]` may be left out);
//! - `- Text` (no bold title) a small change, in the short list at the end.
use super::{
    AppView, pal,
    widgets::{display, glyph, meta, small_button, strong},
};
use gpui::{prelude::*, *};
use gpui_component::{ActiveTheme, button::ButtonVariants};

const NOTES: &str = include_str!("../../whats-new.md");

/// A change with a title: a milestone or a feature.
#[derive(Debug, PartialEq)]
pub struct Change {
    pub icon: String,
    pub title: String,
    pub body: String,
}

/// One version's notes.
#[derive(Debug, Default)]
pub struct Notes {
    pub version: String,
    pub intro: String,
    pub milestones: Vec<Change>,
    pub features: Vec<Change>,
    pub small: Vec<String>,
}

/// "[icon] **Title.** Text" (the icon may be left out) into its parts; `None` without a title.
fn change(text: &str) -> Option<Change> {
    let (icon, rest) = match text.strip_prefix('[').and_then(|r| r.split_once(']')) {
        Some((icon, rest)) => (icon.trim().to_string(), rest.trim_start()),
        None => (String::new(), text),
    };
    let (title, body) = rest.strip_prefix("**")?.split_once("**")?;
    Some(Change {
        icon,
        title: title.trim().trim_end_matches('.').to_string(),
        body: body.trim().to_string(),
    })
}

/// Every version in `text`, newest first.
fn parse(text: &str) -> Vec<Notes> {
    let mut all: Vec<Notes> = Vec::new();
    for line in text.lines().map(str::trim) {
        if let Some(version) = line.strip_prefix("# ") {
            all.push(Notes {
                version: version.trim().to_string(),
                ..Default::default()
            });
            continue;
        }
        let Some(notes) = all.last_mut() else {
            continue;
        };
        if let Some(item) = line.strip_prefix("! ") {
            match change(item) {
                Some(c) => notes.milestones.push(c),
                None => notes.small.push(item.to_string()),
            }
        } else if let Some(item) = line.strip_prefix("- ") {
            match change(item) {
                Some(c) => notes.features.push(c),
                None => notes.small.push(item.to_string()),
            }
        } else if !line.is_empty() {
            if !notes.intro.is_empty() {
                notes.intro.push(' ');
            }
            notes.intro.push_str(line);
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
        let body = |text: &str, size: f32, cx: &App| {
            meta(text.to_string(), cx)
                .w_full()
                .text_size(px(size))
                .line_height(relative(1.5))
        };
        // Milestones: each in its own panel of the accent colour, with a large title.
        let milestones = notes.milestones.iter().map(|m| {
            div()
                .w_full()
                .p_6()
                .rounded(px(14.))
                .border_1()
                .border_color(p.accent.opacity(0.45))
                .bg(linear_gradient(
                    135.,
                    linear_color_stop(p.accent.opacity(0.26), 0.),
                    linear_color_stop(p.accent.opacity(0.06), 1.),
                ))
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_3()
                        .child(
                            glyph(if m.icon.is_empty() { "logo" } else { &m.icon })
                                .size(px(30.))
                                .text_color(p.accent),
                        )
                        .child(display(m.title.clone(), 26.).text_color(p.ink)),
                )
                .child(body(&m.body, 14.5, cx).text_color(p.ink_2))
        });
        let features = notes.features.iter().map(|f| {
            div()
                .flex()
                .items_start()
                .gap_4()
                .child(
                    div()
                        .flex_none()
                        .size(px(38.))
                        .rounded(px(10.))
                        .bg(p.accent_soft)
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(
                            glyph(if f.icon.is_empty() { "check" } else { &f.icon })
                                .size(px(19.))
                                .text_color(p.accent),
                        ),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(strong(f.title.clone()).text_size(px(15.)))
                        .child(body(&f.body, 13.5, cx)),
                )
        });
        let small = (!notes.small.is_empty()).then(|| {
            div()
                .flex()
                .flex_col()
                .gap_2()
                .pt_1()
                .child(
                    div()
                        .text_size(px(12.5))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(p.ink_3)
                        .child("Also in this update"),
                )
                .children(notes.small.iter().map(|text| {
                    div()
                        .flex()
                        .items_start()
                        .gap_2()
                        .child(
                            div()
                                .flex_none()
                                .mt(px(7.))
                                .size(px(4.))
                                .rounded_full()
                                .bg(p.ink_3),
                        )
                        .child(body(text, 12.5, cx).flex_1().min_w_0())
                }))
        });
        let card = div()
            .id("whats-new")
            .occlude()
            .w(px(640.))
            .max_w_full()
            .max_h(relative(0.88))
            .rounded(px(16.))
            .bg(cx.theme().popover)
            .border_1()
            .border_color(p.line)
            .shadow_lg()
            .overflow_hidden()
            .flex()
            .flex_col()
            .child(
                div()
                    .px_8()
                    .pt_8()
                    .pb_5()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(display(
                        format!("What's new in Needle {}", notes.version),
                        30.,
                    ))
                    .when(!notes.intro.is_empty(), |el| {
                        el.child(body(&notes.intro, 15., cx))
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
                    .children(milestones)
                    .children(features)
                    .children(small),
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
    fn reads_each_size_of_change() {
        let all = super::parse(
            "# 2.0.0\nBig news.\n! **A milestone.** It is big.\n- [lyrics] **A feature.** It does this.\n- **No icon.** Still a feature.\n- A small fix.\n\n# 1.9.0\n- **Old.** Before.\n",
        );
        assert_eq!(all.len(), 2);
        let new = &all[0];
        assert_eq!(new.version, "2.0.0");
        assert_eq!(new.intro, "Big news.");
        assert_eq!(new.milestones[0].title, "A milestone");
        assert_eq!(new.milestones[0].body, "It is big.");
        assert_eq!(new.features[0].icon, "lyrics");
        assert_eq!(new.features[0].title, "A feature");
        assert_eq!(new.features[1].icon, "");
        assert_eq!(new.small, vec!["A small fix.".to_string()]);
        assert_eq!(all[1].features.len(), 1);
    }

    #[test]
    fn the_shipped_notes_read() {
        let newest = super::notes(None).expect("whats-new.md has a version");
        assert!(!newest.milestones.is_empty() || !newest.features.is_empty());
        for change in newest.milestones.iter().chain(&newest.features) {
            assert!(!change.title.is_empty() && !change.body.is_empty());
        }
    }
}
