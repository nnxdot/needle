//! The playlist window: make a playlist or change one. A name, a few words about it, and a
//! cover (a picture, or its songs' covers); then the songs: picked one by one, or a smart
//! playlist built from rules chosen in menus, with the songs that match counted as it changes.
use super::{
    AppView, Page, pal,
    widgets::{
        artwork, cover, display, faint, glyph, icon_button, meta, segmented, small_button,
        track_seed,
    },
};
use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme, IndexPath, Sizable,
    button::{Button, ButtonVariants},
    input::{Input, InputEvent, InputState},
    select::{Select, SelectEvent, SelectState},
};
use needle_core::{
    model::{Playlist, Rule, RuleSet, Track},
    rules::{self, KINDS, ORDERS},
};

type Choice = SelectState<Vec<&'static str>>;

/// One rule's row: what it looks at, how, and the value.
pub struct RuleRow {
    field: Entity<Choice>,
    op: Entity<Choice>,
    value: Entity<InputState>,
    _subscriptions: Vec<Subscription>,
}

/// The playlist window while it is open.
pub struct PlaylistEditor {
    /// The playlist being changed; `None` for a new one.
    editing: Option<Playlist>,
    name: Entity<InputState>,
    description: Entity<InputState>,
    smart: bool,
    /// A picture chosen in this window, not yet copied into Needle's artwork folder.
    picked_cover: Option<String>,
    /// The playlist's own picture, if it has one and it is kept.
    cover: Option<String>,
    /// The songs of a playlist of picked songs.
    track_ids: Vec<String>,
    /// Rules made in the builder, or a rule typed by hand (`typed`).
    rows: Vec<RuleRow>,
    any: bool,
    order: Entity<Choice>,
    limit: Entity<InputState>,
    typed: Option<Entity<InputState>>,
    /// How many songs match, and the first few (for the cover), or why the rule does not work.
    count: Option<Result<usize, String>>,
    preview: Vec<Track>,
    generation: u64,
    _subscriptions: Vec<Subscription>,
}

impl PlaylistEditor {
    /// Start with this name.
    pub fn set_name(&self, name: String, window: &mut Window, cx: &mut App) {
        self.name.update(cx, |s, cx| s.set_value(name, window, cx));
    }
}

fn choice(
    items: Vec<&'static str>,
    selected: &str,
    window: &mut Window,
    cx: &mut Context<AppView>,
) -> Entity<Choice> {
    let index = items.iter().position(|i| *i == selected).unwrap_or(0);
    cx.new(|cx| SelectState::new(items, Some(IndexPath::new(index)), window, cx))
}

fn selected(choice: &Entity<Choice>, cx: &App) -> String {
    choice
        .read(cx)
        .selected_value()
        .map(|v| v.to_string())
        .unwrap_or_default()
}

fn placeholder(field: &str) -> &'static str {
    match rules::kind(field).map(|k| k.value) {
        Some(rules::Value::Text(p) | rules::Value::Number(p)) => p,
        _ => "",
    }
}

