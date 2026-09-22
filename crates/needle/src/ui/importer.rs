use super::{
    AppView, Event, pal,
    widgets::{faint, glyph, heading, meta, page_title, small_button, strong},
};
use gpui::{prelude::*, *};
use gpui_component::{
    Disableable, Sizable,
    button::ButtonVariants,
    input::{Input, InputState},
};
use needle_core::import::{self, Detected, ImportReport, SourceKind};
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

pub struct ImportState {
    pub detected: Vec<Detected>,
    pub busy: Option<String>,
    pub cancel: Arc<AtomicBool>,
    pub results: Vec<String>,
    pub lastfm_user: Entity<InputState>,
    pub listenbrainz_user: Entity<InputState>,
}

impl ImportState {
    pub fn new(window: &mut Window, cx: &mut Context<AppView>) -> Self {
        Self {
            detected: vec![],
            busy: None,
            cancel: Arc::new(AtomicBool::new(false)),
            results: vec![],
            lastfm_user: cx.new(|cx| InputState::new(window, cx).placeholder("Last.fm user name")),
            listenbrainz_user: cx
                .new(|cx| InputState::new(window, cx).placeholder("ListenBrainz user name")),
        }
    }
}

enum Job {
    Itunes(PathBuf),
    Spotify(PathBuf),
    Lastfm(String),
    ListenBrainz(String),
    Playlists(PathBuf),
}

impl AppView {
    pub(super) fn refresh_import_sources(&mut self) {
        self.import.detected = import::detect();
    }

    fn start_import(&mut self, job: Job, cx: &mut Context<Self>) {
        if self.import.busy.is_some() {
            return;
        }
        let cancel = Arc::new(AtomicBool::new(false));
        self.import.cancel = cancel.clone();
        self.import.busy = Some("Starting…".into());
        let library = self.library.clone();
        let sender = self.sender.clone();
        std::thread::spawn(move || {
            let progress_sender = sender.clone();
            let mut progress = move |message: &str| {
                let _ = progress_sender.send(Event::ImportProgress(message.to_string()));
            };
            let result: anyhow::Result<ImportReport> = match job {
                Job::Itunes(path) => import::import_itunes(&library, &path, &mut progress),
                Job::Spotify(path) => import::import_spotify(&library, &path, &mut progress),
                Job::Playlists(path) => {
                    import::import_playlist_folder(&library, &path, &mut progress)
                }
                Job::Lastfm(user) => {
                    let key = needle_core::integrations::Credentials::load().lastfm_api_key;
                    import::import_lastfm(&library, &user, &key, cancel, &mut progress)
                }
                Job::ListenBrainz(user) => {
                    import::import_listenbrainz(&library, &user, cancel, &mut progress)
                }
            };
            let _ = sender.send(Event::ImportDone(result.map_err(|e| format!("{e:#}"))));
        });
        cx.notify();
    }

    fn pick_and_import(&mut self, kind: SourceKind) {
        let sender = self.sender.clone();
        std::thread::spawn(move || {
            let dialog = rfd::FileDialog::new();
            let path = match kind {
                SourceKind::Itunes => dialog
                    .set_title("Choose the exported library XML")
                    .add_filter("Library XML", &["xml"])
                    .pick_file(),
                SourceKind::Spotify => dialog
                    .set_title("Choose the Spotify data ZIP (or cancel to pick its folder)")
                    .add_filter("Spotify export", &["zip", "json"])
                    .pick_file()
                    .or_else(|| {
                        rfd::FileDialog::new()
                            .set_title("Choose the Spotify data folder")
                            .pick_folder()
                    }),
                _ => dialog
                    .set_title("Choose a folder of M3U playlists")
                    .pick_folder(),
            };
            if let Some(path) = path {
                let _ = sender.send(Event::ImportPicked(kind, path));
            }
        });
    }

    pub(super) fn import_picked(
        &mut self,
        kind: SourceKind,
        path: PathBuf,
        cx: &mut Context<Self>,
    ) {
        let job = match kind {
            SourceKind::Itunes => Job::Itunes(path),
            SourceKind::Spotify => Job::Spotify(path),
            _ => Job::Playlists(path),
        };
        self.start_import(job, cx);
    }

    fn source_card(
        &self,
        id: &'static str,
        glyph_name: &'static str,
        title: &str,
        body: &str,
        how: &str,
        action: impl IntoElement,
        cx: &App,
    ) -> Stateful<Div> {
        let p = pal(cx);
        div()
            .id(id)
            .p_4()
            .rounded(px(10.))
            .bg(p.chrome)
            .border_1()
            .border_color(p.line_soft)
            .flex()
            .items_start()
            .gap_4()
            .child(
                div()
                    .size(px(36.))
                    .rounded(px(8.))
                    .bg(p.raised)
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(glyph(glyph_name).size(px(18.)).text_color(p.accent)),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(strong(title.to_string()))
                    .child(meta(body.to_string(), cx).w_full())
                    .when(!how.is_empty(), |el| {
                        el.child(faint(how.to_string(), cx).w_full().mt_1())
                    })
                    .child(div().mt_3().child(action)),
            )
    }

