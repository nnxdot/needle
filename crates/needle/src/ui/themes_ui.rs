//! Custom themes in the app: choosing one, the editor in Settings › Appearance, importing
//! (a file picker or dropping a theme file on the window), exporting, and noticing when a
//! theme file changes on disk.
use super::{
    AppView, pal, set_theme,
    theme::{self, Base, Palette},
    themes::{self, CustomTheme, Slot, Themes},
    widgets::{faint, meta, segmented, small_button, strong},
};
use gpui::{prelude::*, *};
use gpui_component::{
    Sizable,
    button::ButtonVariants,
    color_picker::{ColorPicker, ColorPickerEvent, ColorPickerState},
    input::{Input, InputEvent, InputState},
    switch::Switch,
};
use std::{path::Path, time::Instant};

/// The theme being edited, with a colour picker for each colour.
pub struct Editor {
    pub id: String,
    name: Entity<InputState>,
    author: Entity<InputState>,
    pickers: Vec<(Slot, Entity<ColorPickerState>)>,
    _subscriptions: Vec<Subscription>,
}

/// The built-in looks as settings values, in the order people see them.
const BUILT_IN: [&str; 3] = ["dark", "midnight", "light"];

fn all_themes(cx: &App) -> Themes {
    cx.try_global::<Themes>().cloned().unwrap_or_default()
}

impl AppView {
    /// Read every theme again (after a change Needle made itself).
    pub(super) fn reload_themes(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // Any look at the files that started before this reading is now out of date.
        self.themes_generation += 1;
        let themes = themes::load(&themes::sources(&self.library));
        self.use_themes(themes, window, cx);
    }

    /// A reading from `check_themes`: used only if nothing newer was read since it began.
    pub(super) fn themes_found(
        &mut self,
        generation: u64,
        themes: Themes,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if generation == self.themes_generation {
            self.use_themes(themes, window, cx);
        }
    }

    /// Use a fresh reading of the themes: re-apply the chosen one if it is custom, with its
    /// grain and title font.
    pub(super) fn use_themes(
        &mut self,
        themes: Themes,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        cx.set_global(themes);
        self.themes_checked = Instant::now();
        let mode = self.settings.theme.clone();
        if mode.starts_with("custom:") {
            set_theme(&mode, Some(window), cx);
            if self.apply_theme_extras(window, cx) {
                self.persist_settings();
            }
        }
        self.sync_theme_pickers(window, cx);
        cx.notify();
    }

    /// Called from the poll: notice theme files added, changed, or removed outside Needle.
    /// The files are read on another thread, so a slow disk never holds up the window.
    pub(super) fn check_themes(&mut self, cx: &mut Context<Self>) {
        if self.themes_checked.elapsed().as_millis() < 1500
            || self
                .themes_checking
                .swap(true, std::sync::atomic::Ordering::AcqRel)
        {
            return;
        }
        self.themes_checked = Instant::now();
        let (library, current, sender, busy, generation) = (
            self.library.clone(),
            all_themes(cx),
            self.sender.clone(),
            self.themes_checking.clone(),
            self.themes_generation,
        );
        std::thread::spawn(move || {
            let sources = themes::sources(&library);
            if themes::changed(&current, &sources) {
                let _ = sender.send(super::Event::Themes(
                    generation,
                    Box::new(themes::load(&sources)),
                ));
            }
            busy.store(false, std::sync::atomic::Ordering::Release);
        });
    }

    /// Give the chosen custom theme's grain and title font, keeping what was there before
    /// to put back later; or put that back when the theme has neither. Returns whether
    /// anything changed.
    /// At start: the chosen theme's grain and font as its file says now (it may have been
    /// edited while Needle was closed).
    pub(super) fn start_theme_extras(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.apply_theme_extras(window, cx) {
            self.persist_settings();
        }
    }

