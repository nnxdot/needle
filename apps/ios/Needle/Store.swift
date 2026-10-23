import ActivityKit
import AVFoundation
import CryptoKit
import MediaPlayer
import MetricKit
import Network
import Observation
import SwiftUI
import UIKit
import WidgetKit

/// Holds Needle's core for the whole app: the screens, the lock screen, Control Center, the
/// widgets, and Siri share it. What plays is read from the core a few times a second.
@MainActor @Observable
final class Store {
    let core: Needle
    /// Needle's folder in the Files app ("On My iPhone › Needle"): the music Needle reads.
    let musicFolder: URL
    /// Needle's own files (the library, covers, logs), out of the Files app's sight.
    let dataFolder: URL

    /// What plays: on this phone, or on the computer while the phone shows that.
    private(set) var playback: Playback?
    /// What Needle on the computer plays, while connected to it.
    private(set) var pc: PcState?
    /// The player shows (and works) the computer's song: the phone plays nothing, and the
    /// computer does (or the phone has nothing of its own to show).
    private(set) var onComputer = false
    private(set) var scan = ScanStatus(running: false, scanned: 0, imported: 0, current: "", error: nil)
    /// Changes whenever the library may have changed (a scan finished), so lists load again.
    private(set) var libraryVersion = 0
    /// The app's own look and behaviour (Settings › Appearance).
    private(set) var app: AppSettings
    /// Themes made in the app or added from files.
    private(set) var themes: [ThemeInfo] = []
    /// The playing cover's main colour.
    private(set) var coverColor: Color?
    /// The song playing (or shown, the computer's) and whether it plays: song rows follow
    /// these, which change only with the song, not four times a second as `playback` does.
    private(set) var playingID: String?
    private(set) var isPlaying = false
    /// The songs up next, read again whenever the queue changes.
    private(set) var upNext: [Song] = []
    /// When the sleep timer pauses the music; `.distantFuture` for "at the end of the song".
    private(set) var sleepEnds: Date?
    /// A short message to show at the top of the screen.
    var message: String? {
        // VoiceOver hears each message once (two toasts show it while the player is open), and
        // the same one given again is heard (and shown its full time) again.
        didSet {
            guard let message else { return }
            messageCount += 1
            UIAccessibility.post(notification: .announcement, argument: message)
        }
    }
    /// Goes up when stars change (a rating, a favourite): the player's star and Favourites follow.
    private(set) var ratingVersion = 0

    /// Goes up when a song's own lyrics are saved or taken off: the lyrics shown follow.
    private(set) var lyricsVersion = 0
    /// On mobile data (or another costly network): moving covers not yet here are not
    /// downloaded, unless Settings › Appearance › On mobile data is on.
    private(set) var onMobileData = false
    private let network = NWPathMonitor()

    func lyricsChanged() { lyricsVersion += 1 }

    /// Takes a song out of Up Next, at once on the screen too (the queue is read again a moment
    /// later, and a second swipe meanwhile used the old places).
    func removeUpNext(at rows: IndexSet) {
        rows.sorted(by: >).forEach { core.removeUpNext(index: UInt32($0)) }
        upNext.remove(atOffsets: rows)
    }

    /// Moves a song in Up Next, at once on the screen too.
    func moveUpNext(from: IndexSet, to: Int) {
        guard let first = from.first else { return }
        // The core moves to a place in the list as it is after taking the song out.
        core.moveUpNext(from: UInt32(first), to: UInt32(to > first ? to - 1 : to))
        upNext.move(fromOffsets: from, toOffset: to)
    }

    /// Stars or a favourite were changed.
    func ratingsChanged() { ratingVersion += 1 }

    /// Rates a song, then lets what shows its stars know.
    func rate(_ id: String, stars: Int) {
        Task {
            if await ask({ try $0.rate(id: id, stars: UInt8(stars)) }) != nil { ratingsChanged() }
        }
    }

    /// Goes up with each message.
    private(set) var messageCount = 0
    /// A plugin's question, answered with text.
    var question: Question?
    var showPlayer = false
    /// A page to open, from a widget or a link.
    var opening: Route?
    /// A sheet over everything: adding to a playlist, tags, identifying, lyrics timing.
    var sheet: Sheet?
    /// Plugin commands that act on chosen songs, for the songs' menus.
    private(set) var songCommands: [SongCommand] = []

    enum Sheet: Identifiable {
        case addToPlaylist([Song])
        case tags([String])
        case identify(String)
        case timing(Song)
        case speakers

        var id: String {
            switch self {
            case .addToPlaylist(let songs): "add" + songs.map(\.id).joined()
            case .tags(let ids): "tags" + ids.joined()
            case .identify(let id): "identify" + id
            case .timing(let song): "timing" + song.id
            case .speakers: "speakers"
            }
        }
    }

    struct SongCommand: Identifiable {
        let plugin: String
        let pluginName: String
        let command: PluginCommand
        var id: String { plugin + "/" + command.id }
    }

    struct Question: Identifiable {
        let id: UInt64
        let title: String
        let prompt: String
        var answer: String
    }

    @ObservationIgnored private var timer: Timer?
    @ObservationIgnored private var scansSeen: UInt64 = 0
    @ObservationIgnored private var shownError: String?
    @ObservationIgnored private var askComputerNow = false
    /// Music played at the last look (the audio was taken for it).
    @ObservationIgnored private var audioTaken = false

