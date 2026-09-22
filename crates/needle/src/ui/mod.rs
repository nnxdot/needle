mod assets;
mod chrome;
mod library;
mod pages;
mod panel;
mod theme;
mod widgets;

use anyhow::Result;
use gpui::{prelude::*, *};
use gpui_component::{
    Root, TitleBar,
    input::{InputEvent, InputState},
    slider::{SliderEvent, SliderState},
};
use needle_core::{
    audio::{self, Command, PlaybackState, Player, QueueItem},
    database::Library,
    integrations::{self, RecordingMatch},
    model::{Listen, Playlist, Settings, Track},
    query,
    scan::{self, ScanProgress, TagEdit},
};
use rand::seq::SliceRandom;
use std::{
    collections::HashSet,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
pub use theme::{pal, set_theme};

actions!(
    needle,
    [
        TogglePlayback,
        NextTrack,
        PreviousTrack,
        FocusSearch,
        ImportFolder,
        EscapePanel,
        SelectPrevious,
        SelectNext,
        ExtendPrevious,
        ExtendNext,
        SelectAllTracks,
        PlaySelection,
        EditTags,
        ToggleFavorite,
        SeekForward,
        SeekBackward,
        VolumeUp,
        VolumeDown,
        ToggleQueue,
        GoBack,
        FocusNext,
        FocusPrevious,
    ]
);

/// Jump to a sidebar destination: Ctrl+1 … Ctrl+6, Ctrl+, for settings.
#[derive(Clone, PartialEq, serde::Deserialize, schemars::JsonSchema, Action)]
#[action(namespace = needle)]
pub struct GoTo(pub usize);

/// The most tracks one play action queues. Larger libraries play their first 50,000 matches.
const PLAY_LIMIT: usize = 50_000;
const PAGE_SIZE: usize = 1000;

#[derive(Clone, PartialEq, Debug)]
pub enum Page {
    Songs,
    Albums,
    Album { album: String, artist: String },
    Artists,
    Artist(String),
    Favorites,
    Recent,
    History,
    Playlist(String),
    Settings,
}
impl Page {
    fn title(&self) -> String {
        match self {
            Self::Songs => "Songs".into(),
            Self::Albums => "Albums".into(),
            Self::Album { album, .. } => {
                if album.is_empty() {
                    "Unknown album".into()
                } else {
                    album.clone()
                }
            }
            Self::Artists => "Artists".into(),
            Self::Artist(name) => name.clone(),
            Self::Favorites => "Favorites".into(),
            Self::Recent => "Recently added".into(),
            Self::History => "Listening history".into(),
            Self::Playlist(_) => "Playlist".into(),
            Self::Settings => "Settings".into(),
        }
    }
    /// The rule behind the page, before any search text is applied.
    fn base(&self) -> String {
        match self {
            Self::Favorites => "rating >= 4".into(),
            Self::Recent => "recent(30d) order by added_at desc".into(),
            Self::Album { album, artist } => format!(
                "album = {} and (album_artist = {artist} or (album_artist = \"\" and artist = {artist}))",
                quote(album),
                artist = quote(artist)
            ),
            Self::Artist(name) => format!("artist = {0} or album_artist = {0}", quote(name)),
            _ => String::new(),
        }
    }
    fn is_tracks(&self) -> bool {
        !matches!(self, Self::History | Self::Settings)
    }
    fn is_grid(&self) -> bool {
        matches!(self, Self::Albums | Self::Artists | Self::Artist(_))
    }
}

pub fn quote(text: &str) -> String {
    format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""))
}

#[derive(Clone, Copy, PartialEq)]
pub enum Panel {
    Details,
    Queue,
}

