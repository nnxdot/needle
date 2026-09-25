//! Plugins asking the person something: a box to type in, or a file to choose; and files
//! dropped on the window going to the plugin that opens their type. Needle asks in the same
//! box for a music folder's path when no file picker can open.
use super::{
    AppView, pal,
    widgets::{meta, small_button, strong},
};
use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme,
    button::ButtonVariants,
    input::{Input, InputEvent, InputState},
};
use needle_core::plugins::{self, PluginEvent, Question};
use std::path::Path;

/// Music file types, which are not offered to plugins with a message when none opens them.
const MUSIC: [&str; 16] = [
    "flac", "mp3", "wav", "m4a", "mp4", "aac", "ogg", "oga", "opus", "aiff", "aif", "wv", "ape",
    "dsf", "cue", "alac",
];

/// Who is asking, and what the answer is for.
enum AskFor {
    /// A plugin's question, answered through `plugins::answer`.
    Plugin { id: u64, name: String },
    /// The path of a music folder to add.
    MusicFolder,
}

/// A question on screen, waiting for a reply.
pub struct Asking {
    asker: AskFor,
    title: String,
    prompt: String,
    input: Entity<InputState>,
    _subscription: Subscription,
}

impl AppView {
    /// A plugin asked something (from `HostAction::Ask`).
    pub(super) fn plugin_asks(
        &mut self,
        id: u64,
        plugin: String,
        question: Question,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match question {
            Question::Text {
                title,
                prompt,
                default,
            } => {
                // One question at a time: an earlier one still open counts as cancelled.
                if let Some(Asking {
                    asker: AskFor::Plugin { id, .. },
                    ..
                }) = self.asking.take()
                {
                    plugins::answer(id, None);
                }
                let input = cx.new(|cx| {
                    let mut state = InputState::new(window, cx);
                    state.set_value(default, window, cx);
                    state
                });
                input.update(cx, |s, cx| s.focus(window, cx));
                let subscription = cx.subscribe_in(&input, window, |this, _, event, _, cx| {
                    if let InputEvent::PressEnter { .. } = event {
                        this.reply_to_ask(true, cx);
                    }
                });
                self.asking = Some(Asking {
                    asker: AskFor::Plugin { id, name: plugin },
                    title,
                    prompt,
                    input,
                    _subscription: subscription,
                });
                cx.notify();
            }
            Question::File { title, extensions } => {
                if !self.can_pick(cx) {
                    plugins::answer(id, None);
                    return;
                }
                let paths = cx.prompt_for_paths(PathPromptOptions {
                    files: true,
                    directories: false,
                    multiple: false,
                    prompt: Some(title.into()),
                });
                cx.spawn(async move |this, cx| {
                    let chosen = match paths.await {
                        Ok(Ok(Some(paths))) => paths.into_iter().next(),
                        _ => None,
                    };
                    let Some(path) = chosen else {
                        plugins::answer(id, None);
                        return;
                    };
                    let kind = path
                        .extension()
                        .map(|e| e.to_string_lossy().to_lowercase())
                        .unwrap_or_default();
                    let reply = if !extensions.is_empty() && !extensions.contains(&kind) {
                        Err(format!(
                            "{plugin} needs a {} file.",
                            extensions
                                .iter()
                                .map(|e| format!(".{e}"))
                                .collect::<Vec<_>>()
                                .join(" or ")
                        ))
                    } else {
                        plugins::file_map(&path).map_err(|e| format!("{e:#}"))
                    };
                    match reply {
                        Ok(file) => plugins::answer(id, Some(file)),
                        Err(problem) => {
                            plugins::answer(id, None);
                            let _ = this.update(cx, |this, cx| {
                                this.fail(problem);
                                cx.notify();
                            });
                        }
                    }
                })
                .detach();
            }
        }
    }

    /// No file picker can open: ask for the music folder's path instead.
    pub(super) fn ask_music_folder(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(Asking {
            asker: AskFor::Plugin { id, .. },
            ..
        }) = self.asking.take()
        {
            plugins::answer(id, None);
        }
        let music = std::env::var_os("HOME")
            .map(|home| Path::new(&home).join("Music"))
            .filter(|music| music.is_dir())
            .map(|music| music.to_string_lossy().to_string())
            .unwrap_or_default();
        let input = cx.new(|cx| {
            let mut state = InputState::new(window, cx).placeholder("/home/you/Music");
            state.set_value(music, window, cx);
            state
        });
        input.update(cx, |s, cx| s.focus(window, cx));
        let subscription = cx.subscribe_in(&input, window, |this, _, event, _, cx| {
            if let InputEvent::PressEnter { .. } = event {
                this.reply_to_ask(true, cx);
            }
        });
        self.asking = Some(Asking {
            asker: AskFor::MusicFolder,
            title: "Add a music folder".into(),
            prompt: "No file picker can open here, so type the folder's path. Needle reads the music in it and keeps watching it for changes.".into(),
            input,
            _subscription: subscription,
        });
        cx.notify();
    }

