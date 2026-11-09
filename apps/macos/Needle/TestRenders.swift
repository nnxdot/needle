#if DEBUG
import Carbon.HIToolbox
import SwiftUI

/// Test builds: pictures of a view drawn on its own (ImageRenderer), with no window, for when
/// the window cannot be seen (another app full screen on this Mac): from the app's start, not
/// the window's.
/// - `--test-play <album>`: plays that album.
/// - `--render-crate <path.png>`: the record crate, 14 seconds in.
/// - `--render-visualizer <path.png>`: the visualizer in a big player's frame, 16 seconds in,
///   and once more a second later (`<path>-2.png`).
/// - `--render-big <path.png>`: the real big player, 16 seconds in, and a second later.
/// - `--test-mcp-off <seconds>`: turns AI assistants off that long after the start.
/// - `--test-scrobbles <file>`: Clear failed for Last.fm and Retry failed for ListenBrainz, 8
///   seconds in, writing the answers and what Settings then reads.
/// - `--test-202 <file>`: Shuffle stays on, Hide Various Artists-only, and Forget songs that
///   are gone, 6 seconds in, through what the buttons and rows call; a song plays only when
///   the volume is 0 (the made-up songs are loud). With `--test-202-restored` too, no song is
///   started: the presses work on the song restored from the last session, paused.
/// - `--render-letters <path.png>`: the letter bar with A to Z, Korean, Japanese, and Chinese
///   letters, in a short and a tall space.
/// - `--test-spectrum <file>`: the core's spectrum twice a second for 12 seconds.
/// - `--test-sandbox <file>`: for a build in Apple's sandbox. Lists the outputs, sets the
///   volume to 0 and the output to `--test-output <name>` when given, adds the music folder
///   `--test-sandbox-add <path>` when given, lists the folders and albums, plays
///   `--test-sandbox-play <path>` when given (or what came through Open), then writes what
///   plays, once a second for 10 seconds. `<file>` is in the app's container when relative.
/// - `--headless`: no window on screen and no Dock icon (pictures are drawn offscreen).
/// - `--test-scan <folder>`: the volume to 0, that music folder read, then quits.
/// - `--test-206 <folder>`: round 2 of 2.0.6, silent: shortcuts everywhere, theme export,
///   Look up all (MusicBrainz), a read-only server playlist, Fix my library's error, and the
///   Radio page, as words (206.txt) and offscreen pictures; then quits.
@MainActor
enum TestRenders {
    static func start() {
        let args = CommandLine.arguments
        let value = { (flag: String) in args.firstIndex(of: flag).flatMap { args.indices.contains($0 + 1) ? args[$0 + 1] : nil } }
        let store = Store.shared
        if args.contains("--headless") { hideAll() }
        if let folder = value("--test-scan") { Task { await testScan(store, folder) } }
        if let folder = value("--test-206") { Task { await test206(store, folder) } }
        if let title = value("--test-play") {
            Task {
                try? await Task.sleep(for: .seconds(4))
                let albums = JSON.decode([AlbumSummary].self, await store.load { try $0.albumsJson(query: "") }) ?? []
                guard let album = albums.first(where: { $0.album == title }) else { return }
                _ = await store.ask { try $0.playView(query: album.query, playlist: nil, filter: "", start: 0, shuffle: false, reason: "From the album \(title)") }
            }
        }
        if let path = value("--render-crate") {
            Task {
                try? await Task.sleep(for: .seconds(14))
                let albums = JSON.decode([AlbumSummary].self, await store.load { try $0.albumsJson(query: "") }) ?? []
                render(RecordCrate(albums: albums)
                    .frame(width: 1000, height: 760)
                    .background(store.colors.canvas), to: path)
            }
        }
        if let path = value("--render-visualizer") {
            Task {
                try? await Task.sleep(for: .seconds(16))
                for file in [path, path.replacingOccurrences(of: ".png", with: "-2.png")] {
                    // A big player's frame: the cover's colours behind, the visualizer in the
                    // cover's place (the old one filled the frame itself).
                    render(ZStack {
                        LinearGradient(colors: [store.colors.glow.opacity(0.55), .black], startPoint: .top, endPoint: .bottom)
                        Visualizer().frame(width: 480)
                    }
                    .frame(width: 1400, height: 890), to: file)
                    try? await Task.sleep(for: .seconds(1))
                }
            }
        }
        if let path = value("--render-big") {
            Task {
                try? await Task.sleep(for: .seconds(16))
                // The cover's colours read first: a picture runs no `.task`.
                if let p = store.playback { _ = await Visualizer.coverColor(p.artwork, seed: p.album + p.artist) }
                for file in [path, path.replacingOccurrences(of: ".png", with: "-2.png")] {
                    // The real big player, in its look (`big_look`), at a window's size.
                    render(BigPlayer().environment(store).frame(width: 1320, height: 860), to: file)
                    try? await Task.sleep(for: .seconds(1))
                }
            }
        }
        if let seconds = value("--test-mcp-off").flatMap(Double.init) {
            Task {
                try? await Task.sleep(for: .seconds(seconds))
                store.set(["mcp": false])
            }
        }
        if let file = value("--test-scrobbles") {
            Task {
                try? await Task.sleep(for: .seconds(8))
                var lines = ["before: " + store.core.servicesJson()]
                let cleared = await store.ask { try $0.clearScrobbles(service: "lastfm") }
                let tried = await store.ask { try $0.retryScrobbles(service: "listenbrainz") }
                lines.append("clear lastfm: \(cleared.map(String.init) ?? "failed")")
                lines.append("retry listenbrainz: \(tried.map(String.init) ?? "failed")")
                lines.append("after: " + store.core.servicesJson())
                try? lines.joined(separator: "\n").write(toFile: file, atomically: true, encoding: .utf8)
            }
        }
        if let file = value("--test-202") {
            Task { await test202(store, file, play: !args.contains("--test-202-restored")) }
        }
        if let path = value("--render-letters") {
            let letters = (65...90).map { String(UnicodeScalar($0)!) } + ["#"]
                + "ㄱㄴㄷㄹㅁㅂㅅㅇㅈㅊㅋㅌㅍㅎ".map(String.init) + "あかさたなはまやらわ".map(String.init) + ["漢"]
            render(HStack(alignment: .top, spacing: 30) {
                LetterBar(letters: letters) { _ in }.frame(height: 420)
                LetterBar(letters: letters) { _ in }.frame(height: 900)
                LetterBar(letters: Array(letters.prefix(27))) { _ in }.frame(height: 420)
            }
            .padding(20)
            .frame(width: 180, height: 940, alignment: .topLeading)
            .background(store.colors.canvas), to: path)
        }
        if let folder = value("--render-glass") {
            Task { await renderGlass(store, folder, music: value("--test-music")) }
        }
        if let file = value("--test-sandbox") {
            Task { await testSandbox(store, file, output: value("--test-output"), play: value("--test-sandbox-play"), add: value("--test-sandbox-add")) }
        }
        if let file = value("--test-spectrum") {
            Task {
                var lines: [String] = []
                for _ in 0..<24 {
                    try? await Task.sleep(for: .milliseconds(500))
                    let bands = store.core.spectrum(bands: 48)
                    let most = bands.max() ?? 0, mean = bands.reduce(0, +) / Float(max(bands.count, 1))
                    lines.append(String(format: "max %.2f  mean %.2f  low %.2f  high %.2f", most, mean, bands.first ?? 0, bands.last ?? 0))
                }
                try? lines.joined(separator: "\n").write(toFile: file, atomically: true, encoding: .utf8)
            }
        }
    }

