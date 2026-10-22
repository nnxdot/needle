#if DEBUG
import AppKit
import SwiftUI

/// Test builds: `--show stress <minutes>` works the window as a person in a hurry would, for that
/// long: clicks and double-clicks in the sidebar and the page (sent through the window as real
/// mouse events), page switches, Back and Forward, sorting, reloads, songs played (muted), the
/// big player and the sidebar folded. Each step goes in /tmp/needle-stress.log before it is
/// taken, so a crash's last steps are known. Needle quits at the end. With `--stress-music
/// <folder>`, an empty test library first gets that folder and is read. With
/// `--stress-album-leave`, about a third of the steps open an album (or start the next song)
/// and click the sidebar while the new colours come in: the way 1.7.0 and 1.7.1 crashed.
/// Use a test library: `--data-dir /tmp/...`.
@MainActor
enum Stress {
    private static let log: FileHandle? = {
        let path = "/tmp/needle-stress.log"
        FileManager.default.createFile(atPath: path, contents: nil)
        return FileHandle(forWritingAtPath: path)
    }()

    private static func note(_ text: String) {
        log?.write(Data("\(Date().timeIntervalSince1970) \(text)\n".utf8))
        try? log?.synchronize()
    }

    /// Starts the test when Needle was opened with `--show stress`.
    static func startIfAsked() {
        let arguments = CommandLine.arguments
        guard let i = arguments.firstIndex(of: "--show"), arguments.indices.contains(i + 1), arguments[i + 1] == "stress" else { return }
        let minutes = arguments.indices.contains(i + 2) ? Double(arguments[i + 2]) ?? 5 : 5
        let music = arguments.firstIndex(of: "--stress-music").flatMap { arguments.indices.contains($0 + 1) ? arguments[$0 + 1] : nil }
        // macOS writes no crash report for this test copy: the stack goes in the log.
        signal(SIGABRT) { _ in
            let stack = Thread.callStackSymbols.joined(separator: "\n")
            if let data = "CRASH (abort)\n\(stack)\n".data(using: .utf8),
               let file = FileHandle(forWritingAtPath: "/tmp/needle-stress.log") {
                file.seekToEndOfFile()
                file.write(data)
                try? file.synchronize()
            }
            _exit(134)
        }
        run(minutes: minutes, music: music)
    }

