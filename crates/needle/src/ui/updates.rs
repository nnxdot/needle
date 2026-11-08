//! Opening files handed to Needle (from File Explorer or a second launch), and updates.
use super::{AppView, Event};
use gpui::{prelude::*, *};
use gpui_component::{Disableable, button::ButtonVariants};
use needle_core::{audio::QueueItem, scan, update};
use std::path::PathBuf;

const CHECK_EVERY: i64 = 24 * 3600;

/// Where an update stands.
pub enum UpdateState {
    Available(update::Release),
    Downloading(update::Release),
    Failed(String),
}

impl AppView {
    /// Play files opened with Needle, adding them to the library first.
    pub(super) fn open_files(&mut self, files: Vec<PathBuf>) {
        let library = self.library.clone();
        let sender = self.sender.clone();
        std::thread::spawn(move || {
            let mut items = vec![];
            for file in files {
                let Ok(full) = file.canonicalize() else {
                    continue;
                };
                let text = full.to_string_lossy().to_string();
                let is_cue = full
                    .extension()
                    .is_some_and(|e| e.eq_ignore_ascii_case("cue"));
                if is_cue {
                    let _ = needle_core::cue::import_sheet(&library, &full);
                    let prefix = format!("{text}#");
                    let mut tracks = library
                        .search(&format!("path starts with {}", super::quote(&prefix)))
                        .unwrap_or_default();
                    tracks.sort_by_key(|t| t.track_number);
                    items.extend(tracks.into_iter().map(|track| QueueItem {
                        track,
                        reason: "Opened".into(),
                    }));
                    continue;
                }
                let _ = scan::import_one(&library, &full);
                if let Ok(Some(track)) = library.track_by_path(&text) {
                    items.push(QueueItem {
                        track,
                        reason: "Opened".into(),
                    });
                }
            }
            let notice = match items.len() {
                0 => Some(
                    "Needle could not open that. It may not be a music file Needle can play."
                        .to_string(),
                ),
                1 => None,
                n => Some(format!("Playing {n} songs.")),
            };
            let _ = sender.send(Event::Play(items, notice));
        });
    }

    /// Once a day at most, ask needle.nnx.fyi whether a newer Needle is out.
    pub(super) fn check_for_update(&mut self, force: bool) {
        let now = chrono::Utc::now().timestamp();
        if !force
            && (!self.settings.check_updates || now - self.settings.last_update_check < CHECK_EVERY)
        {
            return;
        }
        self.settings.last_update_check = now;
        self.persist_settings();
        let sender = self.sender.clone();
        std::thread::spawn(move || {
            let result = update::check(env!("CARGO_PKG_VERSION")).map_err(|e| format!("{e:#}"));
            let _ = sender.send(Event::Update(result, force));
        });
    }

    pub(super) fn update_checked(
        &mut self,
        result: Result<Option<update::Release>, String>,
        asked: bool,
    ) {
        match result {
            Ok(Some(release)) => {
                self.notify(format!(
                    "Needle {} is out. Update it in Settings › Online services.",
                    release.version
                ));
                self.update = Some(UpdateState::Available(release));
            }
            Ok(None) if asked => self.notify(format!(
                "Needle {} is the latest version.",
                env!("CARGO_PKG_VERSION")
            )),
            Err(e) if asked => self.fail(format!("Could not check for updates: {e}")),
            _ => {}
        }
    }

