//! Music sources in the app: signing in to a server (Settings › Plugins), its songs in the
//! sidebar, and saving a streamed song as a file.
use super::{
    AppView, Page, pal,
    widgets::{count, faint, meta, small_button, strong},
};
use gpui::{prelude::*, *};
use gpui_component::{
    Disableable, Sizable,
    button::ButtonVariants,
    input::{Input, InputState},
};
use needle_core::plugins::{self, PluginEvent, PluginInfo};

/// The plugin that connects Needle to Navidrome and other Subsonic servers.
const SUBSONIC: &str = "subsonic";

/// "just now", "5 min ago", "3 h ago", "2 days ago".
fn ago(seconds: i64) -> String {
    match seconds.max(0) {
        s if s < 60 => "just now".into(),
        s if s < 3600 => format!("{} min ago", s / 60),
        s if s < 86_400 => format!("{} h ago", s / 3600),
        s => format!("{} days ago", s / 86_400),
    }
}

impl AppView {
    /// Sources that are turned on and signed in, for the sidebar.
    pub(super) fn signed_in_sources(&self) -> Vec<(String, String)> {
        self.plugins
            .plugins()
            .into_iter()
            .filter(|p| p.enabled)
            .filter_map(|p| {
                let source = p.source?;
                (source.signed_in && source.songs > 0).then_some((p.manifest.id, source.name))
            })
            .collect()
    }

