import AVFoundation
import MediaPlayer
import Observation
import SwiftUI
import UIKit

/// Holds Needle's core for the whole app: the screens, the lock screen, and Control Center
/// share it. What plays is read from the core a few times a second.
@MainActor @Observable
final class Store {
    let core: Needle
    /// Needle's folder in the Files app ("On My iPhone › Needle"): the music Needle reads.
    let musicFolder: URL

    private(set) var playback: Playback?
    private(set) var scan = ScanStatus(running: false, scanned: 0, imported: 0, current: "", error: nil)
    /// Changes whenever the library may have changed (a scan finished), so lists load again.
    private(set) var libraryVersion = 0
    /// A short message to show at the bottom of the screen.
    var message: String?
    /// A plugin's question, answered with text.
    var question: Question?
    var showPlayer = false
    /// The songs up next, read again whenever the queue changes.
    private(set) var upNext: [Song] = []

    struct Question: Identifiable {
        let id: UInt64
        let title: String
        let prompt: String
        var answer: String
    }

    @ObservationIgnored private var timer: Timer?
    @ObservationIgnored private var wasScanning = false
    @ObservationIgnored private var queueVersion: UInt64 = .max
    @ObservationIgnored private var shownSong: String?
    @ObservationIgnored private var shownArtwork: MPMediaItemArtwork?
    @ObservationIgnored private var ticks = 0

    init() {
        let files = FileManager.default
        let support = files.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
            .appendingPathComponent("Needle", isDirectory: true)
        try? files.createDirectory(at: support, withIntermediateDirectories: true)
        musicFolder = files.urls(for: .documentDirectory, in: .userDomainMask)[0]
        // The app's folder can move when it is updated: the library's paths move with it.
        let home = NSHomeDirectory()
        let defaults = UserDefaults.standard
        if let earlier = defaults.string(forKey: "home"), earlier != home {
            _ = try? relocate(dataDir: support.path, old: earlier, new: home)
        }
        defaults.set(home, forKey: "home")
        do {
            core = try Needle(dataDir: support.path)
        } catch {
            fatalError("Needle could not open its library: \(error)")
        }
        startAudio()
        startRemoteCommands()
        let folder = musicFolder.path
        let core = core
        Task.detached {
            // Already known: it is read again when the app comes to the front.
            if (try? core.folders())?.contains(folder) != true {
                try? core.addFolder(path: folder)
            }
            _ = core.sendCrashReports()
            #if DEBUG
            // For testing in the simulator from a shell: plays the whole library at start.
            if CommandLine.arguments.contains("-play-test") {
                try? await Task.sleep(for: .seconds(3))
                if let songs = try? core.songs(), !songs.isEmpty {
                    try? core.play(ids: songs.map(\.id), start: 0)
                }
            }
            #endif
        }
        timer = Timer.scheduledTimer(withTimeInterval: 0.25, repeats: true) { [weak self] _ in
            MainActor.assumeIsolated { self?.tick() }
        }
    }

    // MARK: - Following the core

    private func tick() {
        let now = core.playback()
        if now != playback { playback = now }
        if now.queueVersion != queueVersion {
            queueVersion = now.queueVersion
            upNext = core.upNext()
        }
        updateNowPlaying(now)
        ticks += 1
        // The rest changes slowly: four times a second is more than enough for the playing song.
        guard ticks % 2 == 0 else { return }
        let status = core.scanStatus()
        if status != scan { scan = status }
        if wasScanning && !status.running { libraryVersion += 1 }
        wasScanning = status.running
        for notice in core.takeNotices() {
            switch notice {
            case .message(let text): message = text
            case .question(let id, _, let title, let prompt, let value, let file, _):
                // Plugins that ask for a file are desktop plugins: a phone has nothing to give.
                if file { core.answer(id: id, text: nil, file: nil) }
                else { question = Question(id: id, title: title, prompt: prompt, answer: value) }
            case .libraryChanged: libraryVersion += 1
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
                await MainActor.run { self.message = "\(error.localizedDescription)" }
            }
        }
    }