impl AppView {
    /// Open the playlist window: for `editing`, or for a new playlist of `tracks` (smart, with
    /// `typed_rule`, when that is given).
    pub(super) fn open_playlist_editor(
        &mut self,
        editing: Option<Playlist>,
        tracks: Vec<String>,
        typed_rule: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let name = cx.new(|cx| InputState::new(window, cx).placeholder("Name your playlist"));
        let description =
            cx.new(|cx| InputState::new(window, cx).placeholder("Add a description (optional)"));
        let limit = cx.new(|cx| InputState::new(window, cx).placeholder("No limit"));
        let set = editing.as_ref().and_then(|p| p.rules.clone());
        let typed_rule = typed_rule.or_else(|| {
            editing
                .as_ref()
                .filter(|p| p.rules.is_none())
                .and_then(|p| p.query.clone())
        });
        if let Some(playlist) = &editing {
            name.update(cx, |s, cx| s.set_value(playlist.name.clone(), window, cx));
            description.update(cx, |s, cx| {
                s.set_value(playlist.description.clone(), window, cx)
            });
        }
        if let Some(limit_value) = set.as_ref().and_then(|s| s.limit) {
            limit.update(cx, |s, cx| s.set_value(limit_value.to_string(), window, cx));
        }
        let order = choice(
            ORDERS.iter().map(|(label, _)| *label).collect(),
            set.as_ref().map_or("", |s| s.order.as_str()),
            window,
            cx,
        );
        let typed = typed_rule.map(|rule| {
            cx.new(|cx| {
                let mut state =
                    InputState::new(window, cx).placeholder("rating >= 4 and not played(30d)");
                state.set_value(rule, window, cx);
                state
            })
        });
        let smart = editing
            .as_ref()
            .map_or(typed.is_some(), |p| p.query.is_some());
        let mut subscriptions = vec![
            cx.subscribe_in(&limit, window, |this, _, event: &InputEvent, _, cx| {
                if matches!(event, InputEvent::Change) {
                    this.editor_changed(cx);
                }
            }),
            cx.subscribe_in(
                &order,
                window,
                |this, _, _: &SelectEvent<Vec<&'static str>>, _, cx| {
                    this.editor_changed(cx);
                },
            ),
        ];
        if let Some(typed) = &typed {
            subscriptions.push(cx.subscribe_in(
                typed,
                window,
                |this, _, event: &InputEvent, _, cx| {
                    if matches!(event, InputEvent::Change) {
                        this.editor_changed(cx);
                    }
                },
            ));
        }
        let rows = match &set {
            Some(set) => set
                .rules
                .iter()
                .map(|rule| self.rule_row(rule, window, cx))
                .collect(),
            None => vec![self.rule_row(
                &Rule {
                    field: "Rating".into(),
                    op: "is at least".into(),
                    value: "4".into(),
                },
                window,
                cx,
            )],
        };
        name.update(cx, |s, cx| s.focus(window, cx));
        self.editor = Some(PlaylistEditor {
            track_ids: editing.as_ref().map_or(tracks, |p| p.track_ids.clone()),
            cover: editing.as_ref().and_then(|p| p.cover.clone()),
            editing,
            name,
            description,
            smart,
            picked_cover: None,
            rows,
            any: set.as_ref().is_some_and(|s| s.any),
            order,
            limit,
            typed,
            count: None,
            preview: vec![],
            generation: 0,
            _subscriptions: subscriptions,
        });
        self.editor_changed(cx);
        cx.notify();
    }

    fn rule_row(&mut self, rule: &Rule, window: &mut Window, cx: &mut Context<Self>) -> RuleRow {
        let field = choice(
            KINDS.iter().map(|k| k.label).collect(),
            &rule.field,
            window,
            cx,
        );
        let kind = rules::kind(&rule.field).unwrap_or(&KINDS[0]);
        let op = choice(
            kind.ops.iter().map(|(l, _)| *l).collect(),
            &rule.op,
            window,
            cx,
        );
        let value = cx.new(|cx| {
            let mut state = InputState::new(window, cx).placeholder(placeholder(kind.label));
            state.set_value(rule.value.clone(), window, cx);
            state
        });
        let subscriptions = vec![
            cx.subscribe_in(&field, window, {
                let field = field.clone();
                move |this, _, _: &SelectEvent<Vec<&'static str>>, window, cx| {
                    this.field_changed(&field, window, cx);
                }
            }),
            cx.subscribe_in(
                &op,
                window,
                |this, _, _: &SelectEvent<Vec<&'static str>>, _, cx| {
                    this.editor_changed(cx);
                },
            ),
            cx.subscribe_in(&value, window, |this, _, event: &InputEvent, _, cx| {
                if matches!(event, InputEvent::Change) {
                    this.editor_changed(cx);
                }
            }),
        ];
        RuleRow {
            field,
            op,
            value,
            _subscriptions: subscriptions,
        }
    }

    /// A rule now looks at something else: offer its ways to compare, and its kind of value.
    fn field_changed(
        &mut self,
        field: &Entity<Choice>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let label = selected(field, cx);
        let Some(kind) = rules::kind(&label) else {
            return;
        };
        let op = choice(kind.ops.iter().map(|(l, _)| *l).collect(), "", window, cx);
        let op_subscription = cx.subscribe_in(
            &op,
            window,
            |this, _, _: &SelectEvent<Vec<&'static str>>, _, cx| {
                this.editor_changed(cx);
            },
        );
        if let Some(editor) = self.editor.as_mut()
            && let Some(row) = editor.rows.iter_mut().find(|r| &r.field == field)
        {
            row.op = op;
            row._subscriptions.push(op_subscription);
            row.value.update(cx, |s, cx| {
                s.set_placeholder(placeholder(kind.label), window, cx);
                if kind.value == rules::Value::None {
                    s.set_value("", window, cx);
                }
            });
        }
        self.editor_changed(cx);
    }

    /// The rules as they stand in the window.
    fn editor_rules(&self, cx: &App) -> Option<RuleSet> {
        let editor = self.editor.as_ref()?;
        Some(RuleSet {
            any: editor.any,
            rules: editor
                .rows
                .iter()
                .map(|row| Rule {
                    field: selected(&row.field, cx),
                    op: selected(&row.op, cx),
                    value: row.value.read(cx).value().to_string(),
                })
                .collect(),
            order: selected(&editor.order, cx),
            limit: editor.limit.read(cx).value().trim().parse().ok(),
        })
    }

    /// The smart playlist's rule as it stands, or what is wrong with it.
    fn editor_query(&self, cx: &App) -> Result<String, String> {
        let editor = self.editor.as_ref().ok_or_else(String::new)?;
        let query = match &editor.typed {
            Some(typed) => typed.read(cx).value().trim().to_string(),
            None => {
                let set = self.editor_rules(cx).unwrap_or_default();
                rules::to_query(&set).map_err(|e| e.to_string())?
            }
        };
        if query.is_empty() {
            return Err("Type a rule, such as  rating >= 4".into());
        }
        needle_core::query::compile(&query, chrono::Utc::now().timestamp())
            .map_err(|e| e.to_string())?;
        Ok(query)
    }

    /// Something changed: count the songs that match (or are picked) again, off the window's
    /// thread.
    fn editor_changed(&mut self, cx: &mut Context<Self>) {
        let query = self.editor_query(cx);
        let Some(editor) = self.editor.as_mut() else {
            return;
        };
        editor.generation += 1;
        let generation = editor.generation;
        let library = self.library.clone();
        let job: Box<dyn FnOnce() -> Result<Vec<Track>, String> + Send> = if editor.smart {
            match query {
                Ok(query) => Box::new(move || library.search(&query).map_err(|e| format!("{e:#}"))),
                Err(problem) => {
                    editor.count = Some(Err(problem));
                    editor.preview.clear();
                    cx.notify();
                    return;
                }
            }
        } else {
            let ids = editor.track_ids.clone();
            Box::new(move || library.tracks_by_ids(&ids).map_err(|e| format!("{e:#}")))
        };
        cx.spawn(async move |this, cx| {
            let found = cx.background_executor().spawn(async move { job() }).await;
            let _ = this.update(cx, |this, cx| {
                if let Some(editor) = this.editor.as_mut()
                    && editor.generation == generation
                {
                    match found {
                        Ok(tracks) => {
                            editor.count = Some(Ok(tracks.len()));
                            editor.preview = tracks.into_iter().take(12).collect();
                        }
                        Err(problem) => editor.count = Some(Err(problem)),
                    }
                    cx.notify();
                }
            });
        })
        .detach();
    }

    fn save_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let query = self.editor_query(cx);
        let set = self.editor_rules(cx);
        let Some(editor) = self.editor.as_ref() else {
            return;
        };
        let name = editor.name.read(cx).value().trim().to_string();
        if name.is_empty() {
            return self.fail("Give the playlist a name.");
        }
        let mut playlist = editor.editing.clone().unwrap_or_else(|| Playlist {
            id: crate::uuid_string(),
            track_ids: editor.track_ids.clone(),
            ..Default::default()
        });
        playlist.name = name;
        playlist.description = editor.description.read(cx).value().trim().to_string();
        playlist.updated_at = chrono::Utc::now().timestamp();
        if editor.smart {
            match query {
                Ok(query) => {
                    playlist.query = Some(query);
                    playlist.rules = if editor.typed.is_some() { None } else { set };
                    playlist.track_ids.clear();
                }
                Err(problem) => return self.fail(problem),
            }
        } else {
            playlist.query = None;
            playlist.rules = None;
        }
        // A new picture: keep a copy with Needle's artwork, so moving the original is fine.
        playlist.cover = editor.cover.clone();
        if let Some(picked) = &editor.picked_cover {
            let folder = self.library.directory.join("artwork").join("playlists");
            let extension = std::path::Path::new(picked)
                .extension()
                .map(|e| e.to_string_lossy().to_lowercase())
                .unwrap_or_else(|| "img".into());
            let copy = folder.join(format!(
                "{}-{}.{extension}",
                playlist.id, playlist.updated_at
            ));
            match std::fs::create_dir_all(&folder).and_then(|_| std::fs::copy(picked, &copy)) {
                Ok(_) => playlist.cover = Some(copy.to_string_lossy().into()),
                Err(error) => return self.fail(format!("Could not use that picture: {error}")),
            }
        }
        let new = editor.editing.is_none();
        match self.library.save_playlist(&playlist) {
            Ok(()) => {
                self.playlists = self.library.playlists().unwrap_or_default();
                self.editor = None;
                self.notify(if new {
                    format!("Made “{}”.", playlist.name)
                } else {
                    format!("Saved “{}”.", playlist.name)
                });
                self.navigate(Page::Playlist(playlist.id.clone()), window, cx);
                self.refresh(cx);
            }
            Err(error) => self.fail(error.to_string()),
        }
        cx.notify();
    }

    /// A playlist's cover: its own picture, or up to four of its songs' covers.
    pub(super) fn playlist_cover(
        &self,
        picture: Option<&str>,
        tracks: &[Track],
        size: f32,
        cx: &App,
    ) -> AnyElement {
        if let Some(picture) = picture {
            return cover(Some(picture), "", size, cx);
        }
        // Different albums first, so the four squares differ.
        let mut seen = std::collections::HashSet::new();
        let covers: Vec<&Track> = tracks
            .iter()
            .filter(|t| t.artwork.is_some() && seen.insert(track_seed(t)))
            .take(4)
            .collect();
        if covers.len() < 4 {
            return artwork(covers.first().copied().or(tracks.first()), size, cx);
        }
        let half = size / 2.;
        div()
            .size(px(size))
            .rounded(px((size * 0.06).clamp(3., 8.)))
            .overflow_hidden()
            .flex()
            .flex_col()
            .child(
                div()
                    .flex()
                    .child(cover(covers[0].artwork.as_deref(), "", half, cx))
                    .child(cover(covers[1].artwork.as_deref(), "", half, cx)),
            )
            .child(
                div()
                    .flex()
                    .child(cover(covers[2].artwork.as_deref(), "", half, cx))
                    .child(cover(covers[3].artwork.as_deref(), "", half, cx)),
            )
            .into_any_element()
    }

    /// The window, over everything else, while it is open.
    pub(super) fn playlist_editor_view(
        &self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<impl IntoElement> {
        let editor = self.editor.as_ref()?;
        let p = pal(cx);
        let room = super::widgets::content_size(window);
        let card_w = f32::from(room.width).min(660.) - 48.;
        let card_h = f32::from(room.height) - 64.;
        const PAD: f32 = 32.;
        let inner = card_w - PAD * 2.;
        let new = editor.editing.is_none();
        let shown_cover = editor.picked_cover.as_deref().or(editor.cover.as_deref());

        // The top: cover, name, description.
        let cover_size = 132.;
        let fields_w = inner - cover_size - 24.;
        let top = div()
            .flex()
            .gap(px(24.))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .rounded(px(10.))
                            .shadow_md()
                            .child(self.playlist_cover(
                                shown_cover,
                                &editor.preview,
                                cover_size,
                                cx,
                            )),
                    )
                    .child(
                        small_button("editor-picture", "Choose picture…")
                            .ghost()
                            .on_click(cx.listener(|this, _, _, cx| this.pick_playlist_picture(cx))),
                    )
                    .when(shown_cover.is_some(), |el| {
                        el.child(
                            small_button("editor-song-covers", "Use song covers")
                                .ghost()
                                .on_click(cx.listener(|this, _, _, cx| {
                                    if let Some(editor) = this.editor.as_mut() {
                                        editor.picked_cover = None;
                                        editor.cover = None;
                                    }
                                    cx.notify();
                                })),
                        )
                    }),
            )
            .child(
                div()
                    .w(px(fields_w))
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(display(
                        if new { "New playlist" } else { "Edit playlist" },
                        26.,
                    ))
                    .child(Input::new(&editor.name))
                    .child(Input::new(&editor.description))
                    .when(new, |el| {
                        let weak = cx.entity().downgrade();
                        el.child(div().flex().child(segmented(
                            "editor-kind",
                            &["Songs I pick", "Smart playlist"],
                            usize::from(editor.smart),
                            cx,
                            move |index, _, cx| {
                                let _ = weak.update(cx, |this, cx| {
                                    if let Some(editor) = this.editor.as_mut() {
                                        editor.smart = index == 1;
                                    }
                                    this.editor_changed(cx);
                                });
                            },
                        )))
                    }),
            );

        // The songs: rules for a smart playlist, or a word about picked songs.
        let songs: AnyElement = if !editor.smart {
            let count = editor.track_ids.len();
            div()
                .w(px(inner))
                .p_4()
                .rounded(px(10.))
                .bg(p.ink.opacity(if p.dark { 0.04 } else { 0.03 }))
                .child(
                    meta(
                        if count == 0 {
                            "An empty playlist. Add songs from any song's menu (Add to playlist), or drag them onto it in the sidebar.".to_string()
                        } else {
                            format!(
                                "{count} {}. Add more from any song's menu, or drag them onto it in the sidebar.",
                                if count == 1 { "song" } else { "songs" }
                            )
                        },
                        cx,
                    )
                    .w(px(inner - 32.))
                    .line_height(relative(1.5)),
                )
                .into_any_element()
        } else if let Some(typed) = &editor.typed {
            div()
                .w(px(inner))
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    div()
                        .text_size(px(14.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("Rule"),
                )
                .child(Input::new(typed))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(faint(
                            "A rule typed by hand, in the same language as the search box.",
                            cx,
                        ))
                        .child(div().flex_1())
                        .child(
                            small_button("editor-builder", "Build it with menus instead")
                                .ghost()
                                .on_click(cx.listener(|this, _, window, cx| {
                                    let row = this.rule_row(
                                        &Rule {
                                            field: "Rating".into(),
                                            op: "is at least".into(),
                                            value: "4".into(),
                                        },
                                        window,
                                        cx,
                                    );
                                    if let Some(editor) = this.editor.as_mut() {
                                        editor.typed = None;
                                        editor.rows = vec![row];
                                    }
                                    this.editor_changed(cx);
                                })),
                        ),
                )
                .into_any_element()
        } else {
            let value_w = inner - 150. - 180. - 32. - 24.;
            let weak = cx.entity().downgrade();
            div()
                .w(px(inner))
                .flex()
                .flex_col()
                .gap_3()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_3()
                        .child(
                            div()
                                .text_size(px(14.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child("Songs that match"),
                        )
                        .child(segmented(
                            "editor-any",
                            &["all rules", "any rule"],
                            usize::from(editor.any),
                            cx,
                            move |index, _, cx| {
                                let _ = weak.update(cx, |this, cx| {
                                    if let Some(editor) = this.editor.as_mut() {
                                        editor.any = index == 1;
                                    }
                                    this.editor_changed(cx);
                                });
                            },
                        )),
                )
                .children(editor.rows.iter().enumerate().map(|(i, row)| {
                    let field = selected(&row.field, cx);
                    let takes_value =
                        rules::kind(&field).is_some_and(|k| k.value != rules::Value::None);
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(div().w(px(150.)).child(Select::new(&row.field).small()))
                        .child(div().w(px(180.)).child(Select::new(&row.op).small()))
                        .child(
                            div()
                                .w(px(value_w))
                                .when(takes_value, |el| el.child(Input::new(&row.value).small())),
                        )
                        .child(
                            icon_button(("editor-remove-rule", i), "close", "Remove this rule")
                                .small()
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    if let Some(editor) = this.editor.as_mut()
                                        && editor.rows.len() > 1
                                    {
                                        editor.rows.remove(i);
                                    }
                                    this.editor_changed(cx);
                                })),
                        )
                }))
                .child(
                    div().flex().child(
                        small_button("editor-add-rule", "Add a rule")
                            .ghost()
                            .icon(super::widgets::icon("plus"))
                            .on_click(cx.listener(|this, _, window, cx| {
                                let row = this.rule_row(
                                    &Rule {
                                        field: "Genre".into(),
                                        op: "is".into(),
                                        value: String::new(),
                                    },
                                    window,
                                    cx,
                                );
                                if let Some(editor) = this.editor.as_mut() {
                                    editor.rows.push(row);
                                }
                                this.editor_changed(cx);
                            })),
                    ),
                )
                .child(
                    div()
                        .pt_2()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(meta("Order", cx))
                        .child(div().w(px(200.)).child(Select::new(&editor.order).small()))
                        .child(div().w_4())
                        .child(meta("At most", cx))
                        .child(div().w(px(90.)).child(Input::new(&editor.limit).small()))
                        .child(meta("songs", cx)),
                )
                .into_any_element()
        };

        // How many songs match, as the rules change.
        let status = match &editor.count {
            None => faint("Counting…", cx).into_any_element(),
            Some(Ok(n)) => div()
                .flex()
                .items_center()
                .gap_2()
                .child(glyph("check").size(px(15.)).text_color(p.accent))
                .child(
                    div()
                        .text_size(px(13.))
                        .text_color(p.ink_2)
                        .child(if *n == 1 {
                            "1 song".to_string()
                        } else {
                            format!("{n} songs")
                        }),
                )
                .into_any_element(),
            Some(Err(problem)) => meta(problem.clone(), cx)
                .w(px(inner - 200.))
                .text_color(p.danger)
                .into_any_element(),
        };

        let card =
            div()
                .id("playlist-editor")
                .occlude()
                .w(px(card_w))
                .max_h(px(card_h))
                .rounded(px(16.))
                .bg(cx.theme().popover)
                .border_1()
                .border_color(p.line)
                .shadow_lg()
                .overflow_hidden()
                .flex()
                .flex_col()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    div()
                        .id("playlist-editor-body")
                        .flex_1()
                        .min_h_0()
                        .overflow_y_scroll()
                        .p(px(PAD))
                        .flex()
                        .flex_col()
                        .gap(px(26.))
                        .child(top)
                        .child(songs),
                )
                .child(
                    div()
                        .flex_shrink_0()
                        .px(px(PAD))
                        .py_4()
                        .border_t_1()
                        .border_color(p.line_soft)
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(status)
                        .child(div().flex_1())
                        .child(small_button("editor-cancel", "Cancel").ghost().on_click(
                            cx.listener(|this, _, _, cx| {
                                this.editor = None;
                                cx.notify();
                            }),
                        ))
                        .child(
                            Button::new("editor-save")
                                .primary()
                                .label(if new { "Make playlist" } else { "Save" })
                                .px_5()
                                .on_click(
                                    cx.listener(|this, _, window, cx| this.save_editor(window, cx)),
                                ),
                        ),
                );
        Some(
            deferred(
                div()
                    .id("playlist-editor-backdrop")
                    .absolute()
                    .inset_0()
                    .occlude()
                    .bg(gpui::black().opacity(if p.dark { 0.55 } else { 0.3 }))
                    .flex()
                    .justify_center()
                    .items_center()
                    .child(card),
            )
            .with_priority(2),
        )
    }

    /// Choose a picture for the playlist.
    fn pick_playlist_picture(&mut self, cx: &mut Context<Self>) {
        if !self.can_pick(cx) {
            return;
        }
        let Some(path) = rfd::FileDialog::new()
            .set_title("Choose a picture for the playlist")
            .add_filter("Pictures", &["png", "jpg", "jpeg", "webp", "gif", "bmp"])
            .pick_file()
        else {
            return;
        };
        if let Some(editor) = self.editor.as_mut() {
            editor.picked_cover = Some(path.to_string_lossy().into());
        }
        cx.notify();
    }
}

