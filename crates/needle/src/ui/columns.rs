//! The song table's columns: choose them (right-click the header), drag to reorder, drag an
//! edge to resize, click to sort. Saved in the settings.
use super::{AppView, Sort, menus::Entry, motion, pal, widgets::glyph};
use gpui::{prelude::*, *};
use gpui_component::ActiveTheme;
use needle_core::model::{ColumnSetting, Track, format_duration};

/// Every column: key, header, sort field, default width.
pub const COLUMNS: &[(&str, &str, &str, f32)] = &[
    ("album", "Album", "album", 220.),
    ("album_artist", "Album artist", "album_artist", 160.),
    ("genre", "Genre", "genre", 120.),
    ("year", "Year", "year", 56.),
    ("quality", "Quality", "format", 92.),
    ("bitrate", "Bitrate", "bitrate", 76.),
    ("sample_rate", "Sample rate", "sample_rate", 88.),
    ("bpm", "BPM", "bpm", 52.),
    ("plays", "Plays", "play_count", 52.),
    ("last_played", "Last played", "last_played", 104.),
    ("added", "Added", "added_at", 96.),
    ("rating", "Rating", "rating", 84.),
    ("time", "Time", "duration", 52.),
];

pub fn defaults() -> Vec<ColumnSetting> {
    ["album", "quality", "time"]
        .iter()
        .map(|key| ColumnSetting {
            key: key.to_string(),
            width: width_of(key),
        })
        .collect()
}

fn width_of(key: &str) -> f32 {
    COLUMNS.iter().find(|c| c.0 == key).map_or(80., |c| c.3)
}
fn info(key: &str) -> Option<&'static (&'static str, &'static str, &'static str, f32)> {
    COLUMNS.iter().find(|c| c.0 == key)
}

fn day(timestamp: Option<i64>) -> String {
    timestamp
        .and_then(|t| chrono::DateTime::from_timestamp(t, 0))
        .map(|d| {
            d.with_timezone(&chrono::Local)
                .format("%-d %b %Y")
                .to_string()
        })
        .unwrap_or_default()
}

/// What a column shows for a track.
pub fn cell_text(key: &str, track: &Track) -> String {
    match key {
        "album" => track.display_album().to_string(),
        "album_artist" => track.album_artist.clone(),
        "genre" => track.genre.clone(),
        "year" => {
            if track.year > 0 {
                track.year.to_string()
            } else {
                Default::default()
            }
        }
        "quality" => super::widgets::quality(track),
        "bitrate" => {
            if track.bitrate > 0 {
                format!("{} kbps", track.bitrate)
            } else {
                Default::default()
            }
        }
        "sample_rate" => {
            if track.sample_rate > 0 {
                {
                    let khz = track.sample_rate as f64 / 1000.;
                    if khz.fract() == 0. {
                        format!("{khz:.0} kHz")
                    } else {
                        format!("{khz:.1} kHz")
                    }
                }
            } else {
                Default::default()
            }
        }
        "bpm" => track.bpm.map(|b| format!("{b:.0}")).unwrap_or_default(),
        "plays" => {
            if track.play_count > 0 {
                track.play_count.to_string()
            } else {
                Default::default()
            }
        }
        "last_played" => day(track.last_played),
        "added" => day(Some(track.added_at)),
        "rating" => "★".repeat(track.rating.clamp(0, 5) as usize),
        "time" => format_duration(track.duration),
        _ => String::new(),
    }
}

/// Numbers sit to the right.
fn right_aligned(key: &str) -> bool {
    matches!(
        key,
        "year" | "bitrate" | "sample_rate" | "bpm" | "plays" | "time"
    )
}

/// A column header being dragged to a new place.
#[derive(Clone)]
pub struct DraggedColumn(pub String);
/// A column edge being dragged; `serial` tells one drag from the next.
#[derive(Clone)]
pub struct ResizingColumn {
    pub key: String,
    pub serial: u64,
    /// The width the column is drawn at, and the widest it may grow to.
    pub shown: f32,
    pub max: f32,
}

/// Row parts that are not columns: margins, number, cover, gaps, heart, more button.
const FIXED: f32 = 32. + 16. + 28. + 38. + 28. + 28. + 12. * 5.;
/// The title always keeps at least this much room.
const TITLE_ROOM: f32 = 200.;

/// Room left beyond the title's minimum.
pub fn spare(width: f32, columns: &[ColumnSetting]) -> f32 {
    (width - FIXED - columns.iter().map(|c| c.width + 12.).sum::<f32>() - TITLE_ROOM).max(0.)
}

