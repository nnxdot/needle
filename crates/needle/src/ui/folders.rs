//! Browse by folder on disk: the music folders, then each folder's subfolders above the songs
//! directly in it.
use super::{
    AppView, Event, Page, pal,
    widgets::{cover, display, faint, glyph, meta},
};
use gpui::{prelude::*, *};
use needle_core::browse::Subfolder;

/// How a path is shown: without Windows' long-path prefix.
pub fn shown(path: &str) -> &str {
    path.trim_start_matches("\\\\?\\")
}

/// The folder path ending in its separator, so "Music" never also matches "Musical".
pub fn with_separator(path: &str) -> String {
    let separator = if path.contains('/') && !path.contains('\\') {
        '/'
    } else {
        '\\'
    };
    format!("{}{separator}", path.trim_end_matches(['\\', '/']))
}

/// The last part of a path.
pub fn name_of(path: &str) -> String {
    let path = path.trim_end_matches(['\\', '/']);
    path.rsplit(['\\', '/'])
        .next()
        .filter(|n| !n.is_empty())
        .unwrap_or(shown(path))
        .to_string()
}

impl AppView {
    /// Load the folders inside `folder` in the background.
    pub(super) fn load_subfolders(&mut self, folder: String) {
        let library = self.library.clone();
        let sender = self.sender.clone();
        std::thread::spawn(move || {
            let list = library.subfolders(&folder).unwrap_or_default();
            let _ = sender.send(Event::Subfolders(folder, list));
        });
    }

    /// The music folder `path` lies in, if any.
    fn root_of(&self, path: &str) -> Option<String> {
        let lower = path.to_lowercase();
        self.library
            .roots()
            .unwrap_or_default()
            .into_iter()
            .filter(|root| lower.starts_with(&root.to_lowercase()))
            .max_by_key(|root| root.len())
    }

    /// "Folders › Music › Albums" above a folder, each part clickable.
    pub(super) fn folder_crumbs(&self, folder: &str) -> Vec<(String, Option<Page>)> {
        let mut crumbs = vec![("Folders".to_string(), Some(Page::Folders))];
        let Some(root) = self.root_of(folder) else {
            return crumbs;
        };
        let mut path = root.trim_end_matches(['\\', '/']).to_string();
        crumbs.push((name_of(&path), Some(Page::Folder(path.clone()))));
        let rest = folder[path.len().min(folder.len())..].trim_matches(['\\', '/']);
        let separator = if path.contains('/') && !path.contains('\\') {
            '/'
        } else {
            '\\'
        };
        let parts: Vec<&str> = rest.split(['\\', '/']).filter(|p| !p.is_empty()).collect();
        for part in parts.iter().take(parts.len().saturating_sub(1)) {
            path = format!("{path}{separator}{part}");
            crumbs.push((part.to_string(), Some(Page::Folder(path.clone()))));
        }
        // The folder itself is the page title, so the trail stops at its parent.
        if parts.is_empty() {
            crumbs.pop();
        }
        crumbs
    }

    /// The Folders page: each music folder as a large card.
    pub(super) fn folders_view(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = pal(cx);
        let roots = self.library.roots().unwrap_or_default();
        div().id("folders").size_full().overflow_y_scroll().child(
            div()
                .px_6()
                .pt_6()
                .pb_10()
                .flex()
                .flex_col()
                .gap_2()
                .child(display("Folders", 34.))
                .child(meta("Your music folders, as they are on disk.", cx).mb_4())
                .when(roots.is_empty(), |el| {
                    el.child(faint("Add a music folder to browse it here.", cx))
                })
                .children(roots.into_iter().enumerate().map(|(i, root)| {
                    let page = Page::Folder(root.trim_end_matches(['\\', '/']).to_string());
                    div()
                        .id(("root", i))
                        .h(px(64.))
                        .px_4()
                        .rounded(px(10.))
                        .bg(p.raised.opacity(0.55))
                        .border_1()
                        .border_color(p.line_soft)
                        .flex()
                        .items_center()
                        .gap_4()
                        .cursor_pointer()
                        .hover(|s| s.bg(p.raised))
                        .child(glyph("folder").size(px(22.)).text_color(p.accent))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .child(
                                    div()
                                        .text_size(px(14.))
                                        .font_weight(FontWeight::MEDIUM)
                                        .truncate()
                                        .child(name_of(&root)),
                                )
                                .child(meta(shown(&root).to_string(), cx).truncate()),
                        )
                        .child(glyph("chevron-right").size(px(16.)).text_color(p.ink_3))
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.navigate(page.clone(), window, cx)
                        }))
                })),
        )
    }

    /// Above a folder's songs: its subfolders as a scrolling row of small covers.
    pub(super) fn folder_strip(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        let Page::Folder(folder) = &self.page else {
            return None;
        };
        let list: &[Subfolder] = match &self.subfolders {
            Some((path, list)) if path == folder => list,
            _ => return None,
        };
        if list.is_empty() {
            return None;
        }
        let p = pal(cx);
        let tiles: Vec<AnyElement> = list
            .iter()
            .enumerate()
            .map(|(i, sub)| {
                let page = Page::Folder(sub.path.clone());
                div()
                    .id(("subfolder", i))
                    .w(px(120.))
                    .flex_shrink_0()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .cursor_pointer()
                    .group("subfolder")
                    .child(
                        div()
                            .relative()
                            .rounded(px(8.))
                            .group_hover("subfolder", |s| s.opacity(0.85))
                            .child(cover(sub.artwork.as_deref(), &sub.name, 120., cx))
                            .child(
                                div()
                                    .absolute()
                                    .left(px(6.))
                                    .bottom(px(6.))
                                    .size(px(24.))
                                    .rounded(px(6.))
                                    .bg(p.chrome.opacity(0.85))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(glyph("folder").size(px(14.)).text_color(p.ink)),
                            ),
                    )
                    .child(
                        div()
                            .text_size(px(12.5))
                            .font_weight(FontWeight::MEDIUM)
                            .truncate()
                            .child(sub.name.clone()),
                    )
                    .child(faint(
                        if sub.tracks == 1 {
                            "1 song".to_string()
                        } else {
                            format!("{} songs", super::widgets::count(sub.tracks))
                        },
                        cx,
                    ))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.navigate(page.clone(), window, cx)
                    }))
                    .into_any_element()
            })
            .collect();
        Some(
            div()
                .id("folder-strip")
                .flex_shrink_0()
                .overflow_x_scroll()
                .pb_4()
                .child(div().px_6().flex().gap_4().children(tiles)),
        )
    }
}
