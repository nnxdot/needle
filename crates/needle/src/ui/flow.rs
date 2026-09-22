//! Moving around: remembered scroll positions, breadcrumbs, quick play from covers, and drag
//! and drop of songs onto playlists, favorites, and the queue.
use super::{AppView, Event, PLAY_LIMIT, Page, pal, widgets::glyph};
use gpui::{prelude::*, *};
use gpui_component::ActiveTheme;
use needle_core::audio::QueueItem;

/// Songs being dragged.
#[derive(Clone)]
pub struct DraggedTracks {
    pub ids: Vec<String>,
    pub label: SharedString,
}

pub struct DragPreview {
    label: SharedString,
    count: usize,
}
impl Render for DragPreview {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = pal(cx);
        div()
            .pl_2()
            .pr_3()
            .py_1()
            .rounded(px(8.))
            .bg(cx.theme().popover)
            .border_1()
            .border_color(p.accent.opacity(0.6))
            .shadow_lg()
            .flex()
            .items_center()
            .gap_2()
            .text_size(px(12.5))
            .text_color(p.ink)
            .child(glyph("songs").size(px(14.)).text_color(p.accent))
            .child(self.label.clone())
            .when(self.count > 1, |el| {
                el.child(div().px(px(6.)).rounded_full().bg(p.accent).text_color(p.accent_ink).text_size(px(11.)).child(self.count.to_string()))
            })
    }
}

pub fn drag_preview(value: &DraggedTracks, cx: &mut App) -> Entity<DragPreview> {
    let (label, count) = (value.label.clone(), value.ids.len());
    cx.new(|_| DragPreview { label, count })
}

fn page_key(page: &Page) -> String {
    format!("{page:?}")
}

impl AppView {
    /// Remember how far down the current page is scrolled.
    pub(super) fn remember_scroll(&mut self) {
        let offset = if self.page.is_grid() {
            self.grid_scroll.0.borrow().base_handle.offset()
        } else {
            self.list_scroll.0.borrow().base_handle.offset()
        };
        self.scroll_memory.insert(page_key(&self.page), offset);
    }

    /// Scroll a freshly loaded page back to where it was, or to the top.
    pub(super) fn restore_scroll(&mut self) {
        let offset = self.pending_scroll.take().unwrap_or_default();
        let handle = if self.page.is_grid() { &self.grid_scroll } else { &self.list_scroll };
        handle.0.borrow().base_handle.set_offset(offset);
    }

    pub(super) fn scroll_for_back(&mut self) {
        self.pending_scroll = self.scroll_memory.get(&page_key(&self.page)).copied();
    }

    /// Play everything a rule matches, in the background.
    pub(super) fn play_rule(&mut self, rule: String, reason: String) {
        let library = self.library.clone();
        let sender = self.sender.clone();
        std::thread::spawn(move || {
            let event = match library.search_page(&rule, 0, PLAY_LIMIT) {
                Ok(page) => Event::Play(
                    page.tracks.into_iter().filter(|t| !t.missing).map(|track| QueueItem { track, reason: reason.clone() }).collect(),
                    None,
                ),
                Err(e) => Event::Error(format!("{e:#}")),
            };
            let _ = sender.send(event);
        });
    }

    /// A round play button that appears over a cover on hover and plays the whole album or
    /// artist without leaving the page.
    pub(super) fn cover_play(&self, index: usize, page: Page, size: f32, cx: &mut Context<Self>) -> impl IntoElement {
        let p = pal(cx);
        let button = (size * 0.22).clamp(34., 46.);
        div()
            .id(("cover-play", index))
            .absolute()
            .right(px(10.))
            .bottom(px(10.))
            .size(px(button))
            .rounded_full()
            .bg(p.accent)
            .shadow_lg()
            .flex()
            .items_center()
            .justify_center()
            .opacity(0.)
            .group_hover("tile", |s| s.opacity(1.))
            .hover(|s| s.bg(p.accent.opacity(0.88)))
            .child(glyph("play").size(px(button * 0.42)).text_color(p.accent_ink))
            .tooltip(|window, cx| gpui_component::tooltip::Tooltip::new("Play").build(window, cx))
            .on_click(cx.listener(move |this, _, _, cx| {
                cx.stop_propagation();
                let reason = match &page {
                    Page::Album { album, .. } => format!("From the album {album}"),
                    Page::Artist(name) => format!("From {name}"),
                    _ => "From your library".into(),
                };
                this.play_rule(page.base(), reason);
            }))
    }

    /// "Albums › Artist" above an album, "Artists" above an artist, "Playlists" above a playlist.
    pub(super) fn breadcrumbs(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        let p = pal(cx);
        let crumbs: Vec<(String, Option<Page>)> = match &self.page {
            Page::Album { artist, .. } => vec![("Albums".into(), Some(Page::Albums)), (if artist.is_empty() { "Unknown artist".into() } else { artist.clone() }, Some(Page::Artist(artist.clone())))],
            Page::Artist(_) => vec![("Artists".into(), Some(Page::Artists))],
            Page::Playlist(id) => vec![(
                if self.playlists.iter().any(|p| &p.id == id && p.query.is_some()) { "Smart playlist" } else { "Playlist" }.into(),
                None,
            )],
            _ => return None,
        };
        Some(div().flex().items_center().gap_1().text_size(px(12.)).children(crumbs.into_iter().enumerate().flat_map(|(i, (label, target))| {
            let separator = (i > 0).then(|| glyph("chevron-right").size(px(12.)).text_color(p.ink_3).into_any_element());
            let crumb = div()
                .id(("crumb", i))
                .text_color(p.ink_3)
                .when(target.is_some(), |el| el.cursor_pointer().hover(|s| s.text_color(p.ink).underline()))
                .child(label)
                .when_some(target, |el, target| el.on_click(cx.listener(move |this, _, window, cx| this.navigate(target.clone(), window, cx))))
                .into_any_element();
            separator.into_iter().chain([crumb])
        })))
    }
}