struct Ghost(SharedString);
impl Render for Ghost {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = pal(cx);
        div()
            .px_2()
            .py_1()
            .rounded(px(6.))
            .bg(cx.theme().popover)
            .border_1()
            .border_color(p.accent)
            .text_size(px(12.))
            .child(self.0.clone())
    }
}
struct Nothing;
impl Render for Nothing {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

pub struct HeaderMenu {
    pub position: Point<Pixels>,
    pub serial: usize,
}

impl AppView {
    /// The columns to draw at this width: the chosen ones, dropping from the end (Time last)
    /// when the title would get too narrow. The album column hides on album pages.
    pub(super) fn visible_columns(&self, width: f32, album_view: bool) -> Vec<ColumnSetting> {
        let mut columns: Vec<ColumnSetting> = self
            .settings
            .columns
            .iter()
            .filter(|c| info(&c.key).is_some() && !(album_view && c.key == "album"))
            .cloned()
            .collect();
        let room = |cols: &[ColumnSetting]| {
            width - FIXED - cols.iter().map(|c| c.width + 12.).sum::<f32>()
        };
        // First squeeze the text columns (down to 100 px each), then drop columns from the end.
        let deficit = TITLE_ROOM - room(&columns);
        if deficit > 0. {
            let text = ["album", "album_artist", "genre"];
            let spare: f32 = columns
                .iter()
                .filter(|c| text.contains(&c.key.as_str()))
                .map(|c| (c.width - 100.).max(0.))
                .sum();
            if spare > 0. {
                let share = (deficit / spare).min(1.);
                for c in columns
                    .iter_mut()
                    .filter(|c| text.contains(&c.key.as_str()))
                {
                    c.width -= (c.width - 100.).max(0.) * share;
                }
            }
        }
        while room(&columns) < TITLE_ROOM && !columns.is_empty() {
            let drop = columns
                .iter()
                .rposition(|c| c.key != "time")
                .unwrap_or(columns.len() - 1);
            columns.remove(drop);
        }
        columns
    }

