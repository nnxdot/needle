import SwiftUI

/// The command palette (⌘K): go anywhere, do anything, and find songs, albums, and artists.
struct CommandPalette: View {
    private var store: Store { .shared }
    @Environment(\.openWindow) private var openWindow
    @Environment(\.openSettings) private var openSettings
    @State private var text = ""
    @State private var active = 0
    @State private var found: PaletteFound?
    @FocusState private var focused: Bool

    struct Item: Identifiable {
        let id = UUID()
        let section: String
        let title: String
        let detail: String
        let icon: String
        var cover: String? = nil
        let run: () -> Void
    }

    var body: some View {
        let items = items
        ZStack(alignment: .top) {
            Color.black.opacity(0.25).ignoresSafeArea().onTapGesture { store.palette = false }
            VStack(spacing: 0) {
                HStack(spacing: 10) {
                    Image(systemName: "command").foregroundStyle(.secondary)
                    TextField("Go to, play, or change anything…", text: $text)
                        .textFieldStyle(.plain)
                        .font(.title3)
                        .focused($focused)
                        .onSubmit { run(items) }
                }
                .padding(16)
                Divider()
                ScrollViewReader { reader in
                    ScrollView {
                        LazyVStack(alignment: .leading, spacing: 0) {
                            if items.isEmpty {
                                Text(text.isEmpty ? "Type to search." : "Nothing found yet…").foregroundStyle(.secondary).padding(16)
                            }
                            ForEach(Array(items.enumerated()), id: \.element.id) { index, item in
                                if index == 0 || items[index - 1].section != item.section {
                                    Text(item.section).font(.caption.weight(.semibold)).foregroundStyle(.secondary)
                                        .padding(.horizontal, 16).padding(.top, 10).padding(.bottom, 4)
                                }
                                row(item, active: index == active)
                                    .id(index)
                                    .onTapGesture { active = index; run(items) }
                            }
                        }
                        .padding(.bottom, 8)
                    }
                    .frame(maxHeight: 420)
                    .onChange(of: active) { _, a in reader.scrollTo(a) }
                }
                Divider()
                HStack(spacing: 14) {
                    Text("↑ ↓ choose").font(.caption)
                    Text("↩ open").font(.caption)
                    Text("Esc close").font(.caption)
                    Spacer()
                }
                .foregroundStyle(.secondary)
                .padding(.horizontal, 16)
                .padding(.vertical, 8)
            }
            .frame(width: 620)
            .pane(RoundedRectangle(cornerRadius: 20, style: .continuous), glass: true)
            .shadow(color: .black.opacity(0.3), radius: 30, y: 12)
            .padding(.top, 90)
        }
        .onAppear { focused = true }
        .onKeyPress(.downArrow) { active = min(active + 1, max(0, items.count - 1)); return .handled }
        .onKeyPress(.upArrow) { active = max(0, active - 1); return .handled }
        .onChange(of: text) { _, t in
            active = 0
            Task {
                try? await Task.sleep(for: .milliseconds(120))
                guard t == text else { return }
                found = await store.load { $0.paletteSearch(text: t) }
            }
        }
    }

    private func row(_ item: Item, active: Bool) -> some View {
        HStack(spacing: 12) {
            if let cover = item.cover {
                Cover(path: cover, seed: item.title, side: 28, corner: 4)
            } else {
                Image(systemName: item.icon).frame(width: 28).foregroundStyle(active ? store.colors.accent : .secondary)
            }
            VStack(alignment: .leading, spacing: 1) {
                Text(item.title).lineLimit(1)
                if !item.detail.isEmpty { Text(item.detail).font(.caption).foregroundStyle(.secondary).lineLimit(1) }
            }
            Spacer()
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 6)
        .background(active ? store.colors.accentSoft : .clear, in: RoundedRectangle(cornerRadius: 8))
        .padding(.horizontal, 6)
        .contentShape(Rectangle())
    }

    private func run(_ items: [Item]) {
        guard active < items.count else { return }
        let item = items[active]
        store.palette = false
        item.run()
    }

    private func matches(_ words: String) -> Bool {
        let t = text.trimmingCharacters(in: .whitespaces).lowercased()
        return t.isEmpty || words.lowercased().contains(t) || t.split(separator: " ").allSatisfy { words.lowercased().contains($0) }
    }

