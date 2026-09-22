use anyhow::Result;
use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme, Disableable, Icon, IconName, Root, Selectable, Sizable, Theme, ThemeMode,
    button::{Button, ButtonVariants},
    input::{Input, InputEvent, InputState},
    slider::{Slider, SliderEvent, SliderState},
};
use needle_core::{
    audio::{self, Command, PlaybackState, Player, QueueItem, Repeat},
    database::Library,
    integrations::{self, RecordingMatch},
    model::{Listen, Playlist, Settings, Track, format_duration},
    query,
    scan::{self, ScanProgress, TagEdit},
};
use std::{
    borrow::Cow,
    collections::BTreeMap,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

actions!(
    needle,
    [
        TogglePlayback,
        NextTrack,
        PreviousTrack,
        FocusSearch,
        ImportFolder,
        EscapePanel
    ]
);

struct Assets;
impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        let shape = match path {
            "needle/play.svg" => {
                Some("<path d='m8 5 11 7-11 7z' fill='currentColor' stroke='none'/>")
            }
            "needle/pause.svg" => Some("<path d='M8 5v14M16 5v14' stroke-width='4'/>"),
            "needle/next.svg" => Some(
                "<path d='m5 5 10 7-10 7z' fill='currentColor' stroke='none'/><path d='M19 5v14'/>",
            ),
            "needle/previous.svg" => Some(
                "<path d='m19 5-10 7 10 7z' fill='currentColor' stroke='none'/><path d='M5 5v14'/>",
            ),
            "needle/disc.svg" => Some(
                "<circle cx='12' cy='12' r='9'/><circle cx='12' cy='12' r='2'/><path d='M5.6 9a7 7 0 0 1 3.4-3.4M15 18.4a7 7 0 0 0 3.4-3.4'/>",
            ),
            "needle/music.svg" => Some(
                "<path d='M9 18V5l12-2v13M9 8l12-2'/><ellipse cx='6' cy='18' rx='3' ry='2'/><ellipse cx='18' cy='16' rx='3' ry='2'/>",
            ),
            "needle/queue.svg" => Some("<path d='M3 6h18M3 12h12M3 18h9m6-3 4 3-4 3z'/>"),
            "needle/history.svg" => Some("<path d='M3 11a9 9 0 1 1 2.5 7M3 4v7h7M12 7v5l3 2'/>"),
            "needle/shuffle.svg" => {
                Some("<path d='m17 3 4 4-4 4M3 17l4-4m-4-6h4l10 10h4m-4-4 4 4-4 4M13 11l4-4h4'/>")
            }
            "needle/repeat.svg" => Some(
                "<path d='m17 2 4 4-4 4M3 11V8a2 2 0 0 1 2-2h16M7 22l-4-4 4-4m14-1v3a2 2 0 0 1-2 2H3'/>",
            ),
            "needle/volume.svg" => {
                Some("<path d='M11 5 6 9H3v6h3l5 4zM15 8a6 6 0 0 1 0 8m3-11a10 10 0 0 1 0 14'/>")
            }
            _ => None,
        };
        if let Some(shape) = shape {
            return Ok(Some(Cow::Owned(format!("<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='1.7' stroke-linecap='round' stroke-linejoin='round'>{shape}</svg>").into_bytes())));
        }
        gpui_component_assets::Assets.load(path)
    }
    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        gpui_component_assets::Assets.list(path)
    }
}

#[derive(Clone, PartialEq)]
enum Page {
    Library,
    Albums,
    Artists,
    Favorites,
    Recent,
    History,
    Queue,
    Playlist(String),
    Settings,
}
impl Page {
    fn title(&self) -> &str {
        match self {
            Self::Library => "Your library",
            Self::Albums => "Albums",
            Self::Artists => "Artists",
            Self::Favorites => "Favorites",
            Self::Recent => "Recently added",
            Self::History => "Listening history",
            Self::Queue => "Play queue",
            Self::Playlist(_) => "Playlist",
            Self::Settings => "Settings",
        }
    }
}
enum Event {
    Imported(ScanProgress),
    Error(String),
    Loaded(u64, Vec<Track>, usize),
    SearchFailed(u64, String),
    Matches(String, Vec<RecordingMatch>),
    LibraryChanged,
    Layout(needle_core::model::Layout),
    SavedTags,
    Notice(String),
}

struct AppView {
    library: Library,
    player: Player,
    playback: PlaybackState,
    settings: Settings,
    page: Page,
    tracks: Vec<Track>,
    playlists: Vec<Playlist>,
    history: Vec<Listen>,
    selected: Option<Track>,
    total: usize,
    matched_total: usize,
    page_offset: usize,
    loop_start: Option<f64>,
    search: Entity<InputState>,
    playlist_name: Entity<InputState>,
    autoplay: Entity<InputState>,
    sync_phrase: Entity<InputState>,
    tag_title: Entity<InputState>,
    tag_artist: Entity<InputState>,
    tag_album: Entity<InputState>,
    volume: Entity<SliderState>,
    seek: Entity<SliderState>,
    focus: FocusHandle,
    _subscriptions: Vec<Subscription>,
    events: crossbeam_channel::Receiver<Event>,
    sender: crossbeam_channel::Sender<Event>,
    scan: Option<ScanProgress>,
    cancel: Arc<AtomicBool>,
    notice: Option<String>,
    query_error: Option<String>,
    generation: u64,
    loading: bool,
    show_save: bool,
    confirm_delete: bool,
    editing: bool,
    matches: Vec<RecordingMatch>,
    lookup_busy: bool,
    pending_mbid: Option<String>,
    _scrobbler: integrations::ScrobbleWorker,
    output_devices: Vec<String>,
    _watcher: Option<notify_placeholder::Watcher>,
    last_history_id: Option<String>,
}

// Keep the watcher's concrete type behind an inferred owner without coupling the UI crate to notify.
mod notify_placeholder {
    pub struct Watcher {
        pub _inner: Box<dyn std::any::Any>,
    }
}

fn icon(name: &str) -> Svg {
    svg()
        .path(SharedString::from(format!("needle/{name}.svg")))
        .size(px(18.0))
}
fn color(value: u32) -> Hsla {
    rgb(value).into()
}
fn muted(cx: &App) -> Hsla {
    cx.theme().muted_foreground
}
fn small(text: impl Into<SharedString>, cx: &App) -> Div {
    div()
        .text_size(px(12.))
        .text_color(muted(cx))
        .child(text.into())
}
fn label(text: impl Into<SharedString>) -> Div {
    div()
        .text_size(px(13.))
        .font_weight(FontWeight::MEDIUM)
        .child(text.into())
}

pub fn run(library: Library) -> Result<()> {
    Application::new().with_assets(Assets).run(move |cx| {
        gpui_component::init(cx);
        let settings = library.settings().unwrap_or_default();
        set_theme(&settings.theme, None, cx);
        cx.bind_keys([
            KeyBinding::new("space", TogglePlayback, Some("Needle && !Input")),
            KeyBinding::new("ctrl-right", NextTrack, Some("Needle && !Input")),
            KeyBinding::new("ctrl-left", PreviousTrack, Some("Needle && !Input")),
            KeyBinding::new("ctrl-f", FocusSearch, Some("Needle")),
            KeyBinding::new("ctrl-o", ImportFolder, Some("Needle")),
            KeyBinding::new("escape", EscapePanel, Some("Needle")),
        ]);
        let bounds = Bounds::centered(None, size(px(1360.), px(880.)), cx);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            window_min_size: Some(size(px(960.), px(640.))),
            titlebar: Some(TitlebarOptions {
                title: Some("Needle — Your music, in its place".into()),
                appears_transparent: false,
                ..Default::default()
            }),
            ..Default::default()
        };
        match cx.open_window(options, move |window, cx| {
            let view = cx.new(|cx| AppView::new(library, window, cx));
            cx.new(|cx| Root::new(view, window, cx))
        }) {
            Ok(_) => cx.activate(true),
            Err(e) => eprintln!("Unable to open Needle: {e:#}"),
        }
        cx.on_window_closed(|cx| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
    });
    Ok(())
}

fn set_theme(mode: &str, window: Option<&mut Window>, cx: &mut App) {
    let dark = mode != "light";
    Theme::change(
        if dark {
            ThemeMode::Dark
        } else {
            ThemeMode::Light
        },
        window,
        cx,
    );
    let theme = Theme::global_mut(cx);
    theme.font_family = "Segoe UI".into();
    theme.font_size = px(14.);
    theme.radius = px(7.);
    theme.shadow = false;
    theme.colors.background = color(if dark { 0x17191b } else { 0xffffff });
    theme.colors.foreground = color(if dark { 0xedeeef } else { 0x222527 });
    theme.colors.muted = color(if dark { 0x202326 } else { 0xf3f4f5 });
    theme.colors.muted_foreground = color(if dark { 0xa3a8af } else { 0x626871 });
    theme.colors.border = color(if dark { 0x303438 } else { 0xe3e5e8 });
    theme.colors.primary = color(if dark { 0xdeb16c } else { 0x855319 });
    theme.colors.primary_foreground = color(if dark { 0x1b1d20 } else { 0xffffff });
    theme.colors.primary_hover = color(if dark { 0xeac082 } else { 0x704310 });
    theme.colors.primary_active = color(if dark { 0xd09f54 } else { 0x623907 });
    theme.colors.secondary = color(if dark { 0x272b2f } else { 0xf0f1f3 });
    theme.colors.secondary_foreground = theme.colors.foreground;
    theme.colors.accent = color(if dark { 0x343028 } else { 0xf3e8d6 });
    theme.colors.accent_foreground = theme.colors.foreground;
}

