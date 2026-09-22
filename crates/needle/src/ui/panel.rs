use super::{
    AppView, Event, Page, Panel, pal,
    widgets::{artwork, faint, glyph, icon, icon_button, meta, quality, segmented, strong},
};
use gpui::{prelude::*, *};
use gpui_component::{
    Disableable, Sizable,
    button::{Button, ButtonVariants},
};
use needle_core::{audio::Command, integrations, model::format_duration};
use std::path::PathBuf;

fn when(timestamp: i64) -> String {
    chrono::DateTime::from_timestamp(timestamp, 0)
        .map(|d| {
            d.with_timezone(&chrono::Local)
                .format("%-d %b %Y")
                .to_string()
        })
        .unwrap_or_else(|| "—".into())
}

impl AppView {
    pub(super) fn panel(
        &self,
        width: f32,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let tab = if self.panel == Panel::Details { 0 } else { 1 };
        let weak = cx.entity().downgrade();
        div()
            .w(px(width))
            .flex_shrink_0()
            .h_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(52.))
                    .px_4()
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(segmented(
                        "panel-tab",
                        &["Details", "Queue"],
                        tab,
                        cx,
                        move |index, _, cx| {
                            let _ = weak.update(cx, |this, cx| {
                                this.panel = if index == 0 {
                                    Panel::Details
                                } else {
                                    Panel::Queue
                                };
                                cx.notify();
                            });
                        },
                    ))
                    .child(
                        icon_button("close-panel", "panel", "Hide panel")
                            .small()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.settings.show_inspector = false;
                                this.persist_settings();
                                cx.notify();
                            })),
                    ),
            )
            .child(match self.panel {
                Panel::Details => self.details(width, cx).into_any_element(),
                Panel::Queue => self.queue(cx).into_any_element(),
            })
    }

    fn fact(&self, key: &str, value: impl Into<SharedString>, cx: &App) -> Div {
        div()
            .flex()
            .gap_3()
            .py(px(5.))
            .child(meta(key.to_string(), cx).w(px(92.)).flex_shrink_0())
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_size(px(12.5))
                    .truncate()
                    .child(value.into()),
            )
    }

    fn details(&self, width: f32, cx: &mut Context<Self>) -> impl IntoElement {
        let p = pal(cx);
        let selected = self.selected_tracks();
        if selected.len() > 1 {
            let duration: f64 = selected.iter().map(|t| t.duration).sum();
            let count = selected.len();
            return div()
                .id("multi-scroll")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .p_4()
                .flex()
                .flex_col()
                .gap_3()
                .child(div().text_size(px(18.)).font_weight(FontWeight::SEMIBOLD).child(format!("{count} tracks selected")))
                .child(meta(format_duration(duration), cx))
                .child(
                    div()
                        .flex()
                        .gap_2()
                        .child(
                            Button::new("multi-play")
                                .primary()
                                .small()
                                .icon(icon("play"))
                                .label("Play")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    let selected = this.selected_tracks();
                                    let reason = this.reason(cx);
                                    this.play_tracks(selected, &reason);
                                })),
                        )
                        .child(
                            Button::new("multi-queue")
                                .small()
                                .icon(icon("queue"))
                                .label("Add to queue")
                                .on_click(cx.listener(|this, _, _, _| {
                                    let selected = this.selected_tracks();
                                    this.enqueue(selected);
                                })),
                        ),
                )
                .child(
                    Button::new("multi-edit")
                        .small()
                        .icon(icon("edit"))
                        .label(format!("Edit tags on {count} tracks"))
                        .on_click(cx.listener(|this, _, window, cx| this.edit_tags(window, cx))),
                )
                .when(self.editing, |el| el.child(self.tag_editor(cx)))
                .child(faint("Shift-click selects a range. Ctrl-click adds or removes one track. Right-click for more.", cx).line_height(relative(1.5)))
                .into_any_element();
        }
        let track = self
            .focused
            .clone()
            .or_else(|| self.playback.current.as_ref().map(|i| i.track.clone()));
        let Some(track) = track else {
            return div()
                .flex_1()
                .p_4()
                .flex()
                .flex_col()
                .gap_2()
                .child(strong("Nothing selected"))
                .child(
                    meta(
                        "Select a track to see its details, rate it, or fix its tags.",
                        cx,
                    )
                    .line_height(relative(1.5)),
                )
                .into_any_element();
        };
        let reason = self
            .playback
            .current
            .as_ref()
            .filter(|i| i.track.id == track.id)
            .map(|i| i.reason.clone());
        let album_page = super::album_page(&track);
        let artist_page = Page::Artist(track.artist.clone());
        let rating = track.rating;
        let id = track.id.clone();
        let (play, queue) = (track.clone(), track.clone());
        let path = track.path.trim_start_matches("\\\\?\\").to_string();
        div()
            .id("details-scroll")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .px_4()
            .pb_6()
            .flex()
            .flex_col()
            .gap_4()
            .child(artwork(Some(&track), if self.editing { 96. } else { width - 32. }, cx))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(div().text_size(px(18.)).line_height(px(24.)).font_weight(FontWeight::SEMIBOLD).child(track.title.clone()))
                    .child(
                        div()
                            .id("details-artist")
                            .text_size(px(13.5))
                            .cursor_pointer()
                            .hover(|s| s.underline())
                            .child(track.display_artist().to_string())
                            .on_click(cx.listener(move |this, _, window, cx| this.navigate(artist_page.clone(), window, cx))),
                    )
                    .child(
                        div()
                            .id("details-album")
                            .text_size(px(12.5))
                            .text_color(p.ink_2)
                            .cursor_pointer()
                            .hover(|s| s.underline().text_color(p.ink))
                            .child(track.display_album().to_string())
                            .on_click(cx.listener(move |this, _, window, cx| this.navigate(album_page.clone(), window, cx))),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div().flex().children((1..=5).map(|star| {
                            let id = id.clone();
                            let lit = rating >= star;
                            div()
                                .id(("star", star as usize))
                                .p(px(3.))
                                .cursor_pointer()
                                .text_color(if lit { p.accent } else { p.ink_3 })
                                .hover(|s| s.text_color(p.accent))
                                .child(glyph(if lit { "star-fill" } else { "star" }).size(px(17.)).text_color(if lit { p.accent } else { p.ink_3 }))
                                .tooltip(move |window, cx| gpui_component::tooltip::Tooltip::new(format!("Rate {star} of 5")).build(window, cx))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.set_rating(std::slice::from_ref(&id), if rating == star { 0 } else { star });
                                    cx.notify();
                                }))
                        })),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_1()
                            .child(
                                icon_button("details-queue", "queue", "Add to queue")
                                    .small()
                                    .disabled(track.missing)
                                    .on_click(cx.listener(move |this, _, _, _| this.enqueue(vec![queue.clone()]))),
                            )
                            .child(
                                icon_button("details-edit", "edit", "Edit tags · Ctrl+E")
                                    .small()
                                    .on_click(cx.listener(|this, _, window, cx| this.edit_tags(window, cx))),
                            )
                            .child(
                                Button::new("details-play")
                                    .primary()
                                    .small()
                                    .icon(icon("play"))
                                    .label("Play")
                                    .disabled(track.missing)
                                    .on_click(cx.listener(move |this, _, _, _| this.play_tracks(vec![play.clone()], "Chosen from track details"))),
                            ),
                    ),
            )
            .when(self.editing, |el| el.child(self.tag_editor(cx)))
            .child(
                div()
                    .pt_3()
                    .border_t_1()
                    .border_color(p.line_soft)
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(faint("Stems", cx))
                    .child(self.stems_view(Some(track.clone()), false, cx)),
            )
            .when_some(reason, |el, reason| {
                el.child(
                    div()
                        .p_3()
                        .rounded(px(8.))
                        .bg(p.raised)
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(faint("Why this track", cx))
                        .child(div().text_size(px(12.5)).line_height(relative(1.45)).child(reason)),
                )
            })
            .child(
                div()
                    .flex()
                    .flex_col()
                    .child(self.fact("Quality", quality(&track), cx))
                    .child(self.fact(
                        "Sample rate",
                        format!(
                            "{:.1} kHz · {}",
                            track.sample_rate as f64 / 1000.,
                            match track.channels {
                                1 => "mono".to_string(),
                                2 => "stereo".to_string(),
                                0 => "—".to_string(),
                                n => format!("{n} channels"),
                            }
                        ),
                        cx,
                    ))
                    .child(self.fact("Length", format_duration(track.duration), cx))
                    .child(self.fact("Year", if track.year > 0 { track.year.to_string() } else { "—".into() }, cx))
                    .child(self.fact("Genre", if track.genre.is_empty() { "—".into() } else { track.genre.clone() }, cx))
                    .child(self.fact("Tempo", track.bpm.map(|v| format!("{v:.0} BPM")).unwrap_or_else(|| "—".into()), cx))
                    .child(self.fact(
                        "Plays",
                        match (track.play_count, track.last_played) {
                            (0, _) => "Never played".into(),
                            (n, Some(last)) => format!("{n} · last {}", when(last)),
                            (n, None) => n.to_string(),
                        },
                        cx,
                    ))
                    .child(self.fact("Added", when(track.added_at), cx))
                    .child(
                        self.fact("ReplayGain", track.replay_gain.map(|g| format!("{g:+.2} dB")).unwrap_or_else(|| "Not measured".into()), cx)
                            .items_center()
                            .child(
                                Button::new("measure-loudness")
                                    .ghost()
                                    .xsmall()
                                    .label(if track.replay_gain.is_some() { "Remeasure" } else { "Measure" })
                                    .on_click({
                                        let id = track.id.clone();
                                        cx.listener(move |this, _, _, cx| {
                                            this.measure_loudness(id.clone());
                                            cx.notify();
                                        })
                                    }),
                            ),
                    )
                    .child(
                        self.fact("Album gain", track.album_replay_gain.map(|g| format!("{g:+.2} dB")).unwrap_or_else(|| "Not measured".into()), cx)
                            .items_center()
                            .child(
                                Button::new("measure-album")
                                    .ghost()
                                    .xsmall()
                                    .label("Measure album")
                                    .on_click({
                                        let id = track.id.clone();
                                        cx.listener(move |this, _, _, cx| {
                                            let library = this.library.clone();
                                            let id = id.clone();
                                            this.notify("Measuring the whole album in the background…");
                                            this.background(move || {
                                                let album = needle_core::analysis::scan_album_loudness(&library, &id)?;
                                                Ok(format!(
                                                    "Album measured: {:.1} LUFS · gain {:+.2} dB across {} tracks.",
                                                    album.integrated_lufs,
                                                    album.replay_gain_db,
                                                    album.tracks.len()
                                                ))
                                            });
                                            cx.notify();
                                        })
                                    }),
                            ),
                    )
                    .child(
                        self.fact("File", path.clone(), cx).child(
                            icon_button("copy-path", "copy", "Copy file path")
                                .xsmall()
                                .on_click(move |_, _, cx| cx.write_to_clipboard(ClipboardItem::new_string(path.clone()))),
                        ),
                    ),
            )
            .child(
                div()
                    .pt_2()
                    .border_t_1()
                    .border_color(p.line_soft)
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(faint("Fix metadata online", cx))
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .child(
                                Button::new("lookup-metadata")
                                    .small()
                                    .icon(icon("globe"))
                                    .label(if self.lookup_busy { "Looking up…" } else { "MusicBrainz" })
                                    .disabled(self.lookup_busy)
                                    .tooltip("Search MusicBrainz by artist and title. Sends only that text.")
                                    .on_click(cx.listener(|this, _, _, cx| this.lookup(cx))),
                            )
                            .child(
                                Button::new("fingerprint-lookup")
                                    .small()
                                    .ghost()
                                    .label("Identify by sound")
                                    .disabled(self.lookup_busy)
                                    .tooltip("Send an audio fingerprint to AcoustID. Needs an AcoustID key.")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        if let Some(track) = this.focused.clone() {
                                            let library = this.library.clone();
                                            let sender = this.sender.clone();
                                            this.lookup_busy = true;
                                            std::thread::spawn(move || {
                                                let key = integrations::acoustid_key().unwrap_or_default();
                                                let result = integrations::acoustid_lookup(&library, &PathBuf::from(&track.path), &key);
                                                let _ = sender.send(match result {
                                                    Ok(matches) => Event::Matches(track.id, matches),
                                                    Err(error) => Event::Error(format!("AcoustID: {error:#}")),
                                                });
                                            });
                                            cx.notify();
                                        }
                                    })),
                            ),
                    )
                    .children(self.matches.iter().enumerate().map(|(index, recording)| {
                        let recording = recording.clone();
                        let track_id = track.id.clone();
                        let release = recording.release_id.clone();
                        div()
                            .p_3()
                            .rounded(px(8.))
                            .bg(p.raised)
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(strong(recording.title.clone()))
                            .child(meta(format!("{} · {}", recording.artist, recording.album), cx))
                            .child(
                                div()
                                    .mt_1()
                                    .flex()
                                    .gap_2()
                                    .child(
                                        Button::new(("review-match", index))
                                            .xsmall()
                                            .label("Use these tags")
                                            .on_click(cx.listener(move |this, _, window, cx| {
                                                this.selection.ids.clear();
                                                this.edit_tags(window, cx);
                                                for (input, value) in [
                                                    (&this.tags.title, recording.title.clone()),
                                                    (&this.tags.artist, recording.artist.clone()),
                                                    (&this.tags.album, recording.album.clone()),
                                                ] {
                                                    input.update(cx, |s, cx| s.set_value(value, window, cx));
                                                }
                                                this.pending_mbid = Some(recording.id.clone());
                                                cx.notify();
                                            })),
                                    )
                                    .when_some(release, |el, release| {
                                        el.child(
                                            Button::new(("cover-match", index))
                                                .xsmall()
                                                .ghost()
                                                .label("Use its cover")
                                                .on_click(cx.listener(move |this, _, _, _| {
                                                    let library = this.library.clone();
                                                    let (id, release) = (track_id.clone(), release.clone());
                                                    this.background(move || {
                                                        integrations::cover_art(&library, &id, &release)?;
                                                        Ok("Cover art saved to the library cache.".into())
                                                    });
                                                })),
                                        )
                                    }),
                            )
                    })),
            )
            .into_any_element()
    }

    fn queue(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = pal(cx);
        let count = self.playback.queue.len();
        let remaining: f64 = self.playback.queue.iter().map(|i| i.track.duration).sum();
        div()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .when_some(self.playback.current.as_ref(), |el, item| {
                el.child(
                    div()
                        .mx_4()
                        .mb_3()
                        .p_3()
                        .rounded(px(8.))
                        .bg(p.raised)
                        .flex()
                        .gap_3()
                        .child(artwork(Some(&item.track), 48., cx))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .flex_col()
                                .gap(px(2.))
                                .child(faint("Now playing", cx))
                                .child(
                                    strong(item.track.title.clone())
                                        .truncate()
                                        .text_color(p.accent),
                                )
                                .child(meta(item.track.display_artist().to_string(), cx).truncate())
                                .child(faint(item.reason.clone(), cx).truncate()),
                        ),
                )
            })
            .child(
                div()
                    .px_4()
                    .pb_2()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .flex()
                            .items_baseline()
                            .gap_2()
                            .child(strong("Next up"))
                            .when(count > 0, |el| {
                                el.child(faint(
                                    format!("{count} · {}", format_duration(remaining)),
                                    cx,
                                ))
                            }),
                    )
                    .when(count > 0, |el| {
                        el.child(
                            Button::new("clear-queue")
                                .ghost()
                                .xsmall()
                                .label("Clear")
                                .on_click(cx.listener(|this, _, _, _| {
                                    this.player.send(Command::ClearQueue)
                                })),
                        )
                    }),
            )
            .when(count == 0, |el| {
                el.child(
                    meta(
                        if self.settings.autoplay_query.trim().is_empty() {
                            "Nothing queued. Right-click any track and choose Add to queue."
                        } else {
                            "Nothing queued. Your autoplay rule picks what comes next."
                        },
                        cx,
                    )
                    .px_4()
                    .line_height(relative(1.5)),
                )
            })
            .child(
                uniform_list(
                    "queue-list",
                    count,
                    cx.processor(|this, range: std::ops::Range<usize>, _, cx| {
                        let p = pal(cx);
                        range
                            .map(|index| {
                                let item = &this.playback.queue[index];
                                let row = div()
                                    .id(("queue-row", index))
                                    .group("queue-row")
                                    .h(px(48.))
                                    .w_full()
                                    .px_2()
                                    .rounded(px(6.))
                                    .flex()
                                    .items_center()
                                    .gap_3()
                                    .cursor_pointer()
                                    .hover(|s| s.bg(p.raised.opacity(0.6)))
                                    .on_click(cx.listener(move |this, event: &ClickEvent, _, _| {
                                        if event.click_count() == 2 {
                                            this.player.send(Command::Jump(index));
                                        }
                                    }))
                                    .child(artwork(Some(&item.track), 34., cx))
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w_0()
                                            .child(
                                                div()
                                                    .text_size(px(13.))
                                                    .truncate()
                                                    .child(item.track.title.clone()),
                                            )
                                            .child(
                                                meta(item.track.display_artist().to_string(), cx)
                                                    .truncate(),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .opacity(0.)
                                            .group_hover("queue-row", |s| s.opacity(1.))
                                            .child(
                                                icon_button(
                                                    ("queue-up", index),
                                                    "arrow-up",
                                                    "Move up",
                                                )
                                                .xsmall()
                                                .disabled(index == 0)
                                                .on_click(cx.listener(move |this, _, _, _| {
                                                    this.player.send(Command::Move(
                                                        index,
                                                        index.saturating_sub(1),
                                                    ))
                                                })),
                                            )
                                            .child(
                                                icon_button(
                                                    ("queue-remove", index),
                                                    "close",
                                                    "Remove",
                                                )
                                                .xsmall()
                                                .on_click(cx.listener(move |this, _, _, _| {
                                                    this.player.send(Command::Remove(index))
                                                })),
                                            ),
                                    );
                                div().w_full().px_2().child(row)
                            })
                            .collect::<Vec<_>>()
                    }),
                )
                .flex_1(),
            )
    }
}
