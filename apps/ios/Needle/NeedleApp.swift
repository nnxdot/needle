import SwiftUI

@main
struct NeedleApp: App {
    @State private var launch: Launch

    init() {
        NeedleFonts.registerAll()
        _launch = State(initialValue: Launch.shared)
    }
    @Environment(\.scenePhase) private var phase

    var body: some Scene {
        WindowGroup {
            if let store = launch.store {
                RootView()
                    .readsWide()
                    #if DEBUG
                    // Test builds: -compact lays the window out as a narrow one (Split View,
                    // Slide Over), which the simulator cannot make from a shell.
                    .modifier(CompactTest())
                    #endif
                    // The text font chosen (Settings › Appearance › Text). Every `Font.needle`
                    // follows a change by itself, so the pages and their places stay.
                    .font(.needle(.body))
                    // What is open is this window's own; what plays is shared.
                    .modifier(PerWindow())
                    .environment(store)
                    .task {
                        WatchLink.shared.start(store)
                        store.moveOldTitleFont()
                        store.allowPictures()
                        if phase == .active { store.cameToFront() }
                    }
            } else {
                RecoveryView(launch: launch)
            }
        }
        // An iPad's keyboard: the Mac's commands and keys (its menu bar and the list ⌘ shows).
        .commands { NeedleCommands() }
        .onChange(of: phase) { _, now in
            guard let store = launch.store else { return }
            // Only here, where the listener sees it: a start in the background (a widget,
            // Control Center, Siri, the Lock Screen, the watch) plays and reads nothing.
            if now == .active { store.cameToFront() }
            // Ticks slowly while no one looks (the lock screen and the watch still follow).
            store.onScreen(now == .active)
            // The phone may end the app while it is away: the queue, the place, and what was
            // heard of a paused song are kept now.
            if now == .background {
                store.wentAway()
                store.core.persist()
            }
        }
    }
}

#if DEBUG
private struct CompactTest: ViewModifier {
    func body(content: Content) -> some View {
        if CommandLine.arguments.contains("-compact") {
            content.environment(\.horizontalSizeClass, .compact).environment(\.wide, false)
        } else {
            content
        }
    }
}
#endif

/// A window's own state (`WindowState`): made with the window, in front when it becomes the
/// key window, and the one its links and keyboard commands act in.
private struct PerWindow: ViewModifier {
    @Environment(Store.self) private var store
    @State private var window = WindowState()

    func body(content: Content) -> some View {
        content
            .environment(window)
            .focusedSceneValue(\.windowState, window)
            .onOpenURL { store.open($0, in: window) }
            .background {
                WindowReader(found: { _ in }, becameKey: { store.windows.came(window) })
                    .frame(width: 0, height: 0)
            }
            .onAppear { store.windows.came(window) }
            .onDisappear { store.windows.went(window) }
    }
}

/// A tab, or, in an iPad's wide window, an entry of the sidebar.
enum Place: Hashable {
    case home, library, radio, search
    /// The sidebar's own entries: a part of the Library, a playlist (by its id), Settings.
    case part(Route)
    case playlist(String)
    case settings
}

struct RootView: View {
    @Environment(Store.self) private var store
    @Environment(WindowState.self) private var window
    @Environment(\.wide) private var wide
    @State private var place = Place.home
    /// Test builds: -show-welcome shows the welcome guide once, with nothing reset.
    @State private var welcomeTest = false
    @State private var paths: [Place: NavigationPath] = [:]
    /// The playlists, for the sidebar.
    @State private var playlists: [Playlist] = []
    @Namespace private var player

    /// The player floats over the page's foot as its own bar (a wide window, iOS 26.1 on), in
    /// place of the tab bar's accessory.
    private var floats: Bool {
        guard wide else { return false }
        #if DEBUG
        // Test builds: -system-accessory keeps the tab bar's own accessory in a wide window.
        if CommandLine.arguments.contains("-system-accessory") { return false }
        #endif
        if #available(iOS 26.1, *) { return true }
        return false
    }