    /// Asks the computer for its state at once (a link was just made or dropped).
    func askComputer() { askComputerNow = true }
    @ObservationIgnored private var queueVersion: UInt64 = .max
    @ObservationIgnored private var shownSong: String?
    @ObservationIgnored private var shownArtwork: MPMediaItemArtwork?
    @ObservationIgnored private var ticks = 0
    @ObservationIgnored private var resumeAfterInterruption = false
    @ObservationIgnored private var local: Playback?
    @ObservationIgnored private var colorFor: String?
    @ObservationIgnored private var sleepSong: String?
    @ObservationIgnored private var sleepActivity: Activity<NeedleActivity>?
    @ObservationIgnored private var syncActivity: Activity<NeedleActivity>?
    @ObservationIgnored private var widgetSong: (String?, Bool) = (nil, false)

    init() {
        let files = FileManager.default
        let support = files.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
            .appendingPathComponent("Needle", isDirectory: true)
        try? files.createDirectory(at: support, withIntermediateDirectories: true)
        dataFolder = support
        musicFolder = files.urls(for: .documentDirectory, in: .userDomainMask)[0]
        // The app's folder can move when it is updated: the library's paths move with it.
        let home = NSHomeDirectory()
        let defaults = UserDefaults.standard
        if let earlier = defaults.string(forKey: "home"), earlier != home {
            // Kept as moved only when the library moved with it; else tried again next time.
            if (try? relocate(dataDir: support.path, old: earlier, new: home)) != nil { defaults.set(home, forKey: "home") }
        } else {
            defaults.set(home, forKey: "home")
        }
        do {
            core = try Needle(dataDir: support.path)
        } catch {
            fatalError("Needle could not open its library: \(error)")
        }
        app = core.appSettings()
        themes = core.themes()
        startAudio()
        startRemoteCommands()
        Actions.handler = { [weak self] action in self?.perform(action) }
        Actions.takeWaiting().forEach(perform)
        let folder = Self.realPath(musicFolder.path)
        let core = core
        let welcomed = app.welcomed
        Task.detached {
            // Already known: it is read again when the app comes to the front.
            if (try? core.folders())?.contains(folder) != true {
                try? core.addFolder(path: folder)
            }
            // Only once the welcome screen has let the person choose.
            if welcomed { _ = core.sendCrashReports() }
            // The app's own crashes (not only the core's): the system tells of them after.
            await MainActor.run { CrashWatcher.shared.start(core) }
            _ = core.reconnectPc()
            // Discord's status, when this build has Discord's library (it waits, unlinked).
            core.startDiscord()
            #if DEBUG
            // For testing in the simulator from a shell: plays the whole library at start.
            if CommandLine.arguments.contains("-play-test") {
                try? await Task.sleep(for: .seconds(3))
                if let songs = try? core.songs(), !songs.isEmpty {
                    // -play-title <title>: from that song.
                    let args = CommandLine.arguments
                    let title = args.firstIndex(of: "-play-title").flatMap { args.indices.contains($0 + 1) ? args[$0 + 1] : nil }
                    let start = title.flatMap { t in songs.firstIndex { $0.title == t } } ?? 0
                    try? core.play(ids: songs.map(\.id), start: UInt32(start))
                }
            }
            #endif
        }
        network.pathUpdateHandler = { [weak self] path in
            let costly = path.isExpensive || path.usesInterfaceType(.cellular)
            Task { @MainActor in if self?.onMobileData != costly { self?.onMobileData = costly } }
        }
        network.start(queue: DispatchQueue(label: "needle.network"))
        timer = Timer.scheduledTimer(withTimeInterval: 0.25, repeats: true) { [weak self] _ in
            MainActor.assumeIsolated { self?.tick() }
        }
        // Also while a finger scrolls a list (the timer's own mode waits for it to stop).
        if let timer { RunLoop.main.add(timer, forMode: .common) }
        followComputer()
        refreshPlugins()
        #if DEBUG
        // For testing from a shell: opens Discord's sign-in page.
        if CommandLine.arguments.contains("-discord-link") {
            Task {
                try? await Task.sleep(for: .seconds(3))
                core.discordLink()
            }
        }
        // For testing from a shell, with -play-test: opens the player (on its lyrics or queue).
        if ["-show-player", "-show-lyrics", "-show-queue"].contains(where: CommandLine.arguments.contains) {
            Task { [weak self] in
                try? await Task.sleep(for: .seconds(5))
                self?.showPlayer = true
            }
        }
        #endif
        Task { await updateWidgets(library: true) }
    }

    // MARK: - Following the core

    private func tick() {
        let now = core.playback()
        local = now
        let shown = showComputer(now)
        if shown != onComputer { onComputer = shown }
        let next = shown ? pc.map(Self.playback(of:)) ?? now : now
        if next != playback { playback = next }
        if next.current?.id != playingID { playingID = next.current?.id }
        if now.playing, !audioTaken { activateAudio() }
        audioTaken = now.playing
        if next.playing != isPlaying { isPlaying = next.playing }
        if next.queueVersion != queueVersion {
            queueVersion = next.queueVersion
            if shown {
                upNext = (pc?.upNext ?? []).map(Self.song(of:))
            } else {
                let (core, version) = (core, next.queueVersion)
                Task {
                    let list = await Task.detached { core.upNext() }.value
                    if self.queueVersion == version { self.upNext = list }
                }
            }
        }
        // A song that could not play says so (without any address, which may hold a key).
        if now.error != shownError {
            shownError = now.error
            if let error = now.error { message = Self.withoutAddresses(error) }
        }
        updateNowPlaying(now)
        followSleep(now)
        if next.current?.artwork != colorFor { colorFor = next.current?.artwork; findCoverColor(next.current?.artwork) }
        if (now.current?.id, now.playing) != widgetSong {
            widgetSong = (now.current?.id, now.playing)
            Task { await updateWidgets(library: false) }
        }
        ticks += 1
        // The rest changes slowly: twice a second is enough.
        guard ticks % 2 == 0 else { return }
        let status = core.scanStatus()
        if status != scan { scan = status }
        if status.changes != scansSeen {
            scansSeen = status.changes
            libraryChanged()
        }
        for notice in core.takeNotices() {
            switch notice {
            case .message(let text): message = text
            case .question(let id, _, let title, let prompt, let value, let file, _):
                // Plugins that ask for a file are desktop plugins: a phone has nothing to give.
                if file { core.answer(id: id, text: nil, file: nil) }
                else { question = Question(id: id, title: title, prompt: prompt, answer: value) }
            case .libraryChanged: libraryChanged()
            }
        }
        if ticks % 8 == 0 { followSync() }
    }

