import AppKit
import Observation
import SwiftUI
import UniformTypeIdentifiers

/// Holds Needle's core for the whole app: every window, the menu bar icon, and the media keys
/// share it. What plays is read from the core several times a second.
@MainActor @Observable
final class Store {
    /// The one store. Views read it here, not from the environment: SwiftUI draws some views
    /// (drag images, menus) without the environment, and a missing store there stops the app.
    static let shared = Store()

    let core: Needle
    let version: String

    // MARK: What the core says

    private(set) var settings: NeedleSettings { didSet { refreshLook() } }
    /// What the views draw with, kept apart from `settings` and changed only when it changes:
    /// a view that reads `settings` redraws at every change to any setting (folding the
    /// sidebar redrew every row of the song table).
    private(set) var colors = Palette.build(Palette.base("dark"), tint: nil, ambient: false)
    private(set) var motion = true
    private(set) var displayFont = "system"
    /// Song rows in the compact height.
    private(set) var compactRows = false
    private(set) var playback: MacPlayback?
    /// The song playing, with every field.
    private(set) var current: Track?
    /// The songs up next (the first few hundred), read again whenever the queue changes.
    private(set) var queue: [QueueEntry] = []
    private(set) var queueSeconds: Double = 0
    /// The playing song's lyrics, once looked up.
    private(set) var lyrics: Lyrics?
    private(set) var lyricsFor: String?
    private(set) var lyricsLooking = false
    /// The timed lyric line being sung; it changes only when the line does.
    private(set) var lyricLine: Int?
    private(set) var scan = ScanStatus(running: false, scanned: 0, imported: 0, current: "", error: nil)
    /// Changes whenever the library may have changed, so pages load again.
    private(set) var libraryVersion = 0
    private(set) var playlists: [Playlist] = []
    private(set) var plugins: [PluginItem] = []
    private(set) var themes: [ThemeFile] = [] { didSet { refreshLook() } }
    /// The playing cover's colour, for tinting the app.
    private(set) var coverTint: NSColor? { didSet { refreshLook() } }
    /// The open album's or artist's colour, from its picture: while one of those pages is
    /// open, the whole window takes it, as Apple Music's pages do (see `Palette.paged`).
    private(set) var pageTint: NSColor? { didSet { refreshLook() } }
    /// The window is in an open page's colours (see `Palette.paged`).
    var isPaged: Bool { settings.coverColors && pageTint != nil && !big }
    /// The accent without an open page's colours: for the always-dark mini player and the
    /// sidebar (whose highlight AppKit keeps from when it was made).
    private(set) var playerAccent: Color = .accentColor
    private(set) var stemsJob: StemJob?
    private(set) var importProgress: String?
    private(set) var update: MacUpdate?

    // MARK: What the window shows

    var route: Route = .home
    private(set) var back: [Route] = []
    private(set) var forward: [Route] = []
    var search = "" {
        didSet { searchChanged(from: oldValue) }
    }
    /// The song the details panel describes: the last one clicked.
    var focused: Track?
    var panel: Panel = .details
    /// The big player is open: the window is the player's, not a page's colours.
    var big = false { didSet { if big != oldValue, pageTint != nil { refreshLook() } } }
    var bigSide: BigSide = .lyrics
    var immersive = false
    var palette = false
    var toast: Toast?
    var question: Question?
    var editor: PlaylistModel?
    /// The playlist whose "Add songs" window is open.
    var addingTo: PlaylistModel?
    /// Songs whose tags are being edited.
    var tagging: [String]?
    var welcome = false
    var whatsNew: Notes?
    /// The A point of an A–B loop being set.
    var loopStart: Double?
    /// The volume before muting.
    var mutedVolume: Float?
    /// A request to focus the search field.
    var focusSearch = 0

    enum Panel: String, CaseIterable { case details = "Details", queue = "Queue", lyrics = "Lyrics" }
    enum BigSide: String, CaseIterable { case lyrics = "Lyrics", queue = "Up next", stems = "Stems" }

    struct Toast: Identifiable, Equatable {
        let id = UUID()
        let text: String
        let error: Bool
    }