    var body: some View {
        @Bindable var window = window
        let colors = store.colors
        TabView(selection: Binding(get: { place }, set: { choose($0) })) {
            // Each tab its own stack, and its own space for covers zooming into their pages.
            Tab("Home", systemImage: "house.fill", value: .home) {
                stack(.home) { HomeView() }
            }
            Tab("Library", systemImage: "square.stack.fill", value: .library) {
                stack(.library) { LibraryView() }
            }
            // Radio, not Sources: stations made from songs are Needle's own, and were hidden in
            // songs' menus; where the music comes from is in Settings › Sources.
            Tab("Radio", systemImage: "dot.radiowaves.left.and.right", value: .radio) {
                stack(.radio) { RadioView() }
            }
            Tab(value: .search, role: .search) {
                stack(.search) { SearchView() }
            }
            // An iPad's sidebar, as the Mac's: the Library's parts, the playlists, and
            // Settings. In a narrow window they leave with the sidebar (the page open folds
            // back into the Library's tab), and the tab bar is the iPhone's four.
            if WideReader.isPad && wide {
                TabSection("Library") {
                    ForEach(LibraryView.parts, id: \.2) { name, symbol, route in
                        Tab(name, systemImage: symbol, value: Place.part(route)) {
                            stack(.part(route)) { RoutePage(route: route) }
                        }
                        .tabPlacement(.sidebarOnly)
                    }
                }
                // Only in the sidebar: the tabs at the top keep the four.
                .tabPlacement(.sidebarOnly)
                .defaultVisibility(.hidden, for: .tabBar)
                TabSection("Playlists") {
                    ForEach(playlists) { playlist in
                        // Songs and albums dragged onto a playlist go into it; never onto a
                        // smart one, nor one read-only on the server.
                        if playlist.smart || playlist.readOnly {
                            playlistTab(playlist)
                        } else {
                            playlistTab(playlist)
                                .dropDestination(for: String.self) { items in add(items, to: playlist) }
                        }
                    }
                }
                .sectionActions {
                    Button("New Playlist", systemImage: "plus") { window.newPlaylist = true }
                }
                // Only in the sidebar: the tabs at the top keep the four.
                .tabPlacement(.sidebarOnly)
                .defaultVisibility(.hidden, for: .tabBar)
                Tab("Settings", systemImage: "gearshape", value: Place.settings) {
                    SettingsSplit()
                }
                .tabPlacement(.sidebarOnly)
            }
        }
        .modifier(SidebarStyle())
        // The sidebar's chosen row and its light or dark, as the Mac's (`SidebarLook`).
        .background {
            if WideReader.isPad {
                SidebarLook(dark: wide ? window.pageDark[place] ?? colors.dark : colors.dark,
                            accent: .sidebarAccent(colors.accent), reduced: Motion.reduced)
            }
        }
        .tabBarMinimizeBehavior(.onScrollDown)
        // Every page fades softly under the bars at the top and under the tab bar, as Apple's
        // own apps do; a page can still ask for its own.
        .scrollEdgeEffectStyle(.soft, for: .all)
        #if DEBUG
        .onAppear { testArguments() }
        #endif
        .modifier(PlayerAccessory(floats: floats, space: player))
        .environment(\.playerSpace, player)
        // With Reduce motion the player comes and goes at once, without the zoom.
        .transaction(value: window.showPlayer) { if Motion.reduced { $0.disablesAnimations = true } }
        .fullScreenCover(isPresented: $window.showPlayer) {
            NowPlayingView()
                .modifier(PlayerZoom(space: player))
                .modifier(WindowCommands(window: window, actions: actions))
                // In a wide window the player's tools are popovers from their buttons.
                .sheet(item: Binding(get: { wide && window.sheet?.isTool == true ? nil : window.sheet }, set: { window.sheet = $0 })) { SheetView(sheet: $0).modifier(WindowCommands(window: window, actions: actions)) }
                .overlay(alignment: .top) { Toast().animation(Motion.fade(.spring), value: store.message) }
                .modifier(QuestionAlert(shown: true))
        }
        .fullScreenCover(isPresented: Binding(get: { !store.app.welcomed || welcomeTest || window.welcomeAgain },
                                              set: { if !$0 { welcomeTest = false; window.welcomeAgain = false } })) {
            WelcomeView { where_, route in welcomeTest = false; window.welcomeAgain = false; go(where_, route) }
        }
        .sheet(item: Binding(get: { window.showPlayer || window.showSettings ? nil : window.sheet }, set: { window.sheet = $0 })) {
            SheetView(sheet: $0).modifier(WindowCommands(window: window, actions: actions))
        }
        // Settings, in a wide window: a large sheet of two columns, as the Mac's window. Its
        // own sheets, messages, and questions show over it.
        .sheet(isPresented: $window.showSettings) {
            SettingsSplit()
                .presentationSizing(.page)
                .modifier(WindowCommands(window: window, actions: actions))
                .sheet(item: Binding(get: { window.showPlayer ? nil : window.sheet }, set: { window.sheet = $0 })) { SheetView(sheet: $0).modifier(WindowCommands(window: window, actions: actions)) }
                .overlay(alignment: .top) { Toast().animation(Motion.fade(.spring), value: store.message) }
                .modifier(QuestionAlert(shown: !window.showPlayer))
        }
        .sheet(isPresented: $window.newPlaylist) {
            NavigationStack { PlaylistEditor {} }.presentationSizing(.form).modifier(WindowCommands(window: window, actions: actions))
        }
        .overlay(alignment: .top) { Toast().animation(Motion.fade(.spring), value: store.message) }
        .modifier(QuestionAlert(shown: !window.showPlayer && !window.showSettings))
        .onChange(of: window.opening) { _, route in
            guard let route else { return }
            window.opening = nil
            window.showPlayer = false
            window.showSettings = false
            go(place, route)
        }
        // Narrowed (Split View, Slide Over): the sidebar's own entries fold back into the
        // Library's tab, with the page that was open.
        .onChange(of: wide) { _, now in if !now { foldBack() } }
        .task(id: store.libraryVersion) {
            guard WideReader.isPad else { return }
            playlists = await store.load { try $0.playlists() } ?? playlists
            // A playlist open in the sidebar left the library (deleted, or taken away by a
            // sync): the list of playlists shows instead.
            if case .playlist(let id) = place, !playlists.contains(where: { $0.id == id }) { place = .part(.playlists) }
        }
        .focusedSceneValue(\.windowActions, actions)
        .grain(store.app.grain)
        #if DEBUG
        // Test builds with -stub-player: the presses that reached the player, for a UI test.
        .overlay(alignment: .bottomLeading) {
            if Store.stubbed {
                Text("presses:" + store.stubPresses.joined(separator: ",") + (Typing.shared.active ? ",typing" : ""))
                    .font(.system(size: 2)).opacity(0.05).allowsHitTesting(false)
                    .accessibilityIdentifier("test-presses")
            }
        }
        #endif
        .tint(colors.accent)
        .preferredColorScheme(colors.dark ? .dark : .light)
    }

