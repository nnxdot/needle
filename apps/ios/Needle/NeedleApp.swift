import SwiftUI

@main
struct NeedleApp: App {
    @State private var store = Store()
    @Environment(\.scenePhase) private var phase

    var body: some Scene {
        WindowGroup {
            RootView()
                .environment(store)
                .onOpenURL { store.open($0) }
        }
        .onChange(of: phase) { _, now in
            // Songs added in the Files app or from a computer while Needle was away.
            if now == .active { store.rescan() }
        }
    }
}

enum Place: Hashable {
    case home, library, sources, search
}

struct RootView: View {
    @Environment(Store.self) private var store
    @State private var place = Place.home
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
            Tab("Sources", systemImage: "globe", value: .sources) {
                NavigationStack(path: path(.sources)) { SourcesView().routes() }
            }
            Tab(value: .search, role: .search) {
                NavigationStack(path: path(.search)) { SearchView().routes() }
            }
        }
        .tabBarMinimizeBehavior(.onScrollDown)
        #if DEBUG
        // For testing from a shell: -tab library|sources|search, and -albums opens Albums there.
        .onAppear {
            let args = CommandLine.arguments
            if let i = args.firstIndex(of: "-tab"), args.indices.contains(i + 1) {
                place = ["library": .library, "sources": .sources, "search": .search][args[i + 1]] ?? .home
            }
            if args.contains("-albums") { paths[place] = NavigationPath([Route.albums]) }
            // -artist <name>, -album <title>: open that page.
            let value = { (flag: String) in args.firstIndex(of: flag).flatMap { args.indices.contains($0 + 1) ? args[$0 + 1] : nil } }
            if let name = value("-artist") { store.opening = .artist(name) }
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
        .fullScreenCover(isPresented: Binding(get: { !store.app.welcomed }, set: { _ in })) {
            WelcomeView { where_, route in go(where_, route) }
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