    func libraryChanged() {
        libraryVersion += 1
        refreshPlugins()
        Task { await updateWidgets(library: true) }
    }

    func refreshPlugins() {
        Task {
            let all = await load { $0.plugins() } ?? []
            songCommands = all.filter(\.enabled).flatMap { p in
                p.commands.filter(\.forSongs).map { SongCommand(plugin: p.id, pluginName: p.name, command: $0) }
            }
        }
    }

    func runCommand(_ command: SongCommand, on songs: [Song]) {
        core.runPluginCommand(plugin: command.plugin, command: command.command.id, songs: songs.map(\.id))
    }

    /// Downloads songs from a music server to play without a connection, or lets them go.
    func keepOffline(_ songs: [Song], keep: Bool) {
        let ids = songs.map(\.id)
        Task {
            if let changed = await ask({ try $0.keepOffline(ids: ids, keep: keep) }) {
                message = keep ? "Keeping \(count(Int(changed), "song")) offline" : "Let go of \(count(Int(changed), "song"))"
            }
        }
    }

    /// Saves songs from a music server as files in Needle's folder, as the phone's own.
    func saveToPhone(_ songs: [Song]) {
        let (ids, folder) = (songs.map(\.id), musicFolder.path)
        message = "Saving to this iPhone…"
        Task {
            if let saved = await ask({ try $0.saveToMusic(ids: ids, folder: folder) }) {
                message = "Saved \(count(Int(saved), "song")) to this iPhone"
                rescan()
            }
        }
    }

    func answer(_ question: Question, with text: String?) {
        core.answer(id: question.id, text: text, file: nil)
        self.question = nil
    }

    /// Runs `work` away from the screen, then shows its error, if any.
    func run(_ work: @escaping @Sendable (Needle) throws -> Void) {
        let core = core
        Task.detached {
            do { try work(core) } catch {
                await MainActor.run { self.message = Self.words(error) }
            }
        }
    }

    /// An error's own words (Needle's errors carry a message; the generated description is the
    /// error's type and fields).
    nonisolated static func words(_ error: Error) -> String {
        if let error = error as? NeedleError, case .Failed(let message) = error { return withoutAddresses(message) }
        return error.localizedDescription
    }

    /// Runs `work` away from the screen and gives back its result, or shows its error.
    func ask<T: Sendable>(_ work: @escaping @Sendable (Needle) throws -> T) async -> T? {
        let core = core
        let result = await Task.detached { Result { try work(core) } }.value
        switch result {
        case .success(let value): return value
        case .failure(let error): message = Self.words(error); return nil
        }
    }

    /// Reads a value away from the screen.
    nonisolated func load<T: Sendable>(_ work: @escaping @Sendable (Needle) throws -> T) async -> T? {
        let core = core
        return await Task.detached { try? work(core) }.value
    }

    // MARK: - Settings

    func updateApp(_ change: (inout AppSettings) -> Void) {
        var next = app
        change(&next)
        app = next
        let saved = next
        inOrder { try $0.setAppSettings(value: saved) }
    }

    @ObservationIgnored private var saving: Task<Void, Never>?

    /// Runs `work` away from the screen after the work given before it: settings saved in the
    /// order they were changed (a slider sends many), so the last one is the one kept.
    func inOrder(_ work: @escaping @Sendable (Needle) throws -> Void) {
        let (core, before) = (core, saving)
        saving = Task.detached {
            await before?.value
            do { try work(core) } catch {
                await MainActor.run { self.message = Self.words(error) }
            }
        }
    }

    func reloadThemes() {
        themes = core.themes()
    }

    // MARK: - Playback

    func play(_ songs: [Song], from start: Int = 0) {
        guard !songs.isEmpty else { message = "These songs are not on this iPhone"; return }
        activateAudio()
        let ids = songs.map(\.id)
        run { try $0.play(ids: ids, start: UInt32(start)) }
        played()
    }

    func shufflePlay(_ songs: [Song]) {
        guard !songs.isEmpty else { return }
        let ids = songs.map(\.id).shuffled()
        run { try $0.play(ids: ids, start: 0) }
        played()
    }

    func playAlbum(_ album: Album, shuffled: Bool = false) {
        playAlbum(key: album.key, shuffled: shuffled)
    }

    func playAlbum(key: String, shuffled: Bool = false) {
        playIDs({ try $0.albumSongs(key: key).map(\.id) }, shuffled: shuffled)
    }