    /// What the keyboard's commands do in this window.
    private var actions: WindowActions {
        WindowActions(go: { go($0, nil) }, back: back, search: { place = .search })
    }

    /// A playlist in the sidebar: its name, and the server's mark (always on a read-only one).
    private func playlistTab(_ playlist: Playlist) -> some TabContent<Place> {
        let symbol = playlist.smart ? "gearshape.2" : playlist.server && (store.app.serverMark || playlist.readOnly) ? "server.rack" : "music.note.list"
        return Tab(value: Place.playlist(playlist.id)) {
            stack(.playlist(playlist.id)) { PlaylistView(playlist: playlist) }
        } label: {
            Label(playlist.name, systemImage: symbol)
        }
        .badge(playlist.readOnly ? Text(Image(systemName: "lock.fill")) : nil)
        .accessibilityValue(playlist.readOnly ? Text("Read-only on the server") : Text(""))
        .tabPlacement(.sidebarOnly)
    }

    /// A tab's stack of pages, with the floating player at its foot in a wide window.
    private func stack<Root: View>(_ at: Place, @ViewBuilder root: @escaping () -> Root) -> some View {
        TabStack(path: path(at), root: root)
            // Over a page in its picture's colours, the bar takes the page's dark or light.
            .modifier(FloatingPlayerBar(shown: floats, source: at == place, space: player, dark: window.pageDark[at] ?? store.colors.dark))
            .environment(\.place, at)
    }