    private static func testSandbox(_ store: Store, _ file: String, output: String?, play: String?, add: String?) async {
        let report = file.hasPrefix("/") ? file : NSHomeDirectory() + "/" + file
        var lines = ["home: \(NSHomeDirectory())", "sandboxed: \(ProcessInfo.processInfo.environment["APP_SANDBOX_CONTAINER_ID"] != nil)"]
        func write() { try? lines.joined(separator: "\n").write(toFile: report, atomically: true, encoding: .utf8) }
        store.setVolume(0)
        let devices = await store.load { $0.outputDevices() } ?? []
        lines.append("outputs: \(devices)")
        if let output {
            store.set(["output_device": output])
            lines.append("output set: \(output)")
        }
        write()
        try? await Task.sleep(for: .seconds(3))
        if let add {
            lines.append("folder readable: \(FileManager.default.isReadableFile(atPath: add))")
            let added: Void? = await store.ask { try $0.addFolder(path: add) }
            lines.append("addFolder: \(added == nil ? "failed" : "ok")")
            try? await Task.sleep(for: .seconds(3))
        }
        let albums = JSON.decode([AlbumSummary].self, await store.load { try $0.albumsJson(query: "") }) ?? []
        lines.append("folders: \((try? store.core.folders()) ?? []), albums: \(albums.map(\.album))")
        if let play {
            lines.append("readable: \(FileManager.default.isReadableFile(atPath: play))")
            let opened = await store.ask { try $0.openFiles(paths: [play]) }
            lines.append("openFiles: \(opened.map(String.init) ?? "failed")")
        }
        for second in 1...10 {
            try? await Task.sleep(for: .seconds(1))
            guard let p = store.playback else { lines.append("\(second): no playback"); continue }
            lines.append("\(second): “\(p.title)” \(p.format) playing \(p.playing) at \(String(format: "%.1f", store.playhead.position)) s, volume \(p.volume), output \(p.output) (\(p.outputDevice ?? "default"), \(p.outputRate) Hz), loading \(p.loading), error \(p.error ?? "none"), notice \(p.outputNotice ?? "none")")
            write()
        }
        lines.append("done")
        write()
    }

