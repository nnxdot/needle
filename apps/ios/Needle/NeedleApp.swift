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
        }
        .fullScreenCover(isPresented: Binding(get: { !store.app.welcomed }, set: { _ in })) {
            WelcomeView { where_, route in go(where_, route) }
        }
        .sheet(item: Binding(get: { store.showPlayer ? nil : store.sheet }, set: { store.sheet = $0 })) { SheetView(sheet: $0) }
        .overlay(alignment: .top) { Toast().animation(.spring, value: store.message) }
        .alert(store.question?.title ?? "", isPresented: Binding(get: { store.question != nil }, set: { if !$0, let q = store.question { store.answer(q, with: nil) } })) {
            if let q = store.question {
                TextField("", text: Binding(get: { store.question?.answer ?? "" }, set: { store.question?.answer = $0 }))
                Button("OK") { store.answer(store.question ?? q, with: store.question?.answer ?? "") }
                Button("Cancel", role: .cancel) { store.answer(q, with: nil) }
            }
        } message: {
            Text(store.question?.prompt ?? "")
        }
        .onChange(of: store.opening) { _, route in
            guard let route else { return }
            store.opening = nil
            store.showPlayer = false
            go(place, route)
        }
        .tint(colors.accent)
        .preferredColorScheme(colors.dark ? .dark : .light)
    }

    private func path(_ place: Place) -> Binding<NavigationPath> {
        Binding(get: { paths[place] ?? NavigationPath() }, set: { paths[place] = $0 })
    }

    private func go(_ where_: Place, _ route: Route?) {
        place = where_
        if let route {
            var next = paths[where_] ?? NavigationPath()
            next.append(route)
            paths[where_] = next
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