    /// Settings opens as its sheet, over the page that was open.
    private func choose(_ next: Place) {
        if next == .settings {
            window.showSettings = true
        } else {
            place = next
        }
    }

    /// Songs and albums dropped onto a playlist in the sidebar go into it.
    private func add(_ items: [String], to playlist: Playlist) {
        guard items.contains(where: SongDrag.isNeedle) else { return }
        let id = playlist.id
        Task {
            let ids = await SongDrag.songs(in: items, store: store)
            guard !ids.isEmpty, let saved = await store.ask({ try $0.addToPlaylist(id: id, songs: ids) }) else { return }
            store.told(saved, done: ids.count == 1 ? "Added to “\(playlist.name)”" : "Added \(count(ids.count, "song")) to “\(playlist.name)”")
            store.libraryChanged()
        }
    }

    /// The sidebar's own entries, in a window that has become narrow: the Library's tab, with
    /// the same page open in it.
    private func foldBack() {
        switch place {
        case .part(let route):
            paths[.library] = NavigationPath([route])
            place = .library
        case .playlist(let id):
            if let playlist = playlists.first(where: { $0.id == id }) {
                paths[.library] = NavigationPath([Route.playlists, Route.playlist(playlist)])
            }
            place = .library
        case .settings:
            place = .home
        default: break
        }
    }

    /// Back, from the keyboard (⌘[): the page under the one on top, in the place open.
    private func back() {
        guard var path = paths[place], !path.isEmpty else { return }
        path.removeLast()
        paths[place] = path
        opened[place] = nil
    }

    /// The page last opened in each place from a link, and how deep it was.
    @State private var opened: [Place: (Int, Route)] = [:]

    private func path(_ place: Place) -> Binding<NavigationPath> {
        // Back, or a page opened inside: what was opened from a link is no longer on top.
        Binding(get: { paths[place] ?? NavigationPath() }, set: { paths[place] = $0; opened[place] = nil })
    }

    private func go(_ where_: Place, _ route: Route?) {
        if where_ == .settings { window.showSettings = true; return }
        place = where_
        if let route {
            var next = paths[where_] ?? NavigationPath()
            // Already the page on top (opened from it again): nothing new.
            if let top = opened[where_], top.0 == next.count, top.1 == route { return }
            next.append(route)
            paths[where_] = next
            opened[where_] = (next.count, route)
        }
    }

