use super::{
    AppView, Event, pal, set_theme,
    widgets::{
        faint, glyph, heading, icon, meta, page_title, segmented, setting_row, small_button, strong,
    },
};
use anyhow::Result;
use gpui::{prelude::*, *};
use gpui_component::{Disableable, Sizable, button::ButtonVariants, input::Input, switch::Switch};
use needle_core::{
    integrations::{self, SecretKind, SecretSource, ServiceState},
    query,
};

impl AppView {
    fn section_title(&self, title: &str, description: &str, cx: &App) -> Div {
        div()
            .mt_10()
            .mb_1()
            .flex()
            .flex_col()
            .gap_1()
            .child(heading(title.to_string()))
            .when(!description.is_empty(), |el| {
                el.child(meta(description.to_string(), cx))
            })
    }

    fn service_line(&self, state: &ServiceState, cx: &App) -> Div {
        let p = pal(cx);
        let text = match (&state.user, state.source, &state.rejected) {
            (_, _, Some(reason)) => format!("Needs signing in again · {reason}"),
            (Some(user), Some(SecretSource::Environment), _) => {
                format!("{user} · set by an environment variable")
            }
            (Some(user), _, _) => format!("Signed in as {user}"),
            (None, Some(SecretSource::Environment), _) => "Set by an environment variable".into(),
            (None, Some(_), _) if state.configured => "Key saved".into(),
            _ => "Not connected".into(),
        };
        div()
            .flex()
            .items_center()
            .gap_2()
            .child(
                div()
                    .size(px(7.))
                    .rounded_full()
                    .bg(if state.rejected.is_some() {
                        p.danger
                    } else if state.configured {
                        p.success
                    } else {
                        p.line
                    }),
            )
            .child(meta(text, cx))
    }

