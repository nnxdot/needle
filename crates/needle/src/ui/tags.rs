use super::{
    AppView, Event, Panel, pal,
    widgets::{faint, meta, strong},
};
use gpui::{prelude::*, *};
use gpui_component::{
    Disableable, Sizable,
    button::{Button, ButtonVariants},
    input::{Input, InputState},
};
use needle_core::scan::{self, Common, TagEdit};
use std::sync::{Arc, atomic::AtomicBool};

pub struct TagFields {
    pub title: Entity<InputState>,
    pub artist: Entity<InputState>,
    pub album: Entity<InputState>,
    pub album_artist: Entity<InputState>,
    pub genre: Entity<InputState>,
    pub year: Entity<InputState>,
    pub track: Entity<InputState>,
}

/// What the editor started with, so only fields the person changed are written.
#[derive(Default)]
pub struct TagSession {
    pub ids: Vec<String>,
    pub start: [Option<String>; 7],
    pub progress: Option<scan::BatchProgress>,
    pub backups: Vec<scan::TagBackup>,
}

impl TagFields {
    pub fn new(window: &mut Window, cx: &mut Context<AppView>) -> Self {
        let mut field = |name: &str| {
            let name = name.to_string();
            cx.new(|cx| InputState::new(window, cx).placeholder(name))
        };
        Self {
            title: field("Title"),
            artist: field("Artist"),
            album: field("Album"),
            album_artist: field("Album artist"),
            genre: field("Genre"),
            year: field("Year"),
            track: field("Track"),
        }
    }
    fn all(&self) -> [&Entity<InputState>; 7] {
        [
            &self.title,
            &self.artist,
            &self.album,
            &self.album_artist,
            &self.genre,
            &self.year,
            &self.track,
        ]
    }
}

fn shared<T: ToString>(value: &Common<T>) -> Option<String> {
    value.value().map(|v| v.to_string())
}

fn number(text: &str, name: &str, max: u32) -> Result<u32, String> {
    if text.is_empty() {
        return Ok(0);
    }
    text.parse::<u32>()
        .ok()
        .filter(|n| *n <= max)
        .ok_or_else(|| {
            format!(
                "{name} must be a whole number, like {}.",
                if max > 999 { "1997" } else { "3" }
            )
        })
}

impl AppView {
    pub(super) fn edit_tags(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let selected = self.selected_tracks();
        let tracks = if selected.len() > 1 {
            selected
        } else if let Some(track) = self.focused.clone() {
            vec![track]
        } else {
            return;
        };
        if tracks.iter().any(|t| t.is_streamed()) {
            self.fail("Songs on a music server cannot be edited here. Change them on the server, or save them to your music first.");
            return;
        }
        let common = scan::common_tags(&tracks);
        let zero_is_empty = |v: Option<String>| v.filter(|v| v != "0");
        let start = [
            shared(&common.title),
            shared(&common.artist),
            shared(&common.album),
            shared(&common.album_artist),
            shared(&common.genre),
            zero_is_empty(shared(&common.year)),
            zero_is_empty(shared(&common.track_number)),
        ];
        let many = tracks.len() > 1;
        for (input, value) in self.tags.all().into_iter().zip(start.iter()) {
            input.update(cx, |s, cx| {
                s.set_value(value.clone().unwrap_or_default(), window, cx);
                s.set_placeholder(
                    if value.is_none() && many {
                        "Mixed · leave empty to keep"
                    } else {
                        ""
                    },
                    window,
                    cx,
                );
            });
        }
        self.pending_mbid = None;
        self.tag_session = TagSession {
            ids: tracks.iter().map(|t| t.id.clone()).collect(),
            start,
            progress: None,
            backups: if many {
                vec![]
            } else {
                scan::tag_backups(&self.library, &tracks[0].id).unwrap_or_default()
            },
        };
        self.editing = true;
        self.panel = Panel::Details;
        self.settings.show_inspector = true;
        cx.notify();
    }

