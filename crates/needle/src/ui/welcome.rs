//! The welcome guide, shown the first time Needle opens: music folders, a look, privacy
//! choices, and a few tips. It can be opened again from Settings › Your data.
use super::{
    AppView, pal, set_theme,
    theme::Base,
    widgets::{faint, heading, meta, small_button, strong},
};
use gpui::{prelude::*, *};
use gpui_component::{ActiveTheme, button::ButtonVariants, switch::Switch};

const STEPS: usize = 4;

impl AppView {
    /// On first start, open the guide; people who already have music just get it marked seen.
    pub(super) fn maybe_welcome(&mut self) {
        if self.settings.welcomed {
            return;
        }
        let has_music = self.library.count().unwrap_or(0) > 0
            || !self.library.roots().unwrap_or_default().is_empty();
        if has_music {
            self.settings.welcomed = true;
            self.persist_settings();
        } else {
            self.welcome_step = Some(0);
        }
    }

    fn finish_welcome(&mut self, cx: &mut Context<Self>) {
        self.welcome_step = None;
        self.settings.welcomed = true;
        self.persist_settings();
        cx.notify();
    }

    pub(super) fn welcome_view(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<impl IntoElement> {
        let step = self.welcome_step?;
        let _ = window;
        let p = pal(cx);
        let body: AnyElement = match step {
            0 => self.welcome_music(cx).into_any_element(),
            1 => self.welcome_look(cx).into_any_element(),
            2 => self.welcome_privacy(cx).into_any_element(),
            _ => self.welcome_tips(cx).into_any_element(),
        };
        let dots = div().flex().gap(px(6.)).children((0..STEPS).map(|i| {
            div()
                .size(px(7.))
                .rounded_full()
                .bg(if i == step { p.accent } else { p.line })
        }));
        let card = div()
            .id("welcome")
            .occlude()
            .w(px(560.))
            .max_w_full()
            .p_8()
            .rounded(px(14.))
            .bg(cx.theme().popover)
            .border_1()
            .border_color(p.line)
            .shadow_lg()
            .flex()
            .flex_col()
            .gap_5()
            .child(body)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(dots)
                    .child(div().flex_1())
                    .when(step > 0, |el| {
                        el.child(small_button("welcome-back", "Back").ghost().on_click(
                            cx.listener(|this, _, _, cx| {
                                this.welcome_step = this.welcome_step.map(|s| s.saturating_sub(1));
                                cx.notify();
                            }),
                        ))
                    })
                    .when(step + 1 < STEPS, |el| {
                        el.child(
                            small_button("welcome-skip", "Skip the guide")
                                .ghost()
                                .on_click(cx.listener(|this, _, _, cx| this.finish_welcome(cx))),
                        )
                        .child(
                            small_button("welcome-next", "Next").on_click(cx.listener(
                                |this, _, _, cx| {
                                    this.welcome_step =
                                        this.welcome_step.map(|s| (s + 1).min(STEPS - 1));
                                    cx.notify();
                                },
                            )),
                        )
                    })
                    .when(step + 1 == STEPS, |el| {
                        el.child(
                            small_button("welcome-done", "Start listening")
                                .on_click(cx.listener(|this, _, _, cx| this.finish_welcome(cx))),
                        )
                    }),
            );
        Some(
            deferred(
                div()
                    .id("welcome-backdrop")
                    .absolute()
                    .inset_0()
                    .occlude()
                    .bg(gpui::black().opacity(if p.dark { 0.55 } else { 0.3 }))
                    .flex()
                    .justify_center()
                    .items_center()
                    .p_6()
                    .child(card),
            )
            .with_priority(3),
        )
    }