/// The "Add songs" window of a playlist of picked songs: search the library, add with +.
pub struct AddSongs {
    playlist: String,
    input: Entity<InputState>,
    results: Vec<Track>,
    generation: u64,
    _subscription: Subscription,
}

impl AppView {
    pub(super) fn open_add_songs(
        &mut self,
        playlist: &Playlist,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let input = cx
            .new(|cx| InputState::new(window, cx).placeholder("Search your songs, or type a rule"));
        input.update(cx, |s, cx| s.focus(window, cx));
        let subscription = cx.subscribe_in(&input, window, |this, _, event: &InputEvent, _, cx| {
            if matches!(event, InputEvent::Change) {
                this.search_to_add(cx);
            }
        });
        self.add_songs = Some(AddSongs {
            playlist: playlist.id.clone(),
            input,
            results: vec![],
            generation: 0,
            _subscription: subscription,
        });
        self.search_to_add(cx);
        cx.notify();
    }

    fn search_to_add(&mut self, cx: &mut Context<Self>) {
        let Some(add) = self.add_songs.as_mut() else {
            return;
        };
        add.generation += 1;
        let generation = add.generation;
        let text = add.input.read(cx).value().trim().to_string();
        let library = self.library.clone();
        cx.spawn(async move |this, cx| {
            let found = cx
                .background_executor()
                .spawn(async move { library.search(&text).unwrap_or_default() })
                .await;
            let _ = this.update(cx, |this, cx| {
                if let Some(add) = this.add_songs.as_mut()
                    && add.generation == generation
                {
                    add.results = found.into_iter().take(100).collect();
                    cx.notify();
                }
            });
        })
        .detach();
    }