    #if DEBUG
    /// For testing from a shell.
    private func testArguments() {
        let args = CommandLine.arguments
        // -tab library|radio|search, and on an iPad -place albums|songs|artists|… (the sidebar's
        // entries, by their route) or -place playlist:<name>.
        if let i = args.firstIndex(of: "-tab"), args.indices.contains(i + 1) {
            place = ["library": .library, "radio": .radio, "search": .search][args[i + 1]] ?? .home
        }
        let value = { (flag: String) in args.firstIndex(of: flag).flatMap { args.indices.contains($0 + 1) ? args[$0 + 1] : nil } }
        if let name = value("-place") {
            let parts: [String: Route] = ["playlists": .playlists, "artists": .artists, "albums": .albums, "songs": .songs, "genres": .genres,
                                          "favorites": .favorites, "recent": .recent, "folders": .folders, "history": .history, "numbers": .numbers]
            if let route = parts[name] { place = .part(route) }
            if name.hasPrefix("playlist:") {
                let wanted = String(name.dropFirst("playlist:".count))
                Task {
                    if let found = await store.load({ try $0.playlists().first { $0.name == wanted } }) ?? nil {
                        try? await Task.sleep(for: .seconds(1))
                        place = .playlist(found.id)
                    }
                }
            }
        }
        // -settings [route]: Settings' sheet in a wide window, on that page.
        if let i = args.firstIndex(of: "-settings") {
            if args.indices.contains(i + 1), !args[i + 1].hasPrefix("-") { UserDefaults.standard.set(args[i + 1], forKey: "settings-page") }
            window.showSettings = true
        }
        // -orient landscape|portrait: turns the window, as if the iPad were turned.
        if let orient = value("-orient") {
            Task {
                try? await Task.sleep(for: .seconds(1))
                let scene = UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }.first
                scene?.requestGeometryUpdate(.iOS(interfaceOrientations: orient == "portrait" ? .portrait : .landscapeRight)) { error in
                    try? "\(error)".write(toFile: NSTemporaryDirectory() + "needle-orient.log", atomically: true, encoding: .utf8)
                }
            }
        }
        // -fake-landscape: the simulator's windowed mode refuses a turn, so the window itself
        // is turned inside the upright screen and laid out at the screen's landscape size (the
        // system's status bar stays where it was). -window-width <points>: the window asks for
        // that width at most, to see a narrow window.
        if args.contains("-fake-landscape") || value("-window-width") != nil {
            Task {
                try? await Task.sleep(for: .seconds(1))
                guard let scene = UIApplication.shared.connectedScenes.compactMap({ $0 as? UIWindowScene }).first else { return }
                if let width = value("-window-width").flatMap(Double.init) {
                    scene.sizeRestrictions?.minimumSize = CGSize(width: width, height: 400)
                    scene.sizeRestrictions?.maximumSize = CGSize(width: width, height: 4000)
                }
                if args.contains("-fake-landscape") {
                    let screen = scene.screen.bounds
                    for window in scene.windows {
                        window.transform = CGAffineTransform(rotationAngle: .pi / 2)
                        window.bounds = CGRect(x: 0, y: 0, width: screen.height, height: screen.width)
                        window.center = CGPoint(x: screen.midX, y: screen.midY)
                    }
                }
            }
        }
        if let i = args.firstIndex(of: "-route"), args.indices.contains(i + 1),
           let route = ["settings": [Route.settings], "appearance": [.settings, .settingsAppearance], "sources": [.settings, .sources],
                         "scrobbling": [.settings, .settingsScrobbling], "library": [.settings, .settingsLibrary],
                         "playback": [.settings, .settingsPlayback], "server": [.settings, .server("subsonic")],
                         "whatsnew": [.settings, .whatsNew], "crate": [.albums, .crate], "numbers": [.numbers], "genremap": [.genreMap], "artists": [.artists], "year": [.wrapped],
                         "playerbuttons": [.settings, .settingsAppearance, .playerButtons],
                         "about": [.settings, .about], "sound": [.settings, .settingsSound], "fixlibrary": [.settings, .fixLibrary],
                         "missingtags": [.settings, .fixLibrary, .albumProblems], "playlists": [.playlists]][args[i + 1]] {
            // After the test turn (-fake-landscape), so the page opens at its size.
            let wait = args.contains("-fake-landscape") ? 2.5 : 0
            Task {
                try? await Task.sleep(for: .seconds(wait))
                paths[place] = NavigationPath(route)
            }
        }
        // -playlist <name> opens that playlist's page, from Playlists.
        if let name = value("-playlist") {
            Task {
                if let found = await store.load({ try $0.playlists().first { $0.name == name } }) ?? nil {
                    paths[place] = NavigationPath([Route.playlists, Route.playlist(found)])
                }
            }
        }
        if args.contains("-albums") { paths[place] = NavigationPath([Route.albums]) }
        if args.contains("-show-welcome") { welcomeTest = true }
        // -test-202: Shuffle stays on, Hide Various Artists-only, and Forget songs that are
        // gone, through what the buttons and rows call, into needle-202-check.log. Plays
        // nothing: the phone's own volume is always full here.
        if args.contains("-test-202") {
            Task {
                try? await Task.sleep(for: .seconds(8))
                var lines: [String] = []
                func pause() async { try? await Task.sleep(for: .seconds(5)) }
                let shuffled = { String(store.playback?.shuffle ?? false) }
                lines.append("song: \(store.playback?.current?.title ?? "none"), playing \(store.playback?.playing ?? false)")
                store.setShuffleStays(true)
                await pause()
                lines.append("stays on: \(store.choices.shuffleStays), shuffle \(shuffled()), lit \(store.shuffleOn)")
                store.shuffleButton()
                await pause()
                lines.append("after a press: shuffle \(shuffled()), lit \(store.shuffleOn)")
                store.shuffleButton()
                await pause()
                lines.append("after a second press: shuffle \(shuffled()), lit \(store.shuffleOn)")
                store.shuffleButton()
                await pause()
                lines.append("pressed on again: shuffle \(shuffled())")
                store.setShuffleStays(false)
                await pause()
                lines.append("setting off: stays \(store.choices.shuffleStays), shuffle \(shuffled()), lit \(store.shuffleOn)")
                lines.append("still not playing: \(!(store.playback?.playing ?? false))")
                let count = { () async -> Int in (await store.load { try $0.artistsBy(query: "", order: "name", albumArtistsOnly: false) } ?? []).count }
                store.updateChoices { $0.hideCompilationArtists = false }
                await pause()
                let all = await count()
                store.updateChoices { $0.hideCompilationArtists = true }
                await pause()
                lines.append("artists: \(all) shown, \(await count()) with Hide Various Artists-only")
                let letters = Set((await store.load { try $0.artistsBy(query: "", order: "name", albumArtistsOnly: false) } ?? []).map(\.letter))
                lines.append("letters: \(letters.sorted().joined(separator: " "))")
                store.updateChoices { $0.hideCompilationArtists = false }
                store.updateChoices { $0.forgetMissing = true }
                await pause()
                lines.append("forget missing saved: \((try? store.core.libraryChoices())?.forgetMissing ?? false)")
                store.updateChoices { $0.forgetMissing = false }
                await pause()
                lines.append("forget missing off: \((try? store.core.libraryChoices())?.forgetMissing ?? true)")
                try? await Task.sleep(for: .seconds(10))
                let late = try? store.core.libraryChoices()
                lines.append("10 s later: shuffle \(shuffled()), saved stays \(late?.shuffleStays ?? true), hide \(late?.hideCompilationArtists ?? true), forget \(late?.forgetMissing ?? true)")
                let log = NSTemporaryDirectory() + "needle-202-check.log"
                try? lines.joined(separator: "\n").write(toFile: log, atomically: true, encoding: .utf8)
            }
        }
        // -landscape: turns Needle on its side, as if the phone were turned, 25 s in (after
        // -show-player has opened the player and the app has settled).
        if args.contains("-landscape") {
            Task {
                try? await Task.sleep(for: .seconds(25))
                let scene = UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }.first
                scene?.requestGeometryUpdate(.iOS(interfaceOrientations: .landscapeRight))
            }
        }
        // -font-test <title font>: 8 s in, changes the title and text fonts (the text font
        // to Inter, or back to Plain with "system"), as Settings does, to see the page stay.
        if let font = value("-font-test") {
            Task {
                try? await Task.sleep(for: .seconds(8))
                store.updateChoices { $0.displayFont = font; $0.textFont = font == "system" ? "system" : "inter" }
            }
        }
        // -theme night|midnight|day: that look (kept, as Settings keeps it).
        if let theme = value("-theme") { store.updateApp { $0.theme = theme } }
        // -artist <name>, -album <title>: open that page.
        if let name = value("-artist") { window.opening = .artist(name) }
        // -sleep <minutes> starts the sleep timer, -speed <times> plays at that speed, and
        // -sheet sleep|speed|timing opens that sheet (timing: the song playing), a few seconds in.
        Task {
            try? await Task.sleep(for: .seconds(3))
            if let minutes = value("-sleep").flatMap(UInt32.init) { store.startSleep("minutes", minutes: minutes) }
            if let speed = value("-speed").flatMap(Float.init) {
                var pace = store.tempo
                pace.speed = speed
                store.setTempo(pace)
            }
            try? await Task.sleep(for: .seconds(1))
            switch value("-sheet") {
            case "sleep": window.sheet = .sleep
            case "speed": window.sheet = .speed
            case "speakers": window.sheet = .speakers
            case "timing": if let song = store.playback?.current { window.sheet = .timing(song) }
            case "add": if let song = store.playback?.current { window.sheet = .addToPlaylist([song]) }
            default: break
            }
        }
        // -check-moving <album title>: asks the core for that album's moving cover (its first
        // song's), plays nothing, and writes what came back to needle-moving-check.log.
        if let title = value("-check-moving") {
            Task {
                let started = Date()
                let found: [String] = await store.load { core in
                    guard let album = try core.albums().first(where: { $0.title == title }),
                          let first = try core.albumSongs(key: album.key).first else { return ["no such album"] }
                    return [album.artist, first.id, core.animatedCover(id: first.id) ?? ""]
                } ?? ["failed"]
                var line = "\(title): " + found.joined(separator: " | ")
                line += String(format: " | %.1f s", Date().timeIntervalSince(started))
                if found.count == 3, !found[2].isEmpty {
                    let path = found[2]
                    let frames = await Task.detached { AnimatedImage.Frames.load(URL(fileURLWithPath: path)) }.value
                    line += " | \(frames?.count ?? 1) frames, \(String(format: "%.2f", frames?.total ?? 0)) s a loop"
                }
                try? (line + "\n").write(toFile: NSTemporaryDirectory() + "needle-moving-check.log", atomically: true, encoding: .utf8)
            }
        }
        if let title = value("-album") {
            Task {
                try? await Task.sleep(for: .seconds(1))
                if let album = await store.load({ try $0.albums().first { $0.title == title } }) ?? nil { window.opening = .album(album) }
            }
        }
    }
    #endif
}

