use super::{
    AppView, pal,
    widgets::{faint, meta, small_button, strong},
};
use gpui::{prelude::*, *};
use gpui_component::{button::ButtonVariants, switch::Switch};
use needle_core::{
    audio::{Command, QueueItem},
    plugins::{self, HostAction, PluginEvent},
};

impl AppView {
    /// Carry out something a plugin asked for.
    pub(super) fn plugin_action(&mut self, action: HostAction, cx: &mut Context<Self>) {
        let items = |this: &Self, ids: Vec<String>| -> Vec<QueueItem> {
            this.library
                .tracks_by_ids(&ids)
                .unwrap_or_default()
                .into_iter()
                .filter(|t| !t.missing)
                .map(|track| QueueItem {
                    track,
                    reason: "Chosen by a plugin".into(),
                })
                .collect()
        };
        match action {
            HostAction::Notify(text) => self.notify(text),
            HostAction::Play(ids) => {
                let items = items(self, ids);
                if !items.is_empty() {
                    self.player.send(Command::Play(items));
                }
            }
            HostAction::Enqueue(ids) => self.player.send(Command::Enqueue(items(self, ids))),
            HostAction::PlayNext(ids) => self.player.send(Command::PlayNext(items(self, ids))),
            HostAction::Toggle => self.player.send(Command::Toggle),
            HostAction::Next => self.player.send(Command::Next),
            HostAction::Previous => self.player.send(Command::Previous),
            HostAction::LibraryChanged => {
                self.playlists = self.library.playlists().unwrap_or_default();
                self.refresh(cx);
            }
            HostAction::EffectParam {
                plugin,
                effect,
                param,
                value,
            } => {
                let mut changed = false;
                for slot in self
                    .settings
                    .dsp
                    .effects
                    .iter_mut()
                    .filter(|s| s.plugin == plugin && s.effect == effect)
                {
                    slot.params.insert(param.clone(), value);
                    // The slider is made again, at the new value, when Sound is drawn next.
                    self.sound.forget_slider(&slot.uid, &param);
                    changed = true;
                }
                if changed {
                    self.player.send(Command::Dsp(self.settings.dsp.clone()));
                    cx.notify();
                }
            }
            HostAction::EffectOn { plugin, effect, on } => {
                let mut found = false;
                for slot in self
                    .settings
                    .dsp
                    .effects
                    .iter_mut()
                    .filter(|s| s.plugin == plugin && s.effect == effect)
                {
                    slot.on = on;
                    found = true;
                }
                if !found && on {
                    self.settings
                        .dsp
                        .effects
                        .push(needle_core::effects::EffectSlot::new(&plugin, &effect));
                }
                if found || on {
                    self.player.send(Command::Dsp(self.settings.dsp.clone()));
                    cx.notify();
                }
            }
        }
    }

    pub(super) fn run_plugin_command(
        &mut self,
        plugin: String,
        command: String,
        track_ids: Vec<String>,
    ) {
        self.plugins.send(PluginEvent::Run {
            plugin,
            command,
            track_ids,
        });
    }

    pub(super) fn plugins_section(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = pal(cx);
        let list = self.plugins.plugins();
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(small_button("plugins-open", "Open plugins folder").on_click(cx.listener(|this, _, _, _| {
                        let folder = this.plugins.folder().to_path_buf();
                        let _ = std::fs::create_dir_all(&folder);
                        let _ = std::process::Command::new("explorer").arg(folder).spawn();
                    })))
                    .child(small_button("plugins-reload", "Reload").ghost().on_click(cx.listener(|this, _, _, cx| {
                        this.plugins.send(PluginEvent::Reload);
                        this.notify("Plugins reloaded.");
                        cx.notify();
                    })))
                    .child(small_button("plugins-examples", "Add example plugins").ghost().on_click(cx.listener(|this, _, _, cx| {
                        match plugins::install_examples(&this.library) {
                            Ok(0) => this.notify("The example plugins are already installed."),
                            Ok(n) => {
                                this.plugins.send(PluginEvent::Reload);
                                this.notify(format!("Added {n} example plugins. Turn one on to try it."));
                            }
                            Err(e) => this.fail(format!("{e:#}")),
                        }
                        cx.notify();
                    }))),
            )
            .when(list.is_empty(), |el| {
                el.child(meta("No plugins yet. Add the examples, or put a plugin folder in the plugins folder and press Reload.", cx))
            })
            .children(list.into_iter().enumerate().map(|(i, plugin)| {
                let id = plugin.manifest.id.clone();
                let enabled = plugin.enabled;
                div()
                    .id(("plugin", i))
                    .p_4()
                    .rounded(px(10.))
                    .bg(p.chrome)
                    .border_1()
                    .border_color(if plugin.error.is_some() { p.danger.opacity(0.4) } else { p.line_soft })
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .flex()
                            .items_start()
                            .gap_4()
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .child(strong(format!(
                                        "{}{}",
                                        plugin.manifest.name,
                                        if plugin.manifest.version.is_empty() { String::new() } else { format!("  {}", plugin.manifest.version) }
                                    )))
                                    .when(!plugin.manifest.description.is_empty(), |el| el.child(meta(plugin.manifest.description.clone(), cx).w_full()))
                                    .when(!plugin.manifest.author.is_empty(), |el| el.child(faint(format!("By {}", plugin.manifest.author), cx))),
                            )
                            .child(Switch::new(("plugin-on", i)).checked(enabled).on_click(cx.listener(move |this, checked: &bool, _, cx| {
                                this.plugins.send(PluginEvent::Enable(id.clone(), *checked));
                                cx.notify();
                            }))),
                    )
                    .child(
                        div().flex().gap_2().children(plugin.manifest.permissions.iter().map(|permission| {
                            div()
                                .px(px(8.))
                                .py(px(2.))
                                .rounded_full()
                                .border_1()
                                .border_color(p.line)
                                .text_size(px(11.5))
                                .text_color(p.ink_2)
                                .child(permission.describe())
                        })),
                    )
                    .when(plugin.manifest.permissions.is_empty(), |el| el.child(faint("Needs no permissions.", cx)))
                    .when(!enabled && !plugin.manifest.permissions.is_empty(), |el| el.child(faint("Turning it on allows everything listed above.", cx)))
                    .when(enabled && !plugin.effects.is_empty(), |el| el.child(faint(format!("Adds to Sound › Effects: {}.", plugin.effects.join(", ")), cx).w_full()))
                    .when_some(plugin.error.clone(), |el, error| el.child(meta(error, cx).w_full().text_color(p.danger)))
                    .when(enabled, |el| {
                        el.child(div().flex().gap_2().children(plugin.commands.iter().filter(|c| !c.for_tracks).enumerate().map(|(j, command)| {
                            let (plugin, command_id) = (command.plugin.clone(), command.id.clone());
                            small_button(SharedString::from(format!("plugin-run-{i}-{j}")), command.title.clone()).on_click(cx.listener(move |this, _, _, _| {
                                this.run_plugin_command(plugin.clone(), command_id.clone(), vec![]);
                            }))
                        })))
                    })
            }))
            .child(faint("Plugins are Rhai scripts in their own folders. Each one lists what it may do, and nothing runs until you turn it on. Learn to write one at needle.nnx.fyi/plugins.", cx).w_full())
    }
}