    pub(super) fn add_songs_view(
        &self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<impl IntoElement> {
        let add = self.add_songs.as_ref()?;
        let playlist = self.playlists.iter().find(|p| p.id == add.playlist)?;
        let p = pal(cx);
        let room = super::widgets::content_size(window);
        let card_w = f32::from(room.width).min(600.) - 48.;
        let card_h = (f32::from(room.height) - 64.).min(640.);
        let inside: std::collections::HashSet<&String> = playlist.track_ids.iter().collect();
        let rows = add.results.iter().enumerate().map(|(i, track)| {
            let added = inside.contains(&track.id);
            let (id, song) = (playlist.id.clone(), track.clone());
            div()
                .id(("add-song", i))
                .h(px(52.))
                .px_2()
                .rounded(px(8.))
                .flex()
                .items_center()
                .gap_3()
                .hover(|s| s.bg(p.ink.opacity(0.05)))
                .child(artwork(Some(track), 38., cx))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .child(
                            div()
                                .text_size(px(13.5))
                                .truncate()
                                .child(track.title.clone()),
                        )
                        .child(meta(track.display_artist().to_string(), cx).truncate()),
                )
                .child(if added {
                    glyph("check")
                        .size(px(18.))
                        .text_color(p.accent)
                        .mr_2()
                        .into_any_element()
                } else {
                    icon_button(("add-song-plus", i), "plus", "Add to this playlist")
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.add_to_playlist(&id, vec![song.clone()]);
                            this.refresh(cx);
                            cx.notify();
                        }))
                        .into_any_element()
                })
        });
        let count = playlist.track_ids.len();
        let card = div()
            .id("add-songs")
            .occlude()
            .w(px(card_w))
            .h(px(card_h))
            .rounded(px(16.))
            .bg(cx.theme().popover)
            .border_1()
            .border_color(p.line)
            .shadow_lg()
            .overflow_hidden()
            .flex()
            .flex_col()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                div()
                    .px_6()
                    .pt_6()
                    .pb_3()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(display(format!("Add songs to “{}”", playlist.name), 22.).truncate())
                    .child(Input::new(&add.input)),
            )
            .child(
                div()
                    .id("add-songs-list")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .px_4()
                    .pb_2()
                    .children(rows)
                    .when(add.results.is_empty(), |el| {
                        el.child(div().p_4().child(faint("No songs match.", cx)))
                    }),
            )
            .child(
                div()
                    .flex_shrink_0()
                    .px_6()
                    .py_3()
                    .border_t_1()
                    .border_color(p.line_soft)
                    .flex()
                    .items_center()
                    .child(faint(
                        if count == 1 {
                            "1 song in the playlist".to_string()
                        } else {
                            format!("{count} songs in the playlist")
                        },
                        cx,
                    ))
                    .child(div().flex_1())
                    .child(
                        Button::new("add-songs-done")
                            .primary()
                            .label("Done")
                            .px_5()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.add_songs = None;
                                cx.notify();
                            })),
                    ),
            );
        Some(
            deferred(
                div()
                    .id("add-songs-backdrop")
                    .absolute()
                    .inset_0()
                    .occlude()
                    .bg(gpui::black().opacity(if p.dark { 0.55 } else { 0.3 }))
                    .flex()
                    .justify_center()
                    .items_center()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.add_songs = None;
                            cx.notify();
                        }),
                    )
                    .child(card),
            )
            .with_priority(2),
        )
    }
}