/// On an iPad, the system's sidebar of floating glass in a wide window, which turns back into
/// the tab bar in a narrow one; an iPhone keeps its tab bar.
private struct SidebarStyle: ViewModifier {
    func body(content: Content) -> some View {
        if WideReader.isPad {
            // The sidebar shows at first, as the Mac's; its button hides it (and the tabs then
            // float at the top).
            content.tabViewStyle(.sidebarAdaptable).defaultAdaptableTabBarPlacement(Self.start)
        } else {
            content
        }
    }

    /// The sidebar at the start on an iPad on its side, where it sits beside the page; held
    /// upright it would cover the page, so the tabs start at the top there.
    private static var start: AdaptableTabBarPlacement {
        #if DEBUG
        // Test builds: -sidebar-hidden starts with the tabs at the top.
        if CommandLine.arguments.contains("-sidebar-hidden") { return .tabBar }
        if CommandLine.arguments.contains("-fake-landscape") { return .sidebar }
        #endif
        let scene = UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }.first
        return scene?.effectiveGeometry.interfaceOrientation.isLandscape == true ? .sidebar : .tabBar
    }
}

/// The mini player on the tab bar, as the iPhone's; in a wide window the player floats over
/// the page's foot instead (`FloatingPlayerBar`).
private struct PlayerAccessory: ViewModifier {
    @Environment(Store.self) private var store
    @Environment(WindowState.self) private var window
    let floats: Bool
    let space: Namespace.ID

