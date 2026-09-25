//! What's new: after an update, one large card shows once with what the new version brings.
//! The notes live in `whats-new.md`, one `# version` section each, newest first. Each change
//! is shown as large as it is:
//!
//! - `! **Title.** Text` a milestone, across the top in its own coloured panel;
//! - `- [icon] **Title.** Text` a feature, with an icon (any glyph name; `[icon]` may be left out);
//! - `- Text` (no bold title) a small change, in the short list at the end.
use super::{
    AppView, motion, pal,
    widgets::{display, glyph, small_button},
};
use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme,
    button::{Button, ButtonVariants},
};

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
        // Widths in pixels: wrapped text is only sized right at a known width.
        const CARD: f32 = 720.;
        const PAD: f32 = 36.;
        const GAP: f32 = 14.;
        let inner = CARD - PAD * 2.;
        let tile_w = (inner - GAP) / 2.;
        let text = |text: &str, size: f32, color: Hsla| {
            div()
                .w_full()
                .text_size(px(size))
                .line_height(relative(1.5))
                .text_color(color)
                .child(text.to_string())
        };
        // Each part fades in a little after the one before.
        let mut order = 0usize;
        let mut arrive = |el: Div, cx: &App| {
            order += 1;
            let delay = order as f32 * 70.;
            let length = 380.;
            let total = delay + length;
            motion::animate(
                el,
                ("whats-new-in", order),
                total as u64,
                cx,
                move |el, t| {
                    let local = ((t * total - delay) / length).clamp(0., 1.);
                    el.opacity(local)
                },
            )
        };

        // The banner: the first milestone as the headline, or the version and its intro.
        let (headline, lede) = match notes.milestones.first() {
            Some(m) => (m.title.clone(), m.body.clone()),
            None => (
                format!("What's new in Needle {}", notes.version),
                notes.intro.clone(),
            ),
        };
        let banner = div()
            .relative()
            .overflow_hidden()
            .px(px(PAD))
            .pt(px(PAD))
            .pb(px(30.))
            .bg(linear_gradient(
                160.,
                linear_color_stop(p.accent.opacity(0.42), 0.),
                linear_color_stop(p.accent.opacity(0.04), 1.),
            ))
            // The logo, large and faint, off the right edge.
            .child(
                glyph("logo")
                    .absolute()
                    .top(px(-40.))
                    .right(px(-50.))
                    .size(px(260.))
                    .text_color(p.accent.opacity(0.16)),
            )
            .child(
                div()
                    .relative()
                    .w(px(inner))
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(
                        div().flex().child(
                            div()
                                .px(px(10.))
                                .py(px(3.))
                                .rounded_full()
                                .bg(p.accent.opacity(0.22))
                                .text_size(px(12.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(p.accent)
                                .child(format!("Needle {}", notes.version)),
                        ),
                    )
                    .child(display(headline, 44.).w(px(inner * 0.8)).text_color(p.ink))
                    .when(!lede.is_empty(), |el| {
                        el.child(text(&lede, 15.5, p.ink_2).w(px(inner * 0.86)))
                    }),
            );

        let mut body: Vec<AnyElement> = Vec::new();
        // More milestones, if a release has several: each in its own panel.
        for m in notes.milestones.iter().skip(1) {
            body.push(arrive(
                div()
                    .w(px(inner))
                    .p_5()
                    .rounded(px(14.))
                    .border_1()
                    .border_color(p.accent.opacity(0.4))
                    .bg(p.accent.opacity(0.08))
                    .flex()
                    .gap_4()
                    .child(
                        div()
                            .flex_none()
                            .size(px(52.))
                            .rounded(px(14.))
                            .bg(p.accent)
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(
                                glyph(if m.icon.is_empty() { "logo" } else { &m.icon })
                                    .size(px(26.))
                                    .text_color(p.canvas),
                            ),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(display(m.title.clone(), 22.))
                            .child(text(&m.body, 14., p.ink_2)),
                    ),
                cx,
            ));
        }
        // Features: tiles, two to a row.
        for pair in notes.features.chunks(2) {
            let mut row = div().w(px(inner)).flex().gap(px(GAP));
            for f in pair {
                row = row.child(
                    div()
                        .w(px(tile_w))
                        .flex_none()
                        .p_5()
                        .rounded(px(14.))
                        .bg(p.ink.opacity(if p.dark { 0.05 } else { 0.035 }))
                        .border_1()
                        .border_color(p.line_soft)
                        .flex()
                        .flex_col()
                        .gap_3()
                        .child(
                            div()
                                .size(px(44.))
                                .rounded(px(12.))
                                .bg(linear_gradient(
                                    135.,
                                    linear_color_stop(p.accent.opacity(0.35), 0.),
                                    linear_color_stop(p.accent.opacity(0.12), 1.),
                                ))
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(
                                    glyph(if f.icon.is_empty() { "check" } else { &f.icon })
                                        .size(px(22.))
                                        .text_color(p.accent),
                                ),
                        )
                        .child(
                            div()
                                .text_size(px(16.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(f.title.clone()),
                        )
                        .child(text(&f.body, 13., p.ink_2)),
                );
            }
            body.push(arrive(row, cx));
        }
        // Small changes: a quiet list in one box.
        if !notes.small.is_empty() {
            body.push(arrive(
                div()
                    .w(px(inner))
                    .px_5()
                    .py_4()
                    .rounded(px(14.))
                    .bg(p.ink.opacity(if p.dark { 0.03 } else { 0.02 }))
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_size(px(13.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(p.ink_2)
                            .child("Also in this update"),
                    )
                    .children(notes.small.iter().map(|item| {
                        div()
                            .flex()
                            .items_start()
                            .gap_3()
                            .child(glyph("check").size(px(14.)).mt(px(2.)).text_color(p.accent))
                            .child(text(item, 13., p.ink_2).w(px(inner - 40. - 26.)))
                    })),
                cx,
            ));
        }

        let card = div()
            .id("whats-new")
            .occlude()
            .w(px(CARD))
            .max_h(relative(0.9))
            .rounded(px(18.))
            .bg(cx.theme().popover)
            .border_1()
            .border_color(p.line)
            .shadow_lg()
            .overflow_hidden()
            .flex()
            .flex_col()
            .child(banner)
            .child(
                div()
                    .id("whats-new-list")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .px(px(PAD))
                    .pt_5()
                    .pb_2()
                    .flex()
                    .flex_col()
                    .gap(px(GAP))
                    .children(body),
            )
            .child(
                div()
                    .px(px(PAD))
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
                    .child(
                        Button::new("whats-new-done")
                            .primary()
                            .label("Got it")
                            .px_6()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.whats_new = None;
                                cx.notify();
                            })),
                    ),
            );
        // The card rises into place.
        let card = motion::animate(card, "whats-new-card", 320, cx, |el, t| {
            el.opacity(t).mt(px(18. * (1. - t)))
        });
        Some(
            deferred(
                div()
                    .id("whats-new-backdrop")
                    .absolute()
                    .inset_0()
                    .occlude()
                    .bg(gpui::black().opacity(if p.dark { 0.6 } else { 0.35 }))
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
                    .child(
                        div()
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                            .child(card),
                    ),
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
