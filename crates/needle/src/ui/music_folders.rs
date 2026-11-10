//! Settings › Library › Music folders: the folders Needle reads, each with Show and Stop
//! reading. Stopping asks once, under the folder's row, in the Mac's words; the songs then
//! leave the library on another thread (`Library::remove_root`), and the files stay.
use super::{
    AppView, Event,
    theme::pal,
    widgets::{count, glyph, icon_button, meta, small_button},
};
use gpui::{prelude::*, *};
use gpui_component::{
    Disableable, Sizable,
    button::{Button, ButtonVariants},
};

/// The folder's name: the last part of its path, with either separator (the same on every
/// system, also for a path written on another one).
fn folder_name(root: &str) -> String {
    let shown = root.trim_start_matches("\\\\?\\");
    shown
        .trim_end_matches(['/', '\\'])
        .rsplit(['/', '\\'])
        .next()
        .filter(|name| !name.is_empty())
        .unwrap_or(shown)
        .to_string()
}

/// What the file manager is called here.
fn show_label() -> &'static str {
    if cfg!(windows) {
        "Show in File Explorer"
    } else {
        "Show in the file manager"
    }
}

/// The note after a folder was stopped.
pub(super) fn removed_note(root: &str, songs: usize) -> String {
    let songs = if songs == 1 {
        "1 song".to_string()
    } else {
        format!("{} songs", count(songs))
    };
    format!(
        "Stopped reading “{}”: {songs} left the library. The files stay.",
        folder_name(root)
    )
}

impl AppView {
    /// The list of music folders, with Add folder and Check for changes under it.
    pub(super) fn music_folders(&self, cx: &mut Context<Self>) -> Div {
        let p = pal(cx);
        let roots = self.library.roots().unwrap_or_default();
        let busy = self.folder_removing.is_some();
        div()
            .py_2()
            .flex()
            .flex_col()
            .gap_1()
            .when(roots.is_empty(), |el| el.child(meta("No folders yet.", cx)))
            .children(roots.into_iter().enumerate().map(|(index, root)| {
                let asked = self.folder_asked.as_deref() == Some(root.as_str());
                let removing = self.folder_removing.as_deref() == Some(root.as_str());
                let name = folder_name(&root);
                let shown = root.trim_start_matches("\\\\?\\").to_string();
                let (show, ask) = (shown.clone(), root.clone());
                let row = div()
                    .h(px(34.))
                    .pl_3()
                    .pr_1()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(glyph("folder").size(px(16.)).text_color(p.ink_3))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_size(px(12.5))
                            .truncate()
                            .child(shown),
                    )
                    .when(removing, |el| el.child(meta("Stopping…", cx)))
                    .child(
                        icon_button(("folder-show", index), "folder", show_label())
                            .small()
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.open_folder(std::path::Path::new(&show), cx);
                            })),
                    )
                    .child(
                        icon_button(("folder-stop", index), "minus", "Stop reading this folder")
                            .small()
                            .disabled(busy)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.folder_asked = Some(ask.clone());
                                cx.notify();
                            })),
                    );
                let confirm = asked.then(|| {
                    let stop = root.clone();
                    div()
                        .px_3()
                        .pb_3()
                        .pt_1()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(
                            div()
                                .text_size(px(13.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(format!("Stop reading “{name}”?")),
                        )
                        .child(meta(
                            "Its songs leave the library, with their plays, stars, and places in playlists. The files stay.",
                            cx,
                        ).w_full())
                        .child(
                            div()
                                .flex()
                                .gap_2()
                                .child(
                                    Button::new(("folder-stop-yes", index))
                                        .small()
                                        .danger()
                                        .label("Stop reading it")
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.remove_folder(stop.clone(), cx);
                                        })),
                                )
                                .child(small_button(("folder-stop-no", index), "Cancel").ghost().on_click(
                                    cx.listener(|this, _, _, cx| {
                                        this.folder_asked = None;
                                        cx.notify();
                                    }),
                                )),
                        )
                });
                div()
                    .rounded(px(6.))
                    .border_1()
                    .border_color(p.line_soft)
                    .flex()
                    .flex_col()
                    .child(row)
                    .children(confirm)
            }))
            .child(
                div()
                    .mt_2()
                    .flex()
                    .gap_2()
                    .child(
                        small_button("settings-add", "Add folder")
                            .icon(super::widgets::icon("plus"))
                            .disabled(busy)
                            .on_click(
                                cx.listener(|this, _, window, cx| this.import_folder(window, cx)),
                            ),
                    )
                    .child(
                        small_button("rescan", "Check for changes")
                            .ghost()
                            .disabled(self.scan.is_some() || busy)
                            .on_click(cx.listener(|this, _, _, cx| this.rescan(cx))),
                    ),
            )
    }

    /// Stop reading `root`, off the UI thread. A scan running now is stopped first, since it
    /// would add the folder back as it ends; the folder goes once it has stopped.
    pub(super) fn remove_folder(&mut self, root: String, cx: &mut Context<Self>) {
        self.folder_asked = None;
        self.folder_removing = Some(root.clone());
        if self.scan.is_some() {
            self.cancel
                .store(true, std::sync::atomic::Ordering::Relaxed);
            self.folder_after_scan = true;
        } else {
            self.start_removing_folder(root);
        }
        cx.notify();
    }

    fn start_removing_folder(&mut self, root: String) {
        let library = self.library.clone();
        let sender = self.sender.clone();
        std::thread::spawn(move || {
            let result = library.remove_root(&root).map_err(|e| format!("{e:#}"));
            let _ = sender.send(Event::FolderRemoved(root, result));
        });
    }

    /// The scan ended (finished, stopped, or failed): a folder waiting for it goes now.
    pub(super) fn scan_ended(&mut self) {
        if std::mem::take(&mut self.folder_after_scan)
            && let Some(root) = self.folder_removing.clone()
        {
            self.start_removing_folder(root);
        }
    }

    /// A folder was stopped: the library, the pages, and the folder watcher follow.
    pub(super) fn folder_removed(
        &mut self,
        root: String,
        result: Result<usize, String>,
        cx: &mut Context<Self>,
    ) {
        self.folder_removing = None;
        match result {
            Ok(songs) => {
                // Watch only the folders still read.
                self._watcher = super::watch(&self.library, &self.sender);
                self.total = self.library.count().unwrap_or(0);
                if self.page == super::Page::Home {
                    self.load_home();
                }
                if self
                    .focused
                    .as_ref()
                    .is_some_and(|t| self.library.track(&t.id).ok().flatten().is_none())
                {
                    self.focused = None;
                }
                self.refresh(cx);
                self.notify(removed_note(&root, songs));
            }
            Err(error) => self.fail(format!("Could not stop reading the folder: {error}")),
        }
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::{folder_name, removed_note};

    #[test]
    fn names_and_notes() {
        assert_eq!(folder_name(r"\\?\D:\Music\Jazz"), "Jazz");
        assert_eq!(folder_name("/home/a/Music/"), "Music");
        assert_eq!(
            removed_note(r"D:\Music", 1),
            "Stopped reading “Music”: 1 song left the library. The files stay."
        );
        assert_eq!(
            removed_note("/m/Rock", 1200),
            "Stopped reading “Rock”: 1,200 songs left the library. The files stay."
        );
    }
}