    /// Plays the songs `find` gives, from `start`; with none, says so, and the player stays shut.
    func playIDs(_ find: @escaping @Sendable (Needle) throws -> [String], shuffled: Bool = false, from start: Int = 0) {
        Task {
            guard var ids = await ask(find) else { return }
            guard !ids.isEmpty else { message = "These songs are not on this iPhone"; return }
            activateAudio()
            if shuffled { ids.shuffle() }
            let at = UInt32(min(start, ids.count - 1))
            if await ask({ try $0.play(ids: ids, start: at) }) != nil { played() }
        }
    }

    /// The phone's audio, taken when music is asked for (not at launch, which stopped other
    /// apps' sound when Needle only opened).
    func activateAudio() {
        try? AVAudioSession.sharedInstance().setActive(true)
    }

    /// Opens the player (if that is on) once a song plays: a radio station takes a moment to
    /// make, and an empty player showed until then.
    func playedWhenReady() {
        guard app.openPlayerOnPlay else { return }
        let before = local?.queueVersion
        Task {
            for _ in 0..<40 {
                try? await Task.sleep(for: .milliseconds(250))
                if local?.current != nil, local?.queueVersion != before { showPlayer = true; return }
            }
        }
    }

    /// All of an artist's songs, the most played first.
    func playArtist(_ name: String, shuffled: Bool = false) {
        playIDs({ try $0.artistAllSongs(name: name).map(\.id) }, shuffled: shuffled)
    }

    /// Every song a search finds (all, with an empty one), from the song at `start`.
    func playSearch(_ query: String, shuffled: Bool = false, from start: Int = 0) {
        playIDs({ try $0.searchSongIds(query: query) }, shuffled: shuffled, from: start)
    }

    /// A folder's real path (links followed: the phone's /var is /private/var), as the library
    /// keeps its folders.
    nonisolated static func realPath(_ path: String) -> String {
        guard let real = realpath(path, nil) else { return path }
        defer { free(real) }
        return String(cString: real)
    }