    /// Reads a value away from the screen.
    nonisolated func load<T: Sendable>(_ work: @escaping @Sendable (Needle) throws -> T) async -> T? {
        let core = core
        return await Task.detached { try? work(core) }.value
    }

    // MARK: - Playback

    func play(_ songs: [Song], from start: Int = 0) {
        let ids = songs.map(\.id)
        run { try $0.play(ids: ids, start: UInt32(start)) }
    }

    func shufflePlay(_ songs: [Song]) {
        guard !songs.isEmpty else { return }
        let ids = songs.map(\.id).shuffled()
        run { try $0.play(ids: ids, start: 0) }
    }

    func playAlbum(_ album: Album, shuffled: Bool = false) {
        let key = album.key
        run { core in
            var ids = try core.albumSongs(key: key).map(\.id)
            if shuffled { ids.shuffle() }
            try core.play(ids: ids, start: 0)
        }
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

    func toggle() { core.toggle() }
    func next() { core.next() }
    func previous() { core.previous() }
    func seek(_ seconds: Double) { core.seek(seconds: seconds) }
    func jump(_ index: Int) { core.jump(index: UInt32(index)) }
    func shuffle() { core.shuffle(); message = "Up next shuffled" }

    func cycleRepeat() {
        let next: RepeatMode = switch playback?.repeat ?? .off {
        case .off: .all
        case .all: .one
        case .one: .off
        }
        core.setRepeat(mode: next)
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

    /// Plays a music file opened from another app (Files, Mail, a browser).
    func open(_ url: URL) {
        let target = musicFolder.appendingPathComponent("Inbox", isDirectory: true)
        let core = core
        Task.detached {
            let scoped = url.startAccessingSecurityScopedResource()
            defer { if scoped { url.stopAccessingSecurityScopedResource() } }
            var path = url.path
            if !path.hasPrefix(target.deletingLastPathComponent().path) {
                try? FileManager.default.createDirectory(at: target, withIntermediateDirectories: true)
                let destination = Self.freeName(for: url.lastPathComponent, in: target)
                if (try? FileManager.default.copyItem(at: url, to: destination)) != nil {
                    path = destination.path
                }
            }
            let song = try? core.openFile(path: path)
            await MainActor.run {
                if song == nil { self.message = "Needle cannot play this file" }
                self.libraryVersion += 1
            }
        }
    }

    nonisolated private static func freeName(for name: String, in folder: URL) -> URL {
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

    // MARK: - The phone's audio, lock screen, and Control Center

    private func startAudio() {
        let session = AVAudioSession.sharedInstance()
        try? session.setCategory(.playback, mode: .default, policy: .longFormAudio)
        try? session.setActive(true)
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
                    if self.playback?.playing == true { self.resumeAfterInterruption = true; self.core.toggle() }
                case .ended:
                    try? AVAudioSession.sharedInstance().setActive(true)
                    if self.resumeAfterInterruption, options?.contains(.shouldResume) == true, self.playback?.playing == false {
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
                guard let self, reason == .oldDeviceUnavailable, self.playback?.playing == true else { return }
                self.core.toggle()
            }
        }
    }

    @ObservationIgnored private var resumeAfterInterruption = false

    private func startRemoteCommands() {
        let commands = MPRemoteCommandCenter.shared()
        let core = core
        commands.playCommand.addTarget { [weak self] _ in
            MainActor.assumeIsolated { if self?.playback?.playing != true { core.toggle() } }
            return .success
        }
        commands.pauseCommand.addTarget { [weak self] _ in
            MainActor.assumeIsolated { if self?.playback?.playing == true { core.toggle() } }
            return .success
        }
        commands.togglePlayPauseCommand.addTarget { _ in core.toggle(); return .success }
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