    struct Question: Identifiable {
        let id: UInt64
        let plugin: String
        let title: String
        let prompt: String
        var answer: String
    }

    @ObservationIgnored private var timer: Timer?
    @ObservationIgnored private var ticks = 0
    @ObservationIgnored private var queueVersion: UInt64 = .max
    @ObservationIgnored private var changes: UInt64 = 0
    @ObservationIgnored private var wasScanning = false
    @ObservationIgnored private var lastError: String?
    @ObservationIgnored private var lastNotice: String?
    @ObservationIgnored private var tintFor: String?
    @ObservationIgnored private var coverFetched: Set<String> = []
    @ObservationIgnored private var toastTask: Task<Void, Never>?
    @ObservationIgnored let nowPlaying = NowPlaying()

    init() {
        // `--data-dir <folder>` opens another library, as on the desktop (for tests).
        let arguments = CommandLine.arguments
        let folder = arguments.firstIndex(of: "--data-dir").flatMap { arguments.indices.contains($0 + 1) ? arguments[$0 + 1] : nil }
            ?? defaultDataDirectory()
        do {
            core = try Needle(dataDir: folder)
        } catch {
            let alert = NSAlert()
            alert.messageText = "Needle could not open its library"
            alert.informativeText = error.message
            alert.runModal()
            exit(1)
        }
        version = core.version()
        settings = JSON.decode(NeedleSettings.self, try? core.settingsJson()) ?? NeedleSettings()
        // The side panel opens when asked for (Lyrics, Queue, Details), as in Apple Music, not
        // at every start.
        if settings.showInspector { set(["show_inspector": false]) }
        refreshLook()
        core.startDesktop()
        reloadPlaylists()
        reloadPlugins()
        reloadThemes()
        nowPlaying.start(self)
        let core = core
        Task.detached { _ = core.sendCrashReports() }
        timer = Timer.scheduledTimer(withTimeInterval: 0.1, repeats: true) { [weak self] _ in
            MainActor.assumeIsolated { self?.tick() }
        }
        maybeWelcome()
        checkForUpdate(force: false)
    }

    // MARK: - Following the core

    private func tick() {
        ticks += 1
        let now = core.macPlayback()
        // Slower while nothing plays: four times a second is plenty.
        if now.playing || ticks % 3 == 0 || now != playback { follow(now) }
        guard ticks % 5 == 0 else { return }
        let status = core.scanStatus()
        if status != scan { scan = status }
        if wasScanning && !status.running {
            if let error = status.error { fail(error) } else if status.imported > 0 {
                notify("\(count(Int(status.imported), "track")) added or updated")
            }
            core.rewatch()
            libraryChanged()
        }
        wasScanning = status.running
        let seen = core.libraryChanges()
        if seen != changes { changes = seen; libraryChanged() }
        for notice in core.takeNotices() {
            switch notice {
            case .message(let text): notify(text)
            case .question(let id, let plugin, let title, let prompt, let value, let file, let extensions):
                if file { askForFile(id: id, title: title, extensions: extensions) }
                else { question = Question(id: id, plugin: plugin, title: title, prompt: prompt, answer: value) }
            case .libraryChanged: libraryChanged()
            }
        }
        let job = core.stemsJob()
        if job != stemsJob { stemsJob = job }
        if let done = core.takeStemsDone() {
            switch done.error {
            case nil: notify("“\(done.title)” is split into stems. Turn on Play from stems to mix it.")
            case let error? where !error.isEmpty: fail(error)
            default: break
            }
        }
        let progress = core.importProgress()
        if progress != importProgress { importProgress = progress }
    }

    private func follow(_ now: MacPlayback) {
        let before = playback
        playback = now
        if now.error != lastError {
            lastError = now.error
            if let error = now.error { fail(error) }
        }
        if now.outputNotice != lastNotice {
            lastNotice = now.outputNotice
            if let notice = now.outputNotice { notify(notice) }
        }
        if now.current != before?.current || now.artwork != before?.artwork || now.rating != before?.rating {
            reloadCurrent(started: now.current != before?.current)
        }
        if now.queueVersion != queueVersion {
            queueVersion = now.queueVersion
            queue = core.queueEntries(offset: 0, limit: 400)
            queueSeconds = core.queueSeconds()
        }
        if now.artwork != tintFor {
            tintFor = now.artwork
            findTint(now.artwork)
        }
        followLyrics(now.position)
        nowPlaying.update(now)
    }