impl AppView {
    fn new(library: Library, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let settings = library.settings().unwrap_or_default();
        let player = Player::new(library.clone());
        let search = cx.new(|cx| {
            InputState::new(window, cx).placeholder("Search your music, or write a rule…")
        });
        let playlist_name = cx.new(|cx| InputState::new(window, cx).placeholder("Playlist name"));
        let autoplay = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("rating >= 4 and not played(7d) shuffle limit 20")
        });
        autoplay.update(cx, |s, cx| {
            s.set_value(settings.autoplay_query.clone(), window, cx)
        });
        let sync_phrase = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Passphrase · at least 12 characters")
                .masked(true)
        });
        let tag_title = cx.new(|cx| InputState::new(window, cx).placeholder("Title"));
        let tag_artist = cx.new(|cx| InputState::new(window, cx).placeholder("Artist"));
        let tag_album = cx.new(|cx| InputState::new(window, cx).placeholder("Album"));
        let volume = cx.new(|_| {
            SliderState::new()
                .min(0.)
                .max(1.)
                .step(0.01)
                .default_value(settings.volume)
        });
        let seek = cx.new(|_| SliderState::new().min(0.).max(1000.).step(1.));
        let focus = cx.focus_handle();
        window.focus(&focus);
        let (sender, events) = crossbeam_channel::unbounded();
        let subscriptions = vec![
            cx.subscribe(&search, |this, _, event, cx| {
                if matches!(event, InputEvent::Change) {
                    this.page_offset = 0;
                    if matches!(this.page, Page::Settings | Page::History | Page::Queue)
                        && !this.search.read(cx).value().is_empty()
                    {
                        this.page = Page::Library;
                    }
                    this.refresh(cx);
                }
            }),
            cx.subscribe(&volume, |this, _, event, _| {
                let SliderEvent::Change(value) = event;
                this.player.send(Command::Volume(value.start()));
            }),
            cx.subscribe(&seek, |this, _, event, _| {
                let SliderEvent::Change(value) = event;
                if let Some(item) = &this.playback.current {
                    this.player.send(Command::Seek(
                        value.start() as f64 / 1000.0 * item.track.duration,
                    ));
                }
            }),
        ];
        let watcher = scan::watch(library.clone(), {
            let sender = sender.clone();
            move || {
                let _ = sender.send(Event::LibraryChanged);
            }
        })
        .ok()
        .map(|w| notify_placeholder::Watcher {
            _inner: Box::new(w),
        });
        let library_for_scrobbles = library.clone();
        let mut view = Self {
            total: library.count().unwrap_or(0),
            playlists: library.playlists().unwrap_or_default(),
            library,
            player,
            playback: PlaybackState::default(),
            settings,
            page: Page::Library,
            tracks: vec![],
            history: vec![],
            selected: None,
            matched_total: 0,
            page_offset: 0,
            loop_start: None,
            search,
            playlist_name,
            autoplay,
            sync_phrase,
            tag_title,
            tag_artist,
            tag_album,
            volume,
            seek,
            focus,
            _subscriptions: subscriptions,
            events,
            sender,
            scan: None,
            cancel: Arc::new(AtomicBool::new(false)),
            notice: None,
            query_error: None,
            generation: 0,
            loading: false,
            show_save: false,
            confirm_delete: false,
            editing: false,
            matches: vec![],
            lookup_busy: false,
            pending_mbid: None,
            _scrobbler: integrations::ScrobbleWorker::start(library_for_scrobbles),
            output_devices: audio::devices().unwrap_or_default(),
            _watcher: watcher,
            last_history_id: None,
        };
        view.refresh(cx);
        cx.spawn_in(window, async move |view, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(120))
                    .await;
                if view
                    .update_in(cx, |view, window, cx| view.poll(window, cx))
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
        view
    }
    fn persist_settings(&mut self) {
        self.settings.volume = self.playback.volume;
        if let Err(e) = self.library.save_settings(&self.settings) {
            self.notice = Some(e.to_string());
        }
    }
    fn poll(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let playback = self.player.state();
        if playback.error != self.playback.error
            && let Some(error) = &playback.error
        {
            self.notice = Some(error.clone());
        }
        let changed = playback.playing != self.playback.playing;
        self.playback = playback;
        let id = self.playback.current.as_ref().map(|i| i.track.id.clone());
        if changed || id != self.last_history_id {
            self.last_history_id = id;
            self.history = self.library.history(200).unwrap_or_default();
        }
        if let Some(item) = &self.playback.current {
            let value = if item.track.duration > 0.0 {
                (self.playback.position / item.track.duration * 1000.0) as f32
            } else {
                0.
            };
            self.seek
                .update(cx, |state, cx| state.set_value(value, window, cx));
        }
        while let Ok(event) = self.events.try_recv() {
            match event {
                Event::Imported(progress) => {
                    if progress.done {
                        self.notice = Some(format!(
                            "{} tracks added or updated · {} already indexed{}",
                            progress.imported,
                            progress.unchanged,
                            if progress.errors.is_empty() {
                                String::new()
                            } else {
                                format!(" · {} files could not be read", progress.errors.len())
                            }
                        ));
                        self.scan = None;
                        self.total = self.library.count().unwrap_or(0);
                        self.refresh(cx);
                        self._watcher = scan::watch(self.library.clone(), {
                            let sender = self.sender.clone();
                            move || {
                                let _ = sender.send(Event::LibraryChanged);
                            }
                        })
                        .ok()
                        .map(|w| notify_placeholder::Watcher {
                            _inner: Box::new(w),
                        });
                    } else {
                        self.scan = Some(progress);
                    }
                }
                Event::Error(error) => {
                    self.notice = Some(error);
                    self.scan = None;
                    self.lookup_busy = false;
                    self.loading = false;
                }
                Event::Loaded(generation, tracks, total) => {
                    if generation == self.generation {
                        self.tracks = tracks;
                        self.matched_total = total;
                        self.loading = false;
                    }
                }
                Event::SearchFailed(generation, error) => {
                    if generation == self.generation {
                        self.query_error = Some(error);
                        self.loading = false;
                    }
                }
                Event::Layout(layout) => {
                    self.settings.layout = layout;
                    self.persist_settings();
                    self.notice = Some("Layout applied.".into());
                }
                Event::LibraryChanged => {
                    self.total = self.library.count().unwrap_or(0);
                    self.refresh(cx);
                }
                Event::Matches(id, matches) => {
                    self.lookup_busy = false;
                    if self.selected.as_ref().is_none_or(|t| t.id != id) {
                        continue;
                    }
                    self.matches = matches;
                    self.lookup_busy = false;
                    if self.matches.is_empty() {
                        self.notice = Some(
                            "No matching recordings found. Try editing the artist or title first."
                                .into(),
                        );
                    }
                }
                Event::SavedTags => {
                    self.editing = false;
                    self.matches.clear();
                    self.notice = Some(
                        "Tags saved. A backup of the original file is in your library data folder."
                            .into(),
                    );
                    if let Some(track) = &self.selected {
                        self.selected = self.library.track(&track.id).ok().flatten();
                    }
                    self.refresh(cx);
                }
                Event::Notice(notice) => {
                    self.notice = Some(notice);
                    self.refresh(cx);
                    self.playlists = self.library.playlists().unwrap_or_default();
                    self.history = self.library.history(200).unwrap_or_default();
                    if let Some(track) = &self.selected {
                        self.selected = self.library.track(&track.id).ok().flatten();
                    }
                }
            }
        }
        cx.notify();
    }
    fn expression(&self, cx: &App) -> String {
        let search = self.search.read(cx).value().to_string();
        let base = match &self.page {
            Page::Favorites => "rating >= 4".into(),
            Page::Recent => "recent(30d)".into(),
            _ => String::new(),
        };
        if base.is_empty() {
            search
        } else if search.trim().is_empty() {
            base
        } else {
            let escaped = search.replace('\\', "\\\\").replace('"', "\\\"");
            format!(
                "({base}) and (title contains \"{escaped}\" or artist contains \"{escaped}\" or album contains \"{escaped}\")"
            )
        }
    }
    fn refresh(&mut self, cx: &mut Context<Self>) {
        let expression = self.expression(cx);
        self.generation += 1;
        let generation = self.generation;
        if let Err(error) = query::compile(&expression, chrono::Utc::now().timestamp()) {
            self.query_error = Some(error.to_string());
            self.loading = false;
            cx.notify();
            return;
        }
        self.query_error = None;
        self.loading = true;
        let library = self.library.clone();
        let sender = self.sender.clone();
        let manual = if let Page::Playlist(id) = &self.page {
            self.playlists.iter().find(|p| &p.id == id).cloned()
        } else {
            None
        };
        let offset = self.page_offset;
        let filter = self.search.read(cx).value().to_string();
        std::thread::spawn(move || {
            let result = (|| -> Result<needle_core::database::SearchPage> {
                if let Some(list) = manual {
                    let mut tracks = library.playlist_tracks(&list)?;
                    if !filter.trim().is_empty() {
                        let ids = library
                            .search(&filter)?
                            .into_iter()
                            .map(|t| t.id)
                            .collect::<std::collections::HashSet<_>>();
                        tracks.retain(|t| ids.contains(&t.id));
                    }
                    let total = tracks.len();
                    let tracks = tracks.into_iter().skip(offset).take(1000).collect();
                    Ok(needle_core::database::SearchPage { tracks, total })
                } else {
                    library.search_page(&expression, offset, 1000)
                }
            })();
            match result {
                Ok(page) => {
                    let _ = sender.send(Event::Loaded(generation, page.tracks, page.total));
                }
                Err(e) => {
                    let _ = sender.send(Event::SearchFailed(generation, format!("{e:#}")));
                }
            }
        });
        cx.notify();
    }
    fn navigate(&mut self, page: Page, window: &mut Window, cx: &mut Context<Self>) {
        self.page = page;
        self.confirm_delete = false;
        if let Page::Playlist(id) = &self.page
            && let Some(list) = self.playlists.iter().find(|p| &p.id == id)
        {
            self.playlist_name
                .update(cx, |s, cx| s.set_value(list.name.clone(), window, cx));
        }
        self.page_offset = 0;
        self.search.update(cx, |s, cx| s.set_value("", window, cx));
        self.show_save = false;
        self.history = self.library.history(200).unwrap_or_default();
        self.refresh(cx);
        window.focus(&self.focus);
    }
    fn import_folder(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.scan.is_some() {
            return;
        }
        let library = self.library.clone();
        let sender = self.sender.clone();
        let cancel = Arc::new(AtomicBool::new(false));
        self.cancel = cancel.clone();
        self.scan = Some(ScanProgress {
            current: "Choose a music folder".into(),
            ..Default::default()
        });
        cx.spawn_in(window, async move |_, _| {
            std::thread::spawn(move || {
                if let Some(folder) = rfd::FileDialog::new()
                    .set_title("Add a music folder to Needle")
                    .pick_folder()
                {
                    if let Err(e) = scan::import(&library, &folder, cancel, |p| {
                        let _ = sender.send(Event::Imported(p));
                    }) {
                        let _ = sender.send(Event::Error(format!("{e:#}")));
                    }
                } else {
                    let _ = sender.send(Event::Imported(ScanProgress {
                        done: true,
                        ..Default::default()
                    }));
                }
            });
        })
        .detach();
        cx.notify();
    }
    fn demo(&mut self, cx: &mut Context<Self>) {
        if self.scan.is_some() {
            return;
        }
        self.scan = Some(ScanProgress {
            current: "Preparing three original demo recordings…".into(),
            ..Default::default()
        });
        let library = self.library.clone();
        let sender = self.sender.clone();
        std::thread::spawn(move || {
            let result = (|| -> Result<()> {
                let path = library.directory.join("demo");
                needle_core::demo::create(&path)?;
                scan::import(&library, &path, Arc::new(AtomicBool::new(false)), |p| {
                    let _ = sender.send(Event::Imported(p));
                })?;
                Ok(())
            })();
            if let Err(e) = result {
                let _ = sender.send(Event::Error(format!("{e:#}")));
            }
        });
        cx.notify();
    }
    fn rescan(&mut self, cx: &mut Context<Self>) {
        if self.scan.is_some() {
            return;
        }
        self.scan = Some(ScanProgress {
            current: "Checking music folders…".into(),
            ..Default::default()
        });
        self.cancel = Arc::new(AtomicBool::new(false));
        let library = self.library.clone();
        let sender = self.sender.clone();
        let cancel = self.cancel.clone();
        std::thread::spawn(move || {
            let result = (|| -> Result<()> {
                let roots = library.roots()?;
                let mut total = ScanProgress::default();
                for root in roots {
                    let p =
                        scan::import(&library, &PathBuf::from(root), cancel.clone(), |mut p| {
                            p.done = false;
                            let _ = sender.send(Event::Imported(p));
                        })?;
                    total.scanned += p.scanned;
                    total.imported += p.imported;
                    total.unchanged += p.unchanged;
                    total.errors.extend(p.errors);
                }
                total.done = true;
                let _ = sender.send(Event::Imported(total));
                Ok(())
            })();
            if let Err(e) = result {
                let _ = sender.send(Event::Error(format!("{e:#}")));
            }
        });
        cx.notify();
    }
    fn play_from(&mut self, index: usize, cx: &mut Context<Self>) {
        let reason = if self.expression(cx).trim().is_empty() {
            format!("Selected from {}", self.page.title().to_lowercase())
        } else {
            format!("Matches {}", self.expression(cx))
        };
        let items = self
            .tracks
            .iter()
            .skip(index)
            .filter(|t| !t.missing)
            .map(|track| QueueItem {
                track: track.clone(),
                reason: reason.clone(),
            })
            .collect();
        self.player.send(Command::Play(items));
        cx.notify();
    }
    fn save_playlist(&mut self, smart: bool, cx: &mut Context<Self>) {
        let name = self.playlist_name.read(cx).value().to_string();
        let mut expression = self.expression(cx);
        if smart && let Page::Playlist(id) = &self.page {
            let list = self.playlists.iter().find(|p| &p.id == id);
            if !expression.trim().is_empty() || list.is_none_or(|p| p.query.is_none()) {
                self.notice=Some("Use All music to write a smart rule, or save the shown tracks as a regular playlist.".into());
                cx.notify();
                return;
            }
            expression = list.and_then(|p| p.query.clone()).unwrap_or_default();
        }
        let playlist = Playlist {
            id: crate::uuid_string(),
            name,
            query: if smart { Some(expression) } else { None },
            track_ids: if smart {
                vec![]
            } else {
                self.tracks.iter().map(|t| t.id.clone()).collect()
            },
            updated_at: chrono::Utc::now().timestamp(),
        };
        match self.library.save_playlist(&playlist) {
            Ok(()) => {
                self.playlists = self.library.playlists().unwrap_or_default();
                self.show_save = false;
                self.notice = Some(format!("Saved {}", playlist.name));
            }
            Err(e) => self.notice = Some(e.to_string()),
        }
        cx.notify();
    }
    fn edit_selected(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(track) = &self.selected {
            self.pending_mbid = None;
            self.tag_title
                .update(cx, |s, cx| s.set_value(track.title.clone(), window, cx));
            self.tag_artist
                .update(cx, |s, cx| s.set_value(track.artist.clone(), window, cx));
            self.tag_album
                .update(cx, |s, cx| s.set_value(track.album.clone(), window, cx));
            self.editing = true;
            cx.notify();
        }
    }
    fn write_tags(&mut self, cx: &mut Context<Self>) {
        if let Some(track) = &self.selected {
            let edit = TagEdit {
                title: Some(self.tag_title.read(cx).value().to_string()),
                artist: Some(self.tag_artist.read(cx).value().to_string()),
                album: Some(self.tag_album.read(cx).value().to_string()),
                musicbrainz_id: self.pending_mbid.clone(),
                ..Default::default()
            };
            let library = self.library.clone();
            let sender = self.sender.clone();
            let id = track.id.clone();
            std::thread::spawn(move || {
                let event = match scan::write_tags(&library, &id, &edit) {
                    Ok(()) => Event::SavedTags,
                    Err(e) => Event::Error(format!("{e:#}")),
                };
                let _ = sender.send(event);
            });
        }
    }
    fn lookup(&mut self, cx: &mut Context<Self>) {
        if let Some(track) = &self.selected {
            let library = self.library.clone();
            let sender = self.sender.clone();
            let id = track.id.clone();
            let artist = track.artist.clone();
            let title = track.title.clone();
            self.lookup_busy = true;
            std::thread::spawn(move || {
                let event = match integrations::musicbrainz_search(&library, &artist, &title) {
                    Ok(m) => Event::Matches(id, m),
                    Err(e) => Event::Error(format!("MusicBrainz: {e:#}")),
                };
                let _ = sender.send(event);
            });
            cx.notify();
        }
    }
}