    /// Text with any web address taken out.
    nonisolated static func withoutAddresses(_ text: String) -> String {
        text.replacingOccurrences(of: #"https?://\S+"#, with: "the server", options: .regularExpression)
    }

    /// Opens the player when a song is chosen, if that is on.
    func played() {
        if app.openPlayerOnPlay { showPlayer = true }
    }

    func playNext(_ songs: [Song]) {
        let ids = songs.map(\.id)
        run { try $0.playNext(ids: ids) }
        message = songs.count == 1 ? "Playing next" : "\(songs.count) songs play next"
    }

    func enqueue(_ songs: [Song]) {
        let ids = songs.map(\.id)
        run { try $0.enqueue(ids: ids) }
        message = "Added to the queue"
    }

    func toggle() {
        if onComputer {
            computer { try $0.pcDo(action: "toggle") }
        } else {
            // Only when there is a song to play: else another app's sound stopped for nothing.
            if local?.current != nil, local?.playing != true { activateAudio() }
            core.toggle()
        }
    }
    func next() { onComputer ? computer { try $0.pcDo(action: "next") } : core.next() }
    func previous() { onComputer ? computer { try $0.pcDo(action: "previous") } : core.previous() }

    func seek(_ seconds: Double) {
        onComputer ? computer { try $0.pcSeek(seconds: seconds) } : core.seek(seconds: seconds)
    }

    /// Goes back or on by `seconds` in the song.
    func jumpBy(_ seconds: Double) {
        guard let p = playback else { return }
        seek(min(max(0, p.position + seconds), max(0, (p.current?.duration ?? 0) - 1)))
    }

    func jump(_ index: Int) {
        if onComputer {
            guard index < upNext.count else { return }
            let id = String(upNext[index].id.dropFirst(Self.pcPrefix.count))
            computer { try $0.pcJump(index: UInt32(index), id: id) }
        } else {
            core.jump(index: UInt32(index))
        }
    }

    func shuffle() {
        onComputer ? computer { try $0.pcDo(action: "shuffle") } : core.shuffle()
        message = "Up next shuffled"
    }

    func cycleRepeat() {
        let next: RepeatMode = switch playback?.repeat ?? .off {
        case .off: .all
        case .all: .one
        case .one: .off
        }
        if onComputer {
            let mode = switch next { case .off: "off"; case .all: "all"; case .one: "one" }
            computer { try $0.pcRepeat(mode: mode) }
        } else {
            core.setRepeat(mode: next)
        }
    }

    /// The computer's volume, 0 to 1.
    func setComputerVolume(_ value: Float) {
        computer { try $0.pcVolume(value: value) }
    }

    private func perform(_ action: Action) {
        switch action {
        case .toggle: toggle()
        case .next: next()
        case .previous: previous()
        case .playAlbum(let key): playAlbum(key: key)
        case .playPlaylist(let id):
            playIDs { try $0.playlistSongs(id: id).map(\.id) }
        case .favourites:
            run { core in
                let ids = try core.favorites().map(\.id).shuffled()
                guard !ids.isEmpty else { throw NeedleError.Failed(message: "No favourites yet") }
                try core.play(ids: ids, start: 0)
            }
        case .radio:
            run { core in
                // From the song played most, or any song when nothing has played yet.
                let top = try core.stats(days: 30).topSongs.compactMap(\.songId)
                let any = try core.songs().randomElement()?.id
                // The first that can start one (a song whose file is gone cannot).
                for id in top + [any].compactMap({ $0 }) where (try? core.startRadio(id: id)) != nil { return }
                throw NeedleError.Failed(message: "Add music first")
            }
        }
    }

    /// Opens a link from a widget, Siri, or another app: `needle://album?id=…` and so on, or a
    /// music file.
    func open(_ url: URL) {
        if url.scheme?.hasPrefix("discord-") == true { return }
        guard url.scheme == "needle" else { return openFile(url) }
        let id = URLComponents(url: url, resolvingAgainstBaseURL: false)?.queryItems?.first { $0.name == "id" }?.value ?? ""
        switch url.host {
        case "player": if playback?.current != nil { showPlayer = true }
        case "stats": opening = .stats
        case "album":
            Task {
                if let album = await load({ try $0.albums().first { $0.key == id } }) ?? nil { opening = .album(album) }
            }
        case "playlist":
            Task {
                if let playlist = await load({ try $0.playlists().first { $0.id == id } }) ?? nil { opening = .playlist(playlist) }
            }
        default: break
        }
    }

    // MARK: - Needle on the computer

    nonisolated static let pcPrefix = "pc:"

    static func song(of pc: PcSong) -> Song {
        Song(id: pcPrefix + pc.id, title: pc.title, artist: pc.artist, album: pc.album, duration: pc.duration,
             trackNumber: 0, format: pc.format, artwork: pc.cover, rating: 0)
    }

    static func playback(of pc: PcState) -> Playback {
        var hasher = Hasher()
        hasher.combine(pc.current?.id)
        pc.upNext.forEach { hasher.combine($0.id) }
        let repeatMode: RepeatMode = switch pc.repeat { case "all": .all; case "one": .one; default: .off }
        return Playback(current: pc.current.map(song(of:)), playing: pc.playing, position: pc.position, volume: pc.volume,
                        repeat: repeatMode, upNext: UInt32(pc.upNext.count),
                        queueVersion: UInt64(bitPattern: Int64(hasher.finalize())), loading: false, error: nil)
    }

    private func showComputer(_ local: Playback) -> Bool {
        guard let pc, pc.current != nil, !local.playing else { return false }
        // Once shown, the computer's player stays while nothing plays here, so pausing the
        // computer does not swap it for the phone's paused song.
        return pc.playing || local.current == nil || onComputer
    }

    private func followComputer() {
        let core = core
        Task { [weak self] in
            var failures = 0
            while true {
                let (connected, heard) = await Task.detached { () -> (Bool, PcState?) in
                    let connected = core.pcConnected()
                    return (connected, connected ? try? core.pcState() : nil)
                }.value
                guard let self else { return }
                // Not answering (another network, the computer off): wait longer each time.
                failures = connected && heard == nil ? min(failures + 1, 5) : 0
                // One missed answer keeps what the computer last said: Wi-Fi drops one now and then.
                if (heard != nil || failures > 2 || !connected), heard != self.pc { self.pc = heard }
                let away = UIApplication.shared.applicationState == .background && !self.onComputer
                let seconds = self.onComputer ? 0.7 : failures > 2 ? min(30, 1.5 * pow(2, Double(failures - 2))) : away ? 15 : 1.5
                // Woken at once when a link is made or dropped.
                var waited = 0.0
                while waited < seconds, !self.askComputerNow {
                    try? await Task.sleep(for: .milliseconds(250))
                    waited += 0.25
                }
                self.askComputerNow = false
            }
        }
    }

    private func computer(_ work: @escaping @Sendable (Needle) throws -> Void) {
        let core = core
        Task.detached {
            do { try work(core) } catch {
                await MainActor.run { self.message = "Your computer did not answer" }
            }
        }
    }

    // MARK: - Sleep timer

    /// Pauses the music after `minutes`, or at the end of the song with `0`; nil turns it off.
    func setSleep(minutes: Int?) {
        if let activity = sleepActivity { Task { await activity.end(nil, dismissalPolicy: .immediate) } }
        sleepActivity = nil
        guard let minutes else { sleepEnds = nil; sleepSong = nil; message = "Sleep timer off"; return }
        if minutes == 0 {
            guard let song = local?.current?.id else { message = "Nothing is playing to end"; return }
            sleepEnds = .distantFuture
            sleepSong = song
            message = "Music pauses after this song"
        } else {
            sleepEnds = Date().addingTimeInterval(TimeInterval(minutes * 60))
            sleepSong = nil
            message = "Music pauses in \(minutes) minutes"
        }
        if ActivityAuthorizationInfo().areActivitiesEnabled {
            let state = NeedleActivity.ContentState(
                title: "Sleep timer",
                detail: minutes == 0 ? "Pauses after this song" : "The music pauses then",
                ends: minutes == 0 ? nil : sleepEnds, progress: nil)
            sleepActivity = try? Activity.request(attributes: NeedleActivity(kind: "sleep"), content: .init(state: state, staleDate: sleepEnds))
        }
    }

    private func followSleep(_ now: Playback) {
        guard let ends = sleepEnds else { return }
        let atEnd = now.current.map { $0.duration > 1 && now.position >= $0.duration - 0.35 } ?? false
        let songEnded = sleepSong != nil && (now.current?.id != sleepSong || atEnd)
        if Date() >= ends || songEnded {
            if now.playing { core.toggle() }
            sleepEnds = nil
            sleepSong = nil
            if let activity = sleepActivity { Task { await activity.end(nil, dismissalPolicy: .immediate) } }
            sleepActivity = nil
        }
    }

    // MARK: - A server's songs coming in, as a Live Activity

    private func followSync() {
        let syncing = core.plugins().compactMap { item -> (String, MusicSource)? in
            guard let source = item.source, source.syncing else { return nil }
            return (item.name, source)
        }.first
        if let (name, source) = syncing {
            let state = NeedleActivity.ContentState(
                title: "Bringing in songs from \(name)",
                detail: source.received > 0 ? "\(source.received.formatted()) songs so far" : "Asking the server…",
                ends: nil, progress: nil)
            if let activity = syncActivity {
                Task { await activity.update(.init(state: state, staleDate: nil)) }
            } else if ActivityAuthorizationInfo().areActivitiesEnabled {
                syncActivity = try? Activity.request(attributes: NeedleActivity(kind: "sync"), content: .init(state: state, staleDate: nil))
            }
        } else if let activity = syncActivity {
            syncActivity = nil
            Task {
                await activity.end(.init(state: .init(title: "Songs brought in", detail: "", ends: nil, progress: 1), staleDate: nil),
                                   dismissalPolicy: .after(.now + 5))
            }
        }
    }

    // MARK: - Music files

    /// Copies music chosen in the Files app into Needle's folder, then reads it.
    func importFiles(_ urls: [URL]) {
        let target = musicFolder
        let core = core
        Task.detached {
            let files = FileManager.default
            var copied = 0
            for url in urls {
                let scoped = url.startAccessingSecurityScopedResource()
                defer { if scoped { url.stopAccessingSecurityScopedResource() } }
                let destination = Self.freeName(for: url.lastPathComponent, in: target)
                do {
                    try files.copyItem(at: url, to: destination)
                    copied += 1
                } catch {
                    await MainActor.run { self.message = "Could not copy \(url.lastPathComponent)" }
                }
            }
            core.rescan()
            let count = copied
            await MainActor.run {
                self.message = count == 1 ? "Adding 1 item to your library" : "Adding \(count) items to your library"
            }
        }
    }

    /// A copy of a file chosen in the Files app, in a temporary folder, for the core to read.
    nonisolated func copyIn(_ url: URL) -> String? {
        let scoped = url.startAccessingSecurityScopedResource()
        defer { if scoped { url.stopAccessingSecurityScopedResource() } }
        let target = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString + "-" + url.lastPathComponent)
        guard (try? FileManager.default.copyItem(at: url, to: target)) != nil else { return nil }
        return target.path
    }