    static func run(minutes: Double, music: String?) {
        Task { @MainActor in
            let store = Store.shared
            note("asked for \(minutes) min")
            try? await Task.sleep(for: .seconds(3))
            // Started from a terminal, macOS may not open the main window: open it from the
            // Window menu, as a person would.
            if (window?.frame.width ?? 0) < 400, let menu = NSApp.mainMenu?.item(withTitle: "Window")?.submenu,
               let item = menu.items.first(where: { $0.title == "Needle" }), let index = menu.items.firstIndex(of: item) {
                NSApp.activate(ignoringOtherApps: true)
                menu.performActionForItem(at: index)
                try? await Task.sleep(for: .seconds(2))
            }
            if let window, window.frame.width >= 400 {
                window.setContentSize(NSSize(width: 1280, height: 820))
                window.center()
            }
            note("windows: \(NSApp.windows.map { "\($0.identifier?.rawValue ?? "-") \($0.isVisible) \(Int($0.frame.width))x\(Int($0.frame.height))" })")
            store.setVolume(0)
            if let music, ((try? store.core.folders()) ?? []).isEmpty {
                note("adding \(music)")
                store.welcome = false
                store.set(["welcomed": true, "crash_reports": false])
                _ = await store.ask { try $0.addFolder(path: music) }
                store.core.rescan()
                for _ in 0..<120 {
                    try? await Task.sleep(for: .seconds(1))
                    let count = await store.load { try $0.songCount() } ?? 0
                    if count > 0 && !store.scan.running { note("read \(count) songs"); break }
                }
                store.libraryChanged()
            }
            let albums = JSON.decode([AlbumSummary].self, await store.load { try $0.albumsJson(query: "") }) ?? []
            let artists = JSON.decode([ArtistSummary].self, await store.load { try $0.artistsJson(query: "") }) ?? []
            note("start: \(albums.count) albums, \(artists.count) artists, \(store.playlists.count) playlists")
            let fields = ["title", "artist", "album", "year", "duration", "added_at", "play_count"]
            let albumThenLeave = CommandLine.arguments.contains("--stress-album-leave")
            let end = Date().addingTimeInterval(minutes * 60)
            var step = 0
            while Date() < end {
                step += 1
                let roll = Int.random(in: 0..<100)
                switch roll {
                case _ where roll < 30 && albumThenLeave:
                    // As in the crash's video: an album's page, then a click in the sidebar
                    // while the page's colours are still coming in.
                    // Or a new song, whose cover's colours come in the same way.
                    if Bool.random(), let a = albums.randomElement() {
                        note("\(step) album then leave: \(a.album)")
                        store.navigate(a.route)
                    } else {
                        note("\(step) next song then leave")
                        if store.playback?.current == nil, let a = albums.randomElement() {
                            store.playPage(rule: store.expression(for: a.route), playlist: nil, shuffle: true)
                        } else {
                            store.next()
                        }
                    }
                    try? await Task.sleep(for: .milliseconds(Int.random(in: 50...900)))
                    click(sidebar: true, double: false)
                case 0..<30: click(sidebar: true, double: false)
                case 30..<50: click(sidebar: false, double: false)
                case 50..<58: click(sidebar: false, double: true)
                case 58..<70:
                    var routes: [Route] = [.home, .songs, .albums, .artists, .favorites, .recent, .history, .folders]
                    routes += store.playlists.map { .playlist($0.id) }
                    if let a = albums.randomElement() { routes.append(a.route) }
                    if let a = artists.randomElement() { routes.append(.artist(a.name)) }
                    let to = routes.randomElement()!
                    note("\(step) navigate \(to)")
                    store.navigate(to)
                case 70..<76: note("\(step) back"); store.goBack()
                case 76..<79: note("\(step) forward"); store.goForward()
                case 79..<85:
                    let field = fields.randomElement()!
                    note("\(step) sort \(field)")
                    store.sort = (field, Bool.random())
                    store.libraryChanged()
                case 85..<89: note("\(step) library changed"); store.libraryChanged()
                case 89..<92:
                    if let a = albums.randomElement() {
                        note("\(step) play album \(a.album)")
                        store.playPage(rule: store.expression(for: a.route), playlist: nil, shuffle: Bool.random())
                    }
                case 92..<94: note("\(step) next song"); store.next()
                case 94..<96: note("\(step) big player"); store.big.toggle()
                case 96..<98:
                    note("\(step) fold sidebar")
                    store.setLayout(["sidebar_hidden": !store.settings.layout.sidebarHidden])
                default: note("\(step) escape"); key(53)
                }
                if step % 25 == 0 { store.setVolume(0) }
                if step % 10 == 0 { note("  page now \(store.route)") }
                try? await Task.sleep(for: .milliseconds(Int.random(in: 15...300)))
            }
            note("done after \(step) steps")
            NSApp.terminate(nil)
        }
    }

    private static var window: NSWindow? {
        // The main window has no identifier here: the biggest window that shows.
        NSApp.windows.filter { $0.isVisible && !($0 is NSPanel) }.max { $0.frame.width * $0.frame.height < $1.frame.width * $1.frame.height }
    }

    /// A click at a random place: in the sidebar's column, or in the page (away from the title
    /// bar, the window's buttons, and the player bar with its volume).
    private static func click(sidebar: Bool, double: Bool) {
        guard let window, let view = window.contentView else { return }
        let size = view.bounds.size
        let x = sidebar ? CGFloat.random(in: 16...200) : CGFloat.random(in: 260...max(261, size.width - 20))
        let y = sidebar ? size.height - CGFloat.random(in: 70...340) : CGFloat.random(in: 110...max(111, size.height - 60))
        note("click \(sidebar ? "sidebar" : "page") \(double ? "double " : "")at \(Int(x)),\(Int(y))")
        for count in 1...(double ? 2 : 1) {
            for type in [NSEvent.EventType.leftMouseDown, .leftMouseUp] {
                if let event = NSEvent.mouseEvent(with: type, location: NSPoint(x: x, y: y), modifierFlags: [],
                                                  timestamp: ProcessInfo.processInfo.systemUptime,
                                                  windowNumber: window.windowNumber, context: nil,
                                                  eventNumber: 0, clickCount: count, pressure: type == .leftMouseDown ? 1 : 0) {
                    NSApp.postEvent(event, atStart: false)
                }
            }
        }
    }

    private static func key(_ code: UInt16) {
        guard let window else { return }
        for type in [NSEvent.EventType.keyDown, .keyUp] {
            if let event = NSEvent.keyEvent(with: type, location: .zero, modifierFlags: [],
                                            timestamp: ProcessInfo.processInfo.systemUptime,
                                            windowNumber: window.windowNumber, context: nil,
                                            characters: "\u{1b}", charactersIgnoringModifiers: "\u{1b}",
                                            isARepeat: false, keyCode: code) {
                NSApp.postEvent(event, atStart: false)
            }
        }
    }
}
#endif