impl Drop for AppView {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
        self.player.shutdown();
    }
}

impl AppView {
    fn nav(
        &self,
        id: &'static str,
        name: &'static str,
        icon_name: &'static str,
        page: Page,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let selected = self.page == page;
        div()
            .id(id)
            .h(px(38.))
            .px_3()
            .mx_2()
            .rounded(px(6.))
            .flex()
            .items_center()
            .gap_3()
            .cursor_pointer()
            .bg(if selected {
                cx.theme().accent
            } else {
                transparent_black()
            })
            .text_color(if selected {
                cx.theme().foreground
            } else {
                muted(cx)
            })
            .hover(|s| s.bg(cx.theme().secondary))
            .child(icon(icon_name).text_color(muted(cx)))
            .child(label(name))
            .on_click(
                cx.listener(move |this, _, window, cx| this.navigate(page.clone(), window, cx)),
            )
    }
    fn sidebar(&self, cx: &mut Context<Self>) -> Div {
        div()
            .w(px(self.settings.layout.sidebar_width.clamp(175., 260.)))
            .flex_shrink_0()
            .h_full()
            .flex()
            .flex_col()
            .border_r_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().muted.opacity(0.32))
            .child(
                div()
                    .h(px(92.))
                    .px_5()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(icon("disc").size(px(28.)).text_color(cx.theme().primary))
                    .child(
                        div()
                            .text_size(px(25.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Needle"),
                    ),
            )
            .child(self.nav("nav-library", "All music", "music", Page::Library, cx))
            .child(self.nav("nav-albums", "Albums", "disc", Page::Albums, cx))
            .child(
                div()
                    .id("nav-artists")
                    .h(px(38.))
                    .px_3()
                    .mx_2()
                    .rounded(px(6.))
                    .flex()
                    .gap_3()
                    .items_center()
                    .cursor_pointer()
                    .text_color(muted(cx))
                    .hover(|s| s.bg(cx.theme().secondary))
                    .child(Icon::new(IconName::User).size(px(18.)))
                    .child(label("Artists"))
                    .on_click(
                        cx.listener(|this, _, window, cx| this.navigate(Page::Artists, window, cx)),
                    ),
            )
            .child(div().h(px(22.)))
            .child(
                div()
                    .id("nav-favorites")
                    .h(px(38.))
                    .px_3()
                    .mx_2()
                    .rounded(px(6.))
                    .flex()
                    .gap_3()
                    .items_center()
                    .cursor_pointer()
                    .bg(if self.page == Page::Favorites {
                        cx.theme().accent
                    } else {
                        transparent_black()
                    })
                    .text_color(muted(cx))
                    .hover(|s| s.bg(cx.theme().secondary))
                    .child(Icon::new(IconName::Heart).size(px(18.)))
                    .child(label("Favorites"))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.navigate(Page::Favorites, window, cx)
                    })),
            )
            .child(self.nav("nav-recent", "Recently added", "history", Page::Recent, cx))
            .child(self.nav(
                "nav-history",
                "Listening history",
                "history",
                Page::History,
                cx,
            ))
            .child(self.nav("nav-queue", "Play queue", "queue", Page::Queue, cx))
            .child(
                div()
                    .mt_7()
                    .mb_2()
                    .px_5()
                    .flex()
                    .justify_between()
                    .items_center()
                    .child(small("Your playlists", cx))
                    .child(
                        Button::new("new-playlist")
                            .ghost()
                            .compact()
                            .icon(IconName::Plus)
                            .tooltip("Save the current selection as a playlist")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.show_save = true;
                                cx.notify();
                            })),
                    ),
            )
            .child(
                div()
                    .id("playlists-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .children(self.playlists.iter().map(|playlist| {
                        let id = playlist.id.clone();
                        let page = Page::Playlist(id.clone());
                        div()
                            .id(SharedString::from(format!("playlist-{id}")))
                            .mx_2()
                            .px_3()
                            .py_2()
                            .rounded(px(6.))
                            .flex()
                            .gap_3()
                            .cursor_pointer()
                            .bg(if self.page == page {
                                cx.theme().accent
                            } else {
                                transparent_black()
                            })
                            .hover(|s| s.bg(cx.theme().secondary))
                            .child(
                                Icon::new(if playlist.query.is_some() {
                                    IconName::SquareTerminal
                                } else {
                                    IconName::GalleryVerticalEnd
                                })
                                .size(px(16.))
                                .text_color(muted(cx)),
                            )
                            .child(
                                div()
                                    .text_size(px(13.))
                                    .truncate()
                                    .child(playlist.name.clone()),
                            )
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.navigate(Page::Playlist(id.clone()), window, cx)
                            }))
                    })),
            )
            .child(
                div()
                    .p_3()
                    .border_t_1()
                    .border_color(cx.theme().border)
                    .child(
                        Button::new("add-folder-side")
                            .w_full()
                            .icon(IconName::FolderOpen)
                            .label("Add music folder")
                            .on_click(
                                cx.listener(|this, _, window, cx| this.import_folder(window, cx)),
                            ),
                    )
                    .child(
                        div()
                            .mt_2()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(small(format!("{} tracks · Local library", self.total), cx))
                            .child(
                                Button::new("settings-nav")
                                    .ghost()
                                    .compact()
                                    .icon(IconName::Settings)
                                    .tooltip("Settings")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.navigate(Page::Settings, window, cx)
                                    })),
                            ),
                    ),
            )
    }
    fn artwork(&self, track: Option<&Track>, size: f32, cx: &App) -> AnyElement {
        if let Some(path) = track.and_then(|t| t.artwork.as_ref()) {
            img(PathBuf::from(path))
                .size(px(size))
                .object_fit(ObjectFit::Cover)
                .rounded(px(5.))
                .into_any_element()
        } else {
            div()
                .size(px(size))
                .flex_shrink_0()
                .rounded(px(5.))
                .bg(cx.theme().secondary)
                .flex()
                .items_center()
                .justify_center()
                .child(
                    icon("disc")
                        .size(px(size * 0.55))
                        .text_color(cx.theme().primary.opacity(0.65)),
                )
                .into_any_element()
        }
    }
    fn empty(&self, cx: &mut Context<Self>) -> Div {
        div().flex_1().flex().flex_col().items_center().justify_center().px_10().pb_10().gap_5()
        .child(div().size(px(96.)).rounded_full().border_1().border_color(cx.theme().border).flex().items_center().justify_center().child(icon("disc").size(px(55.)).text_color(cx.theme().primary)))
        .child(div().text_size(px(30.)).font_weight(FontWeight::SEMIBOLD).child("Make room for your music."))
        .child(div().max_w(px(400.)).text_center().text_size(px(15.)).line_height(relative(1.6)).text_color(muted(cx)).child("Albums you know by heart. Tracks waiting to be found. Bring your collection together, exactly as you like it."))
        .child(Button::new("empty-add").primary().large().icon(IconName::FolderOpen).label("Add a music folder").on_click(cx.listener(|this,_,window,cx|this.import_folder(window,cx))))
        .child(Button::new("empty-demo").ghost().label("Take a listen with the demo library").on_click(cx.listener(|this,_,_,cx|this.demo(cx))))
        .child(small("Your files stay where they are. No account needed.",cx))
    }
    fn track_row(&self, index: usize, cx: &mut Context<Self>) -> Stateful<Div> {
        let track = self.tracks[index].clone();
        let selected = self.selected.as_ref().is_some_and(|t| t.id == track.id);
        let playing = self
            .playback
            .current
            .as_ref()
            .is_some_and(|i| i.track.id == track.id);
        let id = track.id.clone();
        let rating = track.rating;
        let rate_id = id.clone();
        let enqueue = track.clone();
        let selected_track = track.clone();
        div()
            .id(SharedString::from(id))
            .w_full()
            .h(px(self.settings.layout.row_height.clamp(44., 76.)))
            .px_5()
            .flex()
            .items_center()
            .gap_3()
            .cursor_pointer()
            .border_b_1()
            .border_color(cx.theme().border.opacity(0.35))
            .bg(if selected {
                cx.theme().accent.opacity(0.6)
            } else {
                transparent_black()
            })
            .hover(|s| s.bg(cx.theme().secondary.opacity(0.7)))
            .child(
                div()
                    .w(px(24.))
                    .flex_shrink_0()
                    .text_size(px(12.))
                    .text_color(if playing {
                        cx.theme().primary
                    } else {
                        muted(cx)
                    })
                    .child(if playing {
                        "▸".into()
                    } else {
                        format!("{:02}", index + 1)
                    }),
            )
            .child(self.artwork(Some(&track), 34., cx))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .truncate()
                            .text_size(px(13.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(if playing {
                                cx.theme().primary
                            } else {
                                cx.theme().foreground
                            })
                            .child(track.title.clone()),
                    )
                    .child(
                        small(
                            if track.missing {
                                "File unavailable".into()
                            } else {
                                format!("{} · {}", track.display_artist(), track.display_album())
                            },
                            cx,
                        )
                        .truncate(),
                    ),
            )
            .child(
                div()
                    .w(px(55.))
                    .text_size(px(10.))
                    .text_color(muted(cx))
                    .child(track.format.clone()),
            )
            .child(
                Button::new(("rate", index))
                    .ghost()
                    .compact()
                    .icon(IconName::Heart)
                    .selected(rating >= 4)
                    .tooltip("Favorite")
                    .on_click(cx.listener(move |this, _, _, cx| {
                        let next = if rating >= 4 { 0 } else { 5 };
                        if let Err(e) = this.library.rate(&rate_id, next) {
                            this.notice = Some(e.to_string());
                        }
                        if let Some(track) = this.tracks.iter_mut().find(|t| t.id == rate_id) {
                            track.rating = next;
                        }
                        if let Some(track) = this.selected.as_mut().filter(|t| t.id == rate_id) {
                            track.rating = next;
                        }
                        cx.stop_propagation();
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .w(px(46.))
                    .text_right()
                    .text_size(px(12.))
                    .text_color(muted(cx))
                    .child(format_duration(track.duration)),
            )
            .child(
                Button::new(("enqueue", index))
                    .ghost()
                    .compact()
                    .icon(IconName::Plus)
                    .tooltip("Add to queue")
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.player.send(Command::Enqueue(vec![QueueItem {
                            track: enqueue.clone(),
                            reason: "Added to queue by you".into(),
                        }]));
                        cx.stop_propagation();
                    })),
            )
            .on_click(cx.listener(move |this, event: &ClickEvent, window, cx| {
                this.selected = Some(selected_track.clone());
                this.editing = false;
                this.matches.clear();
                window.focus(&this.focus);
                if event.click_count() == 2 {
                    this.play_from(index, cx);
                }
                cx.notify();
            }))
    }
    fn collection(&self, cx: &mut Context<Self>) -> AnyElement {
        if self.total == 0 && self.scan.is_none() {
            return self.empty(cx).into_any_element();
        }
        if self.tracks.is_empty() {
            return div()
                .flex_1()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap_3()
                .child(
                    Icon::new(IconName::Search)
                        .size(px(30.))
                        .text_color(muted(cx)),
                )
                .child(label(if self.loading {
                    "Searching your library…"
                } else {
                    "No tracks match this view"
                }))
                .child(small("Try another artist, album, or rule.", cx))
                .into_any_element();
        }
        if self.page == Page::Albums || self.page == Page::Artists {
            return self.groups(cx);
        }
        div()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(36.))
                    .px_5()
                    .flex()
                    .items_center()
                    .gap_3()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(small("#", cx).w(px(24.)))
                    .child(div().w(px(34.)))
                    .child(small("Title / artist / album", cx).flex_1())
                    .child(small("Format", cx).w(px(78.)))
                    .child(small("Time", cx).w(px(46.)))
                    .child(div().w(px(30.))),
            )
            .child(
                uniform_list(
                    "track-list",
                    self.tracks.len(),
                    cx.processor(|this, range: std::ops::Range<usize>, _, cx| {
                        range
                            .map(|index| this.track_row(index, cx))
                            .collect::<Vec<_>>()
                    }),
                )
                .flex_1(),
            )
            .into_any_element()
    }
    fn groups(&self, cx: &mut Context<Self>) -> AnyElement {
        let artists = self.page == Page::Artists;
        let mut groups: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        for (index, track) in self.tracks.iter().enumerate() {
            let key = if artists {
                track.display_artist().into()
            } else {
                format!("{}\0{}", track.display_album(), track.album_artist)
            };
            groups.entry(key).or_default().push(index);
        }
        div()
            .id("group-scroll")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .p_5()
            .flex()
            .flex_wrap()
            .gap_6()
            .children(groups.into_iter().enumerate().map(|(index, (_, indices))| {
                let first = &self.tracks[indices[0]];
                let title = if artists {
                    first.display_artist()
                } else {
                    first.display_album()
                }
                .to_string();
                let count = indices.len();
                let album = first.album.clone();
                let artist = first.artist.clone();
                div()
                    .id(("group", index))
                    .w(px(165.))
                    .cursor_pointer()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(self.artwork(Some(first), 165., cx))
                    .child(
                        div()
                            .text_size(px(13.))
                            .font_weight(FontWeight::MEDIUM)
                            .truncate()
                            .child(title),
                    )
                    .child(
                        small(
                            if artists {
                                format!("{count} tracks")
                            } else {
                                first.display_artist().into()
                            },
                            cx,
                        )
                        .truncate(),
                    )
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.page = Page::Library;
                        let escaped = if artists {
                            artist.clone()
                        } else {
                            album.clone()
                        }
                        .replace('\\', "\\\\")
                        .replace('"', "\\\"");
                        let expression = format!(
                            "{} = \"{escaped}\"",
                            if artists { "artist" } else { "album" }
                        );
                        this.search
                            .update(cx, |s, cx| s.set_value(expression, window, cx));
                        this.refresh(cx);
                    }))
            }))
            .into_any_element()
    }
    fn queue(&self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .id("queue-scroll")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .p_6()
            .flex()
            .flex_col()
            .gap_4()
            .when_some(self.playback.current.as_ref(), |el, item| {
                el.child(small("Now playing", cx)).child(
                    div()
                        .p_4()
                        .bg(cx.theme().secondary)
                        .rounded(px(8.))
                        .flex()
                        .items_center()
                        .gap_4()
                        .child(self.artwork(Some(&item.track), 50., cx))
                        .child(
                            div()
                                .flex_1()
                                .child(label(item.track.title.clone()))
                                .child(small(item.track.display_artist().to_string(), cx))
                                .child(small(item.reason.clone(), cx)),
                        ),
                )
            })
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(label(format!(
                        "Up next · {} tracks",
                        self.playback.queue.len()
                    )))
                    .child(
                        Button::new("clear-queue")
                            .ghost()
                            .small()
                            .label("Clear upcoming")
                            .on_click(
                                cx.listener(|this, _, _, _| this.player.send(Command::ClearQueue)),
                            ),
                    ),
            )
            .when(self.playback.current.is_some(), |el| {
                el.child(
                    div()
                        .flex()
                        .items_center()
                        .gap_3()
                        .child(small("Practice loop", cx))
                        .child(
                            Button::new("loop-a")
                                .small()
                                .label(
                                    self.loop_start
                                        .map(|p| format!("A · {}", format_duration(p)))
                                        .unwrap_or_else(|| "Set A".into()),
                                )
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.loop_start = Some(this.playback.position);
                                    cx.notify();
                                })),
                        )
                        .child(
                            Button::new("loop-b")
                                .small()
                                .label("Set B & loop")
                                .disabled(self.loop_start.is_none())
                                .on_click(cx.listener(|this, _, _, _| {
                                    if let Some(a) = this.loop_start {
                                        this.player
                                            .send(Command::Loop(Some((a, this.playback.position))));
                                    }
                                })),
                        )
                        .child(
                            Button::new("loop-off")
                                .small()
                                .ghost()
                                .label("Clear loop")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.loop_start = None;
                                    this.player.send(Command::Loop(None));
                                    cx.notify();
                                })),
                        ),
                )
            })
            .when(self.playback.queue.is_empty(), |el| {
                el.child(small(
                    "Add tracks with + in your library. They’ll play in the order you choose.",
                    cx,
                ))
            })
            .children(self.playback.queue.iter().enumerate().map(|(index, item)| {
                div()
                    .py_3()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(small(format!("{}", index + 1), cx).w(px(20.)))
                    .child(self.artwork(Some(&item.track), 38., cx))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(label(item.track.title.clone()).truncate())
                            .child(small(item.reason.clone(), cx).truncate()),
                    )
                    .child(
                        Button::new(("queue-up", index))
                            .ghost()
                            .compact()
                            .icon(IconName::ArrowUp)
                            .tooltip("Move earlier")
                            .disabled(index == 0)
                            .on_click(cx.listener(move |this, _, _, _| {
                                this.player
                                    .send(Command::Move(index, index.saturating_sub(1)))
                            })),
                    )
                    .child(
                        Button::new(("queue-remove", index))
                            .ghost()
                            .compact()
                            .icon(IconName::Close)
                            .tooltip("Remove from queue")
                            .on_click(cx.listener(move |this, _, _, _| {
                                this.player.send(Command::Remove(index))
                            })),
                    )
            }))
            .into_any_element()
    }
    fn history_view(&self, cx: &mut Context<Self>) -> AnyElement {
        let seconds: f64 = self.history.iter().map(|l| l.listened_seconds).sum();
        div()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .child(div().px_6().py_4().child(small(
                format!(
                    "{} recent listening sessions · {} listened · Stored on this device",
                    self.history.len(),
                    format_duration(seconds)
                ),
                cx,
            )))
            .when(self.history.is_empty(), |el| {
                el.child(
                    div()
                        .p_6()
                        .child(label("Your listening story starts here."))
                        .child(small(
                            "Play a track and your listening history will appear automatically.",
                            cx,
                        )),
                )
            })
            .child(
                uniform_list(
                    "history-list",
                    self.history.len(),
                    cx.processor(|this, range: std::ops::Range<usize>, _, cx| {
                        range
                            .map(|index| {
                                let listen = this.history[index].clone();
                                let track_id = listen.track_id.clone();
                                let date = chrono::DateTime::from_timestamp(listen.started_at, 0)
                                    .map(|d| {
                                        d.with_timezone(&chrono::Local)
                                            .format("%b %d · %H:%M")
                                            .to_string()
                                    })
                                    .unwrap_or_default();
                                div()
                                    .id(("listen", index))
                                    .w_full()
                                    .h(px(62.))
                                    .px_6()
                                    .flex()
                                    .items_center()
                                    .gap_3()
                                    .border_b_1()
                                    .border_color(cx.theme().border.opacity(0.5))
                                    .child(icon("history").text_color(muted(cx)))
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w_0()
                                            .child(label(listen.title).truncate())
                                            .child(small(listen.artist, cx)),
                                    )
                                    .child(small(date, cx))
                                    .child(
                                        small(format_duration(listen.listened_seconds), cx)
                                            .w(px(48.)),
                                    )
                                    .child(
                                        Button::new(("history-play", index))
                                            .ghost()
                                            .compact()
                                            .icon(Icon::new(IconName::ArrowRight))
                                            .tooltip("Play again")
                                            .on_click(cx.listener(move |this, _, _, _| {
                                                if let Ok(Some(track)) =
                                                    this.library.track(&track_id)
                                                {
                                                    this.player.send(Command::Play(
                                                        vec![QueueItem {
                                                        track,
                                                        reason:
                                                            "Played again from listening history"
                                                                .into(),
                                                    }],
                                                    ));
                                                }
                                            })),
                                    )
                            })
                            .collect::<Vec<_>>()
                    }),
                )
                .flex_1(),
            )
            .into_any_element()
    }
}