    private static func test202(_ store: Store, _ file: String, play: Bool) async {
        var lines: [String] = []
        func pause() async { try? await Task.sleep(for: .milliseconds(1500)) }
        let song = {
            guard let p = store.playback else { return "no playback" }
            return "“\(p.title)” playing \(p.playing) at \(Int(store.playhead.position)) s, \(p.queue) up next"
        }
        let shuffled = { String(store.playback?.shuffle ?? false) }
        try? await Task.sleep(for: .seconds(6))
        lines.append("volume: \(store.settings.volume)")
        lines.append("at the start: song \(song())")
        if !play {
            lines.append("no new song: the one restored, paused")
        } else if store.settings.volume == 0 {
            lines.append("before playView: song \(song())")
            let albums = JSON.decode([AlbumSummary].self, await store.load { try $0.albumsJson(query: "") }) ?? []
            lines.append("albums: \(albums.count), first “\(albums.first?.album ?? "")” query \(albums.first?.query ?? "")")
            if let album = albums.first {
                do {
                    let started = try store.core.playView(query: album.query, playlist: nil, filter: "", start: 0, shuffle: false, reason: "Test")
                    lines.append("playView: \(started)")
                } catch {
                    lines.append("playView failed: \(error.message)")
                }
            }
            await pause()
            lines.append("after playView: song \(song())")
        } else {
            lines.append("not muted: no song played")
        }
        store.setShuffleStays(true)
        await pause()
        lines.append("stays on: \(store.settings.shuffleStays), shuffle \(shuffled()), song \(song())")
        store.shuffleButton()
        await pause()
        lines.append("after a press: shuffle \(shuffled()), lit \(store.shuffleOn), help \(store.shuffleHelp), song \(song())")
        store.shuffleButton()
        await pause()
        lines.append("after a second press: shuffle \(shuffled()), lit \(store.shuffleOn), song \(song())")
        try? await Task.sleep(for: .seconds(3))
        lines.append("3 s later: song \(song())")
        store.shuffleButton()
        await pause()
        lines.append("pressed on again: shuffle \(shuffled()), song \(song())")
        store.setShuffleStays(false)
        await pause()
        lines.append("setting off: stays \(store.settings.shuffleStays), shuffle \(shuffled()), help \(store.shuffleHelp)")
        if store.playback?.playing == true { store.core.toggle() }
        let count = { () async -> Int in
            (JSON.decode([ArtistSummary].self, await store.load { try $0.artistsJson(query: "") }) ?? []).count
        }
        store.set(["hide_compilation_artists": false])
        await pause()
        let all = await count()
        store.set(["hide_compilation_artists": true])
        await pause()
        lines.append("artists: \(all) shown, \(await count()) with Hide Various Artists-only (setting \(store.settings.hideCompilationArtists))")
        store.set(["hide_compilation_artists": false])
        store.set(["forget_missing": true])
        await pause()
        lines.append("forget missing: \(store.settings.forgetMissing)")
        store.set(["forget_missing": false])
        await pause()
        lines.append("forget missing off: \(store.settings.forgetMissing)")
        try? lines.joined(separator: "\n").write(toFile: file, atomically: true, encoding: .utf8)
    }