    /// Answer the open question: what was typed, or nothing (cancelled).
    fn reply_to_ask(&mut self, send: bool, cx: &mut Context<Self>) {
        let Some(asking) = self.asking.take() else {
            return;
        };
        let text = asking.input.read(cx).value().trim().to_string();
        match asking.asker {
            AskFor::Plugin { id, .. } => {
                plugins::answer(id, send.then(|| serde_json::Value::String(text)));
            }
            AskFor::MusicFolder if send => {
                let folder = std::path::PathBuf::from(&text);
                if text.is_empty() || !folder.is_dir() {
                    // Keep the box open so the path can be fixed.
                    self.fail(format!("There is no folder at \"{text}\"."));
                    self.asking = Some(asking);
                } else {
                    self.scan_folder(Some(folder), cx);
                }
            }
            AskFor::MusicFolder => {}
        }
        cx.notify();
    }

    /// Files dropped on the window that are not themes: each goes to the first turned-on
    /// plugin that opens its type.
    pub(super) fn drop_to_plugins(&mut self, paths: &[std::path::PathBuf]) {
        let list = self.plugins.plugins();
        for path in paths {
            let kind = path
                .extension()
                .map(|e| e.to_string_lossy().to_lowercase())
                .unwrap_or_default();
            if kind.is_empty() {
                continue;
            }
            let Some(plugin) = list.iter().find(|p| {
                p.enabled
                    && p.manifest
                        .opens
                        .iter()
                        .any(|o| o.trim_start_matches('.').eq_ignore_ascii_case(&kind))
            }) else {
                // Music dropped on the window did nothing before plugins could open files, so it
                // still says nothing.
                if !MUSIC.contains(&kind.as_str()) {
                    self.notify(format!("No plugin that is turned on opens .{kind} files."));
                }
                continue;
            };
            match plugins::file_map(Path::new(path)) {
                Ok(file) => {
                    self.plugins.send(PluginEvent::FileDropped {
                        plugin: plugin.manifest.id.clone(),
                        file,
                    });
                    self.notify(format!(
                        "Gave {} to {}.",
                        path.file_name().unwrap_or_default().to_string_lossy(),
                        plugin.manifest.name
                    ));
                }
                Err(error) => self.fail(format!("{error:#}")),
            }
        }
    }

    /// The open question, over everything else.
    pub(super) fn asking_view(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        let asking = self.asking.as_ref()?;
        let p = pal(cx);
        let card = div()
            .id("plugin-ask")
            .occlude()
            .w(px(440.))
            .max_w_full()
            .p_6()
            .rounded(px(12.))
            .bg(cx.theme().popover)
            .border_1()
            .border_color(p.line)
            .shadow_lg()
            .flex()
            .flex_col()
            .gap_3()
            .child(strong(asking.title.clone()))
            .child(meta(asking.prompt.clone(), cx).w_full())
            .child(Input::new(&asking.input))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .children(match &asking.asker {
                        AskFor::Plugin { name, .. } => Some(meta(format!("Asked by {name}"), cx)),
                        AskFor::MusicFolder => None,
                    })
                    .child(div().flex_1())
                    .child(
                        small_button("plugin-ask-cancel", "Cancel")
                            .ghost()
                            .on_click(cx.listener(|this, _, _, cx| this.reply_to_ask(false, cx))),
                    )
                    .child(
                        small_button("plugin-ask-ok", "OK")
                            .primary()
                            .on_click(cx.listener(|this, _, _, cx| this.reply_to_ask(true, cx))),
                    ),
            );
        Some(
            deferred(
                div()
                    .id("plugin-ask-backdrop")
                    .absolute()
                    .inset_0()
                    .occlude()
                    .bg(gpui::black().opacity(if p.dark { 0.5 } else { 0.25 }))
                    .flex()
                    .justify_center()
                    .items_center()
                    .p_6()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| this.reply_to_ask(false, cx)),
                    )
                    .child(card.on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())),
            )
            .with_priority(3),
        )
    }
}