    /// Plays a music file opened from another app (Files, Mail, a browser).
    private func openFile(_ url: URL) {
        let target = musicFolder.appendingPathComponent("Inbox", isDirectory: true)
        let core = core
        Task.detached {
            let scoped = url.startAccessingSecurityScopedResource()
            defer { if scoped { url.stopAccessingSecurityScopedResource() } }
            var path = url.path
            if !Self.realPath(path).hasPrefix(Self.realPath(target.deletingLastPathComponent().path)) {
                try? FileManager.default.createDirectory(at: target, withIntermediateDirectories: true)
                let destination = Self.freeName(for: url.lastPathComponent, in: target)
                if (try? FileManager.default.copyItem(at: url, to: destination)) != nil {
                    path = destination.path
                }
            }
            let song = try? core.openFile(path: path)
            await MainActor.run {
                if song == nil { self.message = "Needle cannot play this file" }
                self.libraryChanged()
            }
        }
    }

    nonisolated static func freeName(for name: String, in folder: URL) -> URL {
        var url = folder.appendingPathComponent(name)
        let base = url.deletingPathExtension().lastPathComponent
        let ext = url.pathExtension
        var n = 2
        while FileManager.default.fileExists(atPath: url.path) {
            url = folder.appendingPathComponent(ext.isEmpty ? "\(base) \(n)" : "\(base) \(n).\(ext)")
            n += 1
        }
        return url
    }

    func rescan() { core.rescan() }

    // MARK: - The cover's colour

    private func findCoverColor(_ path: String?) {
        guard let path else { coverColor = nil; return }
        Task {
            let image = await Covers.load(path, size: 40)
            let color = await Task.detached { image.flatMap(Self.mainColor(of:)) }.value
            if colorFor == path { withAnimation(.easeInOut(duration: 0.8)) { coverColor = color } }
        }
    }

    /// The average colour of a picture, made bright enough to read as a colour on a dark page.
    nonisolated private static func mainColor(of image: UIImage) -> Color? {
        guard let cg = image.cgImage else { return nil }
        var pixel = [UInt8](repeating: 0, count: 4)
        guard let context = CGContext(data: &pixel, width: 1, height: 1, bitsPerComponent: 8, bytesPerRow: 4,
                                      space: CGColorSpaceCreateDeviceRGB(),
                                      bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue) else { return nil }
        context.interpolationQuality = .medium
        context.draw(cg, in: CGRect(x: 0, y: 0, width: 1, height: 1))
        let ui = UIColor(red: CGFloat(pixel[0]) / 255, green: CGFloat(pixel[1]) / 255, blue: CGFloat(pixel[2]) / 255, alpha: 1)
        var h: CGFloat = 0, s: CGFloat = 0, b: CGFloat = 0, a: CGFloat = 0
        ui.getHue(&h, saturation: &s, brightness: &b, alpha: &a)
        return Color(hue: h, saturation: min(1, s * 1.25), brightness: max(0.55, min(0.9, b * 1.2)))
    }

    // MARK: - Widgets

    @ObservationIgnored private var widgetWork: Task<Void, Never>?

    /// One update of the widgets at a time: a slow one (the library's) never writes an older
    /// song over a newer one.
    private func updateWidgets(library: Bool) async {
        let before = widgetWork
        let work = Task { await before?.value; await writeWidgets(library: library) }
        widgetWork = work
        await work.value
    }

