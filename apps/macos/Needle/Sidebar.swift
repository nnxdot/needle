import SwiftUI
import UniformTypeIdentifiers

/// The sidebar: the library, collections, music servers, and playlists; songs dropped on a
/// playlist join it, and on Favorites become favorites. Folded to a strip of icons with the
/// compact sidebar.
struct Sidebar: View {
    @Environment(Store.self) private var store

    var body: some View {
        let rail = store.settings.layout.sidebarHidden && store.settings.layout.compactSidebar
        List(selection: Binding<Route?>(get: { store.route.sidebarRoute }, set: { if let to = $0 { store.navigate(to) } })) {
            Section {
                item("Home", "house", .home)
                item("Songs", "music.note", .songs)
                item("Albums", "square.stack", .albums)
                item("Artists", "music.mic", .artists)
                item("Folders", "folder", .folders)
            }
            Section {
                item("Favorites", "heart", .favorites)
                    .dropDestination(for: String.self) { ids, _ in
                        store.rate(ids, 5)
                        store.notify(ids.count == 1 ? "Added to favorites." : "Added \(ids.count) songs to favorites.")
                        return true
                    }
                item("Recently added", "clock", .recent)
                item("Listening history", "chart.line.uptrend.xyaxis", .history)
                ForEach(store.settings.layout.collections, id: \.self) { key in
                    if let c = Collections.find(key) {
                        item(c.name, c.icon, .collection(key))
                            .contextMenu {
                                Button("Remove from the Sidebar") { setCollection(key, on: false) }
                            }
                    }
                }
            } header: {
                if !rail {
                    HStack {
                        Text("Collections")
                        Spacer()
                        Menu {
                            Text("Show in the sidebar")
                            ForEach(Collections.more, id: \.key) { c in
                                Toggle(c.name, isOn: Binding(
                                    get: { store.settings.layout.collections.contains(c.key) },
                                    set: { setCollection(c.key, on: $0) }))
                            }
                        } label: {
                            Image(systemName: "plus")
                        }
                        .menuStyle(.borderlessButton)
                        .menuIndicator(.hidden)
                        .fixedSize()
                        .help("Add a collection")
                    }
                }
            }
            if !store.servers.isEmpty {
                Section(rail ? "" : "Servers") {
                    ForEach(store.servers, id: \.plugin) { server in
                        item(server.name, "globe", .source(plugin: server.plugin, name: server.name))
                    }
                }
            }
            Section {
                ForEach(store.playlists, id: \.id) { playlist in
                    PlaylistRow(playlist: playlist, rail: rail)
                        .tag(Route.playlist(playlist.id))
                }
                if store.playlists.isEmpty && !rail {
                    Text("Save a search or a set of tracks and it will appear here.")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                        .selectionDisabled()
                }
            } header: {
                if !rail {
                    HStack {
                        Text("Playlists")
                        Spacer()
                        Button { store.editor = .new() } label: { Image(systemName: "plus") }
                            .buttonStyle(.borderless)
                            .help("New playlist")
                    }
                }
            }
        }
        .listStyle(.sidebar)
        .scrollContentBackground(store.glass ? .automatic : .hidden)
        .background { if !store.glass { store.colors.chrome.ignoresSafeArea() } }
        .safeAreaInset(edge: .bottom, spacing: 0) { footer(rail: rail) }
    }

    private func item(_ name: String, _ icon: String, _ route: Route) -> some View {
        Label(name, systemImage: icon)
            .labelStyle(SidebarLabel(rail: store.settings.layout.sidebarHidden && store.settings.layout.compactSidebar))
            .tag(route)
            .help(name)
    }

    private func setCollection(_ key: String, on: Bool) {
        var list = store.settings.layout.collections.filter { $0 != key }
        if on { list.append(key) }
        store.setLayout(["collections": list])
    }