    fn apply_theme_extras(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let chosen = all_themes(cx).find(&self.settings.theme).cloned();
        let extras = chosen
            .as_ref()
            .filter(|t| t.grain.is_some() || t.font.is_some());
        let (grain, font) = match extras {
            None => match self.settings.theme_extras_before.take() {
                Some(before) => before,
                None => return false,
            },
            Some(theme) => {
                if self.settings.theme_extras_before.is_none() {
                    self.settings.theme_extras_before =
                        Some((self.settings.grain, self.settings.display_font.clone()));
                }
                (
                    theme.grain.unwrap_or(self.settings.grain),
                    theme
                        .font
                        .clone()
                        .unwrap_or_else(|| self.settings.display_font.clone()),
                )
            }
        };
        let changed = grain != self.settings.grain || font != self.settings.display_font;
        self.settings.grain = grain;
        self.grain_slider
            .update(cx, |s, cx| s.set_value(grain, window, cx));
        theme::set_display_font(&font);
        self.settings.display_font = font;
        changed
    }

    /// Switch to a look (a built-in one or `custom:<id>`). A custom theme's extras (grain,
    /// title font) are applied too.
    pub(super) fn choose_look(&mut self, mode: &str, window: &mut Window, cx: &mut Context<Self>) {
        set_theme(mode, Some(window), cx);
        self.settings.theme = mode.into();
        // A Delete clicked once is for the look that was showing; it must be clicked twice again.
        self.theme_delete_armed = None;
        // This theme's grain and font come in, or, when it has none, the person's own come
        // back (the ones kept when a theme first set its own).
        self.apply_theme_extras(window, cx);
        if self
            .theme_editor
            .as_ref()
            .is_some_and(|e| format!("custom:{}", e.id) != mode)
        {
            self.theme_editor = None;
        }
        self.persist_settings();
        cx.notify();
    }

    /// Every look to choose from: the built-in ones, then custom themes.
    pub(super) fn look_modes(&self, cx: &App) -> Vec<String> {
        BUILT_IN
            .iter()
            .map(|m| m.to_string())
            .chain(all_themes(cx).list.iter().map(CustomTheme::mode))
            .collect()
    }

    /// Whether the Color swatches change anything: a custom theme may set its own accent or
    /// keep its colours whatever is chosen.
    pub(super) fn accent_choice_applies(&self, cx: &App) -> bool {
        self.current_theme(cx)
            .is_none_or(|t| t.music_colors && !t.colors.contains_key(&Slot::Accent))
    }

    fn current_theme(&self, cx: &App) -> Option<CustomTheme> {
        all_themes(cx).find(&self.settings.theme).cloned()
    }

    /// A new theme copied from the look in use, chosen and opened in the editor.
    fn copy_look(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let folder = themes::folder(&self.library.directory);
        let mut copy = match self.current_theme(cx) {
            Some(current) => CustomTheme {
                name: format!("{} copy", current.name),
                read_only: false,
                ..current
            },
            None => {
                let base = Base::from_name(&self.settings.theme);
                let name = Base::ALL
                    .iter()
                    .find(|(mode, _, _)| Base::from_name(mode) == base)
                    .map_or("Night", |(_, name, _)| name);
                CustomTheme::new("", &format!("My {name}"), base, &folder)
            }
        };
        copy.id = themes::free_id(&folder, &copy.name);
        copy.path = folder.join(format!("{}.toml", copy.id));
        if let Err(error) = copy.save() {
            self.fail(format!("{error:#}"));
            return;
        }
        self.reload_themes(window, cx);
        self.choose_look(&copy.mode(), window, cx);
        self.open_theme_editor(&copy.id, window, cx);
    }