    func body(content: Content) -> some View {
        if #available(iOS 26.1, *) {
            content.tabViewBottomAccessory(isEnabled: !floats) { bar }
        } else {
            content.tabViewBottomAccessory { bar }
        }
    }

    private var bar: some View {
        MiniPlayer { if store.playback?.current != nil { window.showPlayer = true } }
            .matchedTransitionSource(id: "player", in: space)
            .contentShape(.rect)
            .onTapGesture { if store.playback?.current != nil { window.showPlayer = true } }
    }
}

/// The keyboard's commands act in this window from what is presented in it too (the player,
/// Settings, a sheet): a presentation is a part of its own, which the window's values do not
/// reach.
private struct WindowCommands: ViewModifier {
    let window: WindowState
    let actions: WindowActions
    @Environment(\.dismiss) private var dismiss

    func body(content: Content) -> some View {
        content
            .focusedSceneValue(\.windowState, window)
            .focusedSceneValue(\.windowActions, actions)
            // Esc closes it, as Done, Cancel, or a swipe down do.
            .background { EscapeKey { dismiss() }.frame(width: 0, height: 0).accessibilityHidden(true) }
    }
}

/// What the keyboard's commands do in a window: go to a place, back, and search.
struct WindowActions {
    let go: (Place) -> Void
    let back: () -> Void
    let search: () -> Void
}

extension FocusedValues {
    @Entry var windowActions: WindowActions?
}

