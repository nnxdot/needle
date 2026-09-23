//! "Fix my library": duplicates, missing covers, album tags from MusicBrainz, and tidy files.
use super::{
    AppView, Event, pal,
    widgets::{cover, display, faint, glyph, meta, quality, segmented, small_button, track_seed},
};
use gpui::{prelude::*, *};
use gpui_component::{
    Disableable,
    button::ButtonVariants,
    input::{Input, InputState},
};
use needle_core::{
    doctor::{self, AlbumIssue, DuplicateGroup, Plan, Release},
    media,
    model::Track,
    scan,
};

const TABS: [&str; 4] = ["Duplicates", "Covers", "Tags", "Organize"];

/// Results coming back from the background.
pub enum Msg {
    Duplicates(Vec<DuplicateGroup>),
    Merged(usize, Vec<String>),
    Covers(Vec<Track>),
    CoverProgress {
        done: usize,
        total: usize,
        found: usize,
    },
    Issues(Vec<AlbumIssue>),
    Releases(String, Result<Vec<Release>, String>),
    Tagged(String, usize, Vec<String>),
    Plan(Plan),
    Organized(usize, Vec<(String, String)>),
    Undone(usize),
}

/// An album being matched on MusicBrainz.
pub struct Lookup {
    pub issue: AlbumIssue,
    pub tracks: Vec<Track>,
    pub releases: Option<Result<Vec<Release>, String>>,
    pub chosen: usize,
}

pub struct Doctor {
    pub tab: usize,
    pub duplicates: Option<Vec<DuplicateGroup>>,
    /// Waiting for a second click: a group, or `usize::MAX` for all.
    pub confirm_merge: Option<usize>,
    pub covers: Option<Vec<Track>>,
    pub cover_progress: Option<(usize, usize, usize)>,
    pub issues: Option<Vec<AlbumIssue>>,
    pub lookup: Option<Lookup>,
    pub pattern: Entity<InputState>,
    pub plan: Option<Plan>,
    pub confirm_organize: bool,
    pub busy: bool,
}

impl Doctor {
    pub fn new(window: &mut Window, cx: &mut Context<AppView>) -> Self {
        let pattern = cx.new(|cx| InputState::new(window, cx).default_value(doctor::PATTERNS[0]));
        Self {
            tab: 0,
            duplicates: None,
            confirm_merge: None,
            covers: None,
            cover_progress: None,
            issues: None,
            lookup: None,
            pattern,
            plan: None,
            confirm_organize: false,
            busy: false,
        }
    }
}

/// A path shown from its music folder down.
fn below_root<'a>(path: &'a str, roots: &[String]) -> &'a str {
    let lower = path.to_lowercase();
    roots
        .iter()
        .map(|r| r.trim_end_matches(['\\', '/']))
        .filter(|r| lower.starts_with(&r.to_lowercase()))
        .max_by_key(|r| r.len())
        .and_then(|r| path.get(r.len()..))
        .map(|rest| rest.trim_start_matches(['\\', '/']))
        .unwrap_or(super::folders::shown(path))
}

fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

impl AppView {
    fn doctor_send(
        &self,
        work: impl FnOnce(&needle_core::database::Library) -> Msg + Send + 'static,
    ) {
        let library = self.library.clone();
        let sender = self.sender.clone();
        std::thread::spawn(move || {
            let _ = sender.send(Event::Doctor(work(&library)));
        });
    }

    /// Start the checks of the open tab.
    pub(super) fn doctor_load(&mut self) {
        let tab = self.doctor.tab;
        match tab {
            0 => {
                self.doctor.duplicates = None;
                self.doctor_send(|library| {
                    Msg::Duplicates(doctor::find_duplicates(
                        &library.search("").unwrap_or_default(),
                    ))
                });
            }
            1 => {
                self.doctor.covers = None;
                self.doctor_send(|library| {
                    Msg::Covers(library.albums_without_covers().unwrap_or_default())
                });
            }
            2 => {
                self.doctor.issues = None;
                self.doctor_send(|library| {
                    Msg::Issues(doctor::album_issues(
                        &library.search("").unwrap_or_default(),
                    ))
                });
            }
            _ => self.doctor_plan_organize(None),
        }
    }

