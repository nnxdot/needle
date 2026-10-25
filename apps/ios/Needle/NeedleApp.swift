import SwiftUI

@main
struct NeedleApp: App {
    @State private var store = Store.main

    init() {
        NeedleFonts.registerAll()
    }
    @Environment(\.scenePhase) private var phase

    var body: some Scene {
        WindowGroup {
            RootView()
                // The text font chosen (Settings › Appearance › Text): drawn anew when it changes.
                .font(.needle(.body))
                .id("\(store.choices.displayFont)|\(store.choices.textFont)")
                .environment(store)
                .onOpenURL { store.open($0) }
                .task {
                    WatchLink.shared.start(store)
                    store.allowPictures()
                }
        }
        .onChange(of: phase) { _, now in
            // Songs added in the Files app or from a computer while Needle was away.
            if now == .active { store.rescan() }
            // The phone may end the app while it is away: the queue, the place, and what was
            // heard of a paused song are kept now.
            if now == .background { store.core.persist() }
        }
    }
}

enum Place: Hashable {
    case home, library, radio, search
}

struct RootView: View {
    @Environment(Store.self) private var store
    @State private var place = Place.home
    /// Test builds: -show-welcome shows the welcome guide once, with nothing reset.
    @State private var welcomeTest = false
    @State private var paths: [Place: NavigationPath] = [:]
    @Namespace private var player

    var body: some View {
        @Bindable var store = store
        let colors = store.colors
        TabView(selection: $place) {
            Tab("Home", systemImage: "house.fill", value: .home) {
                NavigationStack(path: path(.home)) { HomeView().routes() }
            }
            Tab("Library", systemImage: "square.stack.fill", value: .library) {
                NavigationStack(path: path(.library)) { LibraryView().routes() }
            }
            // Radio, not Sources: stations made from songs are Needle's own, and were hidden in
            // songs' menus; where the music comes from is in Settings › Sources.
            Tab("Radio", systemImage: "dot.radiowaves.left.and.right", value: .radio) {
                NavigationStack(path: path(.radio)) { RadioView().routes() }
            }
            Tab(value: .search, role: .search) {
                NavigationStack(path: path(.search)) { SearchView().routes() }
            }
        }
        .tabBarMinimizeBehavior(.onScrollDown)
        #if DEBUG
        // For testing from a shell: -tab library|radio|search, -albums opens Albums there, and
        // -route settings|appearance|sources opens that page.
        .onAppear {
            let args = CommandLine.arguments
            if let i = args.firstIndex(of: "-tab"), args.indices.contains(i + 1) {
                place = ["library": .library, "radio": .radio, "search": .search][args[i + 1]] ?? .home
            }
            if let i = args.firstIndex(of: "-route"), args.indices.contains(i + 1),
               let route = ["settings": [Route.settings], "appearance": [.settings, .settingsAppearance], "sources": [.settings, .sources],
                             "scrobbling": [.settings, .settingsScrobbling], "library": [.settings, .settingsLibrary],
                             "playback": [.settings, .settingsPlayback], "server": [.settings, .server("subsonic")],
                             "whatsnew": [.settings, .whatsNew], "crate": [.albums, .crate], "numbers": [.numbers], "moodmap": [.moodMap], "artists": [.artists],
                             "playerbuttons": [.settings, .settingsAppearance, .playerButtons]][args[i + 1]] {
                paths[place] = NavigationPath(route)
            }
            if args.contains("-albums") { paths[place] = NavigationPath([Route.albums]) }
            if args.contains("-show-welcome") { welcomeTest = true }
            // -artist <name>, -album <title>: open that page.
            let value = { (flag: String) in args.firstIndex(of: flag).flatMap { args.indices.contains($0 + 1) ? args[$0 + 1] : nil } }
            if let name = value("-artist") { store.opening = .artist(name) }
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
                    if let album = await store.load({ try $0.albums().first { $0.title == title } }) ?? nil { store.opening = .album(album) }
                }
            }
        }
        #endif
        .tabViewBottomAccessory {
            MiniPlayer()
                .matchedTransitionSource(id: "player", in: player)
                .contentShape(.rect)
                .onTapGesture { if store.playback?.current != nil { store.showPlayer = true } }
        }
        .fullScreenCover(isPresented: $store.showPlayer) {
            NowPlayingView()
                .navigationTransition(.zoom(sourceID: "player", in: player))
                .sheet(item: $store.sheet) { SheetView(sheet: $0) }
                .overlay(alignment: .top) { Toast().animation(.spring, value: store.message) }
                .modifier(QuestionAlert(shown: true))
        }
        .fullScreenCover(isPresented: Binding(get: { !store.app.welcomed || welcomeTest }, set: { if !$0 { welcomeTest = false } })) {
            WelcomeView { where_, route in welcomeTest = false; go(where_, route) }
        }
        .sheet(item: Binding(get: { store.showPlayer ? nil : store.sheet }, set: { store.sheet = $0 })) { SheetView(sheet: $0) }
        .overlay(alignment: .top) { Toast().animation(.spring, value: store.message) }
        .modifier(QuestionAlert(shown: !store.showPlayer))
        .onChange(of: store.opening) { _, route in
            guard let route else { return }
            store.opening = nil
            store.showPlayer = false
            go(place, route)
        }
        .tint(colors.accent)
        .preferredColorScheme(colors.dark ? .dark : .light)
    }

    /// The page last opened in each place from a link, and how deep it was.
    @State private var opened: [Place: (Int, Route)] = [:]

    private func path(_ place: Place) -> Binding<NavigationPath> {
        // Back, or a page opened inside: what was opened from a link is no longer on top.
        Binding(get: { paths[place] ?? NavigationPath() }, set: { paths[place] = $0; opened[place] = nil })
    }

    private func go(_ where_: Place, _ route: Route?) {
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
            }
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