    private func writeWidgets(library: Bool) async {
        guard let folder = Snapshot.folder else { return }
        var snapshot = Snapshot.load()
        let now = local
        snapshot.title = now?.current?.title
        snapshot.artist = now?.current?.artist
        snapshot.album = now?.current?.album
        snapshot.playing = now?.playing ?? false
        snapshot.cover = await Self.shareCover(now?.current?.artwork, in: folder)
        if library {
            let home = await load { try $0.home() }
            let playlists = await load { try $0.playlists() } ?? []
            let week = await load { try $0.stats(days: 7) }
            var recent: [Snapshot.Item] = []
            for album in (home?.recent ?? home?.added ?? []).prefix(4) {
                recent.append(.init(id: album.key, title: album.title, detail: album.artist,
                                    cover: await Self.shareCover(album.artwork, in: folder)))
            }
            snapshot.recent = recent
            var lists: [Snapshot.Item] = []
            for playlist in playlists.prefix(4) {
                lists.append(.init(id: playlist.id, title: playlist.name, detail: count(Int(playlist.songs), "song"),
                                   cover: await Self.shareCover(playlist.artwork, in: folder)))
            }
            snapshot.playlists = lists
            snapshot.weekSeconds = week?.seconds ?? 0
            snapshot.weekSongs = Int(week?.songs ?? 0)
            snapshot.topArtist = week?.topArtists.first.map(\.artist).flatMap { $0.isEmpty ? nil : $0 }
        }
        // The song as it is now: it may have changed while the library was read.
        if let current = local?.current?.id, current != now?.current?.id {
            snapshot.title = local?.current?.title
            snapshot.artist = local?.current?.artist
            snapshot.album = local?.current?.album
            snapshot.playing = local?.playing ?? false
            snapshot.cover = await Self.shareCover(local?.current?.artwork, in: folder)
        }
        snapshot.save()
        Self.pruneCovers(in: folder, keeping: snapshot)
        WidgetCenter.shared.reloadAllTimelines()
    }

    /// Takes away the widgets' covers that no widget shows now.
    private static func pruneCovers(in folder: URL, keeping snapshot: Snapshot) {
        let used = Set([snapshot.cover] + snapshot.recent.map(\.cover) + snapshot.playlists.map(\.cover)).compactMap { $0 }
        let files = (try? FileManager.default.contentsOfDirectory(atPath: folder.path)) ?? []
        for file in files where file.hasPrefix("cover-") && file.hasSuffix(".jpg") && !used.contains(file) {
            try? FileManager.default.removeItem(at: folder.appendingPathComponent(file))
        }
    }

    /// A small copy of a cover in the folder the widgets can read; its file name, the same
    /// for the same cover from one start of the app to the next.
    private static func shareCover(_ path: String?, in folder: URL) async -> String? {
        guard let path else { return nil }
        let digest = SHA256.hash(data: Data(path.utf8)).prefix(12).map { String(format: "%02x", $0) }.joined()
        let name = "cover-\(digest).jpg"
        let url = folder.appendingPathComponent(name)
        if FileManager.default.fileExists(atPath: url.path) { return name }
        guard let image = await Covers.load(path, size: 300), let data = image.jpegData(compressionQuality: 0.8) else { return nil }
        try? data.write(to: url)
        return name
    }

    // MARK: - The phone's audio, lock screen, and Control Center

    private func startAudio() {
        let session = AVAudioSession.sharedInstance()
        try? session.setCategory(.playback, mode: .default, policy: .longFormAudio)
        // Made active when music is asked for (see `activateAudio`), not here.
        let center = NotificationCenter.default
        // A call or an alarm pauses the music; it goes on afterwards when the phone says so.
        center.addObserver(forName: AVAudioSession.interruptionNotification, object: nil, queue: .main) { [weak self] note in
            let info = note.userInfo ?? [:]
            let kind = (info[AVAudioSessionInterruptionTypeKey] as? UInt).flatMap(AVAudioSession.InterruptionType.init)
            let options = (info[AVAudioSessionInterruptionOptionKey] as? UInt).map(AVAudioSession.InterruptionOptions.init)
            MainActor.assumeIsolated {
                guard let self else { return }
                switch kind {
                case .began:
                    if self.local?.playing == true { self.resumeAfterInterruption = true; self.core.toggle() }
                case .ended:
                    try? AVAudioSession.sharedInstance().setActive(true)
                    if self.resumeAfterInterruption, options?.contains(.shouldResume) == true, self.local?.playing == false {
                        self.core.toggle()
                    }
                    self.resumeAfterInterruption = false
                default: break
                }
            }
        }
        // Headphones taken out: pause, as every music app does.
        center.addObserver(forName: AVAudioSession.routeChangeNotification, object: nil, queue: .main) { [weak self] note in
            let reason = (note.userInfo?[AVAudioSessionRouteChangeReasonKey] as? UInt).flatMap(AVAudioSession.RouteChangeReason.init)
            MainActor.assumeIsolated {
                guard let self, reason == .oldDeviceUnavailable, self.local?.playing == true else { return }
                self.core.toggle()
            }
        }
    }

    private func startRemoteCommands() {
        let commands = MPRemoteCommandCenter.shared()
        let core = core
        commands.playCommand.addTarget { [weak self] _ in
            MainActor.assumeIsolated {
                guard let self, self.local?.current != nil, self.local?.playing != true else { return }
                self.activateAudio()
                core.toggle()
            }
            return .success
        }
        commands.pauseCommand.addTarget { [weak self] _ in
            MainActor.assumeIsolated { if self?.local?.playing == true { core.toggle() } }
            return .success
        }
        commands.togglePlayPauseCommand.addTarget { [weak self] _ in
            MainActor.assumeIsolated {
                if self?.local?.current != nil, self?.local?.playing != true { self?.activateAudio() }
            }
            core.toggle()
            return .success
        }
        commands.nextTrackCommand.addTarget { _ in core.next(); return .success }
        commands.previousTrackCommand.addTarget { _ in core.previous(); return .success }
        commands.changePlaybackPositionCommand.addTarget { event in
            guard let event = event as? MPChangePlaybackPositionCommandEvent else { return .commandFailed }
            core.seek(seconds: event.positionTime)
            return .success
        }
    }