    pub(super) fn import_view(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = pal(cx);
        let busy = self.import.busy.is_some();
        div()
            .id("import-scroll")
            .size_full()
            .overflow_y_scroll()
            .child(
                div()
                    .max_w(px(760.))
                    .px_8()
                    .pt_6()
                    .pb_16()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(page_title("Bring your music history"))
                    .child(meta("Ratings, play counts, listening history, and playlists come across. Your files are never changed, and importing again skips what is already here.", cx))
                    .when_some(self.import.busy.clone(), |el, message| {
                        let cancel = self.import.cancel.clone();
                        el.child(
                            div()
                                .mt_2()
                                .p_3()
                                .rounded(px(8.))
                                .bg(p.accent_soft)
                                .flex()
                                .items_center()
                                .gap_3()
                                .child(div().flex_1().text_size(px(13.)).child(message))
                                .child(small_button("import-stop", "Stop").ghost().on_click(move |_, _, _| cancel.store(true, Ordering::Relaxed))),
                        )
                    })
                    .children(self.import.results.iter().rev().take(4).map(|r| meta(r.clone(), cx).px_1()))
                    .when(!self.import.detected.is_empty(), |el| {
                        el.child(div().mt_4().child(heading("Found on this computer"))).children(self.import.detected.iter().enumerate().map(|(i, found)| {
                            let (kind, path) = (found.kind, found.path.clone());
                            div()
                                .id(("detected", i))
                                .p_3()
                                .rounded(px(8.))
                                .border_1()
                                .border_color(p.accent.opacity(0.35))
                                .flex()
                                .items_center()
                                .gap_3()
                                .child(glyph("import").size(px(18.)).text_color(p.accent))
                                .child(div().flex_1().min_w_0().child(strong(found.label.clone())).child(faint(found.detail.clone(), cx).truncate()))
                                .child(small_button(("detected-import", i), "Import").primary().disabled(busy).on_click(cx.listener(move |this, _, _, cx| this.import_picked(kind, path.clone(), cx))))
                        }))
                    })
                    .child(div().mt_4().child(heading("Choose a source")))
                    .child(self.source_card(
                        "src-itunes",
                        "albums",
                        "iTunes, Apple Music, or MusicBee",
                        "Star ratings, play counts, last played and date added, and your playlists.",
                        "Export first. iTunes: File › Library › Export Library. Apple Music: File › Library › Export Library. MusicBee: Preferences › Library › export an iTunes XML file.",
                        small_button("pick-itunes", "Choose library XML…").disabled(busy).on_click(cx.listener(|this, _, _, _| this.pick_and_import(SourceKind::Itunes))),
                        cx,
                    ))
                    .child(self.source_card(
                        "src-spotify",
                        "globe",
                        "Spotify",
                        "Every stream in your history, and your playlists, matched to the songs you own.",
                        "Ask Spotify for your data at spotify.com/account/privacy. Choose \"Extended streaming history\" for everything since you joined. Then pick the ZIP or folder it sends.",
                        small_button("pick-spotify", "Choose Spotify download…").disabled(busy).on_click(cx.listener(|this, _, _, _| this.pick_and_import(SourceKind::Spotify))),
                        cx,
                    ))
                    .child(self.source_card(
                        "src-lastfm",
                        "history",
                        "Last.fm",
                        "Every scrobble on your profile. Runs again later to fetch only new ones.",
                        "Needs a Last.fm API key, set in Settings › Listening services.",
                        div()
                            .flex()
                            .gap_2()
                            .child(Input::new(&self.import.lastfm_user).small().w(px(240.)))
                            .child(small_button("import-lastfm", "Import").disabled(busy).on_click(cx.listener(|this, _, _, cx| {
                                let user = this.import.lastfm_user.read(cx).value().trim().to_string();
                                if user.is_empty() {
                                    return this.fail("Enter a Last.fm user name first.");
                                }
                                this.start_import(Job::Lastfm(user), cx);
                            }))),
                        cx,
                    ))
                    .child(self.source_card(
                        "src-listenbrainz",
                        "history",
                        "ListenBrainz",
                        "Every public listen on your profile. No sign-in needed.",
                        "",
                        div()
                            .flex()
                            .gap_2()
                            .child(Input::new(&self.import.listenbrainz_user).small().w(px(240.)))
                            .child(small_button("import-listenbrainz", "Import").disabled(busy).on_click(cx.listener(|this, _, _, cx| {
                                let user = this.import.listenbrainz_user.read(cx).value().trim().to_string();
                                if user.is_empty() {
                                    return this.fail("Enter a ListenBrainz user name first.");
                                }
                                this.start_import(Job::ListenBrainz(user), cx);
                            }))),
                        cx,
                    ))
                    .child(self.source_card(
                        "src-playlists",
                        "playlist",
                        "A folder of playlists",
                        "Every .m3u and .m3u8 file in a folder, for example exported from foobar2000 or MusicBee.",
                        "",
                        small_button("pick-playlists", "Choose folder…").disabled(busy).on_click(cx.listener(|this, _, _, _| this.pick_and_import(SourceKind::PlaylistFolder))),
                        cx,
                    ))
                    .child(faint("Listens for songs you don't have are kept in your history, so your statistics stay complete. Imported listens are never sent to Last.fm or ListenBrainz.", cx).mt_4()),
            )
    }
}