    private func followLyrics(_ position: Double) {
        let line = lyrics.flatMap { lyrics in lyrics.lines.lastIndex { $0.time <= position + 0.15 } }
        if line != lyricLine { lyricLine = line }
    }

    private func reloadCurrent(started: Bool) {
        guard let id = playback?.current else { current = nil; return }
        current = JSON.decode(Track.self, (try? core.trackJson(id: id)) ?? nil)
        if started { lookUpMedia(id) }
    }

    /// Lyrics for a newly playing song, and its cover when it has none (when online lookups
    /// are on).
    private func lookUpMedia(_ id: String) {
        lyrics = nil
        lyricsFor = nil
        lyricsLooking = true
        let core = core
        let online = settings.onlineMedia
        let needsCover = online && current?.artwork == nil && !coverFetched.contains(id)
        Task {
            let found = await Task.detached { try? core.lyrics(id: id) }.value
            guard playback?.current == id else { return }
            lyrics = found ?? nil
            lyricsFor = id
            lyricsLooking = false
            if needsCover {
                coverFetched.insert(id)
                let got = await Task.detached { (try? core.fetchCover(id: id)) ?? 0 }.value
                if got > 0, playback?.current == id { reloadCurrent(started: false); libraryChanged() }
            }
        }
    }

    /// Looks the lyrics up again (after turning on a lyrics plugin, or timing them).
    func reloadLyrics() {
        if let id = playback?.current { lookUpMedia(id) }
    }

    /// Something in the library changed: the pages, the playlists, and the songs shown load
    /// again, once for changes that come together (an import sends many).
    func libraryChanged() {
        core.refreshPlaying()
        libraryTask?.cancel()
        libraryTask = Task {
            try? await Task.sleep(for: .milliseconds(200))
            guard !Task.isCancelled else { return }
            libraryVersion += 1
            reloadPlaylists()
            if let id = playback?.current { current = JSON.decode(Track.self, (try? core.trackJson(id: id)) ?? nil) }
            if let id = focused?.id { focused = JSON.decode(Track.self, (try? core.trackJson(id: id)) ?? nil) }
        }
    }

    @ObservationIgnored private var libraryTask: Task<Void, Never>?

    /// Loads the playlists away from the window (a smart playlist's count runs its search).
    func reloadPlaylists() {
        Task {
            if let next = await load({ try $0.playlists() }) { playlists = next }
        }
    }

    func reloadPlugins() {
        plugins = core.plugins()
    }

    func reloadThemes() {
        themes = JSON.decode([ThemeFile].self, core.themesJson(), plain: true) ?? []
    }

    /// Music servers signed in, for the sidebar: plugin id and name.
    var servers: [(plugin: String, name: String)] {
        plugins.compactMap { p in
            guard p.enabled, let source = p.source, source.signedIn else { return nil }
            return (p.id, source.name)
        }
    }

    // MARK: - Messages

    func notify(_ text: String) { show(Toast(text: text, error: false)) }

    func fail(_ text: String) {
        // Libraries that wrap an error often repeat it: "X.: X."
        var parts: [String] = []
        for part in text.components(separatedBy: ": ") {
            let same = { (a: String, b: String) in a.trimmingCharacters(in: .whitespaces).trimmingCharacters(in: CharacterSet(charactersIn: ".")) == b.trimmingCharacters(in: .whitespaces).trimmingCharacters(in: CharacterSet(charactersIn: ".")) }
            if parts.last.map({ !same($0, part) }) ?? true { parts.append(part) }
        }
        show(Toast(text: parts.joined(separator: ": "), error: true))
    }

    private func show(_ next: Toast) {
        withAnimation(.snappy) { toast = next }
        toastTask?.cancel()
        toastTask = Task {
            try? await Task.sleep(for: .seconds(next.error ? 14 : 6))
            if !Task.isCancelled, toast?.id == next.id { withAnimation(.snappy) { toast = nil } }
        }
    }