extension EnvironmentValues {
    /// Where the player zooms from: the mini player, or the floating bar.
    @Entry var playerSpace: Namespace.ID?
}

/// The player grows out of the mini player; with Reduce motion (the iPhone's or Needle's) it
/// does not.
private struct PlayerZoom: ViewModifier {
    @Environment(Store.self) private var store
    @Environment(\.accessibilityReduceMotion) private var systemReduceMotion
    let space: Namespace.ID

    func body(content: Content) -> some View {
        if systemReduceMotion || store.app.reduceMotion {
            content
        } else {
            content.navigationTransition(.zoom(sourceID: "player", in: space))
        }
    }
}

/// The sheets any screen can ask for.
struct SheetView: View {
    let sheet: Store.Sheet

    var body: some View {
        NavigationStack {
            switch sheet {
            case .addToPlaylist(let songs): AddToPlaylistView(songs: songs)
            case .tags(let ids): TagEditorView(ids: ids)
            case .identify(let id): IdentifyView(id: id)
            case .timing(let song): TimingView(song: song)
            case .speakers: SpeakersView()
            case .sleep: SleepSheet()
            case .speed: SpeedSheet()
            case .whatsNew: WhatsNewSheet()
            }
        }
        // As on the pages: a soft fade under the sheet's own bar.
        .scrollEdgeEffectStyle(.soft, for: .all)
        // Laid out by the sheet's own width (a form on an iPad is narrow).
        .readsWide()
        // A wide window shows it as a form in the middle, not stretched across the screen.
        .formSized()
    }
}

extension Store.Sheet {
    /// The player's small tools: in a wide window popovers from their buttons.
    var isTool: Bool {
        switch self {
        case .sleep, .speed, .speakers: true
        default: false
        }
    }
}

extension View {
    /// In a wide window, a sheet of the form's size in the middle of the screen; on an iPhone
    /// and in a narrow window, the sheet as it was.
    func formSized() -> some View {
        modifier(FormSized())
    }

    /// In a wide window, the player's tools named here (by their sheet's id) open as popovers
    /// from this button, not as sheets.
    func toolPopover(_ ids: Set<String>) -> some View {
        modifier(ToolPopover(ids: ids))
    }
}

private struct FormSized: ViewModifier {
    @Environment(\.wide) private var wide

    func body(content: Content) -> some View {
        if wide { content.presentationSizing(.form) } else { content }
    }
}

private struct ToolPopover: ViewModifier {
    @Environment(Store.self) private var store
    @Environment(WindowState.self) private var window
    @Environment(\.wide) private var wide
    let ids: Set<String>

    func body(content: Content) -> some View {
        content.popover(item: Binding(get: { wide ? window.sheet.flatMap { ids.contains($0.id) ? $0 : nil } : nil },
                                      set: { window.sheet = $0 })) { sheet in
            Group {
                switch sheet {
                case .sleep: SleepSheet().frame(width: 380).fixedSize(horizontal: false, vertical: true)
                case .speed: SpeedSheet().frame(width: 400, height: 430)
                default: NavigationStack { SpeakersView() }.frame(width: 420, height: 520)
                }
            }
            .presentationCompactAdaptation(.popover)
            .focusedSceneValue(\.windowState, window)
            .background { EscapeKey { window.sheet = nil }.frame(width: 0, height: 0).accessibilityHidden(true) }
        }
    }
}

/// A plugin's question, answered with text: over the page, or over the player when it is open
/// (an alert under the player could not show).
struct QuestionAlert: ViewModifier {
    @Environment(Store.self) private var store
    let shown: Bool

    func body(content: Content) -> some View {
        content.alert(store.question?.title ?? "", isPresented: Binding(get: { shown && store.question != nil }, set: { if !$0, let q = store.question { store.answer(q, with: nil) } })) {
            if let q = store.question {
                TextField("", text: Binding(get: { store.question?.answer ?? "" }, set: { store.question?.answer = $0 }))
                Button("OK") { store.answer(store.question ?? q, with: store.question?.answer ?? "") }
                Button("Cancel", role: .cancel) { store.answer(q, with: nil) }
            }
        } message: {
            Text(store.question?.prompt ?? "")
        }
    }
}