    pub(super) fn open_theme_editor(
        &mut self,
        id: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(theme) = all_themes(cx).list.into_iter().find(|t| t.id == id) else {
            return;
        };
        if theme.read_only {
            return;
        }
        let (look, _) = Palette::custom(&theme, None, false);
        let name = cx.new(|cx| {
            let mut state = InputState::new(window, cx).placeholder("Theme name");
            state.set_value(theme.name.clone(), window, cx);
            state
        });
        let author = cx.new(|cx| {
            let mut state = InputState::new(window, cx).placeholder("Your name (optional)");
            state.set_value(theme.author.clone(), window, cx);
            state
        });
        let mut subscriptions = vec![
            cx.subscribe_in(&name, window, |this, input, event, window, cx| {
                if let InputEvent::Change = event {
                    let value = input.read(cx).value().trim().to_string();
                    if !value.is_empty() {
                        this.edit_theme(window, cx, move |t| t.name = value);
                    }
                }
            }),
            cx.subscribe_in(&author, window, |this, input, event, window, cx| {
                if let InputEvent::Change = event {
                    let value = input.read(cx).value().trim().to_string();
                    this.edit_theme(window, cx, move |t| t.author = value);
                }
            }),
        ];
        let mut pickers = vec![];
        for slot in Slot::ALL {
            let picker =
                cx.new(|cx| ColorPickerState::new(window, cx).default_value(slot.get(&look)));
            subscriptions.push(cx.subscribe_in(
                &picker,
                window,
                move |this, _, event: &ColorPickerEvent, window, cx| {
                    if let ColorPickerEvent::Change(Some(color)) = event {
                        let color = *color;
                        this.edit_theme(window, cx, move |t| {
                            t.colors.insert(slot, color);
                        });
                    }
                },
            ));
            pickers.push((slot, picker));
        }
        self.theme_editor = Some(Editor {
            id: id.into(),
            name,
            author,
            pickers,
            _subscriptions: subscriptions,
        });
        cx.notify();
    }

    /// Change the theme being edited, save it, and show the change.
    fn edit_theme(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        change: impl FnOnce(&mut CustomTheme),
    ) {
        let Some(id) = self.theme_editor.as_ref().map(|e| e.id.clone()) else {
            return;
        };
        let Some(mut theme) = all_themes(cx).list.into_iter().find(|t| t.id == id) else {
            return;
        };
        change(&mut theme);
        if let Err(error) = theme.save() {
            self.fail(format!("{error:#}"));
            return;
        }
        self.reload_themes(window, cx);
    }

    /// Show the colours the theme leaves out as they now look.
    fn sync_theme_pickers(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(editor) = &self.theme_editor else {
            return;
        };
        let Some(theme) = all_themes(cx).list.into_iter().find(|t| t.id == editor.id) else {
            self.theme_editor = None;
            return;
        };
        let (look, _) = Palette::custom(&theme, None, false);
        // Every swatch, including set ones: the file may have been changed elsewhere.
        for (slot, picker) in &editor.pickers {
            let color = slot.get(&look);
            picker.update(cx, |p, cx| {
                if p.value().map(themes::hex) != Some(themes::hex(color)) {
                    p.set_value(color, window, cx);
                }
            });
        }
    }

    fn delete_theme(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(theme) = all_themes(cx).list.into_iter().find(|t| t.id == id) else {
            return;
        };
        if theme.read_only {
            return;
        }
        if let Err(error) = std::fs::remove_file(&theme.path) {
            self.fail(format!("Could not delete the theme: {error}"));
            return;
        }
        self.theme_delete_armed = None;
        if self.theme_editor.as_ref().is_some_and(|e| e.id == id) {
            self.theme_editor = None;
        }
        self.reload_themes(window, cx);
        if self.settings.theme == theme.mode() {
            self.choose_look(themes::base_key(theme.base), window, cx);
        }
        self.notify(format!("Deleted the theme {}.", theme.name));
    }