    pub(super) fn write_tags(&mut self, cx: &mut Context<Self>) {
        let values: Vec<String> = self
            .tags
            .all()
            .iter()
            .map(|i| i.read(cx).value().trim().to_string())
            .collect();
        let session = &self.tag_session;
        let many = session.ids.len() > 1;
        // A field is written when it differs from what the editor opened with.
        let changed = |i: usize| -> Option<String> {
            let before = session.start[i].clone();
            let now = &values[i];
            if before.as_deref() == Some(now.as_str()) || (before.is_none() && now.is_empty()) {
                None
            } else {
                Some(now.clone())
            }
        };
        let year = match changed(5).map(|v| number(&v, "Year", 9999)).transpose() {
            Ok(v) => v,
            Err(e) => return self.fail(e),
        };
        let track_number = match changed(6)
            .map(|v| number(&v, "Track number", 999))
            .transpose()
        {
            Ok(v) => v,
            Err(e) => return self.fail(e),
        };
        let edit = TagEdit {
            title: changed(0).filter(|_| !many),
            artist: changed(1),
            album: changed(2),
            album_artist: changed(3),
            genre: changed(4),
            year,
            track_number: track_number.filter(|_| !many),
            musicbrainz_id: self.pending_mbid.clone(),
        };
        if edit.is_empty() {
            self.editing = false;
            return self.notify("Nothing changed.");
        }
        let library = self.library.clone();
        let sender = self.sender.clone();
        let ids = session.ids.clone();
        if !many {
            self.notify("Writing tags and checking the audio is unchanged…");
            std::thread::spawn(move || {
                let _ = sender.send(match scan::write_tags(&library, &ids[0], &edit) {
                    Ok(()) => Event::SavedTags,
                    Err(e) => Event::Error(format!("{e:#}")),
                });
            });
            return;
        }
        let cancel = Arc::new(AtomicBool::new(false));
        self.cancel = cancel.clone();
        self.tag_session.progress = Some(scan::BatchProgress {
            total: ids.len(),
            ..Default::default()
        });
        std::thread::spawn(move || {
            let progress = sender.clone();
            let result = scan::write_tags_batch(&library, &ids, &edit, cancel, |p| {
                let _ = progress.send(Event::BatchProgress(p));
            });
            let _ = sender.send(match result {
                Ok(report) => Event::BatchDone(report),
                Err(e) => Event::Error(format!("{e:#}")),
            });
        });
    }

    pub(super) fn tag_editor(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = pal(cx);
        let session = &self.tag_session;
        let many = session.ids.len() > 1;
        let field = |label: &'static str, input: &Entity<InputState>| {
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(faint(label, cx))
                .child(Input::new(input).small())
        };
        let busy = session.progress.is_some();
        div()
            .p_3()
            .rounded(px(8.))
            .border_1()
            .border_color(p.accent.opacity(0.35))
            .flex()
            .flex_col()
            .gap_3()
            .child(strong(if many { format!("Edit tags on {} tracks", session.ids.len()) } else { "Edit tags".into() }))
            .child(
                meta(
                    if many {
                        "Only fields you change are written. Each file is backed up and checked so its audio stays identical."
                    } else {
                        "Saving writes to the file. Needle keeps a backup and checks the audio is unchanged."
                    },
                    cx,
                )
                .line_height(relative(1.45)),
            )
            .when_some(self.pending_mbid.clone(), |el, id| el.child(faint(format!("MusicBrainz recording {id}"), cx)))
            .when(!many, |el| el.child(field("Title", &self.tags.title)))
            .child(field("Artist", &self.tags.artist))
            .child(field("Album", &self.tags.album))
            .child(field("Album artist", &self.tags.album_artist))
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(field("Genre", &self.tags.genre).flex_1())
                    .child(field("Year", &self.tags.year).w(px(64.)))
                    .when(!many, |el| el.child(field("Track", &self.tags.track).w(px(56.)))),
            )
            .when_some(session.progress.clone(), |el, progress| {
                let fraction = if progress.total == 0 { 0. } else { progress.done as f32 / progress.total as f32 };
                el.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(div().h(px(4.)).rounded_full().bg(p.line).child(div().h_full().rounded_full().bg(p.accent).w(relative(fraction))))
                        .child(faint(
                            format!("{} of {} files · {} failed{}", progress.done, progress.total, progress.failed, if progress.current.is_empty() { String::new() } else { format!(" · {}", progress.current) }),
                            cx,
                        ).truncate()),
                )
            })
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        Button::new("save-tags")
                            .primary()
                            .small()
                            .label(if many { "Save to files" } else { "Save to file" })
                            .disabled(busy)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.write_tags(cx);
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("cancel-tags")
                            .ghost()
                            .small()
                            .label(if busy { "Stop" } else { "Cancel" })
                            .on_click(cx.listener(move |this, _, _, cx| {
                                if busy {
                                    this.cancel.store(true, std::sync::atomic::Ordering::Relaxed);
                                } else {
                                    this.editing = false;
                                }
                                cx.notify();
                            })),
                    ),
            )
            .when(!session.backups.is_empty(), |el| {
                el.child(
                    div()
                        .pt_2()
                        .border_t_1()
                        .border_color(p.line_soft)
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(faint("Earlier versions of this file", cx))
                        .children(session.backups.iter().take(5).enumerate().map(|(i, backup)| {
                            let when = chrono::DateTime::from_timestamp(backup.created_at, 0)
                                .map(|d| d.with_timezone(&chrono::Local).format("%-d %b %Y · %H:%M").to_string())
                                .unwrap_or_default();
                            let backup = backup.clone();
                            let id = session.ids.first().cloned().unwrap_or_default();
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .child(meta(when, cx))
                                .child(
                                    Button::new(("restore-backup", i))
                                        .ghost()
                                        .xsmall()
                                        .label("Restore")
                                        .tooltip("Put this version back. The current file is backed up first, so this can be undone too.")
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            let library = this.library.clone();
                                            let (id, backup) = (id.clone(), backup.clone());
                                            this.editing = false;
                                            this.background(move || {
                                                scan::restore_tag_backup(&library, &id, &backup)?;
                                                Ok("Earlier version restored. The replaced file is kept as a backup.".into())
                                            });
                                            cx.notify();
                                        })),
                                )
                        })),
                )
            })
    }
}
