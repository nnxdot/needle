import SwiftUI

@main
struct NeedleApp: App {
    @State private var store = Store()
    @Environment(\.scenePhase) private var phase

    var body: some Scene {
        WindowGroup {
            RootView()
                .environment(store)
                .tint(.accentColor)
                .preferredColorScheme(.dark)
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
    @Namespace private var player

    var body: some View {
        @Bindable var store = store
        TabView(selection: $place) {
            Tab("Home", systemImage: "house.fill", value: .home) {
                NavigationStack { HomeView().routes() }
            }
            Tab("Library", systemImage: "square.stack.fill", value: .library) {
                NavigationStack { LibraryView().routes() }
            }
            Tab("Sources", systemImage: "globe", value: .sources) {
                NavigationStack { SourcesView().routes() }
            }
            Tab(value: .search, role: .search) {
                NavigationStack { SearchView().routes() }
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
        }
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
    }
}