    fn welcome_music(&self, cx: &mut Context<Self>) -> Div {
        let songs = self.library.count().unwrap_or(0);
        let roots = self.library.roots().unwrap_or_default();
        let scanning = self.scan.as_ref().filter(|s| !s.done);
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(div().text_size(px(26.)).font_weight(FontWeight::SEMIBOLD).child("Welcome to Needle"))
            .child(meta("Needle plays the music files you own. First, show it where they are. It watches the folder, so new music shows up by itself. Your files are never moved or changed unless you ask.", cx))
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(small_button("welcome-folder", if roots.is_empty() { "Choose your music folder…" } else { "Add another folder…" }).on_click(cx.listener(|this, _, window, cx| this.import_folder(window, cx))))
                    .when(songs == 0 && scanning.is_none(), |el| {
                        el.child(small_button("welcome-demo", "Try three demo songs").ghost().on_click(cx.listener(|this, _, _, cx| this.demo(cx))))
                    }),
            )
            .when_some(scanning, |el, scan| {
                el.child(faint(format!("Reading {} songs… {}", scan.scanned, scan.current), cx))
            })
            .when(!roots.is_empty() || songs > 0, |el| {
                el.child(strong(format!(
                    "{songs} songs in {} folder{}.",
                    roots.len(),
                    if roots.len() == 1 { "" } else { "s" }
                )))
            })
    }

    fn welcome_look(&self, cx: &mut Context<Self>) -> Div {
        let p = pal(cx);
        let current = self.settings.theme.clone();
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(heading("Choose a look"))
            .child(meta(
                "You can change it, and much more, in Settings › Appearance.",
                cx,
            ))
            .child(
                div()
                    .flex()
                    .gap_2()
                    .children(Base::ALL.iter().map(|(mode, name, about)| {
                        let active = current == *mode;
                        div()
                            .id(SharedString::from(format!("welcome-theme-{mode}")))
                            .flex_1()
                            .p_3()
                            .rounded(px(10.))
                            .border_1()
                            .cursor_pointer()
                            .border_color(if active { p.accent } else { p.line })
                            .when(active, |el| el.bg(p.accent_soft))
                            .hover(|s| s.border_color(p.ink_3))
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .text_size(px(14.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(*name),
                            )
                            .child(faint(*about, cx))
                            .on_click(cx.listener(move |this, _, window, cx| {
                                set_theme(mode, Some(window), cx);
                                this.settings.theme = (*mode).into();
                                this.persist_settings();
                                cx.notify();
                            }))
                    })),
            )
    }

    fn welcome_privacy(&self, cx: &mut Context<Self>) -> Div {
        let row = |id: &'static str,
                   title: &'static str,
                   about: &'static str,
                   on: bool,
                   cx: &mut Context<Self>,
                   set: fn(&mut AppView, bool)| {
            div()
                .flex()
                .items_center()
                .gap_4()
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(strong(title))
                        .child(meta(about, cx).w_full()),
                )
                .child(
                    div()
                        .flex_none()
                        .child(Switch::new(id).checked(on).on_click(cx.listener(
                            move |this, checked: &bool, _, cx| {
                                set(this, *checked);
                                this.persist_settings();
                                cx.notify();
                            },
                        ))),
                )
        };
        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(heading("Your privacy"))
            .child(meta("Needle needs no account, and your library stays on this computer. Choose what may go online. You can change these later in Settings.", cx))
            .child(row(
                "welcome-online",
                "Look up lyrics, covers, and artist photos",
                "Sends only the artist, album, and song title.",
                self.settings.online_media,
                cx,
                |this, on| this.settings.online_media = on,
            ))
            .child(row(
                "welcome-updates",
                "Check for new versions",
                "Once a day, asks needle.nnx.fyi for the newest version number.",
                self.settings.check_updates,
                cx,
                |this, on| this.settings.check_updates = on,
            ))
            .child(row(
                "welcome-crashes",
                "Send crash reports",
                "If Needle crashes, it tells us where, without file paths, your name, or anything about your music.",
                self.settings.crash_reports,
                cx,
                |this, on| this.settings.crash_reports = on,
            ))
    }

    fn welcome_tips(&self, cx: &mut Context<Self>) -> Div {
        let p = pal(cx);
        let tip = |keys: &'static str, text: &'static str| {
            div()
                .flex()
                .items_start()
                .gap_3()
                .child(
                    div()
                        .flex_none()
                        .w(px(96.))
                        .px(px(7.))
                        .py(px(1.))
                        .rounded(px(4.))
                        .border_1()
                        .border_color(p.line)
                        .bg(p.raised)
                        .text_size(px(12.))
                        .child(keys),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .text_size(px(13.5))
                        .text_color(p.ink_2)
                        .child(text),
                )
        };
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(heading("A few things to know"))
            .child(tip("Ctrl + K", "Go anywhere, run anything, and find songs, albums, and artists."))
            .child(tip("rating >= 4", "The search box takes rules too. Save a rule as a playlist that keeps itself up to date."))
            .child(tip("Sound", "The equalizer button next to the volume opens the equalizer and effects."))
            .child(tip("Play on", "Play on Chromecast, AirPlay, and DLNA speakers, one or several at once."))
            .child(tip("Phone", "Turn on the phone remote in Settings › Playback to control Needle from your phone."))
            .child(faint("Help and answers: needle.nnx.fyi/help", cx))
    }
}