    fn services(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = pal(cx);
        let status = self
            .service_status
            .clone()
            .unwrap_or_else(integrations::secret_status);
        let summary = self.scrobble_summary.clone();
        let from_env = |s: &ServiceState| s.source == Some(SecretSource::Environment);
        let lastfm = status.lastfm.clone();
        let listenbrainz = status.listenbrainz.clone();
        let acoustid = status.acoustid.clone();
        let busy = self.service_busy;
        div()
            .flex()
            .flex_col()
            .when_some(status.store_error.clone(), |el, error| {
                el.child(meta(format!("Windows Credential Manager could not be read: {error}"), cx).text_color(p.danger).py_2())
            })
            // Last.fm
            .child(
                div()
                    .py_4()
                    .border_b_1()
                    .border_color(p.line_soft)
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_6()
                            .child(
                                div()
                                    .flex_1()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .child(strong("Last.fm"))
                                    .child(self.service_line(&lastfm, cx)),
                            )
                            .when(lastfm.configured && !from_env(&lastfm), |el| {
                                el.child(small_button("lastfm-out", "Sign out").ghost().on_click(cx.listener(|this, _, _, cx| {
                                    this.service_job(|| integrations::lastfm_sign_out().map(|_| "Signed out of Last.fm.".into()));
                                    cx.notify();
                                })))
                            })
                            .child(
                                Switch::new("lastfm-enable")
                                    .checked(self.settings.lastfm_enabled)
                                    .disabled(!lastfm.configured)
                                    .tooltip("Scrobble qualified listens to Last.fm")
                                    .on_click(cx.listener(|this, checked: &bool, _, cx| {
                                        this.settings.lastfm_enabled = *checked;
                                        this.persist_settings();
                                        cx.notify();
                                    })),
                            ),
                    )
                    .when(!lastfm.configured, |el| {
                        if let Some(pending) = &self.lastfm_pending {
                            let url = pending.auth_url.clone();
                            el.child(
                                div()
                                    .p_3()
                                    .rounded(px(8.))
                                    .bg(p.raised)
                                    .flex()
                                    .flex_col()
                                    .gap_2()
                                    .child(meta("Approve Needle in the Last.fm page that just opened, then press Continue.", cx))
                                    .child(
                                        div()
                                            .flex()
                                            .gap_2()
                                            .child(small_button("lastfm-continue", if busy { "Checking…" } else { "Continue" }).primary().disabled(busy).on_click(cx.listener(|this, _, _, cx| {
                                                this.lastfm_continue();
                                                cx.notify();
                                            })))
                                            .child(small_button("lastfm-reopen", "Open the page again").ghost().on_click(move |_, _, cx| cx.open_url(&url)))
                                            .child(small_button("lastfm-cancel", "Cancel").ghost().on_click(cx.listener(|this, _, _, cx| {
                                                this.lastfm_pending = None;
                                                cx.notify();
                                            }))),
                                    ),
                            )
                        } else {
                            el.when(!status.lastfm_app.configured, |el| {
                                el.child(meta("Needle needs a Last.fm API account to sign in. Create one at last.fm/api/account/create, then paste its key and shared secret.", cx).line_height(relative(1.45)))
                                    .child(div().flex().gap_2().child(Input::new(&self.lastfm_key).small().flex_1()).child(Input::new(&self.lastfm_secret).small().flex_1()))
                            })
                            .child(
                                div().child(
                                    small_button("lastfm-sign-in", if busy { "Opening…" } else { "Sign in with Last.fm" })
                                        .icon(icon("external"))
                                        .disabled(busy)
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.lastfm_begin(cx);
                                            cx.notify();
                                        })),
                                ),
                            )
                        }
                    })
                    .when_some(summary.as_ref().map(|s| s.lastfm.clone()), |el, queue| self.queue_summary(el, "lastfm", queue, cx)),
            )
            // ListenBrainz
            .child(
                div()
                    .py_4()
                    .border_b_1()
                    .border_color(p.line_soft)
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_6()
                            .child(div().flex_1().flex().flex_col().gap_1().child(strong("ListenBrainz")).child(self.service_line(&listenbrainz, cx)))
                            .when(listenbrainz.configured && !from_env(&listenbrainz), |el| {
                                el.child(small_button("lb-out", "Sign out").ghost().on_click(cx.listener(|this, _, _, cx| {
                                    this.service_job(|| integrations::listenbrainz_sign_out().map(|_| "Signed out of ListenBrainz.".into()));
                                    cx.notify();
                                })))
                            })
                            .child(
                                Switch::new("listenbrainz-enable")
                                    .checked(self.settings.listenbrainz_enabled)
                                    .disabled(!listenbrainz.configured)
                                    .tooltip("Submit qualified listens to ListenBrainz")
                                    .on_click(cx.listener(|this, checked: &bool, _, cx| {
                                        this.settings.listenbrainz_enabled = *checked;
                                        this.persist_settings();
                                        cx.notify();
                                    })),
                            ),
                    )
                    .when(!listenbrainz.configured, |el| {
                        el.child(meta("Paste the user token from listenbrainz.org/settings.", cx)).child(
                            div()
                                .flex()
                                .gap_2()
                                .child(Input::new(&self.listenbrainz_token).small().flex_1())
                                .child(small_button("lb-sign-in", if busy { "Checking…" } else { "Connect" }).disabled(busy).on_click(cx.listener(|this, _, _, cx| {
                                    let token = this.listenbrainz_token.read(cx).value().trim().to_string();
                                    if token.is_empty() {
                                        return this.fail("Paste your ListenBrainz user token first.");
                                    }
                                    this.service_job(move || integrations::listenbrainz_sign_in(&token).map(|user| format!("Connected to ListenBrainz as {user}.")));
                                    cx.notify();
                                }))),
                        )
                    })
                    .when_some(summary.as_ref().map(|s| s.listenbrainz.clone()), |el, queue| self.queue_summary(el, "listenbrainz", queue, cx)),
            )
            // AcoustID
            .child(
                div()
                    .py_4()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_6()
                            .child(
                                div()
                                    .flex_1()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .child(strong("AcoustID"))
                                    .child(meta("Identifies tracks by their sound. Needs a free application key from acoustid.org.", cx))
                                    .child(self.service_line(&acoustid, cx)),
                            )
                            .when(acoustid.configured && !from_env(&acoustid), |el| {
                                el.child(small_button("acoustid-clear", "Remove key").ghost().on_click(cx.listener(|this, _, _, cx| {
                                    this.service_job(|| integrations::clear_secret(SecretKind::AcoustidKey).map(|_| "AcoustID key removed.".into()));
                                    cx.notify();
                                })))
                            }),
                    )
                    .when(!acoustid.configured, |el| {
                        el.child(
                            div()
                                .flex()
                                .gap_2()
                                .child(Input::new(&self.acoustid_key).small().flex_1())
                                .child(small_button("acoustid-save", "Save key").on_click(cx.listener(|this, _, _, cx| {
                                    let key = this.acoustid_key.read(cx).value().trim().to_string();
                                    if key.is_empty() {
                                        return this.fail("Paste your AcoustID application key first.");
                                    }
                                    this.service_job(move || integrations::save_secret(SecretKind::AcoustidKey, &key).map(|_| "AcoustID key saved.".into()));
                                    cx.notify();
                                }))),
                        )
                    }),
            )
            .child(faint("Keys and sessions are stored in Windows Credential Manager, never in your library or its exports. Environment variables override them.", cx).line_height(relative(1.45)).pt_2())
    }

    fn queue_summary(
        &self,
        el: Div,
        service: &'static str,
        queue: integrations::QueueSummary,
        cx: &mut Context<Self>,
    ) -> Div {
        let p = pal(cx);
        if queue.pending == 0 && queue.failed == 0 && queue.sign_in_required.is_none() {
            return el.when(queue.sent > 0, |el| {
                el.child(faint(format!("{} listens sent", queue.sent), cx))
            });
        }
        let mut parts = vec![];
        if queue.pending > 0 {
            parts.push(format!("{} waiting to send", queue.pending));
        }
        if queue.failed > 0 {
            parts.push(format!("{} failed", queue.failed));
        }
        el.child(
            div()
                .flex()
                .items_center()
                .gap_3()
                .child(meta(parts.join(" · "), cx))
                .when_some(
                    queue.sign_in_required.clone().or(queue.last_error.clone()),
                    |el, error| el.child(faint(error, cx).text_color(p.danger).truncate().flex_1()),
                )
                .when(queue.failed > 0, |el| {
                    el.child(
                        small_button(
                            SharedString::from(format!("retry-{service}")),
                            "Retry failed",
                        )
                        .ghost()
                        .on_click(cx.listener(move |this, _, _, cx| {
                            let library = this.library.clone();
                            this.service_job(move || {
                                integrations::retry_failed_scrobbles(&library, Some(service))
                                    .map(|n| format!("{n} listens will be retried."))
                            });
                            cx.notify();
                        })),
                    )
                }),
        )
    }

    fn service_job(&mut self, job: impl FnOnce() -> Result<String> + Send + 'static) {
        self.service_busy = true;
        let sender = self.sender.clone();
        std::thread::spawn(move || {
            let result = job();
            let _ = sender.send(match result {
                Ok(message) => Event::Notice(message),
                Err(e) => Event::Error(format!("{e:#}")),
            });
            let _ = sender.send(Event::ServicesChanged);
        });
    }

    fn lastfm_begin(&mut self, cx: &mut Context<Self>) {
        let key = self.lastfm_key.read(cx).value().trim().to_string();
        let secret = self.lastfm_secret.read(cx).value().trim().to_string();
        let credentials = integrations::Credentials::load();
        let key = if key.is_empty() {
            credentials.lastfm_api_key.clone()
        } else {
            key
        };
        let secret = if secret.is_empty() {
            credentials.lastfm_secret.clone()
        } else {
            secret
        };
        if key.is_empty() || secret.is_empty() {
            return self.fail("Enter your Last.fm API key and shared secret first.");
        }
        self.service_busy = true;
        let sender = self.sender.clone();
        std::thread::spawn(move || {
            let _ = sender.send(match integrations::lastfm_begin(&key, &secret) {
                Ok(pending) => Event::LastfmPending(pending),
                Err(e) => Event::Error(format!("Last.fm: {e:#}")),
            });
            let _ = sender.send(Event::ServicesChanged);
        });
    }

    fn lastfm_continue(&mut self) {
        let Some(pending) = self.lastfm_pending.clone() else {
            return;
        };
        self.service_busy = true;
        let sender = self.sender.clone();
        std::thread::spawn(move || {
            let event = match integrations::lastfm_complete(&pending) {
                Ok(user) => Event::LastfmSignedIn(user),
                Err(e) if integrations::is_not_yet_authorized(&e) => {
                    Event::Error("Last.fm hasn't received your approval yet. Approve Needle in the browser, then press Continue.".into())
                }
                Err(e) => Event::Error(format!("Last.fm: {e:#}")),
            };
            let _ = sender.send(event);
            let _ = sender.send(Event::ServicesChanged);
        });
    }

    pub(super) fn settings_view(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = pal(cx);
        let roots = self.library.roots().unwrap_or_default();
        let weak = cx.entity().downgrade();
        let theme = if self.settings.theme == "light" { 1 } else { 0 };
        let density = if self.settings.layout.row_height < 50. {
            0
        } else {
            1
        };
        let device = |id: SharedString,
                      name: String,
                      selected: bool,
                      value: Option<String>,
                      cx: &mut Context<Self>| {
            div()
                .id(id)
                .h(px(36.))
                .px_3()
                .rounded(px(6.))
                .flex()
                .items_center()
                .gap_3()
                .cursor_pointer()
                .when(selected, |el| el.bg(p.raised))
                .hover(|s| s.bg(p.raised))
                .child(glyph("speaker").size(px(16.)).text_color(if selected {
                    p.accent
                } else {
                    p.ink_3
                }))
                .child(div().flex_1().truncate().text_size(px(13.)).child(name))
                .when(selected, |el| {
                    el.child(glyph("check").size(px(16.)).text_color(p.accent))
                })
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.settings.output_device = value.clone();
                    this.configure();
                    cx.notify();
                }))
        };
        let shortcuts = [
            ("Space", "Play or pause"),
            ("Ctrl + ← / →", "Previous or next track"),
            ("← / →", "Seek 10 seconds"),
            ("Ctrl + ↑ / ↓", "Volume"),
            ("↑ / ↓, Shift", "Move or extend the selection"),
            ("Enter", "Play the selection"),
            ("Ctrl + K or Ctrl + F", "Search"),
            ("Ctrl + E", "Edit tags"),
            ("Ctrl + D", "Favorite"),
            ("Ctrl + J", "Show the queue"),
            ("Ctrl + O", "Add a music folder"),
            ("Alt + ← or Backspace", "Go back"),
        ];
        div()
            .id("settings-scroll")
            .size_full()
            .overflow_y_scroll()
            .child(
                div()
                    .max_w(px(720.))
                    .px_8()
                    .pt_6()
                    .pb_16()
                    .flex()
                    .flex_col()
                    .child(page_title("Settings"))
                    // Playback
                    .child(self.section_title("Playback", "", cx))
                    .child(setting_row(
                        "Exclusive output",
                        "Plays each file at its own sample rate with nothing in between. Volume and ReplayGain are bypassed, so use your device's volume.",
                        Switch::new("exclusive").checked(self.settings.exclusive).on_click(cx.listener(|this, checked: &bool, _, cx| {
                            this.settings.exclusive = *checked;
                            this.configure();
                            cx.notify();
                        })),
                        cx,
                    ))
                    .child(setting_row(
                        "ReplayGain",
                        "Evens out loudness using measured or tagged gain, with peak protection.",
                        Switch::new("replay-gain").checked(self.settings.replay_gain).on_click(cx.listener(|this, checked: &bool, _, cx| {
                            this.settings.replay_gain = *checked;
                            this.configure();
                            cx.notify();
                        })),
                        cx,
                    ))
                    .when(self.settings.replay_gain, |el| {
                        el.child(setting_row(
                            "Keep album dynamics",
                            "Uses album gain when an album has been measured, so quiet songs stay quiet next to loud ones. Falls back to track gain.",
                            Switch::new("album-gain").checked(self.settings.album_gain).on_click(cx.listener(|this, checked: &bool, _, cx| {
                                this.settings.album_gain = *checked;
                                this.configure();
                                cx.notify();
                            })),
                            cx,
                        ))
                    })
                    .child(
                        div()
                            .py_4()
                            .border_b_1()
                            .border_color(p.line_soft)
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(
                                div()
                                    .flex()
                                    .justify_between()
                                    .items_center()
                                    .child(strong("Output device"))
                                    .child(small_button("refresh-devices", "Refresh").ghost().on_click(cx.listener(|this, _, _, cx| {
                                        this.output_devices = needle_core::audio::devices().unwrap_or_default();
                                        cx.notify();
                                    }))),
                            )
                            .child(device("device-default".into(), "System default".into(), self.settings.output_device.is_none(), None, cx))
                            .children(self.output_devices.iter().enumerate().map(|(i, name)| {
                                device(format!("device-{i}").into(), name.clone(), self.settings.output_device.as_ref() == Some(name), Some(name.clone()), cx)
                            })),
                    )
                    .child(
                        div()
                            .py_4()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(strong("When the queue runs out"))
                            .child(meta("Needle keeps playing tracks that match this rule. Leave it empty to stop.", cx))
                            .child(
                                div()
                                    .flex()
                                    .gap_2()
                                    .child(Input::new(&self.autoplay).small().flex_1())
                                    .child(small_button("save-autoplay", "Save").on_click(cx.listener(|this, _, _, cx| {
                                        let rule = this.autoplay.read(cx).value().trim().to_string();
                                        match query::compile(&rule, chrono::Utc::now().timestamp()) {
                                            Ok(_) => {
                                                this.settings.autoplay_query = rule;
                                                this.configure();
                                                this.notify("Autoplay rule saved.");
                                            }
                                            Err(e) => this.fail(e.to_string()),
                                        }
                                        cx.notify();
                                    }))),
                            ),
                    )
                    // Library
                    .child(self.section_title("Music folders", "Needle watches these folders and never moves or deletes your files.", cx))
                    .child(
                        div()
                            .py_2()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .when(roots.is_empty(), |el| el.child(meta("No folders yet.", cx)))
                            .children(roots.into_iter().map(|root| {
                                div()
                                    .h(px(34.))
                                    .px_3()
                                    .rounded(px(6.))
                                    .bg(p.raised)
                                    .flex()
                                    .items_center()
                                    .gap_3()
                                    .child(glyph("folder").size(px(16.)).text_color(p.ink_3))
                                    .child(div().text_size(px(12.5)).truncate().child(root.trim_start_matches("\\\\?\\").to_string()))
                            }))
                            .child(
                                div()
                                    .mt_2()
                                    .flex()
                                    .gap_2()
                                    .child(small_button("settings-add", "Add folder").icon(icon("plus")).on_click(cx.listener(|this, _, _, cx| this.import_folder(cx))))
                                    .child(small_button("rescan", "Check for changes").ghost().disabled(self.scan.is_some()).on_click(cx.listener(|this, _, _, cx| this.rescan(cx)))),
                            ),
                    )
                    // Appearance
                    .child(self.section_title("Appearance", "", cx))
                    .child(setting_row(
                        "Theme",
                        "",
                        segmented("theme", &["Dark", "Light"], theme, cx, {
                            let weak = weak.clone();
                            move |index, window, cx| {
                                let mode = if index == 0 { "dark" } else { "light" };
                                set_theme(mode, Some(window), cx);
                                let _ = weak.update(cx, |this, cx| {
                                    this.settings.theme = mode.into();
                                    this.persist_settings();
                                    cx.notify();
                                });
                            }
                        }),
                        cx,
                    ))
                    .child(setting_row(
                        "Density",
                        "Compact fits more tracks on screen.",
                        segmented("density", &["Compact", "Comfortable"], density, cx, {
                            let weak = weak.clone();
                            move |index, _, cx| {
                                let _ = weak.update(cx, |this, cx| {
                                    this.settings.layout.row_height = if index == 0 { 44. } else { 56. };
                                    this.persist_settings();
                                    cx.notify();
                                });
                            }
                        }),
                        cx,
                    ))
                    .child(setting_row(
                        "Layout file",
                        "Share panel widths and row height as a small JSON file.",
                        div()
                            .flex()
                            .gap_2()
                            .child(small_button("import-layout", "Import").ghost().on_click(cx.listener(|this, _, _, _| {
                                let sender = this.sender.clone();
                                std::thread::spawn(move || {
                                    if let Some(path) = rfd::FileDialog::new().add_filter("Needle layout", &["json"]).pick_file() {
                                        let result = (|| -> Result<needle_core::model::Layout> {
                                            let layout: needle_core::model::Layout = serde_json::from_slice(&std::fs::read(path)?)?;
                                            layout.validate()?;
                                            Ok(layout)
                                        })();
                                        let _ = sender.send(match result {
                                            Ok(layout) => Event::Layout(layout),
                                            Err(e) => Event::Error(e.to_string()),
                                        });
                                    }
                                });
                            })))
                            .child(small_button("export-layout", "Export").ghost().on_click(cx.listener(|this, _, _, _| {
                                let layout = this.settings.layout.clone();
                                this.background(move || {
                                    let Some(path) = rfd::FileDialog::new().set_file_name("needle-layout.json").save_file() else {
                                        return Ok(String::new());
                                    };
                                    std::fs::write(path, serde_json::to_vec_pretty(&layout)?)?;
                                    Ok("Layout exported.".into())
                                });
                            }))),
                        cx,
                    ))
                    // Artwork and lyrics
                    .child(self.section_title("Artwork and lyrics", "Needle always uses covers, .lrc files, and lyrics tags found with your music.", cx))
                    .child(setting_row(
                        "Look things up online",
                        "Finds missing album covers (MusicBrainz and the Cover Art Archive), artist photos (Wikimedia Commons), and lyrics (LRCLIB). Only artist, album, title, and length are sent.",
                        Switch::new("online-media").checked(self.settings.online_media).on_click(cx.listener(|this, checked: &bool, _, cx| {
                            this.settings.online_media = *checked;
                            this.persist_settings();
                            this.artist_images.retain(|_, v| v.is_some());
                            if let Some(item) = this.playback.current.clone() {
                                this.track_started(&item.track);
                            }
                            cx.notify();
                        })),
                        cx,
                    ))
                    .when(self.settings.online_media, |el| {
                        el.child(setting_row(
                            "Find missing album covers",
                            "Looks up every album without a cover. Large libraries take a while: MusicBrainz allows one request per second.",
                            small_button("fetch-covers", "Find covers").on_click(cx.listener(|this, _, _, _| {
                                let library = this.library.clone();
                                this.notify("Looking for missing album covers in the background…");
                                this.background(move || {
                                    let mut found = 0;
                                    for album in library.albums("")?.into_iter().filter(|a| a.artwork.is_none()) {
                                        if let Some(track) = library.album_tracks(&album.key)?.into_iter().next() {
                                            found += needle_core::media::fetch_album_art(&library, &track).unwrap_or(0).min(1);
                                        }
                                    }
                                    Ok(format!("Found covers for {found} albums."))
                                });
                            })),
                            cx,
                        ))
                    })
                    // Services
                    .child(self.section_title("Listening services", "Optional. Nothing is sent until you connect a service and turn it on.", cx))
                    .child(self.services(cx))
                    // Data
                    .child(self.section_title("Your data", "History, ratings, and playlists live in one local database.", cx))
                    .child(setting_row(
                        "Back up the library",
                        "Saves a consistent copy of the database, including recent changes.",
                        small_button("backup", "Back up…").on_click(cx.listener(|this, _, _, _| {
                            let library = this.library.clone();
                            this.background(move || {
                                let Some(path) = rfd::FileDialog::new().set_file_name("needle-library.db").save_file() else {
                                    return Ok(String::new());
                                };
                                library.backup(&path)?;
                                Ok("Library backup saved.".into())
                            });
                        })),
                        cx,
                    ))
                    .child(setting_row(
                        "Import a playlist",
                        "Reads an M3U or M3U8 file. Add its music folder first so the tracks can be matched.",
                        small_button("import-m3u", "Import…").on_click(cx.listener(|this, _, _, _| {
                            let library = this.library.clone();
                            this.background(move || {
                                let Some(path) = rfd::FileDialog::new().add_filter("Playlists", &["m3u", "m3u8"]).pick_file() else {
                                    return Ok(String::new());
                                };
                                let p = library.import_playlist(&path)?;
                                Ok(format!("Imported {} tracks into “{}”.", p.track_ids.len(), p.name))
                            });
                        })),
                        cx,
                    ))
                    .child(
                        div()
                            .py_4()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(strong("Move to another computer"))
                            .child(meta("Exports an encrypted bundle of history, ratings, and playlists. Import it where the same music files exist, using the same passphrase.", cx).line_height(relative(1.45)))
                            .child(
                                div()
                                    .flex()
                                    .gap_2()
                                    .child(Input::new(&self.sync_phrase).small().flex_1())
                                    .child(small_button("sync-export", "Export…").on_click(cx.listener(|this, _, _, cx| this.sync_transfer(true, cx))))
                                    .child(small_button("sync-import", "Import…").ghost().on_click(cx.listener(|this, _, _, cx| this.sync_transfer(false, cx)))),
                            ),
                    )
                    // Keyboard
                    .child(self.section_title("Keyboard", "", cx))
                    .child(
                        div().pt_2().flex().flex_col().children(shortcuts.iter().map(|(keys, what)| {
                            div()
                                .py(px(6.))
                                .flex()
                                .items_center()
                                .child(div().w(px(200.)).flex().child(
                                    div()
                                        .px(px(7.))
                                        .py(px(1.))
                                        .rounded(px(4.))
                                        .border_1()
                                        .border_color(p.line)
                                        .bg(p.raised)
                                        .text_size(px(12.))
                                        .max_w_full()
                                        .flex_none()
                                        .child(*keys),
                                ))
                                .child(meta(*what, cx))
                        })),
                    )
                    .child(
                        div()
                            .mt_10()
                            .pt_4()
                            .border_t_1()
                            .border_color(p.line_soft)
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(faint(format!("Needle {} · nnx", env!("CARGO_PKG_VERSION")), cx))
                            .child(faint(format!("Library data: {}", self.library.directory.display()), cx)),
                    ),
            )
    }

    fn sync_transfer(&mut self, export: bool, cx: &mut Context<Self>) {
        let phrase = self.sync_phrase.read(cx).value().to_string();
        if phrase.chars().count() < 12 {
            return self.fail("Use a passphrase of at least 12 characters.");
        }
        let library = self.library.clone();
        self.background(move || {
            let dialog = rfd::FileDialog::new().add_filter("Needle encrypted library", &["needle"]);
            let path = if export { dialog.set_file_name("library.needle").save_file() } else { dialog.pick_file() };
            let Some(path) = path else {
                return Ok(String::new());
            };
            if export {
                needle_core::sync::export(&library, &path, &phrase)?;
                Ok("Encrypted bundle exported. Audio files and credentials are not included.".into())
            } else {
                let r = needle_core::sync::import(&library, &path, &phrase)?;
                Ok(format!(
                    "Imported {} listens and {} playlists. {} tracks matched; {} need local copies.",
                    r.imported_listens, r.imported_playlists, r.matched_tracks, r.unmatched_tracks
                ))
            }
        });
    }
}