    /// Runs `work` away from the window, then shows its error, if any.
    /// Sends sound settings to the player one at a time, in order; while one is on its way,
    /// only the newest waiting goes next (a slider being dragged sends many).
    func setSound(_ value: SoundSettings) {
        soundNext.put(value)
        let (core, next) = (core, soundNext)
        soundQueue.async {
            guard let value = next.take() else { return }
            do {
                try core.setSoundSettings(value: value)
            } catch {
                let text = error.message
                Task { @MainActor in Store.shared.fail(text) }
            }
        }
    }

    @ObservationIgnored private let soundQueue = DispatchQueue(label: "fyi.nnx.Needle.sound")
    @ObservationIgnored private let soundNext = Latest<SoundSettings>()

    func run(_ work: @escaping @Sendable (Needle) throws -> Void) {
        let core = core
        Task.detached {
            do { try work(core) } catch {
                await MainActor.run { self.fail(error.message) }
            }
        }
    }

    /// Runs `work` away from the window and gives back its result, or shows its error.
    func ask<T: Sendable>(_ work: @escaping @Sendable (Needle) throws -> T) async -> T? {
        let core = core
        let result = await Task.detached { Result { try work(core) } }.value
        switch result {
        case .success(let value): return value
        case .failure(let error): fail(error.message); return nil
        }
    }

    /// Runs `work` away from the window and shows the message it gives back.
    func job(_ work: @escaping @Sendable (Needle) throws -> String) {
        Task {
            if let text = await ask(work), !text.isEmpty { notify(text) }
            libraryChanged()
        }
    }

    /// Reads a value away from the window.
    nonisolated func load<T: Sendable>(_ work: @escaping @Sendable (Needle) throws -> T) async -> T? {
        let core = core
        return await Task.detached { try? work(core) }.value
    }

    // MARK: - Settings

    /// Changes the settings named in `patch` (the desktop's names; objects merge), saves them,
    /// and tells the player.
    func set(_ patch: [String: Any]) {
        do {
            try core.updateSettings(patch: JSON.text(patch))
        } catch {
            fail(error.message)
        }
        reloadSettings()
    }

    func setLayout(_ patch: [String: Any]) { set(["layout": patch]) }

    /// Brings `colors` and the others kept beside `settings` up to date.
    func refreshLook() {
        let look = lookNow()
        if look != colors { colors = look }
        // The mini player is always dark: its lit signs keep the theme's accent, not a page's.
        let plain = pageTint == nil ? look.accent : lookNow(paged: false).accent
        if plain != playerAccent { playerAccent = plain }
        let still = settings.reduceMotion || NSWorkspace.shared.accessibilityDisplayShouldReduceMotion
        if motion == still { motion = !still }
        if displayFont != settings.displayFont { displayFont = settings.displayFont }
        let compact = settings.layout.rowHeight < 50
        if compactRows != compact { compactRows = compact }
    }

    func reloadSettings() {
        guard let next = JSON.decode(NeedleSettings.self, try? core.settingsJson()), next != settings else { return }
        let need = Self.leastWidth(next)
        let (was, now) = (settings.layout, next.layout)
        if was.sidebarHidden != now.sidebarHidden || was.compactSidebar != now.compactSidebar {
            // A column of the split view: room before it opens (see `makeRoom`).
            Self.makeRoom(need)
        } else {
            // The side panel floats over the page: room after the click that opened it, not
            // inside it (a button in the title bar asks).
            DispatchQueue.main.async { Self.makeRoom(need) }
        }
        settings = next
    }

    /// Opens the big player once the click that asks has ended: it hides the title bar, and
    /// the button clicked is in it.
    func openBig() {
        Task { @MainActor in
            await Task.yield()
            big = true
        }
    }