    fn start_update(&mut self, cx: &mut Context<Self>) {
        let Some(UpdateState::Available(release)) = self.update.take() else {
            return;
        };
        self.update = Some(UpdateState::Downloading(release.clone()));
        // The update gets a new private folder of its own in here.
        let directory = std::env::temp_dir();
        let sender = self.sender.clone();
        std::thread::spawn(move || {
            let result = update::download(&release, &directory)
                .and_then(|installer| update::install(&release, &installer))
                .map_err(|e| format!("{e:#}"));
            let _ = sender.send(Event::UpdateStarted(result));
        });
        // The progress bar moves while it downloads: drawn again four times a second.
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(250))
                    .await;
                let going = this
                    .update(cx, |this, cx| {
                        cx.notify();
                        matches!(this.update, Some(UpdateState::Downloading(_)))
                    })
                    .unwrap_or(false);
                if !going {
                    break;
                }
            }
        })
        .detach();
        cx.notify();
    }

    pub(super) fn update_started(&mut self, result: Result<(), String>, cx: &mut Context<Self>) {
        match result {
            // The installer takes over from here and starts Needle again.
            Ok(()) => cx.quit(),
            Err(e) => self.update = Some(UpdateState::Failed(e)),
        }
    }

    /// The Updates rows in Settings.
    pub(super) fn updates_view(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let status = match &self.update {
            Some(UpdateState::Available(r)) => format!(
                "Needle {} is out. You have {}.{}",
                r.version,
                env!("CARGO_PKG_VERSION"),
                if needle_core::network::metered() {
                    " You are on a metered connection: the update downloads tens of MB."
                } else {
                    ""
                }
            ),
            Some(UpdateState::Downloading(r)) => progress_words(&r.version, &update::progress()),
            Some(UpdateState::Failed(e)) => format!("The update did not work: {e}"),
            None => format!(
                "You have Needle {}. Needle checks needle.nnx.fyi once a day.",
                env!("CARGO_PKG_VERSION")
            ),
        };
        let action = match &self.update {
            // A copy Needle cannot update by itself: the download page.
            Some(UpdateState::Available(r)) if r.package.is_none() => {
                let page = if r.page.is_empty() {
                    "https://needle.nnx.fyi".to_string()
                } else {
                    r.page.clone()
                };
                super::widgets::small_button("update-page", format!("Get Needle {}", r.version))
                    .primary()
                    .on_click(move |_, _, cx| cx.open_url(&page))
                    .into_any_element()
            }
            Some(UpdateState::Available(r)) => {
                super::widgets::small_button("update-now", format!("Update to {}", r.version))
                    .primary()
                    .on_click(cx.listener(|this, _, _, cx| this.start_update(cx)))
                    .into_any_element()
            }
            Some(UpdateState::Downloading(_)) => {
                super::widgets::small_button("update-now", "Updating…")
                    .disabled(true)
                    .into_any_element()
            }
            _ => super::widgets::small_button("update-check", "Check now")
                .ghost()
                .on_click(cx.listener(|this, _, _, cx| {
                    this.check_for_update(true);
                    cx.notify();
                }))
                .into_any_element(),
        };
        let bar = matches!(self.update, Some(UpdateState::Downloading(_))).then(|| {
            let p = update::progress();
            let fraction = if p.total > 0 {
                (p.done as f32 / p.total as f32).clamp(0., 1.)
            } else if p.stage == "check" || p.stage == "install" {
                1.
            } else {
                0.
            };
            let pal = super::theme::pal(cx);
            div()
                .mx_4()
                .mb_2()
                .h(px(6.))
                .rounded_full()
                .bg(pal.line_soft)
                .child(
                    div()
                        .h_full()
                        .rounded_full()
                        .bg(pal.accent)
                        .w(relative(fraction)),
                )
        });
        div()
            .flex()
            .flex_col()
            .child(super::widgets::setting_row("Updates", &status, action, cx))
            .children(bar)
            .child(super::widgets::setting_row(
                "What's new",
                &format!("What Needle {} brings.", env!("CARGO_PKG_VERSION")),
                super::widgets::small_button("whats-new-open", "Show").on_click(cx.listener(
                    |this, _, _, cx| {
                        this.whats_new = super::whats_new::notes(Some(env!("CARGO_PKG_VERSION")))
                            .or_else(|| super::whats_new::notes(None));
                        cx.notify();
                    },
                )),
                cx,
            ))
            .child(super::widgets::setting_row(
                "Check for updates automatically",
                "Once a day, when Needle starts.",
                gpui_component::switch::Switch::new("check-updates")
                    .checked(self.settings.check_updates)
                    .on_click(cx.listener(|this, checked: &bool, _, cx| {
                        this.settings.check_updates = *checked;
                        this.persist_settings();
                        cx.notify();
                    })),
                cx,
            ))
    }
}

/// What an update is doing, in words: "Downloading Needle 2.0.4 · 12.3 of 45.6 MB".
fn progress_words(version: &str, p: &update::Progress) -> String {
    let mb = |bytes: u64| bytes as f64 / 1_000_000.;
    match p.stage.as_str() {
        "list" => format!("Getting ready to download Needle {version}…"),
        "download" if p.total > 0 => format!(
            "Downloading Needle {version} · {:.1} of {:.1} MB",
            mb(p.done),
            mb(p.total)
        ),
        "download" => format!("Downloading Needle {version} · {:.1} MB", mb(p.done)),
        "check" => format!("Checking Needle {version}…"),
        "install" => format!("Installing Needle {version}…"),
        _ => format!("Downloading Needle {version} and checking it…"),
    }
}