    /// One header cell: sortable label, draggable to reorder, with a resize edge.
    pub(super) fn column_header(
        &self,
        column: &ColumnSetting,
        spare: f32,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let p = pal(cx);
        let Some(&(key, label, field, _)) = info(&column.key) else {
            return div().into_any_element();
        };
        let state = match self.sort {
            Sort::Asc(f) if f == field => Some("arrow-up"),
            Sort::Desc(f) if f == field => Some("arrow-down"),
            _ => None,
        };
        let serial = self.column_serial;
        div()
            .id(SharedString::from(format!("col-{key}")))
            .w(px(column.width))
            .flex_shrink_0()
            .h_full()
            .relative()
            .flex()
            .items_center()
            .when(right_aligned(key), |el| el.justify_end())
            .drag_over::<DraggedColumn>(move |s, _, _, _| s.bg(p.accent_soft))
            .on_drop(cx.listener(move |this, dragged: &DraggedColumn, _, cx| {
                this.move_column(&dragged.0, key);
                cx.notify();
            }))
            .child(
                // The name: click to sort, drag to move the column.
                div()
                    .id(SharedString::from(format!("col-name-{key}")))
                    .min_w_0()
                    .flex()
                    .items_center()
                    .gap_1()
                    .cursor_pointer()
                    .text_color(if state.is_some() { p.ink } else { p.ink_3 })
                    .hover(|s| s.text_color(p.ink))
                    .child(div().truncate().child(label))
                    .when_some(state, |el, g| {
                        el.child(glyph(g).size(px(12.)).text_color(p.ink))
                    })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.sort = match this.sort {
                            Sort::Asc(f) if f == field => Sort::Desc(field),
                            Sort::Desc(f) if f == field => Sort::Default,
                            _ => Sort::Asc(field),
                        };
                        this.page_offset = 0;
                        this.refresh(cx);
                    }))
                    .on_drag(DraggedColumn(key.to_string()), move |_, _, _, cx| {
                        cx.new(|_| Ghost(label.into()))
                    }),
            )
            .child(
                // The resize edge sits on the left: the title takes the free room, so the
                // columns hang from the right and grow leftward.
                div()
                    .id(SharedString::from(format!("col-edge-{key}")))
                    .absolute()
                    .left(px(-8.))
                    .top(px(6.))
                    .bottom(px(6.))
                    .w(px(4.))
                    .rounded(px(2.))
                    .cursor_col_resize()
                    .bg(p.line_soft)
                    .hover(|s| s.bg(p.accent))
                    .on_drag(
                        ResizingColumn {
                            key: key.to_string(),
                            serial,
                            shown: column.width,
                            max: column.width + spare,
                        },
                        |_, _, _, cx| cx.new(|_| Nothing),
                    ),
            )
            .into_any_element()
    }

    /// Follow a resize drag: the column's new width is its width when the drag started plus
    /// how far the pointer has moved.
    pub(super) fn resize_column(&mut self, drag: &ResizingColumn, x: f32) {
        // Start from the width on screen (a narrow window may have squeezed the column).
        let start = match &self.column_resize {
            Some((serial, key, start_x, start_w)) if *serial == drag.serial && *key == drag.key => {
                (*start_x, *start_w)
            }
            _ => {
                self.column_resize = Some((drag.serial, drag.key.clone(), x, drag.shown));
                (x, drag.shown)
            }
        };
        let width = (start.1 + start.0 - x).clamp(40., drag.max.clamp(40., 480.));
        if let Some(column) = self.settings.columns.iter_mut().find(|c| c.key == drag.key) {
            column.width = width;
        }
    }

    pub(super) fn finish_column_resize(&mut self) {
        if self.column_resize.take().is_some() {
            self.column_serial += 1;
            self.persist_settings();
        }
    }

    /// Put column `key` where `before` is.
    fn move_column(&mut self, key: &str, before: &str) {
        if key == before {
            return;
        }
        let Some(from) = self.settings.columns.iter().position(|c| c.key == key) else {
            return;
        };
        let column = self.settings.columns.remove(from);
        let to = self
            .settings
            .columns
            .iter()
            .position(|c| c.key == before)
            .unwrap_or(self.settings.columns.len());
        self.settings.columns.insert(to, column);
        self.persist_settings();
    }

    fn toggle_column(&mut self, key: &str) {
        if let Some(at) = self.settings.columns.iter().position(|c| c.key == key) {
            self.settings.columns.remove(at);
        } else {
            // New columns go before Time, which stays last.
            let at = self
                .settings
                .columns
                .iter()
                .position(|c| c.key == "time")
                .unwrap_or(self.settings.columns.len());
            self.settings.columns.insert(
                at,
                ColumnSetting {
                    key: key.to_string(),
                    width: width_of(key),
                },
            );
        }
        self.persist_settings();
    }

    /// One cell of a row.
    pub(super) fn column_cell(
        &self,
        column: &ColumnSetting,
        track: &Track,
        cx: &App,
    ) -> AnyElement {
        let p = pal(cx);
        let rating = column.key == "rating";
        div()
            .w(px(column.width))
            .flex_shrink_0()
            .min_w_0()
            .overflow_hidden()
            .when(right_aligned(&column.key), |el| el.text_right())
            .text_size(px(if column.key == "album" { 12.5 } else { 12. }))
            .text_color(if rating {
                p.accent
            } else if column.key == "album" || column.key == "time" {
                p.ink_2
            } else {
                p.ink_3
            })
            .child(
                div()
                    .w_full()
                    .truncate()
                    .child(cell_text(&column.key, track)),
            )
            .into_any_element()
    }

    pub(super) fn open_header_menu(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        self.menu = None;
        self.menu_serial += 1;
        self.header_menu = Some(HeaderMenu {
            position,
            serial: self.menu_serial,
        });
        cx.notify();
    }

    pub(super) fn header_menu_view(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        let menu = self.header_menu.as_ref()?;
        let p = pal(cx);
        let mut entries = vec![Entry::Label("Columns".into())];
        entries.extend(COLUMNS.iter().map(|(key, label, _, _)| {
            let shown = self.settings.columns.iter().any(|c| c.key == *key);
            let key = key.to_string();
            Entry::item_keep(
                if shown { "check" } else { "blank" },
                *label,
                None,
                move |this, _, _| this.toggle_column(&key),
            )
        }));
        entries.push(Entry::Separator);
        entries.push(Entry::item("close", "Reset columns", None, |this, _, _| {
            this.settings.columns = defaults();
            this.persist_settings();
        }));
        let rows = Self::menu_rows(&entries, None, cx);
        let body = div()
            .id("header-menu")
            .occlude()
            .w(px(220.))
            .p_1()
            .rounded(px(9.))
            .bg(cx.theme().popover)
            .border_1()
            .border_color(p.line)
            .shadow_lg()
            .flex()
            .flex_col()
            .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                this.header_menu = None;
                cx.notify();
            }))
            .children(rows);
        let body = motion::animate(body, ("header-menu-in", menu.serial), 140, cx, |el, t| {
            el.opacity(t).mt(px(6. * (1. - t)))
        });
        Some(
            deferred(
                anchored()
                    .position(menu.position)
                    .snap_to_window_with_margin(px(8.))
                    .child(body),
            )
            .with_priority(2),
        )
    }
}
