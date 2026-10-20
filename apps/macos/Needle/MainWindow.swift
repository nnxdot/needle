import SwiftUI
import UniformTypeIdentifiers

/// The main window: the sidebar, the page, the side panel (details, queue, lyrics), and the
/// player floating on glass at the foot; the big player over it all.
struct MainWindow: View {
    private var store: Store { .shared }
    @Environment(\.openWindow) private var openWindow
    @Environment(\.openSettings) private var openSettings
    @FocusState private var searchFocused: Bool
    @State private var suggestions: [Suggestion] = []
    /// The column widths saved in the layout, read once: they set where the columns start.
    /// (Following them as they are saved fed each squeeze of a column back into the split view,
    /// which then never settled, and AppKit stopped the app.)
    @State private var sidebarStart = CGFloat(Store.shared.settings.layout.sidebarWidth)
    @State private var inspectorStart = CGFloat(Store.shared.settings.layout.inspectorWidth)

    var body: some View {
        @Bindable var store = store
        let colors = store.colors
        ZStack {
            // Folded, the sidebar's column is closed; with the compact sidebar a strip of icons
            // (`Rail`) sits beside the page instead. (As a split view column of a fixed width,
            // the strip crashed the app when its edge was dragged.)
            NavigationSplitView(columnVisibility: Binding(
                get: { store.settings.layout.sidebarHidden ? .detailOnly : .all },
                set: { store.setLayout(["sidebar_hidden": $0 == .detailOnly]) })) {
                Sidebar()
                    .toolbar(removing: .sidebarToggle)
                    .navigationSplitViewColumnWidth(min: 175, ideal: sidebarStart, max: 260)
                    .background { WidthKeeper(key: "sidebar_width", range: 175...260, saved: store.settings.layout.sidebarWidth) }
            } detail: {
                detail
            }
            .modifier(SearchField(searchFocused: $searchFocused, suggestions: suggestions))
            .toolbar(removing: .sidebarToggle)
            .toolbar(store.big && !store.immersive ? .hidden : .visible, for: .windowToolbar)
            // No band behind the toolbar over the page only: the window's one background shows
            // through the whole top bar, the search field included.
            .toolbarBackgroundVisibility(.hidden, for: .windowToolbar)
            // In full screen the toolbar shows only while the pointer is at the top.
            .windowToolbarFullScreenVisibility(.onHover)
            .onChange(of: store.search) { _, text in findSuggestions(text) }
            .onChange(of: store.focusSearch) { _, _ in
                // With the sidebar folded away, its search field is too: the palette searches.
                if store.settings.layout.searchHidden || store.settings.layout.sidebarHidden { store.palette = true } else { searchFocused = true }
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
        .containerBackground(for: .window) { WindowBackground() }
        .animation(store.motion ? .smooth(duration: 0.28) : nil, value: store.big)
        .animation(store.motion ? .smooth(duration: 0.18) : nil, value: store.palette)
        .tint(colors.accent)
        .preferredColorScheme(colors.dark ? .dark : .light)
        .frame(minWidth: Store.leastWidth(store.settings), minHeight: 620)
        .onAppear { Store.makeRoom(Store.leastWidth(store.settings)) }
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
        #if DEBUG
        .task { showForTest() }
        #endif
    }

    #if DEBUG
    /// Test builds: `--show settings|mini|big|palette|panel-later|sidebar-flips` opens that at the start (the
    /// side panel two seconds later), to look at it.
    private func showForTest() {
        let arguments = CommandLine.arguments
        if let i = arguments.firstIndex(of: "--window-size"), arguments.indices.contains(i + 1) {
            let parts = arguments[i + 1].split(separator: "x").compactMap { Double($0) }
            if parts.count == 2, let window = NSApp.windows.first(where: { $0.identifier?.rawValue.hasPrefix("main") == true }) {
                window.setContentSize(NSSize(width: parts[0], height: parts[1]))
            }
        }
        // `--artist <name>`, `--album <title>`: open that page.
        let value = { (flag: String) in arguments.firstIndex(of: flag).flatMap { arguments.indices.contains($0 + 1) ? arguments[$0 + 1] : nil } }
        // `--rescan`: read the music folders again.
        if arguments.contains("--rescan") { store.rescan() }
        if let name = value("--artist") { store.navigate(.artist(name)) }
        if let title = value("--album") {
            Task {
                try? await Task.sleep(for: .seconds(1))
                let albums = JSON.decode([AlbumSummary].self, await store.load { try $0.albumsJson(query: "") }) ?? []
                if let album = albums.first(where: { $0.album == title }) { store.navigate(album.route) }
            }
        }
        guard let i = arguments.firstIndex(of: "--show"), arguments.indices.contains(i + 1) else { return }
        switch arguments[i + 1] {
        case "settings": openSettings()
        case "mini": openWindow(id: "mini")
        case "big": store.big = true
        case "palette": store.palette = true
        case "sidebar-flips":
            // Each switch, then the longest the main thread stalled in the 1.5 s after it.
            let probe = StallProbe()
            Task {
                if arguments.contains("songs") { store.navigate(.songs) }
                try? await Task.sleep(for: .seconds(3))
                for _ in 0..<6 {
                    probe.reset()
                    let start = Date()
                    if arguments.contains("noanim") {
                        var instant = Transaction(animation: nil)
                        instant.disablesAnimations = true
                        withTransaction(instant) { store.setLayout(["sidebar_hidden": !store.settings.layout.sidebarHidden]) }
                    } else {
                        store.setLayout(["sidebar_hidden": !store.settings.layout.sidebarHidden])
                    }
                    let call = Date().timeIntervalSince(start)
                    try? await Task.sleep(for: .seconds(1.5))
                    print(String(format: "flip: set %.0f ms, longest stall %.0f ms", call * 1000, probe.longest * 1000))
                }
                fflush(stdout)
            }
        case "panel-later":
            Task { try? await Task.sleep(for: .seconds(2)); store.set(["show_inspector": true]) }
        default: break
        }
    }
    #endif

    private var rail: Bool { store.settings.layout.sidebarHidden && store.settings.layout.compactSidebar }

    #if DEBUG
    /// Test builds: `--drop page,player,panel` leaves those parts out, to find which one a
    /// layout problem comes from.
    private static let drop: Set<String> = {
        let arguments = CommandLine.arguments
        guard let i = arguments.firstIndex(of: "--drop"), arguments.indices.contains(i + 1) else { return [] }
        return Set(arguments[i + 1].split(separator: ",").map(String.init))
    }()
    #else
    private static let drop: Set<String> = []
    #endif

    @ViewBuilder
    private var detail: some View {
        HStack(spacing: 0) {
            if rail { Rail() }
            page
        }
        .inspector(isPresented: Binding(get: { store.settings.showInspector }, set: { store.set(["show_inspector": $0]) })) {
            Group { if Self.drop.contains("panel") { Color.clear } else { Inspector() } }
                .inspectorColumnWidth(min: 240, ideal: inspectorStart, max: 340)
                .background { WidthKeeper(key: "inspector_width", range: 240...340, saved: store.settings.layout.inspectorWidth) }
        }
        .toolbar { toolbar }
        // The page's name stays the window's, but leaves the bar to the player, as in Apple Music.
        .toolbar(removing: store.settings.layout.playerOnTop ? .title : nil)
        .navigationTitle(title)
    }

    /// The page and the player at its foot.
    @ViewBuilder
    private var page: some View {
        Group { if Self.drop.contains("page") { Color.clear } else { PageView() } }
            // The page column's least size is its own, not its page's: a page that asked for
            // more room than the window had set the split view fighting over the columns until
            // AppKit stopped the app.
            .frame(minWidth: 360, idealWidth: 360, maxWidth: .infinity, minHeight: 300, idealHeight: 300, maxHeight: .infinity)
            // Clipped at the sides and foot, not at the top: an album's or artist's picture goes
            // on under the bar's glass.
            .clipShape(OpenAtTop())
            // The player floats over the foot of the page, which slides under it (or sits in
            // the title bar; see `toolbar`).
            .safeAreaInset(edge: .bottom, spacing: 0) {
                if !store.settings.layout.playerOnTop && !Self.drop.contains("player") {
                    PlayerBar().padding(.horizontal, 20).padding(.bottom, 14).padding(.top, 4)
                }
            }
    }

    /// The page's box, and the bar above it.
    private struct OpenAtTop: Shape {
        func path(in rect: CGRect) -> Path {
            Path(CGRect(x: rect.minX, y: rect.minY - 160, width: rect.width, height: rect.height + 160))
        }
    }

    private var title: String {
        if case .playlist = store.route { return store.pagePlaylist?.name ?? "Playlist" }
        return store.route.title
    }

    @ToolbarContentBuilder
    private var toolbar: some ToolbarContent {
        // The sidebar's button leads Back and Forward, in one place whether the sidebar is open,
        // folded, or a strip of icons.
        ToolbarItemGroup(placement: .navigation) {
            SidebarButton()
            Button { store.goBack() } label: { Image(systemName: "chevron.left") }
                .help("Back · ⌘[")
                .disabled(store.back.isEmpty)
            Button { store.goForward() } label: { Image(systemName: "chevron.right") }
                .help("Forward · ⌘]")
                .disabled(store.forward.isEmpty)
        }
        // With Back and Forward, not alone at the far right over the side panel.
        ToolbarItem(placement: .navigation) {
            Button { store.palette = true } label: { Image(systemName: "command") }
                .help("Command palette: go anywhere, do anything · ⌘K")
        }
        // Right after Back and Forward: as the bar's middle item macOS pushed it over to the
        // right, half over the side panel.
        if store.settings.layout.playerOnTop && !Self.drop.contains("player") {
            // Its own capsule of glass, not one with the buttons before it.
            if #available(macOS 26, *) {
                ToolbarItem(placement: .navigation) { TopPlayer().padding(.vertical, 3).pane(Capsule()).padding(.leading, 6) }
                    .sharedBackgroundVisibility(.hidden)
            } else {
                ToolbarItem(placement: .navigation) { TopPlayer() }
            }
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

/// The search field at the top of the sidebar (unless hidden in Settings), with completions
/// for rules.
private struct SearchField: ViewModifier {
    private var store: Store { .shared }
    var searchFocused: FocusState<Bool>.Binding
    let suggestions: [Suggestion]

    // The field stays on with the sidebar folded (it folds away with it): turning it off and on
    // rebuilt the whole window, the song table included, at each switch of the sidebar.
    func body(content: Content) -> some View {
        @Bindable var store = store
        if store.settings.layout.searchHidden {
            content
        } else {
            content
                // At the top of the sidebar, as in Apple Music: in the toolbar it sat across the
                // page and the side panel, half on each background.
                .searchable(text: $store.search, placement: .sidebar, prompt: "Search")
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

#if DEBUG
/// Test builds: the longest gap between ticks of a 5 ms timer on the main thread, which is how
/// long the window could not draw.
@MainActor
final class StallProbe {
    private(set) var longest: TimeInterval = 0
    private var last = Date()
    private var timer: Timer?

    init() {
        timer = Timer.scheduledTimer(withTimeInterval: 0.005, repeats: true) { [weak self] _ in
            MainActor.assumeIsolated {
                guard let self else { return }
                let now = Date()
                self.longest = max(self.longest, now.timeIntervalSince(self.last))
                self.last = now
            }
        }
        RunLoop.main.add(timer!, forMode: .common)
    }

    func reset() {
        longest = 0
        last = Date()
    }
}
#endif

/// Folds the sidebar (to its strip of icons, with the compact sidebar) or opens it. Needle's
/// own, with Back and Forward: the system's jumped between the sidebar's far edge and the
/// toolbar.
struct SidebarButton: View {
    private var store: Store { .shared }

    var body: some View {
        let hidden = store.settings.layout.sidebarHidden
        Button { store.setLayout(["sidebar_hidden": !hidden]) } label: { Image(systemName: "sidebar.left") }
            .help(hidden ? "Show the sidebar · ⌃⌘S" : "Hide the sidebar · ⌃⌘S")
    }
}

/// Remembers a column's width in the layout (shared with the layout file) a moment after it
/// is dragged.
struct WidthKeeper: View {
    private var store: Store { .shared }
    let key: String
    let range: ClosedRange<Float>
    let saved: Float
    @State private var width: Float = 0

    var body: some View {
        GeometryReader { geo in
            Color.clear
                .onAppear { width = Float(geo.size.width) }
                .onChange(of: geo.size.width) { _, w in width = Float(w) }
        }
        .task(id: width) {
            try? await Task.sleep(for: .milliseconds(700))
            // Only a width it can be dragged to: folding or closing the column passes through
            // smaller ones, and saving those shrank the column and redrew the window again.
            guard !Task.isCancelled, range.contains(width) else { return }
            let next = width.rounded()
            if abs(next - saved) >= 2 { store.setLayout([key: next]) }
        }
    }
}

/// Behind the whole window, the sidebar and side panel included: the look's page colour, or
/// with Ambient background on, the playing cover, blurred.
struct WindowBackground: View {
    private var store: Store { .shared }

    var body: some View {
        ZStack {
            if store.settings.ambient {
                store.colors.chrome
                if let p = store.playback, p.hasSong {
                    // Full strength, dimmed only a little: mixed into the grey beneath and
                    // darkened twice, the colours looked washed out.
                    CoverBackdrop(path: p.artwork, seed: p.album + p.artist, moving: store.motion,
                                  dim: store.colors.dark ? 0.35 : 0, light: !store.colors.dark)
                        .opacity(store.colors.dark ? 1 : 0.85)
                } else {
                    store.colors.glow.opacity(0.25)
                }
            } else {
                store.colors.canvas
            }
        }
        .ignoresSafeArea()
    }
}

/// The page the window shows.
struct PageView: View {
    private var store: Store { .shared }
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
            case .sound:
                Form { SoundPane() }
                    .formStyle(.grouped)
                    .scrollContentBackground(.hidden)
                    .frame(maxWidth: 820)
                    .frame(maxWidth: .infinity)
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
    private var store: Store { .shared }

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
    private var store: Store { .shared }

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
            .pane(Capsule())
            .shadow(color: .black.opacity(0.18), radius: 16, y: 6)
            .transition(.move(edge: .bottom).combined(with: .opacity))
            .id(toast.id)
        }
    }
}

extension Notes: Identifiable {
    public var id: String { version }
}