    private var items: [Item] {
        var list: [Item] = []
        let pages: [(String, String, Route)] = [
            ("Home", "house", .home), ("Songs", "music.note", .songs), ("Albums", "square.stack", .albums),
            ("Artists", "music.mic", .artists), ("Folders", "folder", .folders), ("Favorites", "heart", .favorites),
            ("Recently added", "clock", .recent), ("Listening history", "chart.line.uptrend.xyaxis", .history),
            ("Sound and equalizer", "slider.vertical.3", .sound), ("Fix my library", "wrench.and.screwdriver", .doctor),
            ("Import from other apps", "square.and.arrow.down", .importing),
            ("Your year in music", "sparkles", .wrapped(Calendar.current.component(.year, from: Date()))),
        ]
        for (name, icon, route) in pages where matches(name) {
            list.append(Item(section: "Go to", title: name, detail: "", icon: icon) { store.navigate(route) })
        }
        if matches("Settings") {
            list.append(Item(section: "Go to", title: "Settings", detail: "⌘,", icon: "gearshape") { openSettings() })
        }
        let playing = store.playback?.playing == true
        var actions: [(String, String, String, () -> Void)] = [
            (playing ? "Pause" : "Play", "Space", playing ? "pause" : "play", { store.toggle() }),
            ("Next track", "⌘→", "forward", { store.next() }),
            ("Previous track", "⌘←", "backward", { store.previous() }),
            ("Shuffle up next", "", "shuffle", { store.core.shuffle() }),
            ("Clear the queue", "", "xmark.circle", { store.core.clearQueue() }),
            ("Open the big player", "⌘P", "rectangle.expand.vertical", { store.big = true }),
            ("Full screen player", "⌃⌘F", "arrow.up.left.and.arrow.down.right", { store.setImmersive(true) }),
            ("Switch to the mini player", "⌥⌘M", "pip", { openWindow(id: "mini") }),
            ("Show the queue", "⌘J", "list.bullet", { store.panel = .queue; store.set(["show_inspector": true]) }),
            ("Show the lyrics", "⌘L", "quote.bubble", { store.panel = .lyrics; store.set(["show_inspector": true]) }),
            ("Add a music folder", "⌘O", "folder.badge.plus", { store.addFolder() }),
            ("Check folders for changes", "", "arrow.clockwise", { store.rescan() }),
            ("Switch the look: Night, Midnight, Day, and your themes", "", "paintpalette", { nextLook() }),
        ]
        if store.settings.tray {
            actions.append(("Hide to the menu bar", "The music plays on", "menubar.arrow.up.rectangle", { hideToMenuBar() }))
        }
        if let current = store.current {
            actions += [
                ("Go to the album playing", "", "square.stack", { store.navigate(current.albumRoute) }),
                ("Go to the artist playing", "", "music.mic", { store.navigate(.artist(current.artist)) }),
                ("Start radio from this song", "Songs that sound like the one playing", "dot.radiowaves.left.and.right", { store.startRadio(current) }),
                ("Time the lyrics of this song", "Tap along to time lines or words", "timer", { store.navigate(.timing) }),
                ("Split the song playing into stems", "", "square.stack.3d.up", {
                    let id = current.id
                    store.run { try $0.splitStems(id: id) }
                }),
            ]
        }
        for (name, detail, icon, run) in actions where matches(name) {
            list.append(Item(section: "Actions", title: name, detail: detail, icon: icon, run: run))
        }
        for playlist in store.playlists where !text.isEmpty && matches(playlist.name) {
            list.append(Item(section: "Playlists", title: playlist.name, detail: playlist.smart ? "Smart playlist" : "Playlist",
                             icon: "music.note.list", cover: playlist.artwork) { store.navigate(.playlist(playlist.id)) })
        }
        if let found, !text.isEmpty {
            for song in found.songs {
                list.append(Item(section: "Songs", title: song.title, detail: "\(song.artist) · \(song.album)", icon: "music.note", cover: song.artwork) {
                    store.play([song.asTrack], reason: "Chosen from the command palette")
                })
            }
            for album in found.albums {
                list.append(Item(section: "Albums", title: album.title.isEmpty ? "Unknown album" : album.title, detail: album.artist,
                                 icon: "square.stack", cover: album.artwork) {
                    Task {
                        if let json = await store.load({ try $0.albumsJson(query: "") }),
                           let summary = JSON.decode([AlbumSummary].self, json)?.first(where: { $0.key == album.key }) {
                            store.navigate(summary.route)
                        }
                    }
                })
            }
            for artist in found.artists {
                list.append(Item(section: "Artists", title: artist, detail: "", icon: "music.mic") { store.navigate(.artist(artist)) })
            }
        }
        return list
    }

    /// Closes the window; Needle stays in the menu bar and the music plays on.
    private func hideToMenuBar() {
        NSApp.windows.filter { $0.identifier?.rawValue.hasPrefix("main") == true || $0.title == "Needle" }.forEach { $0.close() }
    }

    private func nextLook() {
        let looks = Palette.bases.map(\.0) + store.themes.map(\.id)
        let now = looks.firstIndex(of: store.settings.theme) ?? 0
        store.set(["theme": looks[(now + 1) % looks.count]])
    }
}