    /// `--render-glass <folder> [--test-music <folder>]`: the volume to 0 first (checked before
    /// anything plays), the music folder read, its first album played and paused at once, then
    /// the real glass parts drawn into <folder>: the player at the page's foot with its lyric
    /// strip, the title bar's group (macOS 26), a long message, the side panel, the mini
    /// player, and the folded sidebar, in Night and in Day; and what the store says of them.
    private static func renderGlass(_ store: Store, _ folder: String, music: String?) async {
        var notes: [String] = []
        func note(_ line: String) {
            notes.append(line)
            try? notes.joined(separator: "\n").write(toFile: folder + "/glass.txt", atomically: true, encoding: .utf8)
        }
        try? FileManager.default.createDirectory(atPath: folder, withIntermediateDirectories: true)
        store.setVolume(0)
        store.set(["welcomed": true, "player_lyrics": true, "music_colors": true, "ambient": false,
                   "window_material": "glass", "theme": "dark"])
        try? await Task.sleep(for: .seconds(1))
        note("volume \(store.playback?.volume ?? -1) settings volume \(store.settings.volume)")
        if let music {
            _ = await store.ask { try $0.addFolder(path: music) }
            for _ in 0..<30 where store.scan.running || store.libraryVersion == 0 {
                try? await Task.sleep(for: .seconds(1))
            }
            try? await Task.sleep(for: .seconds(2))
        }
        let albums = JSON.decode([AlbumSummary].self, await store.load { try $0.albumsJson(query: "") }) ?? []
        note("albums \(albums.map(\.album))")
        // Silent, or nothing plays.
        if !albums.isEmpty, (store.playback?.volume ?? 1) <= 0.001 {
            // The song with timed lyrics (a .lrc beside it), paused a little way in.
            _ = await store.ask { try $0.playView(query: "title contains \"Glass\"", playlist: nil, filter: "", start: 0, shuffle: false, reason: "test") }
            for _ in 0..<30 where !store.isPlaying { try? await Task.sleep(for: .milliseconds(100)) }
            if store.isPlaying { store.toggle() }
            store.seek(4.2)
        } else {
            note("not played: volume not 0, or no album")
        }
        try? await Task.sleep(for: .seconds(4))
        note("playing \(store.playback?.title ?? "none") paused \(!(store.playback?.playing ?? false)) tint \(store.paneTint.map { "\($0)" } ?? "none") lyrics shows \(LyricStrip.shows) timed \(store.lyricsTimed)")
        let backdrop = { (dark: Bool) -> AnyView in
            AnyView((dark ? Color.black : Color.white)
                .overlay {
                    if let p = store.playback { CoverBackdrop(path: p.artwork, seed: p.album + p.artist, moving: false, dim: dark ? 0.35 : 0, light: !dark) }
                }
                // Rows under the glass, to see what it does to them.
                .overlay(alignment: .topLeading) {
                    VStack(alignment: .leading, spacing: 14) {
                        ForEach(0..<12, id: \.self) { i in
                            Text("Song row \(i + 1) — the page under the player").font(.system(size: 15, weight: .semibold))
                                .foregroundStyle(dark ? .white : .black)
                        }
                    }
                    .padding(24)
                }
                .clipped())
        }
        for dark in [true, false] {
            store.set(["theme": dark ? "dark" : "light"])
            try? await Task.sleep(for: .seconds(1))
            let look = dark ? "night" : "day"
            render(ZStack(alignment: .bottom) { backdrop(dark); MainWindow.PlayerFoot() }.frame(width: 900, height: 260), to: "\(folder)/foot-\(look).png")
            if #available(macOS 26, *) {
                render(ZStack { backdrop(dark); TitleBarGlass() }.frame(width: 1060, height: 90), to: "\(folder)/titlebar-\(look).png")
            }
            store.toast = Store.Toast(text: "Needle could not reach your music server. Check that it is on and that this Mac is on the same network, then try again. The songs you keep on this Mac still play.", error: true)
            render(ZStack(alignment: .bottom) { backdrop(dark); ToastView().padding(.bottom, 20) }.frame(width: 700, height: 200), to: "\(folder)/toast-\(look).png")
            store.toast = nil
            store.panel = .queue
            render(ZStack(alignment: .trailing) {
                backdrop(dark)
                Inspector().frame(width: MainWindow.panelWidth).frame(maxHeight: .infinity)
                    .clipShape(RoundedRectangle(cornerRadius: Radius.large, style: .continuous))
                    .pane(RoundedRectangle(cornerRadius: Radius.large, style: .continuous), tinted: true)
                    .padding(10)
            }.frame(width: 700, height: 560), to: "\(folder)/panel-\(look).png")
            render(HStack(spacing: 0) { Rail(); backdrop(dark) }.frame(width: 400, height: 700).background(store.colors.canvas), to: "\(folder)/rail-\(look).png")
        }
        MiniPlayer.showControls = true
        render(MiniPlayer().frame(width: 300, height: 300), to: "\(folder)/mini.png")
        MiniPlayer.showControls = false
        note("done")
    }

    // MARK: - 2.0.6, round 2

    /// `--headless`: the app hides at once, with no Dock icon and no window on screen; pictures
    /// are drawn offscreen.
    static func hideAll() {
        NSApp.setActivationPolicy(.prohibited)
        NSApp.hide(nil)
        NotificationCenter.default.addObserver(forName: NSApplication.didUnhideNotification, object: nil, queue: .main) { _ in
            MainActor.assumeIsolated { NSApp.hide(nil) }
        }
    }

    /// `--test-scan <folder>`: the volume to 0, then the music folder read; quits when done.
    static func testScan(_ store: Store, _ folder: String) async {
        store.setVolume(0)
        store.set(["welcomed": true, "online_media": true])
        try? await Task.sleep(for: .seconds(2))
        _ = await store.ask { try $0.addFolder(path: folder) }
        try? await Task.sleep(for: .seconds(2))
        for _ in 0..<60 where store.scan.running { try? await Task.sleep(for: .seconds(1)) }
        try? await Task.sleep(for: .seconds(2))
        NSApp.terminate(nil)
    }

    /// `--test-206 <folder>`: round 2's checks and pictures, silent (the volume to 0 first, and
    /// nothing is played), into <folder>: what the calls answer in 206.txt, and the real views
    /// drawn offscreen (no window on screen). Quits when done.
    static func test206(_ store: Store, _ folder: String) async {
        try? FileManager.default.createDirectory(atPath: folder, withIntermediateDirectories: true)
        var lines: [String] = []
        func note(_ line: String) {
            lines.append(line)
            try? lines.joined(separator: "\n").write(toFile: folder + "/206.txt", atomically: true, encoding: .utf8)
        }
        store.setVolume(0)
        store.set(["welcomed": true, "online_media": true, "theme": "dark", "music_colors": false])
        try? await Task.sleep(for: .seconds(3))
        note("volume \(store.settings.volume), playing \(store.isPlaying), sandboxed \(ProcessInfo.processInfo.environment["APP_SANDBOX_CONTAINER_ID"] != nil)")

        // Shortcuts everywhere: keys held, keys taken, a press delivered, and let go.
        store.set(["global_shortcuts": [["play", "ctrl-alt-shift-f17"], ["volume_up", "ctrl-alt-shift-f18"],
                                        ["volume_down", "ctrl-alt-shift-f19"], ["next", "ctrl-alt-shift-f16"]]])
        GlobalKeys.testOn = true
        GlobalKeys.apply()
        note("hot keys held: \(GlobalKeys.heldCount), taken: \(GlobalKeys.taken.sorted())")
        var other: EventHotKeyRef?
        let again = RegisterEventHotKey(UInt32(kVK_F17), UInt32(controlKey | optionKey | shiftKey),
                                        EventHotKeyID(signature: 0x5445_5354, id: 1), GetApplicationEventTarget(), 0, &other)
        note("the same keys registered a second time: status \(again) (\(again == noErr ? "accepted" : "refused"))")
        if let other { UnregisterEventHotKey(other) }
        if !store.isPlaying {
            GlobalKeys.testPress("volume_up")
            try? await Task.sleep(for: .milliseconds(500))
            note("after Volume up's hot key: volume \(store.playback?.volume ?? store.settings.volume)")
            GlobalKeys.testPress("volume_down")
            try? await Task.sleep(for: .milliseconds(500))
            note("after Volume down's hot key: volume \(store.playback?.volume ?? store.settings.volume)")
        }
        store.setVolume(0)
        await snapshot(Form { GlobalShortcutsSection() }.formStyle(.grouped).frame(width: 620, height: 470), dark: true, to: folder + "/keyboard.png")
        GlobalKeys.testOn = false
        GlobalKeys.apply()
        note("switch off: hot keys held \(GlobalKeys.heldCount)")
        GlobalKeys.testOn = nil

        // Theme files to share.
        let themes = store.core.themesFolder()
        try? FileManager.default.createDirectory(atPath: themes, withIntermediateDirectories: true)
        try? "name = \"Test Night\"\nauthor = \"Needle tests\"\n".write(toFile: themes + "/test-night.toml", atomically: true, encoding: .utf8)
        switch await store.read({ try $0.exportTheme(id: "custom:test-night") }) {
        case .success(let file): note("export theme: file name \(file.fileName), \(file.text.count) characters")
        case .failure(let failure): note("export theme failed: \(failure.message)")
        }
        switch await store.read({ try $0.exportTheme(id: "custom:not-there") }) {
        case .success: note("export of a missing theme: no error (wrong)")
        case .failure(let failure): note("export of a missing theme: \(failure.message)")
        }

        // Look up all, for real (MusicBrainz), and the matches written.
        let problems = await store.load { $0.albumProblems() } ?? []
        note("albums with gaps: \(problems.map { "\($0.album) (\($0.problems.joined(separator: ", ")))" })")
        var bulk = TagsTab.Bulk(total: problems.count)
        for (i, p) in problems.enumerated() {
            let (album, artist, ids) = (p.album, p.artist, p.songIds)
            let result = await store.read { try $0.lookUpSureMatch(album: album, artist: artist, songIds: ids, start: i == 0) }
            switch result {
            case .success(let match?):
                note("look up “\(album)”: sure match \(match.release.title) (\(match.release.year)), \(match.changes) songs change")
                bulk.found.append(TagsTab.Found(album: p, match: match))
                bulk.chosen.insert(match.release.id)
            case .success(nil): note("look up “\(album)”: no sure match")
            case .failure(let failure):
                note("look up “\(album)”: could not ask (\(failure.message))")
                bulk.failed += 1
            }
            bulk.done = i + 1
        }
        bulk.finished = true
        await snapshot(TagsTab(bulk: bulk).padding(20).frame(width: 760, height: 420).background(store.colors.canvas), dark: true, to: folder + "/lookup-all.png")
        if !bulk.chosen.isEmpty {
            let ids = Array(bulk.chosen)
            switch await store.read({ try $0.applySureMatches(releaseIds: ids) }) {
            case .success(let report): note("write: \(report.saved) songs saved, failed \(report.failed)")
            case .failure(let failure): note("write failed: \(failure.message)")
            }
            note("albums with gaps after writing: \((await store.load { $0.albumProblems() } ?? []).map(\.album))")
        }

        // A read-only playlist on a music server (put in the test library before the start).
        store.reloadPlaylists()
        try? await Task.sleep(for: .seconds(1))
        if let readOnly = store.playlists.first(where: \.readOnly) {
            note("read-only playlist: \(readOnly.name), server \(readOnly.server)")
            let route = Route.playlist(readOnly.id)
            store.navigate(route)
            let model = SongsModel()
            await model.load(store, route)
            note("its page: \(model.total) songs, arrangeable \(model.playlist?.readOnly == false)")
            await snapshot(VStack(alignment: .leading, spacing: 0) {
                SongsHeader(route: route, model: model)
                Divider()
                VStack(alignment: .leading, spacing: 4) {
                    ForEach(store.playlists, id: \.id) { PlaylistRow(playlist: $0, rail: false) }
                }
                .padding(16)
                .frame(width: 260, alignment: .leading)
            }
            .frame(width: 900, height: 470, alignment: .topLeading).background(store.colors.canvas), dark: true, to: folder + "/read-only.png")
            // Adding to it from elsewhere: refused here, never "Added …".
            store.addToPlaylist(readOnly, model.tracks.prefix(1).map(\.id))
            try? await Task.sleep(for: .milliseconds(300))
            note("adding to it: “\(store.toast?.text ?? "nothing said")”")
            // The core itself, asked to add a song not in it: where the song went.
            let inside = Set(model.tracks.map(\.id))
            let all = await store.load { try $0.songs().map(\.id) } ?? []
            let (id, songs) = (readOnly.id, all.filter { !inside.contains($0) }.prefix(1).map { $0 })
            switch await store.read({ try $0.addSongsToPlaylist(id: id, ids: songs) }) {
            case .success(let saved):
                note("core add: id \(saved.id), copy \(saved.copyId ?? "none") “\(saved.copyName ?? "")”, note “\(saved.note ?? "none")”, changed \(saved.changed)")
                store.playlistSaved(saved, from: id, follow: true, done: "Added (wrong if shown)")
                try? await Task.sleep(for: .milliseconds(300))
                note("said: “\(store.toast?.text ?? "nothing")”, page now \(store.route)")
            case .failure(let failure): note("core add failed: \(failure.message)")
            }
            switch await store.read({ try $0.deletePlaylist(id: id) }) {
            case .success: note("delete: done (wrong for read-only)")
            case .failure(let failure): note("delete: \(failure.message)")
            }
        } else {
            note("no read-only playlist in the test library")
        }

        // Fix my library when the core cannot read the library.
        DoctorPage.testError = "database disk image is malformed"
        store.navigate(.doctor)
        await snapshot(DoctorPage().frame(width: 900, height: 330).background(store.colors.canvas), dark: true, to: folder + "/doctor-error.png")
        DoctorPage.testError = nil

        // The Radio page, night and day.
        for dark in [true, false] {
            store.set(["theme": dark ? "dark" : "light"])
            try? await Task.sleep(for: .seconds(1))
            await snapshot(RadioPage().frame(width: 1100, height: 900).background(store.colors.canvas), dark: dark,
                           to: folder + "/radio-\(dark ? "night" : "day").png", settle: 4)
        }
        store.set(["theme": "dark"])
        note("volume at the end \(store.settings.volume), playing \(store.isPlaying)")
        note("done")
        NSApp.terminate(nil)
    }

    /// A real view drawn offscreen: in a window that is never put on screen, given `settle`
    /// seconds for its `.task`s to load, then drawn into a picture.
    private static func snapshot(_ view: some View, dark: Bool, to path: String, settle: Double = 2) async {
        let host = NSHostingView(rootView: AnyView(view.environment(\.colorScheme, dark ? .dark : .light)))
        host.frame = CGRect(origin: .zero, size: host.fittingSize)
        let window = NSWindow(contentRect: host.frame, styleMask: [.borderless], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.appearance = NSAppearance(named: dark ? .darkAqua : .aqua)
        window.contentView = host
        try? await Task.sleep(for: .seconds(settle))
        host.layoutSubtreeIfNeeded()
        guard let rep = host.bitmapImageRepForCachingDisplay(in: host.bounds) else { return }
        host.cacheDisplay(in: host.bounds, to: rep)
        try? rep.representation(using: .png, properties: [:])?.write(to: URL(fileURLWithPath: path))
        window.contentView = nil
    }

    private static func render(_ view: some View, to path: String) {
        let renderer = ImageRenderer(content: view.environment(\.colorScheme, Store.shared.colors.dark ? .dark : .light))
        renderer.scale = 1
        guard let tiff = renderer.nsImage?.tiffRepresentation,
              let png = NSBitmapImageRep(data: tiff)?.representation(using: .png, properties: [:]) else { return }
        try? png.write(to: URL(fileURLWithPath: path))
    }
}
#endif
