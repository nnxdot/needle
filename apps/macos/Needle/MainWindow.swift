import SwiftUI
import UniformTypeIdentifiers

/// The main window: the sidebar, the page, the side panel (details, queue, lyrics), and the
/// player floating on glass at the foot; the big player over it all.
struct MainWindow: View {
    @Environment(Store.self) private var store
    @Environment(\.openWindow) private var openWindow
    @FocusState private var searchFocused: Bool
    @State private var suggestions: [Suggestion] = []

    var body: some View {
        @Bindable var store = store
        let colors = store.colors
        ZStack {
            NavigationSplitView(columnVisibility: Binding(
                get: { store.settings.layout.sidebarHidden && !store.settings.layout.compactSidebar ? .detailOnly : .all },
                set: { store.setLayout(["sidebar_hidden": $0 == .detailOnly]) })) {
                Sidebar()
                    .navigationSplitViewColumnWidth(min: rail ? 64 : 200, ideal: rail ? 64 : 224, max: rail ? 64 : 290)
            } detail: {
                detail
            }
            .modifier(SearchField(searchFocused: $searchFocused, suggestions: suggestions))
            .onChange(of: store.search) { _, text in findSuggestions(text) }
            .onChange(of: store.focusSearch) { _, _ in
                if store.settings.layout.searchHidden { store.palette = true } else { searchFocused = true }
            }

            if store.big || store.immersive {
                BigPlayer()
                    .transition(store.motion ? .move(edge: .bottom).combined(with: .opacity) : .opacity)
                    .zIndex(2)
            }
            if store.palette {
                CommandPalette()
                    .transition(.opacity.combined(with: .scale(scale: 0.98)))
                    .zIndex(3)
            }
            if store.settings.grain > 0.001 {
                Grain(amount: Double(store.settings.grain)).zIndex(4)
            }
        }
        .overlay(alignment: .bottom) { ToastView().padding(.bottom, store.big ? 0 : 110).zIndex(5) }
        .background { WindowBackground() }
        .animation(store.motion ? .smooth(duration: 0.28) : nil, value: store.big)
        .animation(store.motion ? .smooth(duration: 0.18) : nil, value: store.palette)
        .tint(colors.accent)
        .preferredColorScheme(colors.dark ? .dark : .light)
        .frame(minWidth: 900, minHeight: 620)
        .onDrop(of: [.fileURL], isTargeted: nil) { providers in
            drop(providers)
            return true
        }
        .sheet(item: $store.editor) { playlist in PlaylistEditor(playlist: playlist).environment(store) }
        .sheet(item: $store.addingTo) { playlist in AddSongs(playlist: playlist).environment(store) }
        .sheet(item: Binding(get: { store.tagging.map { TagSheet.Target(ids: $0) } }, set: { store.tagging = $0?.ids })) { target in
            TagSheet(ids: target.ids).environment(store)
        }
        .sheet(item: $store.question) { question in PluginQuestion(question: question).environment(store) }
        .sheet(isPresented: $store.welcome) { Welcome().environment(store) }
        .sheet(item: $store.whatsNew) { notes in WhatsNew(notes: notes).environment(store) }
    }

    private var rail: Bool { store.settings.layout.sidebarHidden && store.settings.layout.compactSidebar }

    @ViewBuilder
    private var detail: some View {
        let colors = store.colors
        PageView()
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            .background { if !store.settings.ambient { colors.canvas.ignoresSafeArea() } }
            .safeAreaInset(edge: .bottom, spacing: 0) {
                if !store.settings.layout.playerOnTop {
                    PlayerBar().padding(.horizontal, 14).padding(.bottom, 12).padding(.top, 4)
                }
            }
            .inspector(isPresented: Binding(get: { store.settings.showInspector }, set: { store.set(["show_inspector": $0]) })) {
                Inspector()
                    .inspectorColumnWidth(min: 280, ideal: 310, max: 380)
            }
            .toolbar { toolbar }
            .navigationTitle(title)
    }

    private var title: String {
        if case .playlist = store.route { return store.pagePlaylist?.name ?? "Playlist" }
        return store.route.title
    }

    @ToolbarContentBuilder
    private var toolbar: some ToolbarContent {
        ToolbarItemGroup(placement: .navigation) {
            Button { store.goBack() } label: { Image(systemName: "chevron.left") }
                .help("Back · ⌘[")
                .disabled(store.back.isEmpty)
            Button { store.goForward() } label: { Image(systemName: "chevron.right") }
                .help("Forward · ⌘]")
                .disabled(store.forward.isEmpty)
        }
        if store.settings.layout.playerOnTop {
            ToolbarItem(placement: .principal) {
                TopPlayer()
            }
        }
        ToolbarItem(placement: .primaryAction) {
            Button { store.palette = true } label: { Image(systemName: "command") }
                .help("Command palette: go anywhere, do anything · ⌘K")
        }
    }

    private func findSuggestions(_ text: String) {
        let core = store.core
        let units = UInt32(text.utf16.count)
        Task {
            let found = await Task.detached { core.suggest(input: text, cursor: units) }.value
            if store.search == text { suggestions = found }
        }
    }