    /// The narrowest the main window may be with the columns these settings open: the page
    /// needs about 560 points beside the sidebar (or its strip of icons) and the side panel.
    /// (Narrower, SwiftUI's split view kept moving the columns and never settled, and AppKit
    /// stopped the app.)
    static func leastWidth(_ settings: NeedleSettings) -> CGFloat {
        let layout = settings.layout
        let sidebar: CGFloat = layout.sidebarHidden ? (layout.compactSidebar ? Rail.width : 0) : 220
        // The player in the title bar needs its width beside Back and Forward.
        let page: CGFloat = layout.playerOnTop ? max(560, TopPlayer.width + 90) : 560
        return max(760, page + sidebar + (settings.showInspector ? 310 : 0))
    }

    /// Widens the main window, at once, when a column is about to open and there is no room
    /// for it. It runs before the column opens: widened while the column slid open, the split
    /// view never settled, and AppKit stopped the app.
    static func makeRoom(_ width: CGFloat) {
        guard let window = NSApp?.windows.first(where: { $0.identifier?.rawValue.hasPrefix("main") == true }),
              !window.styleMask.contains(.fullScreen), window.frame.width < width else { return }
        var frame = window.frame
        frame.size.width = width
        if let screen = window.screen?.visibleFrame {
            frame.size.width = min(width, screen.width)
            frame.origin.x = max(screen.minX, min(frame.origin.x, screen.maxX - frame.width))
        }
        window.setFrame(frame, display: true, animate: false)
    }

    // MARK: - Pages

    func navigate(_ to: Route) {
        if to == route {
            if !search.isEmpty { search = "" }
            return
        }
        back.append(route)
        if back.count > 50 { back.removeFirst() }
        forward.removeAll()
        open(to)
    }

    func goBack() {
        if immersive { setImmersive(false); return }
        if big { big = false; return }
        guard let to = back.popLast() else { return }
        forward.append(route)
        open(to)
    }

    func goForward() {
        guard let to = forward.popLast() else { return }
        back.append(route)
        open(to)
    }

    private func open(_ to: Route) {
        searchQuiet = true
        search = ""
        searchQuiet = false
        sort = nil
        selectedTracks = []
        route = to
        big = false
        // A page without a colour of its own goes back to the theme's. From one album or
        // artist to another, the colour stays until the next one's is found: no flash between.
        if pageTint != nil, !to.isPaged { withAnimation(motion ? .easeInOut(duration: 0.35) : nil) { pageTint = nil } }
    }

    /// An album or artist page's colour, once found; only while that page is still open.
    func setPageTint(_ color: NSColor?, for page: Route) {
        guard route == page, color != pageTint else { return }
        withAnimation(motion ? .easeInOut(duration: 0.45) : nil) { pageTint = color }
    }

    @ObservationIgnored private var searchQuiet = false

    private func searchChanged(from old: String) {
        guard !searchQuiet, old != search else { return }
        // Typing on a page that does not search goes to Songs.
        if !search.isEmpty, !route.searchesInside {
            back.append(route)
            forward.removeAll()
            // As `open` does, but the search stays.
            sort = nil
            selectedTracks = []
            route = .songs
            big = false
            if pageTint != nil { pageTint = nil }
        }
    }

    /// The songs selected in the song list.
    var selectedTracks: [Track] = []
    var selectedIDs: Set<String> { Set(selectedTracks.map(\.id)) }
    /// The song list's order, when a column was clicked: the field and the direction.
    var sort: (field: String, ascending: Bool)? {
        get { sortField.map { ($0, sortAscending) } }
        set { sortField = newValue?.field; sortAscending = newValue?.ascending ?? true }
    }
    private(set) var sortField: String?
    private(set) var sortAscending = true

    /// The playlist a page shows, when it shows one.
    var pagePlaylist: Playlist? {
        guard case .playlist(let id) = route else { return nil }
        return playlists.first { $0.id == id }
    }

    /// The playlist model of a page, with its rule.
    func playlistModel(_ id: String) -> PlaylistModel? {
        JSON.decode(PlaylistModel.self, (try? core.playlistJson(id: id)) ?? nil)
    }