    fn doctor_plan_organize(&mut self, pattern: Option<String>) {
        let pattern = pattern.unwrap_or_else(|| doctor::PATTERNS[0].to_string());
        self.doctor.plan = None;
        self.doctor.confirm_organize = false;
        self.doctor_send(move |library| {
            let roots = library.roots().unwrap_or_default();
            Msg::Plan(doctor::plan_organize(
                &library.search("").unwrap_or_default(),
                &roots,
                &pattern,
            ))
        });
    }

    pub(super) fn doctor_message(&mut self, msg: Msg) {
        let d = &mut self.doctor;
        match msg {
            Msg::Duplicates(groups) => d.duplicates = Some(groups),
            Msg::Merged(count, failed) => {
                d.busy = false;
                d.confirm_merge = None;
                if failed.is_empty() {
                    self.notify(format!(
                        "Merged {}. The extra copies are in the Recycle Bin.",
                        plural(count, "song", "songs")
                    ));
                } else {
                    self.fail(format!(
                        "Merged {count}; {} could not be merged: {}",
                        failed.len(),
                        failed.join("; ")
                    ));
                }
                self.doctor_load();
                self.playlists = self.library.playlists().unwrap_or_default();
            }
            Msg::Covers(list) => d.covers = Some(list),
            Msg::CoverProgress { done, total, found } => {
                d.cover_progress = (done < total).then_some((done, total, found));
                if done >= total {
                    d.busy = false;
                    self.notify(format!(
                        "Found {} of {}.",
                        plural(found, "cover", "covers"),
                        total
                    ));
                    self.doctor_load();
                }
            }
            Msg::Issues(list) => d.issues = Some(list),
            Msg::Releases(album, result) => {
                if let Some(lookup) = &mut d.lookup
                    && lookup.issue.album == album
                {
                    lookup.releases = Some(result);
                    lookup.chosen = 0;
                }
            }
            Msg::Tagged(album, saved, failed) => {
                d.busy = false;
                d.lookup = None;
                if failed.is_empty() {
                    self.notify(format!(
                        "Updated {} of “{album}”.",
                        plural(saved, "song", "songs")
                    ));
                } else {
                    self.fail(format!(
                        "Updated {saved}; {} failed: {}",
                        failed.len(),
                        failed.join("; ")
                    ));
                }
                self.doctor_load();
            }
            Msg::Plan(plan) => d.plan = Some(plan),
            Msg::Organized(done, failed) => {
                d.busy = false;
                if failed.is_empty() {
                    self.notify(format!(
                        "Moved {}. You can undo this.",
                        plural(done, "file", "files")
                    ));
                } else {
                    let first = failed
                        .first()
                        .map(|f| format!("{}: {}", f.0, f.1))
                        .unwrap_or_default();
                    self.fail(format!(
                        "Moved {done}; {} could not move. {first}",
                        failed.len()
                    ));
                }
                let pattern = None;
                self.doctor_plan_organize(pattern);
            }
            Msg::Undone(count) => {
                self.doctor.busy = false;
                self.notify(format!("Moved {} back.", plural(count, "file", "files")));
                self.doctor_plan_organize(None);
            }
        }
    }

    fn merge_groups(&mut self, groups: Vec<DuplicateGroup>, cx: &mut Context<Self>) {
        self.doctor.busy = true;
        self.doctor_send(move |library| {
            let mut merged = 0;
            let mut failed = vec![];
            for group in groups {
                let keep = group.tracks[group.keep].clone();
                match library.merge_duplicates(&keep, &group.tracks, true) {
                    Ok(()) => merged += group.tracks.len() - 1,
                    Err(e) => failed.push(format!("{}: {e:#}", keep.title)),
                }
            }
            Msg::Merged(merged, failed)
        });
        cx.notify();
    }