impl AppView {
    fn inspector(&self, cx: &mut Context<Self>) -> Div {
        let track = self
            .selected
            .as_ref()
            .or_else(|| self.playback.current.as_ref().map(|i| &i.track));
        let mut panel = div()
            .w(px(self.settings.layout.inspector_width.clamp(240., 340.)))
            .flex_shrink_0()
            .h_full()
            .border_l_1()
            .border_color(cx.theme().border)
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(55.))
                    .px_5()
                    .flex()
                    .justify_between()
                    .items_center()
                    .child(label("Track details"))
                    .child(
                        Button::new("close-inspector")
                            .ghost()
                            .compact()
                            .icon(IconName::PanelRightClose)
                            .tooltip("Hide track details")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.settings.show_inspector = false;
                                this.persist_settings();
                                cx.notify();
                            })),
                    ),
            );
        let Some(track) = track else {
            return panel.child(
                div()
                    .p_5()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(self.artwork(None, 220., cx))
                    .child(label("A closer listen"))
                    .child(small(
                        "Select a track to see its tags, file details, and place in your library.",
                        cx,
                    )),
            );
        };
        let rating = track.rating;
        let id = track.id.clone();
        let reason = self
            .playback
            .current
            .as_ref()
            .filter(|i| i.track.id == track.id)
            .map(|i| i.reason.clone());
        let track_clone = track.clone();
        let play_track = track.clone();
        let mut body=div().id("inspector-scroll").flex_1().min_h_0().overflow_y_scroll().px_5().pb_5().flex().flex_col().gap_4()
            .child(self.artwork(Some(track),self.settings.layout.inspector_width.clamp(240.,340.)-40.,cx))
            .child(div().flex().flex_col().gap_1().child(div().text_size(px(21.)).font_weight(FontWeight::SEMIBOLD).child(track.title.clone())).child(div().text_size(px(14.)).text_color(muted(cx)).child(track.display_artist().to_string())).child(small(track.display_album().to_string(),cx)))
            .child(div().flex().items_center().gap_1().children((1..=5).map(|star|{let id=id.clone();Button::new(("star",star as usize)).ghost().compact().icon(IconName::Star).selected(rating>=star).tooltip(format!("Rate {star} of 5")).on_click(cx.listener(move|this,_,_,cx|{let next=if rating==star{0}else{star};match this.library.rate(&id,next){Ok(())=>{if let Some(track)=this.selected.as_mut(){track.rating=next;}
if let Some(track)=this.tracks.iter_mut().find(|t|t.id==id){track.rating=next;}},Err(e)=>this.notice=Some(e.to_string())}cx.notify();}))})))
            .child(div().flex().gap_2().child(Button::new("detail-play").primary().flex_1().icon(Icon::empty().path("needle/play.svg")).label("Play").disabled(track.missing).on_click(cx.listener(move|this,_,_,_|this.player.send(Command::Play(vec![QueueItem{track:play_track.clone(),reason:"Selected from track details".into()}]))))).child(Button::new("edit-tags").icon(IconName::Settings2).label("Edit tags").on_click(cx.listener(move|this,_,window,cx|{this.selected=Some(track_clone.clone());this.edit_selected(window,cx);}))))
            .child(div().border_t_1().border_color(cx.theme().border).pt_4().flex().flex_col().gap_3()
                .child(self.detail("Format",format!("{} · {}",track.format,if track.bit_depth>0{format!("{}-bit",track.bit_depth)}else{format!("{} kbps",track.bitrate)}),cx))
                .child(self.detail("Sample rate",format!("{:.1} kHz",track.sample_rate as f64/1000.),cx))
                .child(self.detail("Duration",format_duration(track.duration),cx))
                .child(self.detail("Year",if track.year>0{track.year.to_string()}else{"—".into()},cx))
                .child(self.detail("Genre",if track.genre.is_empty(){"—".into()}else{track.genre.clone()},cx))
                .child(self.detail("Tempo",track.bpm.map(|v|format!("{v:.0} BPM")).unwrap_or_else(||"—".into()),cx))
                .child(self.detail("Plays",track.play_count.to_string(),cx))
                .child(self.detail("ReplayGain",track.replay_gain.map(|gain|format!("{gain:+.2} dB")).unwrap_or_else(||"Not measured".into()),cx)))
            .child(Button::new("measure-loudness").w_full().label("Measure loudness").on_click({let id=track.id.clone();cx.listener(move|this,_,_,cx|{let library=this.library.clone();let sender=this.sender.clone();let id=id.clone();this.notice=Some("Measuring loudness in the background…".into());std::thread::spawn(move||{let result=needle_core::analysis::scan_loudness(&library,&id);let event=match result{Ok(result)=>Event::Notice(format!("Measured {:.1} LUFS · ReplayGain {:+.2} dB. Enable ReplayGain in Settings to apply it.",result.integrated_lufs,result.replay_gain_db)),Err(error)=>Event::Error(format!("{error:#}"))};let _=sender.send(event);});cx.notify();})}))
            .when_some(reason,|el,reason|el.child(div().pt_4().border_t_1().border_color(cx.theme().border).flex().flex_col().gap_2().child(label("Why this track?")).child(small(reason,cx))))
            .child(div().pt_4().border_t_1().border_color(cx.theme().border).flex().flex_col().gap_2().child(label("File location")).child(small(track.path.trim_start_matches("\\\\?\\").to_string(),cx).overflow_hidden()).child(Button::new("copy-path").ghost().small().icon(IconName::Copy).label("Copy file path").on_click({let path=track.path.clone();move|_,_,cx|cx.write_to_clipboard(ClipboardItem::new_string(path.clone()))})))
            .child(Button::new("lookup-metadata").w_full().label(if self.lookup_busy{"Looking up recording…"}else{"Look up on MusicBrainz"}).disabled(self.lookup_busy).on_click({let track=track.clone();cx.listener(move|this,_,_,cx|{this.selected=Some(track.clone());this.lookup(cx);})}));
        if self.editing {
            body=body.child(div().border_t_1().border_color(cx.theme().border).pt_4().flex().flex_col().gap_3().child(label("Edit file tags")).when(self.pending_mbid.is_some(),|el|el.child(small(format!("MusicBrainz ID: {}",self.pending_mbid.as_deref().unwrap_or_default()),cx))).child(small("Review the fields below. Saving writes them to this file and keeps an original backup.",cx)).child(Input::new(&self.tag_title)).child(Input::new(&self.tag_artist)).child(Input::new(&self.tag_album)).child(div().flex().gap_2().child(Button::new("save-tags").primary().label("Save to file").on_click(cx.listener(|this,_,_,cx|this.write_tags(cx)))).child(Button::new("cancel-tags").ghost().label("Cancel").on_click(cx.listener(|this,_,_,cx|{this.editing=false;cx.notify();})))));
        }
        body = body.child(
            Button::new("fingerprint-lookup")
                .small()
                .label("Identify with AcoustID")
                .disabled(self.lookup_busy)
                .tooltip("Send an audio fingerprint to AcoustID; requires your application API key")
                .on_click(cx.listener(|this, _, _, cx| {
                    if let Some(track) = this.selected.clone() {
                        let library = this.library.clone();
                        let sender = this.sender.clone();
                        this.lookup_busy = true;
                        std::thread::spawn(move || {
                            let key = std::env::var("NEEDLE_ACOUSTID_API_KEY").unwrap_or_default();
                            let result = integrations::acoustid_lookup(
                                &library,
                                &PathBuf::from(&track.path),
                                &key,
                            );
                            let _ = sender.send(match result {
                                Ok(matches) => Event::Matches(track.id, matches),
                                Err(error) => Event::Error(format!("AcoustID: {error:#}")),
                            });
                        });
                        cx.notify();
                    }
                })),
        );
        if !self.matches.is_empty() {
            body = body.child(label("Matching recordings")).children(
                self.matches.iter().enumerate().map(|(index, recording)| {
                    let recording = recording.clone();
                    div()
                        .py_3()
                        .border_t_1()
                        .border_color(cx.theme().border)
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(label(recording.title.clone()))
                        .child(small(
                            format!("{} · {}", recording.artist, recording.album),
                            cx,
                        ))
                        .when(recording.release_id.is_some(), |el| {
                            let release_id = recording.release_id.clone().unwrap_or_default();
                            let track_id = track.id.clone();
                            el.child(
                                Button::new(("cover-match", index))
                                    .small()
                                    .label("Use this release?s cover")
                                    .on_click(cx.listener(move |this, _, _, _| {
                                        let library = this.library.clone();
                                        let sender = this.sender.clone();
                                        let release = release_id.clone();
                                        let id = track_id.clone();
                                        std::thread::spawn(move || {
                                            let event = match integrations::cover_art(
                                                &library, &id, &release,
                                            ) {
                                                Ok(_) => Event::Notice(
                                                    "Cover art saved to the library cache.".into(),
                                                ),
                                                Err(e) => Event::Error(format!("Cover art: {e:#}")),
                                            };
                                            let _ = sender.send(event);
                                        });
                                    })),
                            )
                        })
                        .child(
                            Button::new(("review-match", index))
                                .small()
                                .label("Review these tags")
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.tag_title.update(cx, |s, cx| {
                                        s.set_value(recording.title.clone(), window, cx)
                                    });
                                    this.tag_artist.update(cx, |s, cx| {
                                        s.set_value(recording.artist.clone(), window, cx)
                                    });
                                    this.tag_album.update(cx, |s, cx| {
                                        s.set_value(recording.album.clone(), window, cx)
                                    });
                                    this.pending_mbid = Some(recording.id.clone());
                                    this.editing = true;
                                    cx.notify();
                                })),
                        )
                }),
            );
        }
        panel = panel.child(body);
        panel
    }
    fn detail(&self, key: &str, value: String, cx: &App) -> Div {
        div()
            .flex()
            .gap_3()
            .justify_between()
            .child(small(key.to_string(), cx))
            .child(div().text_size(px(12.)).text_right().child(value))
    }
    fn settings_section(&self, title: &str, description: &str, cx: &App) -> Div {
        div()
            .mt_6()
            .mb_4()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .text_size(px(18.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(title.to_string()),
            )
            .child(small(description.to_string(), cx))
    }
    fn sync_transfer(&mut self, export: bool, cx: &mut Context<Self>) {
        let phrase = self.sync_phrase.read(cx).value().to_string();
        if phrase.chars().count() < 12 {
            self.notice = Some("Enter a passphrase with at least 12 characters.".into());
            cx.notify();
            return;
        }
        let library = self.library.clone();
        let sender = self.sender.clone();
        std::thread::spawn(move || {
            let dialog = rfd::FileDialog::new().add_filter("Needle encrypted library", &["needle"]);
            let path = if export {
                dialog.set_file_name("library.needle").save_file()
            } else {
                dialog.pick_file()
            };
            if let Some(path) = path {
                let result = if export {
                    needle_core::sync::export(&library, &path, &phrase).map(|_| {
                        "Encrypted library exported. Audio files and credentials are excluded."
                            .into()
                    })
                } else {
                    needle_core::sync::import(&library,&path,&phrase).map(|r|format!("Imported {} listens and {} playlists. {} tracks matched; {} need local copies.",r.imported_listens,r.imported_playlists,r.matched_tracks,r.unmatched_tracks))
                };
                let _ = sender.send(match result {
                    Ok(message) => Event::Notice(message),
                    Err(e) => Event::Error(format!("{e:#}")),
                });
            }
        });
    }
    fn settings_view(&self, cx: &mut Context<Self>) -> AnyElement {
        let roots = self.library.roots().unwrap_or_default();
        div().id("settings-scroll").flex_1().min_h_0().overflow_y_scroll().px_8().pb_8().flex().flex_col()
            .child(self.settings_section("Playback","Choose the output and processing used for your music.",cx))
            .child(Button::new("device-default").w_full().label("System default output").selected(self.settings.output_device.is_none()).on_click(cx.listener(|this,_,_,cx|{this.settings.output_device=None;this.settings.volume=this.playback.volume;this.player.send(Command::Configure(this.settings.clone()));cx.notify();})))
            .when(cfg!(windows),|el|el.child(div().mt_3().flex().items_center().justify_between().gap_4().child(div().flex_1().child(label("Exclusive output")).child(small("Use the file’s native sample rate. Volume and ReplayGain are bypassed; control volume on your audio device.",cx))).child(Button::new("exclusive-mode").label(if self.settings.exclusive{"Exclusive"}else{"Shared"}).selected(self.settings.exclusive).on_click(cx.listener(|this,_,_,cx|{this.settings.exclusive= !this.settings.exclusive;this.settings.volume=this.playback.volume;this.player.send(Command::Configure(this.settings.clone()));cx.notify();})))))
            .children(self.output_devices.iter().enumerate().map(|(index,device)|{let device=device.clone();Button::new(("device",index)).mt_2().w_full().label(device.clone()).selected(self.settings.output_device.as_ref()==Some(&device)).on_click(cx.listener(move|this,_,_,cx|{this.settings.output_device=Some(device.clone());this.settings.volume=this.playback.volume;this.player.send(Command::Configure(this.settings.clone()));cx.notify();}))}))
            .child(div().mt_4().flex().items_center().justify_between().gap_4().child(div().flex_1().child(label("ReplayGain")).child(small("Use track loudness tags and peak protection when available.",cx))).child(Button::new("replay-gain").label(if self.settings.replay_gain{"On"}else{"Off"}).selected(self.settings.replay_gain).on_click(cx.listener(|this,_,_,cx|{this.settings.replay_gain= !this.settings.replay_gain;this.settings.volume=this.playback.volume;this.player.send(Command::Configure(this.settings.clone()));cx.notify();}))))
            .child(small("The signal path shows the negotiated output rate. Exclusive mode releases the device when playback is paused.",cx).mt_3())
            .child(self.settings_section("When the queue ends","Choose the next tracks with a library rule. Leave blank to stop playback.",cx))
            .child(Input::new(&self.autoplay))
            .child(Button::new("save-autoplay").mt_3().label("Save autoplay rule").on_click(cx.listener(|this,_,_,cx|{let rule=this.autoplay.read(cx).value().to_string();match query::compile(&rule,chrono::Utc::now().timestamp()){Ok(_)=>{this.settings.autoplay_query=rule;this.settings.volume=this.playback.volume;this.player.send(Command::Configure(this.settings.clone()));this.notice=Some("Autoplay rule saved.".into());},Err(e)=>this.notice=Some(e.to_string())}cx.notify();})))
            .child(self.settings_section("Music folders","Needle watches these folders for changes. Your audio files stay in place.",cx))
            .children(roots.into_iter().map(|root|div().py_2().flex().gap_2().child(Icon::new(IconName::Folder).size(px(16.))).child(small(root.trim_start_matches("\\\\?\\").to_string(),cx))))
            .child(div().mt_3().flex().gap_3().child(Button::new("settings-add").icon(IconName::Plus).label("Add folder").on_click(cx.listener(|this,_,window,cx|this.import_folder(window,cx)))).child(Button::new("rescan").label("Rescan folders").on_click(cx.listener(|this,_,_,cx|this.rescan(cx)))))
            .child(self.settings_section("Appearance","A quiet workspace for long listening sessions.",cx))
            .child(div().flex().gap_3().children([("dark","Dark"),("light","Light")].into_iter().map(|(mode,name)|Button::new(mode).label(name).selected(self.settings.theme==mode).on_click(cx.listener(move|this,_,window,cx|{this.settings.theme=mode.into();set_theme(mode,Some(window),cx);this.persist_settings();cx.notify();})))))
            .child(div().mt_3().flex().gap_3().child(Button::new("export-layout").label("Export layout").on_click(cx.listener(|this,_,_,_|{let layout=this.settings.layout.clone();let sender=this.sender.clone();std::thread::spawn(move||{if let Some(path)=rfd::FileDialog::new().set_file_name("needle-layout.json").save_file(){let result=(||->Result<()>{std::fs::write(path,serde_json::to_vec_pretty(&layout)?)?;Ok(())})();let _=sender.send(match result{Ok(())=>Event::Notice("Layout exported.".into()),Err(e)=>Event::Error(e.to_string())});}});}))).child(Button::new("import-layout").label("Import layout").on_click(cx.listener(|this,_,_,_|{let sender=this.sender.clone();std::thread::spawn(move||{if let Some(path)=rfd::FileDialog::new().add_filter("Needle layout",&["json"]).pick_file(){let result=(||->Result<needle_core::model::Layout>{let layout:needle_core::model::Layout=serde_json::from_slice(&std::fs::read(path)?)?;layout.validate()?;Ok(layout)})();let _=sender.send(match result{Ok(layout)=>Event::Layout(layout),Err(e)=>Event::Error(e.to_string())});}});}))))
            .child(self.settings_section("Listening services","Scrobbling is optional. Credentials are read from your environment; they are never stored in library exports.",cx))
            .child(div().flex().gap_3().child(Button::new("lastfm-enable").label(if self.settings.lastfm_enabled{"Last.fm · On"}else{"Last.fm · Off"}).selected(self.settings.lastfm_enabled).on_click(cx.listener(|this,_,_,cx|{this.settings.lastfm_enabled= !this.settings.lastfm_enabled;this.persist_settings();cx.notify();}))).child(Button::new("listenbrainz-enable").label(if self.settings.listenbrainz_enabled{"ListenBrainz · On"}else{"ListenBrainz · Off"}).selected(self.settings.listenbrainz_enabled).on_click(cx.listener(|this,_,_,cx|{this.settings.listenbrainz_enabled= !this.settings.listenbrainz_enabled;this.persist_settings();cx.notify();}))))
            .child(small("Set NEEDLE_LISTENBRAINZ_TOKEN or your NEEDLE_LASTFM_API_KEY, NEEDLE_LASTFM_SECRET, and NEEDLE_LASTFM_SESSION before opening Needle.",cx).mt_3())
            .child(Button::new("submit-listens").mt_3().label("Submit queued listens").on_click(cx.listener(|this,_,_,_|{let library=this.library.clone();let sender=this.sender.clone();std::thread::spawn(move||{let result=integrations::flush_scrobbles(&library,&integrations::Credentials::from_environment());let _=sender.send(match result{Ok(count)=>Event::Notice(format!("Submitted {count} listens. Missing credentials leave listens safely queued.")),Err(e)=>Event::Error(e.to_string())});});})))
            .child(self.settings_section("Move your listening data","Export an encrypted bundle of history, ratings, and playlists. Import it on another device with the same passphrase and local music files.",cx))
            .child(Input::new(&self.sync_phrase))
            .child(div().mt_3().flex().gap_3().child(Button::new("sync-export").label("Export encrypted bundle").on_click(cx.listener(|this,_,_,cx|this.sync_transfer(true,cx)))).child(Button::new("sync-import").label("Import bundle").on_click(cx.listener(|this,_,_,cx|this.sync_transfer(false,cx)))))
            .child(self.settings_section("Library backup & playlists","Back up the database or bring in an existing M3U playlist after importing its music folder.",cx))
            .child(div().flex().gap_3().child(Button::new("backup").label("Back up database").on_click(cx.listener(|this,_,_,_|{let library=this.library.clone();let sender=this.sender.clone();std::thread::spawn(move||{if let Some(path)=rfd::FileDialog::new().set_file_name("needle-library.db").save_file(){let event=match library.backup(&path){Ok(())=>Event::Notice("Library backup saved.".into()),Err(e)=>Event::Error(e.to_string())};let _=sender.send(event);}});}))).child(Button::new("import-m3u").label("Import M3U playlist").on_click(cx.listener(|this,_,_,_|{let library=this.library.clone();let sender=this.sender.clone();std::thread::spawn(move||{if let Some(path)=rfd::FileDialog::new().add_filter("Playlists",&["m3u","m3u8"]).pick_file(){let event=match library.import_playlist(&path){Ok(p)=>Event::Notice(format!("Imported {} tracks into {}",p.track_ids.len(),p.name)),Err(e)=>Event::Error(e.to_string())};let _=sender.send(event);}});}))))
            .child(div().mt_8().pt_4().border_t_1().border_color(cx.theme().border).child(small(format!("Needle {} · nnx",env!("CARGO_PKG_VERSION")),cx)).child(small(format!("Library data: {}",self.library.directory.display()),cx)).child(small("Space · Play / pause     Ctrl+F · Search     Ctrl+O · Add folder     Ctrl+← / → · Previous / next",cx).mt_3()))
            .into_any_element()
    }
    fn transport(&self, cx: &mut Context<Self>) -> Div {
        let current = self.playback.current.as_ref().map(|i| &i.track);
        let playing = self.playback.playing;
        div().h(px(132.)).flex_shrink_0().border_t_1().border_color(cx.theme().border).bg(cx.theme().muted.opacity(0.42)).flex().flex_col()
            .child(div().flex_1().px_6().flex().items_center().gap_6()
                .child(div().w(px(300.)).flex_shrink_0().flex().items_center().gap_3().child(self.artwork(current,52.,cx)).child(div().flex_1().min_w_0().child(label(current.map(|t|t.title.clone()).unwrap_or_else(||"Nothing playing yet".into())).truncate()).child(small(current.map(|t|t.display_artist().to_string()).unwrap_or_else(||"Find something you love.".into()),cx).truncate())))
                .child(div().flex_1().min_w_0().flex().flex_col().gap_2().items_center()
                    .child(div().flex().items_center().gap_3()
                        .child(Button::new("shuffle").ghost().compact().icon(Icon::empty().path("needle/shuffle.svg")).tooltip("Shuffle this view").on_click(cx.listener(|this,_,_,cx|{let mut tracks=this.tracks.clone();let now=chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default() as usize;let len=tracks.len();if len>1{for i in (1..len).rev(){let j=(now.wrapping_mul(i+17).rotate_left((i%32) as u32))%(i+1);tracks.swap(i,j);}}this.player.send(Command::Play(tracks.into_iter().filter(|t|!t.missing).map(|track|QueueItem{track,reason:"Shuffled from the current view".into()}).collect()));cx.notify();})))
                        .child(Button::new("previous").ghost().icon(Icon::empty().path("needle/previous.svg")).tooltip("Previous · Ctrl+Left").on_click(cx.listener(|this,_,_,_|this.player.send(Command::Previous))))
                        .child(Button::new("play-pause").primary().large().rounded_full().icon(Icon::empty().path(if playing{"needle/pause.svg"}else{"needle/play.svg"})).tooltip(if playing{"Pause · Space"}else{"Play · Space"}).on_click(cx.listener(|this,_,_,cx|{if this.playback.current.is_some(){this.player.send(Command::Toggle)}else{this.play_from(0,cx);}})))
                        .child(Button::new("next").ghost().icon(Icon::empty().path("needle/next.svg")).tooltip("Next · Ctrl+Right").on_click(cx.listener(|this,_,_,_|this.player.send(Command::Next))))
                        .child(Button::new("repeat").ghost().compact().icon(Icon::empty().path("needle/repeat.svg")).selected(self.playback.repeat!=Repeat::Off).tooltip(format!("Repeat: {:?}",self.playback.repeat)).on_click(cx.listener(|this,_,_,_|{let next=match this.playback.repeat{Repeat::Off=>Repeat::All,Repeat::All=>Repeat::One,Repeat::One=>Repeat::Off};this.player.send(Command::Repeat(next));}))))
                    .child(div().w_full().flex().items_center().gap_3().child(small(format_duration(self.playback.position),cx).w(px(36.))).child(Slider::new(&self.seek).flex_1()).child(small(current.map(|t|format_duration(t.duration)).unwrap_or_else(||"0:00".into()),cx).w(px(36.)))))
                .child(div().w(px(220.)).flex_shrink_0().flex().items_center().gap_3().child(icon("volume").text_color(muted(cx))).child(Slider::new(&self.volume).w(px(110.)).disabled(self.playback.exclusive)).child(Button::new("transport-queue").ghost().icon(Icon::empty().path("needle/queue.svg")).tooltip("View queue").on_click(cx.listener(|this,_,window,cx|this.navigate(Page::Queue,window,cx))))))
            .child(div().h(px(28.)).px_6().border_t_1().border_color(cx.theme().border.opacity(0.6)).flex().items_center().justify_between()
                .child(small(if let Some(track)=current{if self.playback.output_rate==0{format!("{} ? {:.1} kHz ? Output opens when playback starts",track.format,track.sample_rate as f64/1000.)}else if self.playback.exclusive{format!("{} · {:.1} kHz → DSP bypassed → WASAPI exclusive · {:.1} kHz → {}",track.format,track.sample_rate as f64/1000.,self.playback.output_rate as f64/1000.,self.playback.output)}else{format!("{} · {:.1} kHz → {} → System mixer · {:.1} kHz → {}",track.format,track.sample_rate as f64/1000.,if self.playback.replay_gain{"ReplayGain + volume"}else{"Volume"},self.playback.output_rate as f64/1000.,self.playback.output)}}else{"Local files. Your library. Your listening history.".into()},cx))
                .child(small(if self.playback.repeat==Repeat::One{"Repeat one"}else if self.playback.repeat==Repeat::All{"Repeat all"}else{"nnx"},cx)))
    }
}