    /// Make the text boxes of each sign-in form (called from the poll, which has a window).
    pub(super) fn ensure_source_inputs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        for plugin in self.plugins.plugins() {
            let Some(source) = plugin.source.filter(|_| plugin.enabled) else {
                continue;
            };
            for field in source.fields {
                let key = format!("{}/{}", plugin.manifest.id, field.id);
                if let std::collections::hash_map::Entry::Vacant(slot) =
                    self.source_inputs.entry(key)
                {
                    slot.insert(cx.new(|cx| {
                        InputState::new(window, cx)
                            .placeholder(field.placeholder.clone())
                            .masked(field.secret)
                    }));
                }
            }
        }
    }

    fn sign_in_source(&mut self, plugin: &PluginInfo, window: &mut Window, cx: &mut Context<Self>) {
        let Some(source) = &plugin.source else {
            return;
        };
        let mut fields = std::collections::BTreeMap::new();
        for field in &source.fields {
            let key = format!("{}/{}", plugin.manifest.id, field.id);
            if let Some(input) = self.source_inputs.get(&key) {
                fields.insert(field.id.clone(), input.read(cx).value().to_string());
                // A typed password does not stay on screen.
                if field.secret {
                    input.update(cx, |s, cx| s.set_value("", window, cx));
                }
            }
        }
        self.plugins.send(PluginEvent::SourceSignIn {
            plugin: plugin.manifest.id.clone(),
            fields,
        });
        self.notify(format!("Signing in to {}…", source.name));
        cx.notify();
    }

    /// Install the Navidrome / Subsonic plugin if needed, and turn it on.
    fn set_up_subsonic(&mut self, cx: &mut Context<Self>) {
        if let Err(error) = plugins::install_examples(&self.library) {
            self.fail(format!("{error:#}"));
            return;
        }
        self.plugins.send(PluginEvent::Reload);
        self.plugins
            .send(PluginEvent::Enable(SUBSONIC.into(), true));
        self.notify("Type your server's address and sign in below.");
        cx.notify();
    }

    /// The card at the top of Settings › Plugins that sets up a music server in one click.
    pub(super) fn music_server_card(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let ready = self
            .plugins
            .plugins()
            .iter()
            .any(|p| p.manifest.id == SUBSONIC && p.enabled);
        if ready {
            return None;
        }
        let p = pal(cx);
        Some(
            div()
                .p_4()
                .rounded(px(10.))
                .border_1()
                .border_color(p.accent.opacity(0.4))
                .bg(p.accent_soft)
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
                        .child(strong("Play music from your server"))
                        .child(
                            meta("Navidrome, Airsonic, Gonic, and other Subsonic servers. Your server's songs show with your own and play straight away.", cx)
                                .w_full(),
                        ),
                )
                .child(
                    small_button("subsonic-set-up", "Set up")
                        .primary()
                        .on_click(cx.listener(|this, _, _, cx| this.set_up_subsonic(cx))),
                )
                .into_any_element(),
        )
    }

    /// Sign-in form, or how the source is doing, inside its plugin's card.
    pub(super) fn source_block(
        &self,
        plugin: &PluginInfo,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let source = plugin.source.clone().filter(|_| plugin.enabled)?;
        let p = pal(cx);
        let id = plugin.manifest.id.clone();
        let body = if source.signed_in {
            let status = if source.syncing {
                "Getting the list of songs…".to_string()
            } else {
                let when = source
                    .synced_at
                    .map(|at| format!(" · updated {}", ago(chrono::Utc::now().timestamp() - at)))
                    .unwrap_or_default();
                format!("Signed in. {} songs{when}.", count(source.songs))
            };
            let (show, sync, out) = (id.clone(), id.clone(), id.clone());
            let name = source.name.clone();
            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(strong(status))
                .child(
                    div()
                        .flex()
                        .gap_2()
                        .when(source.songs > 0, |el| {
                            el.child(small_button(SharedString::from(format!("source-show-{id}")), "Show songs").on_click(
                                cx.listener(move |this, _, window, cx| {
                                    this.navigate(
                                        Page::Source {
                                            plugin: show.clone(),
                                            name: name.clone(),
                                        },
                                        window,
                                        cx,
                                    )
                                }),
                            ))
                        })
                        .child(
                            small_button(SharedString::from(format!("source-sync-{id}")), "Update now")
                                .ghost()
                                .disabled(source.syncing)
                                .tooltip("Get the server's list of songs again")
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.plugins.send(PluginEvent::SourceSync(sync.clone()));
                                    cx.notify();
                                })),
                        )
                        .child(
                            small_button(SharedString::from(format!("source-out-{id}")), "Sign out")
                                .ghost()
                                .tooltip("Forget the password. Its songs stay, marked missing, until you sign in again.")
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.plugins.send(PluginEvent::SourceSignOut(out.clone()));
                                    cx.notify();
                                })),
                        ),
                )
        } else {
            let fields = source.fields.iter().map(|field| {
                let key = format!("{id}/{}", field.id);
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(
                        div()
                            .w(px(130.))
                            .flex_none()
                            .text_size(px(13.))
                            .child(field.label.clone()),
                    )
                    .child(
                        div().flex_1().children(
                            self.source_inputs
                                .get(&key)
                                .map(|input| Input::new(input).small()),
                        ),
                    )
            });
            let plugin = plugin.clone();
            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(strong(format!("Sign in to {}", source.name)))
                .children(fields)
                .child(
                    div().flex().child(
                        small_button(SharedString::from(format!("source-in-{id}")), "Sign in")
                            .primary()
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.sign_in_source(&plugin, window, cx)
                            })),
                    ),
                )
                .child(
                    faint(
                        "Passwords are kept in Windows Credential Manager, not in Needle's files.",
                        cx,
                    )
                    .w_full(),
                )
        };
        Some(
            div()
                .mt_1()
                .p_3()
                .rounded(px(8.))
                .bg(p.raised)
                .flex()
                .flex_col()
                .gap_2()
                .child(body)
                .when_some(source.error, |el, error| {
                    el.child(meta(error, cx).w_full().text_color(p.danger))
                })
                .child(
                    faint(
                        "Songs stream as you play them. The last 2 GB you played stay on this computer, so they play again without the network.",
                        cx,
                    )
                    .w_full(),
                )
                .into_any_element(),
        )
    }

    /// Download streamed songs into the first music folder, where Needle adds them as files.
    pub(super) fn save_streamed(&mut self, ids: &[String]) {
        let tracks: Vec<_> = self
            .library
            .tracks_by_ids(ids)
            .unwrap_or_default()
            .into_iter()
            .filter(|t| t.is_streamed())
            .collect();
        if tracks.is_empty() {
            return;
        }
        let Some(root) = self.library.roots().unwrap_or_default().into_iter().next() else {
            self.fail("Add a music folder in Settings › Library first, so Needle knows where to save songs.");
            return;
        };
        let library = self.library.clone();
        let total = tracks.len();
        self.notify(if total == 1 {
            format!("Saving {}…", tracks[0].title)
        } else {
            format!("Saving {total} songs…")
        });
        self.background(move || {
            let folder = std::path::PathBuf::from(&root);
            let mut saved = vec![];
            for track in &tracks {
                saved.push(needle_core::sources::save_to(track, &folder)?);
            }
            for path in &saved {
                needle_core::scan::import_one(&library, path)?;
            }
            Ok(if total == 1 {
                format!("Saved {} to {}.", tracks[0].title, root)
            } else {
                format!("Saved {total} songs to {root}.")
            })
        });
    }
}