    /// The full rule for what `route` shows, with the search and the chosen order.
    func expression(for route: Route, playlistRule: String? = nil) -> String {
        let text = search.trimmingCharacters(in: .whitespaces)
        if route == .albums || route == .artists { return text }
        var base = route.base
        if case .source(let plugin, _) = route { base = core.sourceRule(plugin: plugin) }
        if case .playlist = route { base = playlistRule ?? "" }
        var rule: String
        if base.isEmpty {
            rule = text
        } else if text.isEmpty {
            rule = base
        } else {
            let q = Rule.quote(text)
            let plain = base.components(separatedBy: " order by ").first ?? base
            rule = "(\(plain)) and (title contains \(q) or artist contains \(q) or album contains \(q))"
        }
        if let (field, ascending) = sort, !rule.contains("shuffle"), !rule.contains(" limit ") {
            if let range = rule.range(of: " order by ") { rule = String(rule[..<range.lowerBound]) }
            let direction = ascending ? "asc" : "desc"
            if rule.trimmingCharacters(in: .whitespaces).isEmpty {
                rule = "order by \(field) \(direction)"
            } else if core.checkQuery(query: rule).explanation?.hasPrefix("Title, artist") == true {
                let q = Rule.quote(rule)
                rule = "(title contains \(q) or artist contains \(q) or album contains \(q) or genre contains \(q)) order by \(field) \(direction)"
            } else {
                rule += " order by \(field) \(direction)"
            }
        }
        return rule
    }

    /// Why songs from the page playing are playing.
    var reason: String {
        route.reason(search: search.trimmingCharacters(in: .whitespaces), playlistName: pagePlaylist?.name)
    }

    // MARK: - Playing

    /// Plays everything the page shows, from its song `start`, or all of it shuffled.
    func playPage(rule: String, playlist: String?, start: Int = 0, shuffle: Bool = false) {
        let filter = playlist == nil ? "" : search.trimmingCharacters(in: .whitespaces)
        let reason = reason
        Task {
            guard let n = await ask({ try $0.playView(query: rule, playlist: playlist, filter: filter, start: UInt32(start), shuffle: shuffle, reason: reason) }) else { return }
            if n == 0 { notify("Nothing here can be played. The files may be unavailable.") }
            else if n >= 50_000 { notify("Playing the first 50,000 matching tracks.") }
        }
    }

    /// Plays everything the page shows, shuffled (the shuffle button beside the controls).
    func shuffleView() {
        guard route.isSongs else {
            core.shuffle()
            notify("Up next shuffled.")
            return
        }
        if case .playlist(let id) = route, let model = playlistModel(id) {
            playPage(rule: expression(for: route, playlistRule: model.query), playlist: model.smart ? nil : id, shuffle: true)
        } else {
            playPage(rule: expression(for: route), playlist: nil, shuffle: true)
        }
    }

    func play(_ tracks: [Track], from start: Int = 0, reason: String? = nil) {
        let ids = tracks.map(\.id)
        let reason = reason ?? self.reason
        run { _ = try $0.playSongs(ids: ids, start: UInt32(start), reason: reason) }
    }

    func playNext(_ ids: [String]) {
        run { _ = try $0.playNextSongs(ids: ids, reason: "Chosen to play next by you") }
        notify(ids.count == 1 ? "Playing next." : "\(ids.count) tracks play next.")
    }

    func enqueue(_ ids: [String]) {
        run { _ = try $0.enqueueSongs(ids: ids, reason: "Added to the queue by you") }
        notify(ids.count == 1 ? "Added to the queue." : "Added \(ids.count) tracks to the queue.")
    }

    func toggle() { core.toggle() }
    func next() { core.next() }
    func previous() { core.previous() }

    func seek(_ seconds: Double) {
        core.seek(seconds: max(0, seconds))
    }

    func seekBy(_ seconds: Double) {
        guard let p = playback, p.current != nil else { return }
        seek(min(max(0, p.position + seconds), max(0, p.duration - 1)))
    }

    func setVolume(_ value: Float) {
        core.setVolume(value: value)
        if let p = playback { var next = p; next.volume = value; playback = next }
    }

    func nudgeVolume(_ delta: Float) {
        setVolume(min(1, max(0, (playback?.volume ?? 0.65) + delta)))
    }

    func toggleMute() {
        if let restore = mutedVolume {
            mutedVolume = nil
            setVolume(restore)
        } else {
            mutedVolume = max(0.05, playback?.volume ?? 0.65)
            setVolume(0)
        }
    }