    fn find_covers(&mut self, cx: &mut Context<Self>) {
        let Some(list) = self.doctor.covers.clone() else {
            return;
        };
        if list.is_empty() {
            return;
        }
        self.doctor.busy = true;
        self.doctor.cover_progress = Some((0, list.len(), 0));
        let library = self.library.clone();
        let sender = self.sender.clone();
        std::thread::spawn(move || {
            let total = list.len();
            let mut found = 0;
            for (i, track) in list.iter().enumerate() {
                if media::fetch_album_art(&library, track).unwrap_or(0) > 0 {
                    found += 1;
                }
                let _ = sender.send(Event::Doctor(Msg::CoverProgress {
                    done: i + 1,
                    total,
                    found,
                }));
            }
        });
        cx.notify();
    }

    fn look_up_album(&mut self, issue: AlbumIssue, cx: &mut Context<Self>) {
        let tracks = self
            .library
            .tracks_by_ids(&issue.track_ids)
            .unwrap_or_default();
        let (album, artist, count) = (issue.album.clone(), issue.artist.clone(), tracks.len());
        self.doctor.lookup = Some(Lookup {
            issue,
            tracks,
            releases: None,
            chosen: 0,
        });
        self.doctor_send(move |_| {
            let artist = if artist == "Various artists" {
                String::new()
            } else {
                artist
            };
            Msg::Releases(
                album.clone(),
                doctor::find_releases(&album, &artist, count).map_err(|e| format!("{e:#}")),
            )
        });
        cx.notify();
    }

    fn apply_release(&mut self, cx: &mut Context<Self>) {
        let Some(lookup) = &self.doctor.lookup else {
            return;
        };
        let Some(Ok(releases)) = &lookup.releases else {
            return;
        };
        let Some(release) = releases.get(lookup.chosen) else {
            return;
        };
        let edits = doctor::release_edits(&lookup.tracks, release);
        let album = release.title.clone();
        self.doctor.busy = true;
        self.doctor_send(move |library| {
            let mut saved = 0;
            let mut failed = vec![];
            for (id, edit) in &edits {
                match scan::write_tags(library, id, edit) {
                    Ok(()) => saved += 1,
                    Err(e) => failed.push(format!("{e:#}")),
                }
            }
            Msg::Tagged(album, saved, failed)
        });
        cx.notify();
    }

    fn organize(&mut self, cx: &mut Context<Self>) {
        let Some(plan) = &self.doctor.plan else {
            return;
        };
        let moves = plan.moves.clone();
        self.doctor.busy = true;
        self.doctor.confirm_organize = false;
        self.doctor_send(move |library| {
            let roots = library.roots().unwrap_or_default();
            match library.organize(&moves, &roots) {
                Ok((done, failed)) => Msg::Organized(done.len(), failed),
                Err(e) => Msg::Organized(0, vec![(String::new(), format!("{e:#}"))]),
            }
        });
        cx.notify();
    }

    // ------------------------------------------------------------ views