    private func drop(_ providers: [NSItemProvider]) {
        var urls: [URL] = []
        let group = DispatchGroup()
        for provider in providers {
            group.enter()
            _ = provider.loadObject(ofClass: URL.self) { url, _ in
                if let url { DispatchQueue.main.async { urls.append(url) } }
                group.leave()
            }
        }
        group.notify(queue: .main) { store.open(urls) }
    }
}

/// The search field in the toolbar (unless hidden in Settings), with completions for rules.
private struct SearchField: ViewModifier {
    @Environment(Store.self) private var store
    var searchFocused: FocusState<Bool>.Binding
    let suggestions: [Suggestion]

    func body(content: Content) -> some View {
        @Bindable var store = store
        if store.settings.layout.searchHidden {
            content
        } else {
            content
                .searchable(text: $store.search, placement: .toolbar, prompt: "Search, or write a rule like  rating >= 4")
                .searchFocused(searchFocused)
                .searchSuggestions {
                    ForEach(suggestions, id: \.label) { s in
                        HStack {
                            Text(s.label)
                            Spacer()
                            Text(s.detail).foregroundStyle(.secondary)
                        }
                        .searchCompletion(completed(s))
                    }
                }
        }
    }

    private func completed(_ s: Suggestion) -> String {
        let text = store.search as NSString
        let start = min(Int(s.start), text.length), end = min(max(Int(s.end), start), text.length)
        return text.replacingCharacters(in: NSRange(location: start, length: end - start), with: s.insert)
    }
}

/// Behind the window: the playing cover, blurred, with Ambient background on.
struct WindowBackground: View {
    @Environment(Store.self) private var store

    var body: some View {
        if store.settings.ambient {
            ZStack {
                store.colors.chrome
                if let p = store.playback, p.hasSong {
                    CoverBackdrop(path: p.artwork, seed: p.album + p.artist, moving: store.motion)
                        .opacity(store.colors.dark ? 0.85 : 0.55)
                } else {
                    store.colors.glow.opacity(0.25)
                }
            }
            .ignoresSafeArea()
        }
    }
}

/// The page the window shows.
struct PageView: View {
    @Environment(Store.self) private var store
    @State private var total: UInt32?

    var body: some View {
        Group {
            switch store.route {
            case .home: library { HomePage() }
            case .albums: library { AlbumsPage() }
            case .artists: library { ArtistsPage() }
            case .artist(let name): ArtistPage(name: name)
            case .folders: FoldersPage()
            case .history: HistoryPage()
            case .wrapped(let year): WrappedPage(year: year)
            case .sound: ScrollView { SoundPane().padding(28).frame(maxWidth: 760) }.frame(maxWidth: .infinity)
            case .importing: ImportPage()
            case .timing: TimingPage()
            case .doctor: DoctorPage()
            default: library { SongsPage(route: store.route) }
            }
        }
        .id(store.route)
        .transition(.opacity)
        .task(id: store.libraryVersion) { total = await store.load { try $0.songCount() } }
    }

    /// Library pages, or the way in while the library is empty.
    @ViewBuilder
    private func library<Page: View>(@ViewBuilder _ page: () -> Page) -> some View {
        if total == 0 && !store.scan.running {
            Onboarding()
        } else {
            page()
        }
    }
}

/// An empty library: where music lives, and how to bring it in.
struct Onboarding: View {
    @Environment(Store.self) private var store

    var body: some View {
        VStack(spacing: 16) {
            Image(systemName: "music.note.house").font(.system(size: 52)).foregroundStyle(store.colors.accent)
            Text("Bring your music home.").font(store.display(26, weight: .semibold))
            Text("Point Needle at the folders where your music lives. It reads your files in place, keeps its own index, and never uploads anything.")
                .multilineTextAlignment(.center).foregroundStyle(store.colors.ink2).frame(maxWidth: 440)
            HStack {
                Button { store.addFolder() } label: { Label("Add a music folder", systemImage: "folder.badge.plus") }
                    .glassButton(prominent: true)
                Button("Try three demo recordings") { store.demo() }.glassButton()
            }
            .controlSize(.large)
            Text("⌘O adds a folder at any time. No account needed.").font(.caption).foregroundStyle(store.colors.ink3)
            Button { store.navigate(.importing) } label: {
                Label("Coming from iTunes, Spotify, or Last.fm? Bring your history", systemImage: "square.and.arrow.down")
            }
            .buttonStyle(.link)
        }
        .padding(40)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}

/// The short message at the foot of the window.
struct ToastView: View {
    @Environment(Store.self) private var store

    var body: some View {
        if let toast = store.toast {
            HStack(spacing: 10) {
                Image(systemName: toast.error ? "exclamationmark.triangle.fill" : "info.circle.fill")
                    .foregroundStyle(toast.error ? store.colors.danger : store.colors.accent)
                Text(toast.text).lineLimit(4).frame(maxWidth: 520, alignment: .leading)
                Button { store.toast = nil } label: { Image(systemName: "xmark").font(.caption.bold()) }
                    .buttonStyle(.borderless)
            }
            .padding(.vertical, 10)
            .padding(.horizontal, 16)
            .pane(Capsule(), glass: true)
            .shadow(color: .black.opacity(0.18), radius: 16, y: 6)
            .transition(.move(edge: .bottom).combined(with: .opacity))
            .id(toast.id)
        }
    }
}

extension Notes: Identifiable {
    public var id: String { version }
}