    /// Add a theme file to the theme folder and switch to it.
    pub(super) fn import_theme(
        &mut self,
        path: &Path,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let result = (|| -> anyhow::Result<CustomTheme> {
            // Refuses big files before reading them, so this stays quick.
            let text = themes::read_theme_file(path)?;
            let folder = themes::folder(&self.library.directory);
            let probe = CustomTheme::parse(&text, "import", path)?;
            let id = themes::free_id(&folder, &probe.name);
            std::fs::create_dir_all(&folder)?;
            let target = folder.join(format!("{id}.toml"));
            // Claim the name: an import never replaces a theme another one just saved.
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&target)?;
            std::io::Write::write_all(&mut file, text.as_bytes())?;
            CustomTheme::parse(&text, &id, &target)
        })();
        match result {
            Ok(theme) => {
                self.reload_themes(window, cx);
                self.choose_look(&theme.mode(), window, cx);
                self.notify(format!("Added the theme {}.", theme.name));
            }
            Err(error) => self.fail(format!(
                "{} is not a Needle theme: {error:#}",
                path.file_name().unwrap_or_default().to_string_lossy()
            )),
        }
    }

    fn pick_theme_file(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.can_pick(cx) {
            return;
        }
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Add theme".into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(paths))) = paths.await else {
                return;
            };
            let _ = this.update_in(cx, |this, window, cx| {
                for path in paths {
                    this.import_theme(&path, window, cx);
                }
            });
        })
        .detach();
    }

    fn export_theme(&mut self, theme: CustomTheme, cx: &mut Context<Self>) {
        let downloads = std::env::var_os("USERPROFILE")
            .map(|home| Path::new(&home).join("Downloads"))
            .filter(|d| d.is_dir())
            .unwrap_or_else(|| themes::folder(&self.library.directory));
        let target = cx.prompt_for_new_path(
            &downloads,
            Some(&format!("{}.toml", themes::slug(&theme.name))),
        );
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(path))) = target.await else {
                return;
            };
            let result = std::fs::write(&path, theme.to_toml());
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(()) => this.notify(format!(
                        "Saved {}. Send it to a friend, or drop it on Needle to add it.",
                        path.display()
                    )),
                    Err(error) => this.fail(format!("Could not save the theme: {error}")),
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Buttons under the looks: copy, edit, share, and theme files that could not be read.
    pub(super) fn theme_actions(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let current = self.current_theme(cx);
        let problems = all_themes(cx).problems;
        let editing = self.theme_editor.is_some();
        let armed = current
            .as_ref()
            .is_some_and(|t| self.theme_delete_armed.as_deref() == Some(t.id.as_str()));
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        small_button("theme-copy", "Make a copy")
                            .tooltip("A new theme of your own, starting from this look")
                            .on_click(
                                cx.listener(|this, _, window, cx| this.copy_look(window, cx)),
                            ),
                    )
                    .when_some(
                        current.clone().filter(|t| !t.read_only && !editing),
                        |el, t| {
                            el.child(small_button("theme-edit", "Edit colors").ghost().on_click(
                                cx.listener(move |this, _, window, cx| {
                                    this.open_theme_editor(&t.id, window, cx)
                                }),
                            ))
                        },
                    )
                    .when_some(current.clone(), |el, t| {
                        el.child(small_button("theme-export", "Export…").ghost().on_click(
                            cx.listener(move |this, _, _, cx| this.export_theme(t.clone(), cx)),
                        ))
                    })
                    .when_some(current.clone().filter(|t| !t.read_only), |el, t| {
                        let button = small_button(
                            "theme-delete",
                            if armed {
                                "Click again to delete"
                            } else {
                                "Delete"
                            },
                        )
                        .ghost();
                        el.child(if armed { button.danger() } else { button }.on_click(
                            cx.listener(move |this, _, window, cx| {
                                if this.theme_delete_armed.as_deref() == Some(t.id.as_str()) {
                                    this.delete_theme(&t.id, window, cx);
                                } else {
                                    this.theme_delete_armed = Some(t.id.clone());
                                    cx.notify();
                                }
                            }),
                        ))
                    })
                    .child(div().flex_1())
                    .child(
                        small_button("theme-import", "Add a theme…")
                            .ghost()
                            .tooltip("Add a theme file. You can also drop one on the window.")
                            .on_click(
                                cx.listener(|this, _, window, cx| this.pick_theme_file(window, cx)),
                            ),
                    )
                    .child(
                        small_button("theme-folder", "Open themes folder")
                            .ghost()
                            .on_click(cx.listener(|this, _, _, _| {
                                let folder = themes::folder(&this.library.directory);
                                let _ = std::fs::create_dir_all(&folder);
                                let _ = std::process::Command::new("explorer").arg(folder).spawn();
                            })),
                    ),
            )
            .when_some(current.filter(|t| t.read_only), |el, t| {
                el.child(faint(
                    format!(
                        "{} comes with the plugin {}. Make a copy to change it.",
                        t.name,
                        t.id.split('/').next().unwrap_or_default()
                    ),
                    cx,
                ))
            })
            .children(problems.into_iter().map(|(file, problem)| {
                faint(format!("{file} could not be read: {problem}"), cx).text_color(pal(cx).danger)
            }))
    }

    /// The editor for the theme being changed, under the looks.
    pub(super) fn theme_editor_view(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let editor = self.theme_editor.as_ref()?;
        let theme = all_themes(cx)
            .list
            .into_iter()
            .find(|t| t.id == editor.id)?;
        let p = pal(cx);
        let (_, adjusted) = Palette::custom(&theme, None, false);
        let weak = cx.entity().downgrade();
        let base_index = [Base::Night, Base::Midnight, Base::Day]
            .iter()
            .position(|b| *b == theme.base)
            .unwrap_or(0);
        // One line per color, each the same height: wrapped text in two columns was measured
        // as one line, so the list spilled over the settings below it.
        let rows: Vec<_> = editor
            .pickers
            .iter()
            .map(|(slot, picker)| {
                let slot = *slot;
                let own = theme.colors.contains_key(&slot);
                let moved = adjusted.contains(&slot);
                div()
                    .h(px(34.))
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(ColorPicker::new(picker).small().flex_none())
                    .child(
                        div()
                            .w(px(150.))
                            .flex_none()
                            .text_size(px(13.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .truncate()
                            .child(slot.name()),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_size(px(12.))
                            .truncate()
                            .text_color(if moved { p.accent } else { p.ink_3 })
                            .child(if moved {
                                "Moved a little so text stays readable".to_string()
                            } else {
                                slot.about().to_string()
                            }),
                    )
                    .when(own, |el| {
                        el.child(
                            small_button(
                                SharedString::from(format!("theme-reset-{}", slot.key())),
                                "Reset",
                            )
                            .ghost()
                            .flex_none()
                            .tooltip("Let this color follow the base look and the others")
                            .on_click(cx.listener(
                                move |this, _, window, cx| {
                                    this.edit_theme(window, cx, move |t| {
                                        t.colors.remove(&slot);
                                    });
                                },
                            )),
                        )
                    })
            })
            .collect();
        let grid = div().flex().flex_col().children(rows);
        Some(
            div()
                .mt_3()
                .p_4()
                .rounded(px(10.))
                .border_1()
                .border_color(p.line)
                .flex()
                .flex_col()
                .gap_3()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_3()
                        .child(strong(format!("Editing {}", theme.name)))
                        .child(div().flex_1())
                        .child(
                            small_button("theme-editor-file", "Open the file")
                                .ghost()
                                .on_click({
                                    let path = theme.path.clone();
                                    move |_, _, _| {
                                        let _ = std::process::Command::new("explorer")
                                            .arg("/select,")
                                            .arg(&path)
                                            .spawn();
                                    }
                                }),
                        )
                        .child(small_button("theme-editor-done", "Done").on_click(cx.listener(
                            |this, _, _, cx| {
                                this.theme_editor = None;
                                cx.notify();
                            },
                        ))),
                )
                .child(meta("Changes show straight away and are saved as you go. Colors you leave alone follow the base look and the colors you set.", cx).w_full())
                .child(
                    div()
                        .flex()
                        .gap_3()
                        .child(Input::new(&editor.name).small().flex_1())
                        .child(Input::new(&editor.author).small().flex_1()),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_4()
                        .child(strong("Starts from"))
                        .child(segmented(
                            "theme-base",
                            &["Night", "Midnight", "Day"],
                            base_index,
                            cx,
                            {
                                let weak = weak.clone();
                                move |index, window, cx| {
                                    let base = [Base::Night, Base::Midnight, Base::Day][index];
                                    let _ = weak.update(cx, |this, cx| {
                                        this.edit_theme(window, cx, move |t| t.base = base);
                                    });
                                }
                            },
                        ))
                        .child(div().flex_1())
                        .child(strong("Colors from the music"))
                        .child(
                            Switch::new("theme-music")
                                .checked(theme.music_colors)
                                .tooltip("Let the cover that is playing tint the colors this theme leaves alone")
                                .on_click(cx.listener(|this, checked: &bool, window, cx| {
                                    let on = *checked;
                                    this.edit_theme(window, cx, move |t| t.music_colors = on);
                                })),
                        ),
                )
                .child(grid)
                .into_any_element(),
        )
    }
}