    func cycleRepeat() {
        let next: RepeatMode = switch playback?.repeat ?? "off" {
        case "off": .all
        case "all": .one
        default: .off
        }
        core.setRepeat(mode: next)
    }

    /// The A–B loop: set A, then B, and that part repeats; again to clear it.
    func loopButton() {
        guard let p = playback, p.current != nil else { return }
        if p.loopStart != nil {
            loopStart = nil
            core.setLoop(start: nil, end: nil)
        } else if let a = loopStart {
            loopStart = nil
            let b = p.position
            if b > a + 0.5 {
                core.setLoop(start: a, end: b)
                notify("Looping \(clock(a)) – \(clock(b))")
            } else {
                fail("Point B must come after point A.")
            }
        } else {
            loopStart = p.position
            notify("Point A set at \(clock(p.position)). Click the loop button again at point B.")
        }
    }

    /// Stars, 0 to 5; 4 and 5 make a favorite.
    func rate(_ ids: [String], _ stars: Int) {
        for id in ids { try? core.rate(id: id, stars: UInt8(stars)) }
        if let f = focused, ids.contains(f.id) { focused?.rating = stars }
        if let c = current, ids.contains(c.id) { current?.rating = stars }
        libraryVersion += 1
    }

    func toggleFavorite(_ tracks: [Track]) {
        let all = tracks.allSatisfy(\.favorite)
        rate(tracks.map(\.id), all ? 0 : 5)
    }

    func startRadio(_ track: Track) {
        notify("Tuning in…")
        let id = track.id
        Task {
            if let n = await ask({ try $0.radioFrom(id: id) }) { notify("Radio from “\(track.title)”: \(n) songs.") }
        }
    }

    func artistRadio(_ name: String) {
        notify("Tuning in…")
        Task {
            if let n = await ask({ try $0.artistRadio(artist: name) }) { notify("\(name) radio: \(n) songs.") }
        }
    }

    func blend(_ first: String, _ second: String) {
        notify("Tuning in…")
        Task {
            if let n = await ask({ try $0.startBlend(first: first, second: second) }) { notify("\(first) × \(second): \(n) songs.") }
        }
    }

    func addToPlaylist(_ playlist: Playlist, _ ids: [String]) {
        let id = playlist.id
        Task {
            guard let added = await ask({ try $0.addSongsToPlaylist(id: id, ids: ids) }) else { return }
            notify(added == 0 ? "Already in “\(playlist.name)”." : "Added \(count(Int(added), "track")) to “\(playlist.name)”.")
            libraryChanged()
        }
    }

    // MARK: - The big player and full screen

    func setImmersive(_ on: Bool) {
        immersive = on
        guard let window = NSApp.windows.first(where: { $0.identifier?.rawValue.hasPrefix("main") == true }) ?? NSApp.mainWindow else { return }
        if on != window.styleMask.contains(.fullScreen) { window.toggleFullScreen(nil) }
    }

    // MARK: - Folders and files

    /// Adds a music folder the listener picks, and reads it.
    func addFolder() {
        let panel = NSOpenPanel()
        panel.canChooseDirectories = true
        panel.canChooseFiles = false
        panel.allowsMultipleSelection = true
        panel.prompt = "Add"
        panel.message = "Add a music folder to Needle"
        guard panel.runModal() == .OK else { return }
        let paths = panel.urls.map(\.path)
        run { core in
            for path in paths { try core.addFolder(path: path) }
        }
        notify("Adding the folder…")
    }

    func rescan() {
        core.rescan()
    }

    func demo() {
        let folder = (core.dataDirectory() as NSString).appendingPathComponent("demo")
        run { try $0.addDemoLibrary(folder: folder) }
        notify("Preparing three original demo recordings…")
    }