    @ViewBuilder
    private func footer(rail: Bool) -> some View {
        VStack(alignment: .leading, spacing: 2) {
            if store.scan.running {
                HStack(spacing: 8) {
                    ProgressView().controlSize(.small)
                    if !rail {
                        VStack(alignment: .leading, spacing: 1) {
                            Text(store.scan.scanned == 0 ? "Importing…" : "Importing · \(store.scan.scanned.formatted()) files")
                                .font(.callout.weight(.medium))
                            Text(store.scan.current).font(.caption).foregroundStyle(.secondary).lineLimit(1).truncationMode(.middle)
                        }
                    }
                }
                .padding(10)
                .frame(maxWidth: .infinity, alignment: .leading)
                .pane(RoundedRectangle(cornerRadius: 10), glass: store.glass, fill: store.colors.raised)
                .padding(.bottom, 4)
            }
            footerButton("Add music folder", "folder.badge.plus", rail) { store.addFolder() }
            footerButton("Fix my library", "wrench.and.screwdriver", rail) { store.navigate(.doctor) }
            footerButton("Import", "square.and.arrow.down", rail) { store.navigate(.importing) }
            SettingsLink {
                Label("Settings", systemImage: "gearshape").labelStyle(SidebarLabel(rail: rail))
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .padding(.vertical, 4)
            .padding(.horizontal, 8)
            .help("Settings · ⌘,")
        }
        .padding(.horizontal, 10)
        .padding(.bottom, 10)
    }

    private func footerButton(_ name: String, _ icon: String, _ rail: Bool, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Label(name, systemImage: icon).labelStyle(SidebarLabel(rail: rail))
                .frame(maxWidth: .infinity, alignment: .leading)
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .padding(.vertical, 4)
        .padding(.horizontal, 8)
        .help(name)
    }
}

/// A sidebar label, or its icon alone on the folded strip.
struct SidebarLabel: LabelStyle {
    let rail: Bool
    func makeBody(configuration: Configuration) -> some View {
        if rail {
            configuration.icon.font(.title3).frame(maxWidth: .infinity)
        } else {
            Label(configuration)
        }
    }
}

/// A playlist in the sidebar: its covers, its name, a menu, and a place to drop songs.
struct PlaylistRow: View {
    @Environment(Store.self) private var store
    let playlist: Playlist
    let rail: Bool
    @State private var confirmDelete = false

    var body: some View {
        HStack(spacing: 8) {
            PlaylistArt(playlist: playlist, side: rail ? 28 : 20)
            if !rail {
                Text(playlist.name).lineLimit(1)
                if playlist.smart {
                    Spacer()
                    Image(systemName: "sparkles").font(.caption2).foregroundStyle(.secondary)
                }
            }
        }
        .frame(maxWidth: .infinity, alignment: rail ? .center : .leading)
        .help(playlist.name)
        .dropDestination(for: String.self) { ids, _ in
            guard !playlist.smart else { return false }
            store.addToPlaylist(playlist, ids)
            return true
        }
        .contextMenu {
            Button("Play") { playAll(shuffle: false) }
            Button("Shuffle") { playAll(shuffle: true) }
            Button("Add to Queue") { enqueue() }
            Divider()
            Button(playlist.smart ? "Edit Playlist or Rules…" : "Edit Playlist…") {
                store.editor = store.playlistModel(playlist.id)
            }
            Button("Export as M3U8…") { export() }
            Divider()
            Button("Delete Playlist…", role: .destructive) { confirmDelete = true }
        }
        .confirmationDialog("Delete “\(playlist.name)”?", isPresented: $confirmDelete) {
            Button("Delete Playlist", role: .destructive) {
                let id = playlist.id
                store.run { try $0.deletePlaylist(id: id) }
                if store.route == .playlist(id) { store.navigate(.songs) }
                store.notify("Playlist deleted. Your music files are untouched.")
                Task { try? await Task.sleep(for: .milliseconds(200)); store.reloadPlaylists() }
            }
        } message: {
            Text("Your music files are untouched.")
        }
    }

    private func playAll(shuffle: Bool) {
        let id = playlist.id
        let rule = store.playlistModel(id)?.query ?? ""
        let reason = "From your playlist \(playlist.name)"
        Task {
            _ = await store.ask { try $0.playView(query: rule, playlist: rule.isEmpty ? id : nil, filter: "", start: 0, shuffle: shuffle, reason: reason) }
        }
    }

    private func enqueue() {
        let id = playlist.id
        Task {
            if let songs = await store.ask({ try $0.playlistSongs(id: id) }) {
                store.enqueue(songs.map(\.id))
            }
        }
    }

    private func export() {
        let panel = NSSavePanel()
        panel.nameFieldStringValue = "\(playlist.name).m3u8"
        guard panel.runModal() == .OK, let path = panel.url?.path else { return }
        let id = playlist.id
        store.job { try $0.exportPlaylist(id: id, path: path); return "Playlist exported." }
    }
}
