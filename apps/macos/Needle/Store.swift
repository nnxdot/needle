import AppKit
import Observation
import SwiftUI
import UniformTypeIdentifiers

/// Holds Needle's core for the whole app: every window, the menu bar icon, and the media keys
/// share it. What plays is read from the core several times a second.
@MainActor @Observable
final class Store {
    let core: Needle
    let version: String

    // MARK: What the core says

    private(set) var settings: NeedleSettings
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
    private(set) var scan = ScanStatus(running: false, scanned: 0, imported: 0, current: "", error: nil)
    /// Changes whenever the library may have changed, so pages load again.
    private(set) var libraryVersion = 0
    private(set) var playlists: [Playlist] = []
    private(set) var plugins: [PluginItem] = []
    private(set) var themes: [ThemeFile] = []
    /// The playing cover's colour, for tinting the app.
    private(set) var coverTint: NSColor?
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
    var big = false
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
            alert.informativeText = error.localizedDescription
            alert.runModal()
            exit(1)
        }
        version = core.version()
        settings = JSON.decode(NeedleSettings.self, try? core.settingsJson()) ?? NeedleSettings()
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
        let job = core.stemsInfo(id: "").job
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
        nowPlaying.update(now)
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

    func libraryChanged() {
        libraryVersion += 1
        reloadPlaylists()
        if let id = playback?.current { current = JSON.decode(Track.self, (try? core.trackJson(id: id)) ?? nil) }
        if let id = focused?.id { focused = JSON.decode(Track.self, (try? core.trackJson(id: id)) ?? nil) }
    }

    func reloadPlaylists() {
        playlists = (try? core.playlists()) ?? []
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
    func run(_ work: @escaping @Sendable (Needle) throws -> Void) {
        let core = core
        Task.detached {
            do { try work(core) } catch {
                await MainActor.run { self.fail(error.localizedDescription) }
            }
        }
    }

    /// Runs `work` away from the window and gives back its result, or shows its error.
    func ask<T: Sendable>(_ work: @escaping @Sendable (Needle) throws -> T) async -> T? {
        let core = core
        let result = await Task.detached { Result { try work(core) } }.value
        switch result {
        case .success(let value): return value
        case .failure(let error): fail(error.localizedDescription); return nil
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
            fail(error.localizedDescription)
        }
        reloadSettings()
    }

    func setLayout(_ patch: [String: Any]) { set(["layout": patch]) }

    func reloadSettings() {
        if let next = JSON.decode(NeedleSettings.self, try? core.settingsJson()), next != settings { settings = next }
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
    }

    @ObservationIgnored private var searchQuiet = false

    private func searchChanged(from old: String) {
        guard !searchQuiet, old != search else { return }
        // Typing on a page that does not search goes to Songs.
        if !search.isEmpty, !route.searchesInside {
            back.append(route)
            forward.removeAll()
            route = .songs
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