#[derive(Clone, Copy, PartialEq)]
pub enum Sort {
    Default,
    Asc(&'static str),
    Desc(&'static str),
}

pub struct Toast {
    text: String,
    error: bool,
    shown: Instant,
}

#[derive(Default)]
pub struct Selection {
    ids: HashSet<String>,
    anchor: Option<usize>,
    cursor: Option<usize>,
}

pub struct TrackMenu {
    position: Point<Pixels>,
    index: usize,
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
    Play(Vec<QueueItem>, Option<String>),
    Enqueue(Vec<QueueItem>, String),
    ServicesChanged,
    LastfmPending(integrations::LastfmPending),
    LastfmSignedIn(String),
}

pub struct TagFields {
    title: Entity<InputState>,
    artist: Entity<InputState>,
    album: Entity<InputState>,
    genre: Entity<InputState>,
    year: Entity<InputState>,
}

pub struct AppView {
    library: Library,
    player: Player,
    playback: PlaybackState,
    settings: Settings,
    page: Page,
    back: Vec<Page>,
    tracks: Vec<Track>,
    playlists: Vec<Playlist>,
    history: Vec<Listen>,
    selection: Selection,
    /// The track the details panel describes: the last one clicked.
    focused: Option<Track>,
    panel: Panel,
    menu: Option<TrackMenu>,
    sort: Sort,
    total: usize,
    matched_total: usize,
    page_offset: usize,
    loop_start: Option<f64>,
    search: Entity<InputState>,
    playlist_name: Entity<InputState>,
    autoplay: Entity<InputState>,
    sync_phrase: Entity<InputState>,
    tags: TagFields,
    volume: Entity<SliderState>,
    seek: Entity<SliderState>,
    focus: FocusHandle,
    list_scroll: UniformListScrollHandle,
    _subscriptions: Vec<Subscription>,
    events: crossbeam_channel::Receiver<Event>,
    sender: crossbeam_channel::Sender<Event>,
    scan: Option<ScanProgress>,
    cancel: Arc<AtomicBool>,
    toast: Option<Toast>,
    query_error: Option<String>,
    explanation: Option<String>,
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
    _watcher: Option<Box<dyn std::any::Any>>,
    last_history_id: Option<String>,
    muted_volume: Option<f32>,
    groups: Vec<library::Group>,
    service_status: Option<integrations::ServiceStatus>,
    scrobble_summary: Option<integrations::ScrobbleSummary>,
    service_busy: bool,
    lastfm_pending: Option<integrations::LastfmPending>,
    lastfm_key: Entity<InputState>,
    lastfm_secret: Entity<InputState>,
    listenbrainz_token: Entity<InputState>,
    acoustid_key: Entity<InputState>,
}

pub fn run(library: Library) -> Result<()> {
    Application::new()
        .with_assets(assets::Assets)
        .run(move |cx| {
            gpui_component::init(cx);
            let settings = library.settings().unwrap_or_default();
            set_theme(&settings.theme, None, cx);
            let tracks = Some("Needle && !Input");
            cx.bind_keys([
                KeyBinding::new("space", TogglePlayback, tracks),
                KeyBinding::new("ctrl-right", NextTrack, tracks),
                KeyBinding::new("ctrl-left", PreviousTrack, tracks),
                KeyBinding::new("right", SeekForward, tracks),
                KeyBinding::new("left", SeekBackward, tracks),
                KeyBinding::new("ctrl-up", VolumeUp, tracks),
                KeyBinding::new("ctrl-down", VolumeDown, tracks),
                KeyBinding::new("up", SelectPrevious, tracks),
                KeyBinding::new("down", SelectNext, tracks),
                KeyBinding::new("shift-up", ExtendPrevious, tracks),
                KeyBinding::new("shift-down", ExtendNext, tracks),
                KeyBinding::new("ctrl-a", SelectAllTracks, tracks),
                KeyBinding::new("enter", PlaySelection, tracks),
                KeyBinding::new("ctrl-e", EditTags, tracks),
                KeyBinding::new("ctrl-d", ToggleFavorite, tracks),
                KeyBinding::new("alt-left", GoBack, tracks),
                KeyBinding::new("backspace", GoBack, tracks),
                KeyBinding::new("ctrl-f", FocusSearch, Some("Needle")),
                KeyBinding::new("ctrl-k", FocusSearch, Some("Needle")),
                KeyBinding::new("ctrl-o", ImportFolder, Some("Needle")),
                KeyBinding::new("ctrl-j", ToggleQueue, Some("Needle")),
                KeyBinding::new("escape", EscapePanel, Some("Needle")),
                KeyBinding::new("tab", FocusNext, tracks),
                KeyBinding::new("shift-tab", FocusPrevious, Some("Needle")),
                KeyBinding::new("ctrl-1", GoTo(0), Some("Needle")),
                KeyBinding::new("ctrl-2", GoTo(1), Some("Needle")),
                KeyBinding::new("ctrl-3", GoTo(2), Some("Needle")),
                KeyBinding::new("ctrl-4", GoTo(3), Some("Needle")),
                KeyBinding::new("ctrl-5", GoTo(4), Some("Needle")),
                KeyBinding::new("ctrl-6", GoTo(5), Some("Needle")),
                KeyBinding::new("ctrl-,", GoTo(6), Some("Needle")),
            ]);
            let bounds = Bounds::centered(None, size(px(1380.), px(880.)), cx);
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                window_min_size: Some(size(px(900.), px(620.))),
                titlebar: Some(TitleBar::title_bar_options()),
                ..Default::default()
            };
            match cx.open_window(options, move |window, cx| {
                window.set_window_title("Needle");
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

fn watch(library: &Library, sender: &crossbeam_channel::Sender<Event>) -> Option<Box<dyn std::any::Any>> {
    let sender = sender.clone();
    scan::watch(library.clone(), move || {
        let _ = sender.send(Event::LibraryChanged);
    })
    .ok()
    .map(|w| Box::new(w) as Box<dyn std::any::Any>)
}

impl AppView {
    fn new(library: Library, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let settings = library.settings().unwrap_or_default();
        let player = Player::new(library.clone());
        let search = cx.new(|cx| {
            InputState::new(window, cx).placeholder("Search, or write a rule like  rating >= 4")
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
                .placeholder("Passphrase, at least 12 characters")
                .masked(true)
        });
        let field = |name: &str, window: &mut Window, cx: &mut Context<Self>| {
            let name = name.to_string();
            cx.new(|cx| InputState::new(window, cx).placeholder(name))
        };
        let secret = |name: &str, window: &mut Window, cx: &mut Context<Self>| {
            let name = name.to_string();
            cx.new(|cx| InputState::new(window, cx).placeholder(name).masked(true))
        };
        let lastfm_key = field("Last.fm API key", window, cx);
        let lastfm_secret = secret("Shared secret", window, cx);
        let listenbrainz_token = secret("ListenBrainz user token", window, cx);
        let acoustid_key = secret("AcoustID application key", window, cx);
        let tags = TagFields {
            title: field("Title", window, cx),
            artist: field("Artist", window, cx),
            album: field("Album", window, cx),
            genre: field("Genre", window, cx),
            year: field("Year", window, cx),
        };
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
            cx.subscribe_in(&search, window, |this, _, event, window, cx| match event {
                InputEvent::Change => {
                    this.page_offset = 0;
                    let typing = !this.search.read(cx).value().is_empty();
                    if typing && !matches!(this.page, Page::Songs | Page::Favorites | Page::Recent | Page::Playlist(_) | Page::Album { .. }) {
                        this.back.push(this.page.clone());
                        this.page = Page::Songs;
                    }
                    this.refresh(cx);
                }
                InputEvent::PressEnter { .. } => {
                    window.focus(&this.focus);
                    if this.selection.cursor.is_none() && !this.tracks.is_empty() {
                        this.select_single(0, cx);
                    }
                }
                _ => {}
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
        let library_for_scrobbles = library.clone();
        let watcher = watch(&library, &sender);
        let mut view = Self {
            total: library.count().unwrap_or(0),
            playlists: library.playlists().unwrap_or_default(),
            history: library.history(200).unwrap_or_default(),
            library,
            player,
            playback: PlaybackState::default(),
            settings,
            page: Page::Songs,
            back: vec![],
            tracks: vec![],
            selection: Selection::default(),
            focused: None,
            panel: Panel::Details,
            menu: None,
            sort: Sort::Default,
            matched_total: 0,
            page_offset: 0,
            loop_start: None,
            search,
            playlist_name,
            autoplay,
            sync_phrase,
            tags,
            volume,
            seek,
            focus,
            list_scroll: UniformListScrollHandle::new(),
            _subscriptions: subscriptions,
            events,
            sender,
            scan: None,
            cancel: Arc::new(AtomicBool::new(false)),
            toast: None,
            query_error: None,
            explanation: None,
            generation: 0,
            loading: true,
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
            muted_volume: None,
            groups: vec![],
            service_status: None,
            scrobble_summary: None,
            service_busy: false,
            lastfm_pending: None,
            lastfm_key,
            lastfm_secret,
            listenbrainz_token,
            acoustid_key,
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

    fn notify(&mut self, text: impl Into<String>) {
        self.toast = Some(Toast {
            text: text.into(),
            error: false,
            shown: Instant::now(),
        });
    }
    fn fail(&mut self, text: impl Into<String>) {
        self.toast = Some(Toast {
            text: text.into(),
            error: true,
            shown: Instant::now(),
        });
    }

    fn refresh_services(&mut self) {
        self.service_status = Some(integrations::secret_status());
        self.scrobble_summary = integrations::scrobble_summary(&self.library).ok();
    }

    fn persist_settings(&mut self) {
        self.settings.volume = self.playback.volume;
        if let Err(e) = self.library.save_settings(&self.settings) {
            self.fail(e.to_string());
        }
    }
    fn configure(&mut self) {
        self.settings.volume = self.playback.volume;
        self.player.send(Command::Configure(self.settings.clone()));
        self.persist_settings();
    }

    fn poll(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let playback = self.player.state();
        if playback.error != self.playback.error
            && let Some(error) = &playback.error
        {
            self.fail(error.clone());
        }
        let changed = playback.playing != self.playback.playing
            || playback.current.as_ref().map(|i| &i.track.id)
                != self.playback.current.as_ref().map(|i| &i.track.id);
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
        if let Some(toast) = &self.toast {
            let life = if toast.error { 14 } else { 6 };
            if toast.shown.elapsed() > Duration::from_secs(life) {
                self.toast = None;
            }
        }
        while let Ok(event) = self.events.try_recv() {
            match event {
                Event::Imported(progress) => {
                    if progress.done {
                        let unreadable = if progress.errors.is_empty() {
                            String::new()
                        } else {
                            format!(" · {} files could not be read", progress.errors.len())
                        };
                        if progress.scanned > 0 || !unreadable.is_empty() {
                            self.notify(format!(
                                "{} tracks added or updated · {} already in your library{unreadable}",
                                progress.imported, progress.unchanged
                            ));
                        }
                        self.scan = None;
                        self.total = self.library.count().unwrap_or(0);
                        self.refresh(cx);
                        self._watcher = watch(&self.library, &self.sender);
                    } else {
                        self.scan = Some(progress);
                    }
                }
                Event::Error(error) => {
                    self.fail(error);
                    self.scan = None;
                    self.lookup_busy = false;
                    self.loading = false;
                }
                Event::Loaded(generation, tracks, total) => {
                    if generation == self.generation {
                        self.tracks = tracks;
                        self.matched_total = total;
                        self.loading = false;
                        self.reconcile_selection();
                        if self.page.is_grid() {
                            self.groups = library::group(&self.tracks, self.page == Page::Artists);
                        }
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
                    self.notify("Layout applied.");
                }
                Event::LibraryChanged => {
                    self.total = self.library.count().unwrap_or(0);
                    self.refresh(cx);
                }
                Event::Matches(id, matches) => {
                    self.lookup_busy = false;
                    if self.focused.as_ref().is_none_or(|t| t.id != id) {
                        continue;
                    }
                    self.matches = matches;
                    if self.matches.is_empty() {
                        self.notify("No matching recordings found. Try correcting the artist or title first.");
                    }
                }
                Event::SavedTags => {
                    self.editing = false;
                    self.matches.clear();
                    self.notify("Tags saved. The original file is backed up in your library folder.");
                    if let Some(track) = &self.focused {
                        self.focused = self.library.track(&track.id).ok().flatten();
                    }
                    self.refresh(cx);
                }
                Event::Notice(notice) => {
                    if !notice.is_empty() {
                        self.notify(notice);
                    }
                    self.refresh(cx);
                    self.playlists = self.library.playlists().unwrap_or_default();
                    self.history = self.library.history(200).unwrap_or_default();
                    if let Some(track) = &self.focused {
                        self.focused = self.library.track(&track.id).ok().flatten();
                    }
                }
                Event::Play(items, notice) => {
                    if items.is_empty() {
                        self.notify("Nothing here can be played. The files may be unavailable.");
                    } else {
                        self.player.send(Command::Play(items));
                    }
                    if let Some(notice) = notice {
                        self.notify(notice);
                    }
                }
                Event::ServicesChanged => {
                    self.service_busy = false;
                    self.refresh_services();
                }
                Event::LastfmPending(pending) => {
                    cx.open_url(&pending.auth_url);
                    self.lastfm_pending = Some(pending);
                }
                Event::LastfmSignedIn(user) => {
                    self.lastfm_pending = None;
                    self.settings.lastfm_enabled = true;
                    self.persist_settings();
                    self.notify(format!("Signed in to Last.fm as {user}. Scrobbling is on."));
                }
                Event::Enqueue(items, notice) => {
                    self.player.send(Command::Enqueue(items));
                    self.notify(notice);
                }
            }
        }
        cx.notify();
    }

    fn search_text(&self, cx: &App) -> String {
        self.search.read(cx).value().trim().to_string()
    }

    /// The full rule for what the current page shows, including search text and column sort.
    fn expression(&self, cx: &App) -> String {
        let search = self.search_text(cx);
        let base = self.page.base();
        let mut expression = if base.is_empty() {
            search
        } else if search.is_empty() {
            base
        } else {
            let text = quote(&search);
            let base = base.split(" order by ").next().unwrap_or_default().to_string();
            format!(
                "({base}) and (title contains {text} or artist contains {text} or album contains {text})"
            )
        };
        if let Sort::Asc(field) | Sort::Desc(field) = self.sort
            && !expression.contains(" order by ")
            && !expression.contains("shuffle")
            && !expression.contains(" limit ")
        {
            let direction = if matches!(self.sort, Sort::Asc(_)) { "asc" } else { "desc" };
            if expression.trim().is_empty() {
                expression = format!("order by {field} {direction}");
            } else if query::compile(&expression, 0).is_ok_and(|q| q.explanation.starts_with("Title, artist")) {
                let text = quote(&expression);
                expression = format!(
                    "(title contains {text} or artist contains {text} or album contains {text} or genre contains {text}) order by {field} {direction}"
                );
            } else {
                expression = format!("{expression} order by {field} {direction}");
            }
        }
        expression
    }

    fn manual_playlist(&self) -> Option<Playlist> {
        if let Page::Playlist(id) = &self.page {
            self.playlists
                .iter()
                .find(|p| &p.id == id && p.query.is_none())
                .cloned()
        } else {
            None
        }
    }

    fn refresh(&mut self, cx: &mut Context<Self>) {
        if !self.page.is_tracks() {
            return;
        }
        let mut expression = self.expression(cx);
        if let Page::Playlist(id) = &self.page
            && let Some(rule) = self.playlists.iter().find(|p| &p.id == id).and_then(|p| p.query.clone())
        {
            let search = self.search_text(cx);
            expression = if search.is_empty() {
                rule
            } else {
                let text = quote(&search);
                format!("({rule}) and (title contains {text} or artist contains {text} or album contains {text})")
            };
        }
        self.generation += 1;
        let generation = self.generation;
        match query::compile(&expression, chrono::Utc::now().timestamp()) {
            Ok(q) => {
                self.query_error = None;
                self.explanation = Some(q.explanation);
            }
            Err(error) => {
                self.query_error = Some(error.to_string());
                self.explanation = None;
                self.loading = false;
                cx.notify();
                return;
            }
        }
        self.loading = true;
        let library = self.library.clone();
        let sender = self.sender.clone();
        let manual = self.manual_playlist();
        let offset = self.page_offset;
        let filter = self.search_text(cx);
        let limit = if self.page.is_grid() { 500_000 } else { PAGE_SIZE };
        std::thread::spawn(move || {
            let result = (|| -> Result<needle_core::database::SearchPage> {
                if let Some(list) = manual {
                    let mut tracks = library.playlist_tracks(&list)?;
                    if !filter.is_empty() {
                        let ids = library
                            .search(&filter)?
                            .into_iter()
                            .map(|t| t.id)
                            .collect::<HashSet<_>>();
                        tracks.retain(|t| ids.contains(&t.id));
                    }
                    let total = tracks.len();
                    let tracks = tracks.into_iter().skip(offset).take(PAGE_SIZE).collect();
                    Ok(needle_core::database::SearchPage { tracks, total })
                } else {
                    library.search_page(&expression, offset, limit)
                }
            })();
            let _ = sender.send(match result {
                Ok(page) => Event::Loaded(generation, page.tracks, page.total),
                Err(e) => Event::SearchFailed(generation, format!("{e:#}")),
            });
        });
        cx.notify();
    }

    fn navigate(&mut self, page: Page, window: &mut Window, cx: &mut Context<Self>) {
        if page != self.page {
            self.back.push(self.page.clone());
            if self.back.len() > 50 {
                self.back.remove(0);
            }
        }
        self.open(page, window, cx);
    }
    fn go_back(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(page) = self.back.pop() {
            self.open(page, window, cx);
        }
    }
    fn open(&mut self, page: Page, window: &mut Window, cx: &mut Context<Self>) {
        self.page = page;
        self.confirm_delete = false;
        self.menu = None;
        self.sort = Sort::Default;
        if let Page::Playlist(id) = &self.page
            && let Some(list) = self.playlists.iter().find(|p| &p.id == id)
        {
            self.playlist_name
                .update(cx, |s, cx| s.set_value(list.name.clone(), window, cx));
        }
        self.page_offset = 0;
        self.tracks.clear();
        self.selection = Selection::default();
        self.search.update(cx, |s, cx| s.set_value("", window, cx));
        self.show_save = false;
        if self.page == Page::History {
            self.history = self.library.history(200).unwrap_or_default();
        }
        if self.page == Page::Settings {
            self.output_devices = audio::devices().unwrap_or_default();
            self.refresh_services();
        }
        self.groups.clear();
        self.refresh(cx);
        window.focus(&self.focus);
    }

    fn reconcile_selection(&mut self) {
        let ids = &self.selection.ids;
        if ids.is_empty() {
            return;
        }
        let present: HashSet<String> = self
            .tracks
            .iter()
            .filter(|t| ids.contains(&t.id))
            .map(|t| t.id.clone())
            .collect();
        self.selection.cursor = self
            .selection
            .cursor
            .filter(|&c| self.tracks.get(c).is_some_and(|t| present.contains(&t.id)));
        self.selection.ids = present;
    }

    fn select_single(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(track) = self.tracks.get(index) {
            self.selection.ids = HashSet::from([track.id.clone()]);
            self.selection.anchor = Some(index);
            self.selection.cursor = Some(index);
            self.focus_track(track.clone());
        }
        cx.notify();
    }
    fn extend_to(&mut self, index: usize, cx: &mut Context<Self>) {
        let anchor = self.selection.anchor.unwrap_or(index);
        let (a, b) = (anchor.min(index), anchor.max(index));
        self.selection.ids = self.tracks[a..=b.min(self.tracks.len() - 1)]
            .iter()
            .map(|t| t.id.clone())
            .collect();
        self.selection.anchor = Some(anchor);
        self.selection.cursor = Some(index);
        cx.notify();
    }
    fn click_track(&mut self, index: usize, event: &ClickEvent, window: &mut Window, cx: &mut Context<Self>) {
        let Some(track) = self.tracks.get(index).cloned() else {
            return;
        };
        let modifiers = event.modifiers();
        self.menu = None;
        window.focus(&self.focus);
        if modifiers.shift && !self.tracks.is_empty() {
            self.extend_to(index, cx);
        } else if modifiers.control {
            if !self.selection.ids.remove(&track.id) {
                self.selection.ids.insert(track.id.clone());
            }
            self.selection.anchor = Some(index);
            self.selection.cursor = Some(index);
        } else {
            self.select_single(index, cx);
            if event.click_count() == 2 {
                self.play_view(index, false, cx);
            }
        }
        cx.notify();
    }
    fn focus_track(&mut self, track: Track) {
        if self.focused.as_ref().is_none_or(|t| t.id != track.id) {
            self.editing = false;
            self.matches.clear();
        }
        self.focused = Some(track);
        if self.panel == Panel::Queue && !self.settings.show_inspector {
            self.panel = Panel::Details;
        }
    }
    fn move_cursor(&mut self, delta: isize, extend: bool, cx: &mut Context<Self>) {
        if self.tracks.is_empty() {
            return;
        }
        let last = self.tracks.len() as isize - 1;
        let next = match self.selection.cursor {
            Some(c) => (c as isize + delta).clamp(0, last) as usize,
            None => 0,
        };
        if extend {
            self.extend_to(next, cx);
        } else {
            self.select_single(next, cx);
        }
        let strategy = if delta < 0 { ScrollStrategy::Top } else { ScrollStrategy::Bottom };
        self.list_scroll.scroll_to_item(next, strategy);
    }
    fn selected_tracks(&self) -> Vec<Track> {
        self.tracks
            .iter()
            .filter(|t| self.selection.ids.contains(&t.id))
            .cloned()
            .collect()
    }

    fn import_folder(&mut self, cx: &mut Context<Self>) {
        if self.scan.is_some() {
            return;
        }
        let library = self.library.clone();
        let sender = self.sender.clone();
        let cancel = Arc::new(AtomicBool::new(false));
        self.cancel = cancel.clone();
        self.scan = Some(ScanProgress {
            current: "Choose a music folder…".into(),
            ..Default::default()
        });
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
                let mut total = ScanProgress::default();
                for root in library.roots()? {
                    let p = scan::import(&library, &PathBuf::from(root), cancel.clone(), |mut p| {
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

    fn reason(&self, cx: &App) -> String {
        match &self.page {
            Page::Album { album, .. } => format!("From the album {album}"),
            Page::Artist(name) => format!("From {name}"),
            Page::Playlist(id) => self
                .playlists
                .iter()
                .find(|p| &p.id == id)
                .map(|p| format!("From your playlist {}", p.name))
                .unwrap_or_else(|| "From a playlist".into()),
            _ => {
                let search = self.search_text(cx);
                if search.is_empty() {
                    format!("From {}", self.page.title().to_lowercase())
                } else {
                    format!("Matches “{search}”")
                }
            }
        }
    }

    /// Play everything the current view matches — across pages — starting at `index` of the shown page.
    fn play_view(&mut self, index: usize, shuffle: bool, cx: &mut Context<Self>) {
        let reason = if shuffle {
            format!("Shuffled · {}", self.reason(cx))
        } else {
            self.reason(cx)
        };
        let start = self.page_offset + index;
        let complete = self.page_offset == 0 && self.tracks.len() >= self.matched_total;
        let sender = self.sender.clone();
        let finish = move |mut tracks: Vec<Track>, start: usize, truncated: bool| {
            let mut tracks: Vec<Track> = if shuffle {
                let first = (start > 0 || !tracks.is_empty()).then(|| tracks.get(start).cloned()).flatten();
                tracks.shuffle(&mut rand::thread_rng());
                if let (Some(first), true) = (first, start > 0) {
                    tracks.retain(|t| t.id != first.id);
                    tracks.insert(0, first);
                }
                tracks
            } else {
                tracks.drain(start.min(tracks.len())..).collect()
            };
            tracks.retain(|t| !t.missing);
            let items = tracks
                .into_iter()
                .map(|track| QueueItem {
                    track,
                    reason: reason.clone(),
                })
                .collect();
            let notice = truncated.then(|| format!("Playing the first {} matching tracks.", widgets::count(PLAY_LIMIT)));
            let _ = sender.send(Event::Play(items, notice));
        };
        if complete {
            finish(self.tracks.clone(), index, false);
            return;
        }
        let library = self.library.clone();
        let manual = self.manual_playlist();
        let expression = self.expression(cx);
        let error = self.sender.clone();
        std::thread::spawn(move || {
            let result = if let Some(list) = manual {
                library.playlist_tracks(&list)
            } else {
                library
                    .search_page(&expression, 0, PLAY_LIMIT)
                    .map(|page| page.tracks)
            };
            match result {
                Ok(tracks) => {
                    let truncated = tracks.len() >= PLAY_LIMIT;
                    finish(tracks, start, truncated)
                }
                Err(e) => {
                    let _ = error.send(Event::Error(format!("{e:#}")));
                }
            }
        });
    }
    fn play_tracks(&mut self, tracks: Vec<Track>, reason: &str) {
        let items = tracks
            .into_iter()
            .filter(|t| !t.missing)
            .map(|track| QueueItem {
                track,
                reason: reason.into(),
            })
            .collect();
        let _ = self.sender.send(Event::Play(items, None));
    }
    fn enqueue(&mut self, tracks: Vec<Track>) {
        let count = tracks.len();
        let items = tracks
            .into_iter()
            .filter(|t| !t.missing)
            .map(|track| QueueItem {
                track,
                reason: "Added to the queue by you".into(),
            })
            .collect();
        let _ = self.sender.send(Event::Enqueue(
            items,
            if count == 1 {
                "Added to the queue.".into()
            } else {
                format!("Added {count} tracks to the queue.")
            },
        ));
    }
    fn toggle_playback(&mut self, cx: &mut Context<Self>) {
        if self.playback.current.is_some() {
            self.player.send(Command::Toggle)
        } else if !self.tracks.is_empty() {
            self.play_view(self.selection.cursor.unwrap_or(0), false, cx);
        }
    }

    fn set_rating(&mut self, ids: &[String], rating: i64) {
        for id in ids {
            if let Err(e) = self.library.rate(id, rating) {
                self.fail(e.to_string());
                return;
            }
            for track in self.tracks.iter_mut().filter(|t| &t.id == id) {
                track.rating = rating;
            }
            if let Some(track) = self.focused.as_mut().filter(|t| &t.id == id) {
                track.rating = rating;
            }
        }
    }

    fn save_playlist(&mut self, smart: bool, cx: &mut Context<Self>) {
        let name = self.playlist_name.read(cx).value().trim().to_string();
        if name.is_empty() {
            self.fail("Give the playlist a name first.");
            return;
        }
        let expression = self.expression(cx);
        if smart && expression.trim().is_empty() {
            self.fail("A smart playlist needs a rule. Type one in search first, like  rating >= 4.");
            return;
        }
        let selected = self.selected_tracks();
        let track_ids = if selected.len() > 1 { selected } else { self.tracks.clone() }
            .into_iter()
            .map(|t| t.id)
            .collect();
        let playlist = Playlist {
            id: crate::uuid_string(),
            name,
            query: smart.then_some(expression),
            track_ids: if smart { vec![] } else { track_ids },
            updated_at: chrono::Utc::now().timestamp(),
        };
        match self.library.save_playlist(&playlist) {
            Ok(()) => {
                self.playlists = self.library.playlists().unwrap_or_default();
                self.show_save = false;
                self.notify(format!("Saved “{}”.", playlist.name));
            }
            Err(e) => self.fail(e.to_string()),
        }
        cx.notify();
    }
    fn add_to_playlist(&mut self, playlist_id: &str, tracks: Vec<Track>) {
        let Some(mut playlist) = self.playlists.iter().find(|p| p.id == playlist_id).cloned() else {
            return;
        };
        let count = tracks.len();
        playlist.track_ids.extend(tracks.into_iter().map(|t| t.id));
        playlist.updated_at = chrono::Utc::now().timestamp();
        match self.library.save_playlist(&playlist) {
            Ok(()) => {
                self.playlists = self.library.playlists().unwrap_or_default();
                self.notify(format!(
                    "Added {} to “{}”.",
                    if count == 1 { "1 track".to_string() } else { format!("{count} tracks") },
                    playlist.name
                ));
            }
            Err(e) => self.fail(e.to_string()),
        }
    }

    fn edit_tags(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(track) = self.focused.clone() else {
            return;
        };
        self.pending_mbid = None;
        let values = [
            (&self.tags.title, track.title.clone()),
            (&self.tags.artist, track.artist.clone()),
            (&self.tags.album, track.album.clone()),
            (&self.tags.genre, track.genre.clone()),
            (
                &self.tags.year,
                if track.year > 0 { track.year.to_string() } else { String::new() },
            ),
        ];
        for (input, value) in values {
            input.update(cx, |s, cx| s.set_value(value, window, cx));
        }
        self.editing = true;
        self.panel = Panel::Details;
        self.settings.show_inspector = true;
        cx.notify();
    }
    fn write_tags(&mut self, cx: &mut Context<Self>) {
        let Some(track) = &self.focused else {
            return;
        };
        let value = |input: &Entity<InputState>| input.read(cx).value().trim().to_string();
        let year = value(&self.tags.year);
        let year = if year.is_empty() {
            None
        } else {
            match year.parse::<u32>() {
                Ok(y) if y <= 9999 => Some(y),
                _ => {
                    self.fail("Year must be a number, like 1997.");
                    return;
                }
            }
        };
        let edit = TagEdit {
            title: Some(value(&self.tags.title)),
            artist: Some(value(&self.tags.artist)),
            album: Some(value(&self.tags.album)),
            genre: Some(value(&self.tags.genre)),
            year,
            musicbrainz_id: self.pending_mbid.clone(),
            ..Default::default()
        };
        let library = self.library.clone();
        let sender = self.sender.clone();
        let id = track.id.clone();
        self.notify("Writing tags and checking the audio is unchanged…");
        std::thread::spawn(move || {
            let _ = sender.send(match scan::write_tags(&library, &id, &edit) {
                Ok(()) => Event::SavedTags,
                Err(e) => Event::Error(format!("{e:#}")),
            });
        });
    }
    fn lookup(&mut self, cx: &mut Context<Self>) {
        if let Some(track) = &self.focused {
            let library = self.library.clone();
            let sender = self.sender.clone();
            let id = track.id.clone();
            let artist = track.artist.clone();
            let title = track.title.clone();
            self.lookup_busy = true;
            std::thread::spawn(move || {
                let _ = sender.send(match integrations::musicbrainz_search(&library, &artist, &title) {
                    Ok(m) => Event::Matches(id, m),
                    Err(e) => Event::Error(format!("MusicBrainz: {e:#}")),
                });
            });
            cx.notify();
        }
    }
    fn measure_loudness(&mut self, id: String) {
        let library = self.library.clone();
        let sender = self.sender.clone();
        self.notify("Measuring loudness in the background…");
        std::thread::spawn(move || {
            let _ = sender.send(match needle_core::analysis::scan_loudness(&library, &id) {
                Ok(result) => Event::Notice(format!(
                    "Measured {:.1} LUFS · ReplayGain {:+.2} dB. Turn on ReplayGain in Settings to use it.",
                    result.integrated_lufs, result.replay_gain_db
                )),
                Err(error) => Event::Error(format!("{error:#}")),
            });
        });
    }
    fn background(&self, job: impl FnOnce() -> Result<String> + Send + 'static) {
        let sender = self.sender.clone();
        std::thread::spawn(move || {
            let _ = sender.send(match job() {
                Ok(message) => Event::Notice(message),
                Err(e) => Event::Error(format!("{e:#}")),
            });
        });
    }
    fn nudge_volume(&mut self, delta: f32, window: &mut Window, cx: &mut Context<Self>) {
        let volume = (self.playback.volume + delta).clamp(0., 1.);
        self.player.send(Command::Volume(volume));
        self.volume
            .update(cx, |s, cx| s.set_value(volume, window, cx));
    }
}

impl Drop for AppView {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
        self.persist_settings();
        self.player.shutdown();
    }
}

impl Render for AppView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = pal(cx);
        let width = window.viewport_size().width;
        let show_panel = self.settings.show_inspector
            && width > px(1080.)
            && self.total > 0
            && (self.page.is_tracks() || self.panel == Panel::Queue);
        let sidebar = self.settings.layout.sidebar_width.clamp(200., 260.);
        let panel_width = self.settings.layout.inspector_width.clamp(280., 340.) + 16.;
        let content_width = f32::from(width) - sidebar - if show_panel { panel_width } else { 0. };
        div()
            .id("needle-app")
            .key_context("Needle")
            .track_focus(&self.focus)
            .size_full()
            .bg(p.canvas)
            .text_color(p.ink)
            .font_family(gpui_component::ActiveTheme::theme(&**cx).font_family.clone())
            .text_size(px(13.))
            .flex()
            .flex_col()
            .overflow_hidden()
            .on_action(cx.listener(|this, _: &TogglePlayback, _, cx| this.toggle_playback(cx)))
            .on_action(cx.listener(|this, _: &NextTrack, _, _| this.player.send(Command::Next)))
            .on_action(cx.listener(|this, _: &PreviousTrack, _, _| this.player.send(Command::Previous)))
            .on_action(cx.listener(|this, _: &SeekForward, _, _| {
                if this.playback.current.is_some() {
                    this.player.send(Command::Seek(this.playback.position + 10.))
                }
            }))
            .on_action(cx.listener(|this, _: &SeekBackward, _, _| {
                if this.playback.current.is_some() {
                    this.player.send(Command::Seek((this.playback.position - 10.).max(0.)))
                }
            }))
            .on_action(cx.listener(|this, _: &VolumeUp, window, cx| this.nudge_volume(0.05, window, cx)))
            .on_action(cx.listener(|this, _: &VolumeDown, window, cx| this.nudge_volume(-0.05, window, cx)))
            .on_action(cx.listener(|this, _: &SelectPrevious, _, cx| this.move_cursor(-1, false, cx)))
            .on_action(cx.listener(|this, _: &SelectNext, _, cx| this.move_cursor(1, false, cx)))
            .on_action(cx.listener(|this, _: &ExtendPrevious, _, cx| this.move_cursor(-1, true, cx)))
            .on_action(cx.listener(|this, _: &ExtendNext, _, cx| this.move_cursor(1, true, cx)))
            .on_action(cx.listener(|this, _: &SelectAllTracks, _, cx| {
                this.selection.ids = this.tracks.iter().map(|t| t.id.clone()).collect();
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &PlaySelection, _, cx| {
                let selected = this.selected_tracks();
                if selected.len() > 1 {
                    let reason = this.reason(cx);
                    this.play_tracks(selected, &reason);
                } else {
                    this.play_view(this.selection.cursor.unwrap_or(0), false, cx);
                }
            }))
            .on_action(cx.listener(|this, _: &EditTags, window, cx| this.edit_tags(window, cx)))
            .on_action(cx.listener(|this, _: &ToggleFavorite, _, cx| {
                let selected = this.selected_tracks();
                let rating = if selected.iter().all(|t| t.rating >= 4) { 0 } else { 5 };
                let ids: Vec<String> = selected.into_iter().map(|t| t.id).collect();
                this.set_rating(&ids, rating);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &ToggleQueue, _, cx| {
                if this.settings.show_inspector && this.panel == Panel::Queue {
                    this.settings.show_inspector = false;
                } else {
                    this.settings.show_inspector = true;
                    this.panel = Panel::Queue;
                }
                this.persist_settings();
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &GoBack, window, cx| this.go_back(window, cx)))
            .on_action(|_: &FocusNext, window, _| window.focus_next())
            .on_action(|_: &FocusPrevious, window, _| window.focus_prev())
            .on_action(cx.listener(|this, GoTo(index): &GoTo, window, cx| {
                let page = match index {
                    0 => Page::Songs,
                    1 => Page::Albums,
                    2 => Page::Artists,
                    3 => Page::Favorites,
                    4 => Page::Recent,
                    5 => Page::History,
                    _ => Page::Settings,
                };
                this.navigate(page, window, cx);
            }))
            .on_action(cx.listener(|this, _: &FocusSearch, window, cx| {
                this.search.update(cx, |s, cx| s.focus(window, cx))
            }))
            .on_action(cx.listener(|this, _: &ImportFolder, _, cx| this.import_folder(cx)))
            .on_action(cx.listener(|this, _: &EscapePanel, window, cx| {
                if this.menu.take().is_none() && !this.show_save && !this.editing {
                    if !this.search_text(cx).is_empty() {
                        this.search.update(cx, |s, cx| s.set_value("", window, cx));
                        this.refresh(cx);
                    } else {
                        this.selection = Selection::default();
                    }
                }
                this.show_save = false;
                this.editing = false;
                window.focus(&this.focus);
                cx.notify();
            }))
            .child(self.title_bar(sidebar, cx))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .child(self.sidebar(sidebar, cx))
                    .child(self.main(content_width, window, cx))
                    .when(show_panel, |el| el.child(self.panel(panel_width, window, cx))),
            )
            .child(self.player_bar(width, cx))
            .children(self.toast(cx))
            .children(self.track_menu(cx))
    }
}