    /// Files opened with Needle (Finder, a drop on the window): themes are added, other files
    /// a plugin opens go to it, and music plays.
    func open(_ urls: [URL]) {
        var music: [String] = []
        for url in urls {
            let ext = url.pathExtension.lowercased()
            if ext == "toml" {
                importTheme(url)
            } else if ["m3u", "m3u8"].contains(ext) {
                let path = url.path
                job { try $0.importPlaylistFile(path: path) }
            } else if let given = try? core.dropToPlugin(path: url.path) {
                notify(given)
            } else {
                music.append(url.path)
            }
        }
        guard !music.isEmpty else { return }
        Task { [music] in
            guard let n = await ask({ try $0.openFiles(paths: music) }) else { return }
            if n == 0 { notify("Needle could not open that. It may not be a music file Needle can play.") }
            else if n > 1 { notify("Playing \(n) songs.") }
            libraryChanged()
        }
    }

    func importTheme(_ url: URL) {
        Task {
            if let name = await ask({ try $0.importTheme(path: url.path) }) {
                reloadThemes()
                notify("Added the theme \(name).")
            }
        }
    }

    /// Shows a file in Finder.
    func reveal(_ path: String) {
        NSWorkspace.shared.activateFileViewerSelecting([URL(fileURLWithPath: path)])
    }

    func copy(_ text: String) {
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(text, forType: .string)
    }

    // MARK: - Plugins asking

    func answer(_ question: Question, with text: String?) {
        core.answer(id: question.id, text: text, file: nil)
        self.question = nil
    }

    private func askForFile(id: UInt64, title: String, extensions: [String]) {
        let panel = NSOpenPanel()
        panel.message = title
        panel.canChooseFiles = true
        panel.canChooseDirectories = extensions.isEmpty
        if !extensions.isEmpty {
            panel.allowedContentTypes = extensions.compactMap { UTType(filenameExtension: $0.trimmingCharacters(in: CharacterSet(charactersIn: "."))) }
        }
        let chosen = panel.runModal() == .OK ? panel.url?.path : nil
        core.answer(id: id, text: nil, file: chosen)
    }

    // MARK: - The cover's colour

    private func findTint(_ path: String?) {
        guard let path else { withAnimation(.easeInOut(duration: 0.8)) { coverTint = nil }; return }
        Task {
            let color = await Task.detached { Covers.vivid(path) }.value
            if tintFor == path { withAnimation(.easeInOut(duration: 0.8)) { coverTint = color } }
        }
    }

    // MARK: - Welcome, What's new, and updates

    private func maybeWelcome() {
        if !settings.welcomed {
            welcome = true
            set(["whats_new_seen": version])
        } else if settings.whatsNewSeen != version {
            whatsNew = core.desktopWhatsNew().first { $0.version == version }
            set(["whats_new_seen": version])
        }
    }

    /// Once a day at most (or now, when asked), asks needle.nnx.fyi whether a newer Needle is out.
    func checkForUpdate(force: Bool) {
        let day = Int(Date().timeIntervalSince1970) - 86_400
        guard force || (settings.checkUpdates && settings.lastUpdateCheck < day) else { return }
        Task {
            let found = await ask { try $0.checkMacUpdate() }
            reloadSettings()
            update = found ?? nil
            if let found = found ?? nil {
                notify(force ? "Needle \(found.version) is out. You have \(version)." : "Needle \(found.version) is out. Update it in Settings › Online services.")
            } else if force {
                notify("Needle \(version) is the latest version.")
            }
        }
    }

    func installUpdate() {
        guard let update else { return }
        notify("Downloading Needle \(update.version) and checking it…")
        Task {
            if await ask({ try $0.installMacUpdate() }) != nil {
                core.shutdownDesktop()
                NSApp.terminate(nil)
            }
        }
    }

    func quit() {
        core.shutdownDesktop()
    }
}

extension MacPlayback {
    var hasSong: Bool { current != nil }
    var fraction: Double { duration > 0 ? min(1, max(0, position / duration)) : 0 }
}

/// A value handed from one thread to another, where only the newest counts.
final class Latest<Value>: @unchecked Sendable {
    private let lock = NSLock()
    private var value: Value?

    func put(_ next: Value) {
        lock.lock()
        value = next
        lock.unlock()
    }

    /// The newest value, once.
    func take() -> Value? {
        lock.lock()
        defer { value = nil; lock.unlock() }
        return value
    }
}