impl Render for AppView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let title = if let Page::Playlist(id) = &self.page {
            self.playlists
                .iter()
                .find(|p| &p.id == id)
                .map(|p| p.name.clone())
                .unwrap_or_else(|| "Playlist".into())
        } else {
            self.page.title().to_string()
        };
        let is_collection = !matches!(self.page, Page::Settings | Page::History | Page::Queue);
        let total_duration: f64 = self.tracks.iter().map(|t| t.duration).sum();
        let mut center = div()
            .flex_1()
            .min_w_0()
            .h_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(72.))
                    .flex_shrink_0()
                    .px_6()
                    .flex()
                    .items_center()
                    .gap_4()
                    .child(
                        Input::new(&self.search)
                            .prefix(Icon::new(IconName::Search))
                            .flex_1(),
                    )
                    .child(
                        Button::new("inspector-toggle")
                            .ghost()
                            .icon(IconName::PanelRight)
                            .tooltip("Show track details")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.settings.show_inspector = !this.settings.show_inspector;
                                this.persist_settings();
                                cx.notify();
                            })),
                    ),
            )
            .child(
                div()
                    .px_6()
                    .pt_3()
                    .pb_5()
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_4()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(
                                div()
                                    .text_size(px(27.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(title),
                            )
                            .when(is_collection, |el| {
                                el.child(small(
                                    format!(
                                        "{} tracks · {}",
                                        self.tracks.len(),
                                        format_duration(total_duration)
                                    ),
                                    cx,
                                ))
                            }),
                    )
                    .when(is_collection && self.total > 0, |el| {
                        el.child(
                            div()
                                .flex()
                                .gap_2()
                                .child(
                                    Button::new("play-view")
                                        .primary()
                                        .icon(Icon::empty().path("needle/play.svg"))
                                        .label(if self.matched_total > 1000 {
                                            "Play this page"
                                        } else {
                                            "Play"
                                        })
                                        .disabled(
                                            self.tracks.is_empty() || self.query_error.is_some(),
                                        )
                                        .on_click(
                                            cx.listener(|this, _, _, cx| this.play_from(0, cx)),
                                        ),
                                )
                                .child(
                                    Button::new("save-view")
                                        .icon(IconName::Plus)
                                        .label("Save playlist")
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.show_save = !this.show_save;
                                            cx.notify();
                                        })),
                                ),
                        )
                    }),
            );
        if let Page::Playlist(id) = &self.page
            && let Some(playlist) = self.playlists.iter().find(|p| &p.id == id).cloned()
        {
            let export = playlist.clone();
            let rename = playlist.clone();
            let delete = playlist.clone();
            center=center.child(div().mx_6().mb_3().flex().flex_col().gap_2().when_some(playlist.query.clone(),|el,rule|el.child(small(format!("Live rule: {rule}"),cx))).child(div().flex().gap_2().child(Input::new(&self.playlist_name).flex_1()).child(Button::new("rename-playlist").label("Rename").on_click(cx.listener(move|this,_,_,cx|{let mut playlist=rename.clone();playlist.name=this.playlist_name.read(cx).value().to_string();playlist.updated_at=chrono::Utc::now().timestamp();match this.library.save_playlist(&playlist){Ok(())=>this.playlists=this.library.playlists().unwrap_or_default(),Err(e)=>this.notice=Some(e.to_string())}cx.notify();}))).child(Button::new("export-playlist").label("Export M3U").on_click(cx.listener(move|this,_,_,_|{let library=this.library.clone();let sender=this.sender.clone();let playlist=export.clone();std::thread::spawn(move||{if let Some(path)=rfd::FileDialog::new().set_file_name("playlist.m3u8").save_file(){let event=match library.export_playlist(&playlist,&path){Ok(())=>Event::Notice("Playlist exported.".into()),Err(e)=>Event::Error(e.to_string())};let _=sender.send(event);}});}))).child(Button::new("delete-playlist").label(if self.confirm_delete{"Confirm delete"}else{"Delete"}).on_click(cx.listener(move|this,_,window,cx|{if this.confirm_delete{match this.library.delete_playlist(&delete.id){Ok(())=>{this.playlists=this.library.playlists().unwrap_or_default();this.navigate(Page::Library,window,cx);this.notice=Some("Playlist deleted. Music files remain in your library.".into());},Err(e)=>this.notice=Some(e.to_string())}}else{this.confirm_delete=true;}cx.notify();})))));
        }
        if self.show_save {
            center = center.child(
                div()
                    .mx_6()
                    .mb_4()
                    .p_3()
                    .bg(cx.theme().secondary)
                    .rounded(px(7.))
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(Input::new(&self.playlist_name))
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .child(
                                Button::new("save-smart")
                                    .primary()
                                    .label("Save as smart playlist")
                                    .on_click(
                                        cx.listener(|this, _, _, cx| this.save_playlist(true, cx)),
                                    ),
                            )
                            .child(
                                Button::new("save-static")
                                    .label("Save shown tracks")
                                    .on_click(
                                        cx.listener(|this, _, _, cx| this.save_playlist(false, cx)),
                                    ),
                            )
                            .child(
                                Button::new("cancel-save")
                                    .ghost()
                                    .icon(IconName::Close)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.show_save = false;
                                        cx.notify();
                                    })),
                            ),
                    ),
            );
        }
        if let Some(error) = &self.query_error {
            center = center.child(
                div()
                    .px_6()
                    .pb_4()
                    .text_size(px(13.))
                    .text_color(color(0xf1a19a))
                    .child(error.clone()),
            );
        }
        if let Some(scan) = &self.scan {
            let cancel = self.cancel.clone();
            center = center.child(
                div()
                    .px_6()
                    .py_3()
                    .bg(cx.theme().secondary)
                    .flex()
                    .gap_3()
                    .items_center()
                    .child(
                        div()
                            .flex_1()
                            .child(label(format!("Importing · {} tracks", scan.scanned)))
                            .child(small(scan.current.clone(), cx)),
                    )
                    .child(
                        Button::new("cancel-import")
                            .ghost()
                            .small()
                            .label("Stop")
                            .on_click(move |_, _, _| cancel.store(true, Ordering::Relaxed)),
                    ),
            );
        }
        if let Some(notice) = &self.notice {
            center = center.child(
                div()
                    .mx_6()
                    .mb_3()
                    .p_3()
                    .rounded(px(6.))
                    .bg(cx.theme().secondary)
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(Icon::new(IconName::Info).size(px(16.)).flex_shrink_0())
                    .child(div().flex_1().text_size(px(12.)).child(notice.clone()))
                    .child(
                        Button::new("dismiss-notice")
                            .ghost()
                            .compact()
                            .icon(IconName::Close)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.notice = None;
                                cx.notify();
                            })),
                    ),
            );
        }
        center = center.child(match self.page {
            Page::Settings => self.settings_view(cx),
            Page::Queue => self.queue(cx),
            Page::History => self.history_view(cx),
            _ => self.collection(cx),
        });
        if is_collection && self.matched_total > 1000 {
            center = center.child(
                div()
                    .h(px(45.))
                    .px_6()
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_between()
                    .border_t_1()
                    .border_color(cx.theme().border)
                    .child(small(
                        format!(
                            "{}–{} of {} tracks",
                            self.page_offset + 1,
                            self.page_offset + self.tracks.len(),
                            self.matched_total
                        ),
                        cx,
                    ))
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .child(
                                Button::new("previous-page")
                                    .small()
                                    .label("Previous page")
                                    .disabled(self.page_offset == 0)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.page_offset = this.page_offset.saturating_sub(1000);
                                        this.refresh(cx);
                                    })),
                            )
                            .child(
                                Button::new("next-page")
                                    .small()
                                    .label("Next page")
                                    .disabled(
                                        self.page_offset + self.tracks.len() >= self.matched_total,
                                    )
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.page_offset += 1000;
                                        this.refresh(cx);
                                    })),
                            ),
                    ),
            );
        }
        div()
            .id("needle-app")
            .key_context("Needle")
            .track_focus(&self.focus)
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .font_family(cx.theme().font_family.clone())
            .flex()
            .flex_col()
            .overflow_hidden()
            .on_action(cx.listener(|this, _: &TogglePlayback, _, cx| {
                if this.playback.current.is_some() {
                    this.player.send(Command::Toggle)
                } else {
                    this.play_from(0, cx)
                }
            }))
            .on_action(cx.listener(|this, _: &NextTrack, _, _| this.player.send(Command::Next)))
            .on_action(
                cx.listener(|this, _: &PreviousTrack, _, _| this.player.send(Command::Previous)),
            )
            .on_action(cx.listener(|this, _: &FocusSearch, window, cx| {
                this.search.update(cx, |s, cx| s.focus(window, cx))
            }))
            .on_action(
                cx.listener(|this, _: &ImportFolder, window, cx| this.import_folder(window, cx)),
            )
            .on_action(cx.listener(|this, _: &EscapePanel, window, cx| {
                this.show_save = false;
                this.editing = false;
                window.focus(&this.focus);
                cx.notify();
            }))
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if this.focus.is_focused(window) && event.keystroke.key == "enter" {
                    let index = this
                        .selected
                        .as_ref()
                        .and_then(|t| this.tracks.iter().position(|r| r.id == t.id))
                        .unwrap_or(0);
                    this.play_from(index, cx);
                }
            }))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .child(self.sidebar(cx))
                    .child(center)
                    .when(
                        self.settings.show_inspector
                            && is_collection
                            && self.total > 0
                            && window.viewport_size().width > px(1100.),
                        |el| el.child(self.inspector(cx)),
                    ),
            )
            .child(self.transport(cx))
    }
}