    pub(super) fn doctor_view(&self, width: f32, cx: &mut Context<Self>) -> AnyElement {
        let d = &self.doctor;
        let body = match d.tab {
            0 => self.duplicates_tab(cx),
            1 => self.covers_tab(cx),
            2 => self.tags_tab(width, cx),
            _ => self.organize_tab(cx),
        };
        div()
            .id("doctor")
            .size_full()
            .overflow_y_scroll()
            .child(
                div()
                    .px_6()
                    .pt_6()
                    .pb(px(64.))
                    .flex()
                    .flex_col()
                    .gap_4()
                    .child(
                        div()
                            .flex()
                            .items_end()
                            .gap_4()
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .child(display("Fix my library", 34.))
                                    .child(meta("Find copies, fill in covers and tags, and tidy your files.", cx)),
                            )
                            .child(segmented("doctor-tab", &TABS, d.tab, cx, {
                                let view = cx.entity().downgrade();
                                move |i, _, cx| {
                                    view.update(cx, |this, cx| {
                                        this.doctor.tab = i;
                                        this.doctor_load();
                                        cx.notify();
                                    })
                                    .ok();
                                }
                            })),
                    )
                    .child(body),
            )
            .into_any_element()
    }

    fn loading(&self, text: &str, cx: &App) -> AnyElement {
        div()
            .py_8()
            .child(meta(text.to_string(), cx))
            .into_any_element()
    }

    fn all_clear(&self, title: &str, detail: &str, cx: &App) -> AnyElement {
        let p = pal(cx);
        div()
            .py_10()
            .flex()
            .flex_col()
            .items_center()
            .gap_2()
            .child(glyph("check").size(px(26.)).text_color(p.success))
            .child(
                div()
                    .text_size(px(15.))
                    .font_weight(FontWeight::MEDIUM)
                    .child(title.to_string()),
            )
            .child(meta(detail.to_string(), cx))
            .into_any_element()
    }

    fn duplicates_tab(&self, cx: &mut Context<Self>) -> AnyElement {
        let p = pal(cx);
        let d = &self.doctor;
        let Some(groups) = &d.duplicates else {
            return self.loading("Looking for songs you have more than once…", cx);
        };
        if groups.is_empty() {
            return self.all_clear("No duplicates", "Every song is in your library once.", cx);
        }
        let extra: usize = groups.iter().map(|g| g.tracks.len() - 1).sum();
        let all = d.confirm_merge == Some(usize::MAX);
        let mut list = div().flex().flex_col().gap_3().child(
            div()
                .flex()
                .items_center()
                .gap_3()
                .child(
                    meta(
                        format!(
                            "{} with extra copies. Merging keeps the best-sounding copy (marked), moves plays, ratings, history, and playlists to it, and puts the other files in the Recycle Bin.",
                            plural(groups.len(), "song", "songs")
                        ),
                        cx,
                    )
                    .flex_1()
                    .min_w_0(),
                )
                .child(
                    small_button("merge-all", if all { format!("Click again to merge {extra}") } else { format!("Merge all ({extra})") })
                        .when(all, |b| b.danger())
                        .when(!all, |b| b.primary())
                        .disabled(d.busy)
                        .on_click(cx.listener(|this, _, _, cx| {
                            if this.doctor.confirm_merge == Some(usize::MAX) {
                                let groups = this.doctor.duplicates.clone().unwrap_or_default();
                                this.merge_groups(groups, cx);
                            } else {
                                this.doctor.confirm_merge = Some(usize::MAX);
                            }
                            cx.notify();
                        })),
                ),
        );
        for (g, group) in groups.iter().enumerate().take(200) {
            let confirm = d.confirm_merge == Some(g);
            let rows: Vec<AnyElement> = group
                .tracks
                .iter()
                .enumerate()
                .map(|(i, t)| {
                    let kept = i == group.keep;
                    div()
                        .id(("dup-row", g * 1000 + i))
                        .flex()
                        .items_center()
                        .gap_3()
                        .px_2()
                        .py_1()
                        .rounded(px(6.))
                        .cursor_pointer()
                        .when(kept, |el| el.bg(p.accent_soft))
                        .hover(|s| s.bg(p.raised))
                        .child(
                            div()
                                .size(px(16.))
                                .rounded_full()
                                .border_1()
                                .border_color(if kept { p.accent } else { p.line })
                                .flex()
                                .items_center()
                                .justify_center()
                                .when(kept, |el| {
                                    el.child(div().size(px(8.)).rounded_full().bg(p.accent))
                                }),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .child(div().text_size(px(13.)).truncate().child(format!(
                                    "{} · {}",
                                    t.album,
                                    quality(t)
                                )))
                                .child(
                                    faint(super::folders::shown(&t.path).to_string(), cx)
                                        .truncate(),
                                ),
                        )
                        .child(faint(
                            if t.play_count == 1 {
                                "1 play".into()
                            } else {
                                format!("{} plays", t.play_count)
                            },
                            cx,
                        ))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if let Some(groups) = &mut this.doctor.duplicates
                                && let Some(group) = groups.get_mut(g)
                            {
                                group.keep = i;
                            }
                            cx.notify();
                        }))
                        .into_any_element()
                })
                .collect();
            let first = &group.tracks[group.keep];
            list = list.child(
                div()
                    .p_3()
                    .rounded(px(10.))
                    .border_1()
                    .border_color(p.line_soft)
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(cover(first.artwork.as_deref(), &track_seed(first), 36., cx))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .child(
                                        div()
                                            .text_size(px(14.))
                                            .font_weight(FontWeight::MEDIUM)
                                            .truncate()
                                            .child(first.title.clone()),
                                    )
                                    .child(meta(first.display_artist().to_string(), cx).truncate()),
                            )
                            .child(
                                small_button(
                                    ("merge", g),
                                    if confirm {
                                        "Click again to merge"
                                    } else {
                                        "Merge"
                                    },
                                )
                                .ghost()
                                .when(confirm, |b| b.danger())
                                .disabled(d.busy)
                                .on_click(cx.listener(
                                    move |this, _, _, cx| {
                                        if this.doctor.confirm_merge == Some(g) {
                                            if let Some(group) = this
                                                .doctor
                                                .duplicates
                                                .as_ref()
                                                .and_then(|l| l.get(g))
                                                .cloned()
                                            {
                                                this.merge_groups(vec![group], cx);
                                            }
                                        } else {
                                            this.doctor.confirm_merge = Some(g);
                                        }
                                        cx.notify();
                                    },
                                )),
                            ),
                    )
                    .children(rows),
            );
        }
        list.into_any_element()
    }

    fn covers_tab(&self, cx: &mut Context<Self>) -> AnyElement {
        let p = pal(cx);
        let d = &self.doctor;
        let Some(list) = &d.covers else {
            return self.loading("Looking for albums without covers…", cx);
        };
        if list.is_empty() {
            return self.all_clear("Every album has a cover", "Nothing to fill in.", cx);
        }
        let online = self.settings.online_media;
        let action: AnyElement = match d.cover_progress {
            Some((done, total, found)) => {
                meta(format!("Looking… {done} of {total}, {found} found"), cx).into_any_element()
            }
            None if !online => small_button("covers-online", "Turn on online lookups")
                .ghost()
                .on_click(cx.listener(|this, _, _, cx| {
                    this.settings.online_media = true;
                    this.persist_settings();
                    cx.notify();
                }))
                .into_any_element(),
            None => small_button(
                "covers-find",
                format!("Find {} online", plural(list.len(), "cover", "covers")),
            )
            .primary()
            .disabled(d.busy)
            .on_click(cx.listener(|this, _, _, cx| this.find_covers(cx)))
            .into_any_element(),
        };
        let tiles: Vec<AnyElement> = list
            .iter()
            .enumerate()
            .take(300)
            .map(|(i, t)| {
                div()
                    .id(("nocover", i))
                    .flex()
                    .items_center()
                    .gap_3()
                    .py_1()
                    .child(cover(None, &track_seed(t), 40., cx))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(div().text_size(px(13.5)).truncate().child(t.album.clone()))
                            .child(faint(t.display_album_artist().to_string(), cx).truncate()),
                    )
                    .into_any_element()
            })
            .collect();
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(
                        meta(
                            if online {
                                format!(
                                    "{} with no cover. Needle can look for them on MusicBrainz and the Cover Art Archive, sending only the album and artist names.",
                                    plural(list.len(), "album", "albums")
                                )
                            } else {
                                format!(
                                    "{} with no cover. Online lookups are off; turn them on to search the Cover Art Archive.",
                                    plural(list.len(), "album", "albums")
                                )
                            },
                            cx,
                        )
                        .flex_1()
                        .min_w_0(),
                    )
                    .child(action),
            )
            .child(div().pt_2().border_t_1().border_color(p.line_soft).flex().flex_col().children(tiles))
            .into_any_element()
    }

    fn tags_tab(&self, width: f32, cx: &mut Context<Self>) -> AnyElement {
        let p = pal(cx);
        let d = &self.doctor;
        if let Some(lookup) = &d.lookup {
            return self.lookup_view(lookup, width, cx);
        }
        let Some(list) = &d.issues else {
            return self.loading("Checking album tags…", cx);
        };
        if list.is_empty() {
            return self.all_clear(
                "Album tags look complete",
                "Every album has a year, track numbers, and an album artist where needed.",
                cx,
            );
        }
        let rows: Vec<AnyElement> = list
            .iter()
            .enumerate()
            .take(300)
            .map(|(i, issue)| {
                let clicked = issue.clone();
                div()
                    .id(("issue", i))
                    .flex()
                    .items_center()
                    .gap_3()
                    .px_2()
                    .py(px(6.))
                    .rounded(px(6.))
                    .hover(|s| s.bg(p.raised.opacity(0.6)))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(
                                div()
                                    .text_size(px(13.5))
                                    .font_weight(FontWeight::MEDIUM)
                                    .truncate()
                                    .child(issue.album.clone()),
                            )
                            .child(
                                faint(
                                    format!(
                                        "{} · {} · {}",
                                        issue.artist,
                                        plural(issue.track_ids.len(), "song", "songs"),
                                        issue.problems.join(", ")
                                    ),
                                    cx,
                                )
                                .truncate(),
                            ),
                    )
                    .child(
                        small_button(("lookup", i), "Look up")
                            .ghost()
                            .disabled(!self.settings.online_media)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.look_up_album(clicked.clone(), cx)
                            })),
                    )
                    .into_any_element()
            })
            .collect();
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(meta(
                if self.settings.online_media {
                    format!(
                        "{} with gaps in their tags. Look one up to match it with a release on MusicBrainz; you see every change before anything is written, and Needle keeps a backup of each file.",
                        plural(list.len(), "album", "albums")
                    )
                } else {
                    format!(
                        "{} with gaps in their tags. Turn on online lookups in Settings to match them on MusicBrainz.",
                        plural(list.len(), "album", "albums")
                    )
                },
                cx,
            ))
            .child(div().flex().flex_col().children(rows))
            .into_any_element()
    }

    fn lookup_view(&self, lookup: &Lookup, width: f32, cx: &mut Context<Self>) -> AnyElement {
        let p = pal(cx);
        let back = small_button("lookup-back", "Back to the list")
            .ghost()
            .on_click(cx.listener(|this, _, _, cx| {
                this.doctor.lookup = None;
                cx.notify();
            }));
        let head = div()
            .flex()
            .items_center()
            .gap_3()
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .child(
                        div()
                            .text_size(px(16.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .truncate()
                            .child(lookup.issue.album.clone()),
                    )
                    .child(meta(
                        format!(
                            "{} · {}",
                            lookup.issue.artist,
                            plural(lookup.tracks.len(), "song", "songs")
                        ),
                        cx,
                    )),
            )
            .child(back);
        let releases = match &lookup.releases {
            None => {
                return div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(head)
                    .child(self.loading("Searching MusicBrainz…", cx))
                    .into_any_element();
            }
            Some(Err(e)) => {
                return div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(head)
                    .child(meta(format!("The search failed: {e}"), cx))
                    .into_any_element();
            }
            Some(Ok(r)) if r.is_empty() => {
                return div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(head)
                    .child(meta("MusicBrainz has no release that matches.", cx))
                    .into_any_element();
            }
            Some(Ok(r)) => r,
        };
        let choices: Vec<AnyElement> = releases
            .iter()
            .enumerate()
            .map(|(i, r)| {
                let active = i == lookup.chosen;
                let detail = [
                    r.year.map(|y| y.to_string()).unwrap_or_default(),
                    r.country.clone(),
                    r.format.clone(),
                    plural(r.tracks.len(), "track", "tracks"),
                ]
                .into_iter()
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join(" · ");
                div()
                    .id(("release", i))
                    .w(px(((width - 80.) / 2.).clamp(220., 320.)))
                    .flex_shrink_0()
                    .p_3()
                    .rounded(px(8.))
                    .border_1()
                    .border_color(if active { p.accent } else { p.line_soft })
                    .when(active, |el| el.bg(p.accent_soft))
                    .cursor_pointer()
                    .child(
                        div()
                            .text_size(px(13.5))
                            .font_weight(FontWeight::MEDIUM)
                            .truncate()
                            .child(r.title.clone()),
                    )
                    .child(faint(r.artist.clone(), cx).truncate())
                    .child(faint(detail, cx).truncate())
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(l) = &mut this.doctor.lookup {
                            l.chosen = i;
                        }
                        cx.notify();
                    }))
                    .into_any_element()
            })
            .collect();
        let release = &releases[lookup.chosen.min(releases.len() - 1)];
        let matches = doctor::match_tracks(&lookup.tracks, release);
        let edits = doctor::release_edits(&lookup.tracks, release);
        let rows: Vec<AnyElement> = lookup
            .tracks
            .iter()
            .zip(&matches)
            .enumerate()
            .map(|(i, (t, m))| {
                let new = match m {
                    Some(r) => format!("{:02}  {}", r.number, r.title),
                    None => "No match; left as it is".into(),
                };
                let changes = edits.iter().any(|e| e.0 == t.id);
                div()
                    .id(("match", i))
                    .flex()
                    .items_center()
                    .gap_3()
                    .py(px(5.))
                    .border_b_1()
                    .border_color(p.line_soft)
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_size(px(13.))
                            .text_color(p.ink_2)
                            .truncate()
                            .child(format!("{:02}  {}", t.track_number.max(0), t.title)),
                    )
                    .child(glyph("chevron-right").size(px(14.)).text_color(p.ink_3))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_size(px(13.))
                            .truncate()
                            .text_color(if m.is_none() {
                                p.ink_3
                            } else if changes {
                                p.ink
                            } else {
                                p.ink_2
                            })
                            .when(changes, |el| el.font_weight(FontWeight::MEDIUM))
                            .child(new),
                    )
                    .into_any_element()
            })
            .collect();
        let count = edits.len();
        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(head)
            .child(faint("Pick the release that is your copy", cx))
            .child(div().id("releases").overflow_x_scroll().child(div().flex().gap_3().children(choices)))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(
                        meta(
                            if count == 0 {
                                "These songs already match this release.".to_string()
                            } else {
                                format!(
                                    "{} will change: title, track number, year, album, and artists where they differ. Changed songs are in bold.",
                                    plural(count, "song", "songs")
                                )
                            },
                            cx,
                        )
                        .flex_1()
                        .min_w_0(),
                    )
                    .child(
                        small_button("apply-release", "Write tags")
                            .primary()
                            .disabled(count == 0 || self.doctor.busy)
                            .on_click(cx.listener(|this, _, _, cx| this.apply_release(cx))),
                    ),
            )
            .child(div().flex().flex_col().children(rows))
            .into_any_element()
    }

    fn organize_tab(&self, cx: &mut Context<Self>) -> AnyElement {
        let p = pal(cx);
        let d = &self.doctor;
        let presets: Vec<AnyElement> = doctor::PATTERNS
            .iter()
            .enumerate()
            .map(|(i, pattern)| {
                small_button(("preset", i), *pattern)
                    .ghost()
                    .on_click(cx.listener(move |this, _, window, cx| {
                        let pattern = doctor::PATTERNS[i].to_string();
                        this.doctor
                            .pattern
                            .update(cx, |s, cx| s.set_value(pattern.clone(), window, cx));
                        this.doctor_plan_organize(Some(pattern));
                        cx.notify();
                    }))
                    .into_any_element()
            })
            .collect();
        let pattern_row = div()
            .flex()
            .items_center()
            .gap_2()
            .child(div().flex_1().min_w_0().child(Input::new(&d.pattern)))
            .child(
                small_button("preview", "Preview")
                    .ghost()
                    .on_click(cx.listener(|this, _, _, cx| {
                        let pattern = this.doctor.pattern.read(cx).value().to_string();
                        this.doctor_plan_organize(Some(pattern));
                        cx.notify();
                    })),
            );
        let can_undo = self.library.can_undo_organize();
        let mut body = div()
            .flex()
            .flex_col()
            .gap_3()
            .child(meta(
                "Rename and move song files inside your music folders by a pattern. Words in braces are filled in from each song's tags: {album_artist} {artist} {album} {title} {year} {track} {disc} {disc-}. Lyrics files move with their songs, and emptied folders are removed.",
                cx,
            ))
            .child(pattern_row)
            .child(div().flex().gap_1().children(presets));
        let Some(plan) = &d.plan else {
            return body
                .child(self.loading("Working out where each file goes…", cx))
                .into_any_element();
        };
        let confirm = d.confirm_organize;
        body = body.child(
            div()
                .mt_2()
                .flex()
                .items_center()
                .gap_3()
                .child(
                    meta(
                        format!(
                            "{} to move · {} already in place · {} left alone",
                            plural(plan.moves.len(), "file", "files"),
                            plan.in_place,
                            plan.skipped.len()
                        ),
                        cx,
                    )
                    .flex_1()
                    .min_w_0(),
                )
                .when(can_undo, |el| {
                    el.child(
                        small_button("undo-organize", "Undo last move")
                            .ghost()
                            .disabled(d.busy)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.doctor.busy = true;
                                this.doctor_send(|library| {
                                    Msg::Undone(library.undo_organize().unwrap_or(0))
                                });
                                cx.notify();
                            })),
                    )
                })
                .child(
                    small_button(
                        "organize",
                        if confirm {
                            format!("Click again to move {}", plan.moves.len())
                        } else {
                            "Move files".into()
                        },
                    )
                    .when(confirm, |b| b.danger())
                    .when(!confirm, |b| b.primary())
                    .disabled(plan.moves.is_empty() || d.busy)
                    .on_click(cx.listener(|this, _, _, cx| {
                        if this.doctor.confirm_organize {
                            this.organize(cx);
                        } else {
                            this.doctor.confirm_organize = true;
                        }
                        cx.notify();
                    })),
                ),
        );
        let roots = self.library.roots().unwrap_or_default();
        let rows: Vec<AnyElement> = plan
            .moves
            .iter()
            .take(200)
            .enumerate()
            .map(|(i, m)| {
                div()
                    .id(("move", i))
                    .py(px(5.))
                    .border_b_1()
                    .border_color(p.line_soft)
                    .child(faint(below_root(&m.from, &roots).to_string(), cx).truncate())
                    .child(
                        div()
                            .text_size(px(13.))
                            .truncate()
                            .child(format!("→ {}", below_root(&m.to, &roots))),
                    )
                    .into_any_element()
            })
            .collect();
        body = body.child(div().flex().flex_col().children(rows));
        if plan.moves.len() > 200 {
            body = body.child(faint(format!("and {} more", plan.moves.len() - 200), cx));
        }
        if !plan.skipped.is_empty() {
            body = body.child(faint("Left alone", cx).mt_4()).children(
                plan.skipped.iter().take(50).map(|(path, why)| {
                    faint(format!("{} · {why}", below_root(path, &roots)), cx).truncate()
                }),
            );
        }
        body.into_any_element()
    }
}