    private func updateNowPlaying(_ now: Playback) {
        let center = MPNowPlayingInfoCenter.default()
        guard let song = now.current else {
            if shownSong != nil { center.nowPlayingInfo = nil; shownSong = nil }
            return
        }
        if song.id != shownSong {
            shownSong = song.id
            shownArtwork = nil
            if let path = song.artwork, let image = UIImage(contentsOfFile: path) {
                shownArtwork = MPMediaItemArtwork(boundsSize: image.size) { _ in image }
            }
        }
        var info: [String: Any] = [
            MPMediaItemPropertyTitle: song.title,
            MPMediaItemPropertyArtist: song.artist,
            MPMediaItemPropertyAlbumTitle: song.album,
            MPMediaItemPropertyPlaybackDuration: song.duration,
            MPNowPlayingInfoPropertyElapsedPlaybackTime: now.position,
            MPNowPlayingInfoPropertyPlaybackRate: now.playing ? 1.0 : 0.0,
        ]
        if let art = shownArtwork { info[MPMediaItemPropertyArtwork] = art }
        // Only when something the lock screen shows has changed, or the time has drifted.
        let old = center.nowPlayingInfo
        let drift = abs((old?[MPNowPlayingInfoPropertyElapsedPlaybackTime] as? Double ?? -10) - now.position)
        let rate = old?[MPNowPlayingInfoPropertyPlaybackRate] as? Double
        if old?[MPMediaItemPropertyTitle] as? String != song.title || rate != (now.playing ? 1.0 : 0.0) || drift > 1.5 || (old?[MPMediaItemPropertyArtwork] == nil && shownArtwork != nil) {
            center.nowPlayingInfo = info
            center.playbackState = now.playing ? .playing : .paused
        }
    }

    func shutdown() { core.shutdown() }
}

/// Crashes the system reports (through MetricKit, at the next start): the app's own, which the
/// core's reporter does not see. Each is kept and sent as the core's are, when crash reports
/// are on: what went wrong, and where in the crashed thread (no names, no files).
final class CrashWatcher: NSObject, MXMetricManagerSubscriber {
    static let shared = CrashWatcher()
    nonisolated(unsafe) private var core: Needle?

    func start(_ core: Needle) {
        guard self.core == nil else { return }
        self.core = core
        MXMetricManager.shared.add(self)
    }

    func didReceive(_ payloads: [MXDiagnosticPayload]) {
        guard let core else { return }
        var kept = false
        for payload in payloads {
            for crash in payload.crashDiagnostics ?? [] where core.keepCrashReport(text: Self.text(crash)) {
                kept = true
            }
        }
        if kept { _ = core.sendCrashReports() }
    }

    /// The crash as Needle sends it.
    static func text(_ crash: MXCrashDiagnostic) -> String {
        let version = Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? "?"
        let meta = crash.metaData
        var out = "Needle \(version) on ios \(meta.platformArchitecture) (the system's report, build \(meta.applicationBuildVersion))\n"
        out += "iOS \(meta.osVersion)\n"
        out += "Exception: \(crash.exceptionType?.stringValue ?? "?") code \(crash.exceptionCode?.stringValue ?? "?") signal \(crash.signal?.stringValue ?? "?")\n"
        if let reason = crash.terminationReason { out += "Termination: \(reason)\n" }
        if #available(iOS 17, *), let reason = crash.exceptionReason {
            out += "Message: \(reason.className): \(reason.composedMessage)\n"
        }
        guard let tree = try? JSONSerialization.jsonObject(with: crash.callStackTree.jsonRepresentation()) as? [String: Any],
              let stacks = tree["callStacks"] as? [[String: Any]] else { return out }
        let crashed = stacks.first { ($0["threadAttributed"] as? Bool) == true } ?? stacks.first
        var lines: [String] = []
        var builds: [String: String] = [:]
        func walk(_ frames: [[String: Any]]) {
            for frame in frames where lines.count < 60 {
                let binary = frame["binaryName"] as? String ?? "?"
                let offset = (frame["offsetIntoBinaryTextSegment"] as? NSNumber)?.uint64Value ?? 0
                lines.append(String(format: "%3d  %@  + 0x%llx", lines.count, binary, offset))
                if let uuid = frame["binaryUUID"] as? String { builds[binary] = uuid }
                if let sub = frame["subFrames"] as? [[String: Any]] { walk(sub) }
            }
        }
        walk(crashed?["callStackRootFrames"] as? [[String: Any]] ?? [])
        out += "\nCrashed thread:\n" + lines.joined(separator: "\n") + "\n"
        // Which build of each part, to find the names of the places later.
        if let needle = builds["Needle"] { out += "\nNeedle build \(needle)\n" }
        return out
    }
}

extension Store {
    /// A song's moving cover: on mobile data with "On mobile data" off, only one already
    /// here (nothing is downloaded); otherwise the core may fetch it from the server.
    func movingCover(_ id: String) async -> URL? {
        let keptOnly = onMobileData && !app.movingCoversOnMobileData
        let path = await load { keptOnly ? $0.animatedCoverKept(id: id) : $0.animatedCover(id: id) } ?? nil
        return path.map(URL.init(fileURLWithPath:))
    }
}
