import PhotosUI
import SwiftUI

/// A playlist: its songs to play, take out, and move; its name, words, rule, and picture.
struct PlaylistView: View {
    @Environment(Store.self) private var store
    @Environment(\.dismiss) private var dismiss
    let playlist: Playlist
    @State private var detail: PlaylistDetail?
    @State private var editing = false
    @State private var picture: PhotosPickerItem?
    @State private var picking = false
    @State private var exported: URL?
    @State private var moving = false
    @State private var deleting = false
    @State private var selecting = false
    @State private var selected = Set<String>()
    /// The playlist as it is now (its picture, after a change).
    @State private var current: Playlist?

    var body: some View {
        let songs = detail?.songs ?? []
        let smart = detail?.rule != nil
        List(selection: selecting ? $selected : nil) {
            VStack(spacing: 6) {
                PlaylistCover(playlist: current ?? playlist)
                    .frame(width: 230, height: 230)
                    .clipShape(.rect(cornerRadius: 18))
                    .shadow(color: .black.opacity(0.4), radius: 20, y: 12)
                    .padding(.bottom, 10)
                Text(detail?.name ?? playlist.name).font(.title2.bold()).multilineTextAlignment(.center)
                if let words = detail?.description, !words.isEmpty {
                    Text(words).font(.subheadline).foregroundStyle(.secondary).multilineTextAlignment(.center)
                }
                if let rule = detail?.rule {
                    Label(rule, systemImage: "gearshape.2").font(.caption).foregroundStyle(.secondary)
                }
                Text(count(songs.count, "song") + " · " + clock(songs.reduce(0) { $0 + $1.duration })).font(.footnote).foregroundStyle(.secondary)
                if !songs.isEmpty { PlayButtons(songs: songs).padding(.top, 8) }
            }
            .frame(maxWidth: .infinity)
            .listRowSeparator(.hidden)
            .listRowBackground(Color.clear)
            .selectionDisabled()
            ForEach(Array(songs.enumerated()), id: \.offset) { i, song in
                SongRow(song: song) { store.play(songs, from: i) }
                    .tag(song.id)
            }
            .onDelete(perform: smart ? nil : { rows in
                guard let detail else { return }
                // From the end, so the places before stay right.
                for row in rows.sorted(by: >) {
                    let at = detail.positions[row]
                    store.run { try $0.removeFromPlaylist(id: detail.id, index: at) }
                }
                Task { try? await Task.sleep(for: .milliseconds(200)); await reload() }
            })
            .onMove(perform: smart ? nil : { from, to in
                guard let detail, let first = from.first else { return }
                let target = to > first ? to - 1 : to
                let (a, b) = (detail.positions[first], detail.positions[min(target, detail.positions.count - 1)])
                store.run { try $0.moveInPlaylist(id: detail.id, from: a, to: b) }
                self.detail?.songs.move(fromOffsets: from, toOffset: to)
                Task { try? await Task.sleep(for: .milliseconds(200)); await reload() }
            })
        }
        .listStyle(.plain)
        .navigationBarTitleDisplayMode(.inline)
        .themed()
        .toolbar {
            if selecting {
                ToolbarItem(placement: .topBarLeading) { Button("Done") { selecting = false; selected = [] } }
                if !selected.isEmpty { SelectionActions(songs: songs.filter { selected.contains($0.id) }) }
            } else {
                ToolbarItem(placement: .topBarTrailing) {
                    Menu {
                        Button("Select", systemImage: "checkmark.circle") { selecting = true }
                        Button("Edit", systemImage: "pencil") { editing = true }
                        if !smart {
                            Button("Choose a picture", systemImage: "photo") { picking = true }
                            if detail?.artwork != nil {
                                Button("Use the songs' covers", systemImage: "square.grid.2x2") {
                                    let id = playlist.id
                                    Task {
                                        if await store.ask({ try $0.setPlaylistPicture(id: id, path: nil) }) != nil { store.libraryChanged() }
                                    }
                                }
                            }
                        }
                        Button("Export as a playlist file", systemImage: "square.and.arrow.up") { export() }
                        Divider()
                        Button("Delete playlist", systemImage: "trash", role: .destructive) { deleting = true }
                    } label: { Image(systemName: "ellipsis") }
                }
            }
        }
        .environment(\.editMode, .constant(selecting ? .active : .inactive))
        .sheet(isPresented: $editing) {
            NavigationStack { PlaylistEditor(detail: detail) { Task { await reload() } } }
        }
        .photosPicker(isPresented: $picking, selection: $picture, matching: .images)
        .onChange(of: picture) { _, item in
            guard let item else { return }
            Task {
                guard let data = try? await item.loadTransferable(type: Data.self) else { return }
                let url = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString + ".jpg")
                try? data.write(to: url)
                let (id, path) = (playlist.id, url.path)
                if await store.ask({ try $0.setPlaylistPicture(id: id, path: path) }) != nil { store.libraryChanged() }
                picture = nil
            }
        }
        .fileMover(isPresented: $moving, file: exported) { _ in }
        .confirmationDialog("Delete “\(playlist.name)”?", isPresented: $deleting, titleVisibility: .visible) {
            Button("Delete playlist", role: .destructive) {
                let id = playlist.id
                Task {
                    if await store.ask({ try $0.deletePlaylist(id: id) }) != nil {
                        store.libraryChanged()
                        dismiss()
                    }
                }
            }
        } message: {
            Text("The songs stay in your library.")
        }
        .task(id: store.libraryVersion) { await reload() }
    }

    private func reload() async {
        let id = playlist.id
        detail = await store.load { try $0.playlistDetail(id: id) }
        // The whole list only when the picture changed (or the first time): it is costly.
        if current == nil || current?.picture != detail?.artwork || Int(current?.songs ?? 0) != detail?.songs.count
            || current?.artwork != (detail?.artwork ?? detail?.songs.first(where: { $0.artwork != nil })?.artwork) {
            current = await store.load { try $0.playlists().first { $0.id == id } } ?? nil
        }
    }

    private func export() {
        let name = (detail?.name ?? playlist.name).replacingOccurrences(of: "/", with: "-")
        let path = FileManager.default.temporaryDirectory.appendingPathComponent(name + ".m3u8").path
        let id = playlist.id
        Task {
            if await store.ask({ try $0.exportPlaylist(id: id, path: path) }) != nil {
                exported = URL(fileURLWithPath: path)
                moving = true
            }
        }
    }
}

/// Makes a playlist or changes one: its name, its words, and, for a smart one, its rule.
struct PlaylistEditor: View {
    @Environment(Store.self) private var store
    @Environment(\.dismiss) private var dismiss
    var detail: PlaylistDetail?
    /// Songs to put in a new playlist.
    var songs: [Song] = []
    let done: () -> Void
    @State private var name = ""
    @State private var words = ""
    @State private var smart = false
    @State private var rule = ""
    @State private var problem: String?
    @State private var hints: [Suggestion] = []

    var body: some View {
        Form {
            Section {
                TextField("Name", text: $name)
                TextField("Description", text: $words, axis: .vertical)
            }
            if detail == nil && songs.isEmpty || detail?.rule != nil {
                Section {
                    if detail == nil { Toggle("Smart playlist", isOn: $smart) }
                    if smart || detail?.rule != nil {
                        TextField("rating is at least 4", text: $rule, axis: .vertical)
                            .textInputAutocapitalization(.never)
                            .autocorrectionDisabled()
                            .font(.body.monospaced())
                        if !hints.isEmpty {
                            ScrollView(.horizontal) {
                                HStack {
                                    ForEach(Array(hints.prefix(8).enumerated()), id: \.offset) { _, hint in
                                        Button(hint.label) { rule = apply(hint, to: rule) }
                                            .buttonStyle(.glass).controlSize(.small)
                                    }
                                }
                            }
                            .scrollIndicators(.hidden)
                        }
                        if let problem { Text(problem).font(.caption).foregroundStyle(.red) }
                    }
                } footer: {
                    if smart || detail?.rule != nil {
                        Text("A smart playlist follows a rule, as a search does: “genre = Jazz and year >= 1960”, “rating is at least 4”, “played in the last 30 days”.")
                    }
                }
            }
        }
        .navigationTitle(detail == nil ? "New playlist" : "Edit playlist")
        .navigationBarTitleDisplayMode(.inline)
        .themed()
        .toolbar {
            ToolbarItem(placement: .cancellationAction) { Button("Cancel") { dismiss() } }
            ToolbarItem(placement: .confirmationAction) {
                Button(detail == nil ? "Make" : "Save") { save() }
                    .disabled(name.trimmingCharacters(in: .whitespaces).isEmpty || (smart && (rule.isEmpty || problem != nil)))
            }
        }
        .onAppear {
            guard let detail else { return }
            name = detail.name
            words = detail.description
            rule = detail.rule ?? ""
            smart = detail.rule != nil
        }
        .task(id: rule) {
            guard smart || detail?.rule != nil else { return }
            try? await Task.sleep(for: .milliseconds(200))
            let r = rule
            problem = r.isEmpty ? nil : await store.load { $0.checkRule(rule: r) } ?? nil
            hints = await store.load { $0.suggest(input: r, cursor: UInt32(r.utf16.count)) } ?? []
        }
    }

    private func save() {
        let (n, w, r) = (name, words, smart ? rule : nil)
        let ids = songs.map(\.id)
        Task {
            if let detail {
                let id = detail.id
                guard await store.ask({ try $0.editPlaylist(id: id, name: n, description: w, rule: r) }) != nil else { return }
            } else {
                guard await store.ask({ try $0.createPlaylist(name: n, description: w, songs: ids, rule: r) }) != nil else { return }
                store.message = "Made “\(n)”"
            }
            store.libraryChanged()
            done()
            dismiss()
        }
    }
}

/// A completion for a rule or search, put in at its place.
func apply(_ hint: Suggestion, to text: String) -> String {
    let units = Array(text.utf16)
    let start = min(Int(hint.start), units.count), end = min(max(Int(hint.end), start), units.count)
    let before = String(utf16CodeUnits: Array(units[..<start]), count: start)
    let after = String(utf16CodeUnits: Array(units[end...]), count: units.count - end)
    return before + hint.insert + after
}

/// Adds songs to a playlist, or to a new one.
struct AddToPlaylistView: View {
    @Environment(Store.self) private var store
    @Environment(\.dismiss) private var dismiss
    let songs: [Song]
    @State private var playlists: [Playlist] = []
    @State private var making = false

    var body: some View {
        List {
            Button { making = true } label: { Label("New playlist", systemImage: "plus") }
            ForEach(playlists.filter { !$0.smart }) { playlist in
                Button {
                    let (id, ids) = (playlist.id, songs.map(\.id))
                    Task {
                        if await store.ask({ try $0.addToPlaylist(id: id, songs: ids) }) != nil {
                            store.message = "Added to “\(playlist.name)”"
                            store.libraryChanged()
                            dismiss()
                        }
                    }
                } label: {
                    HStack(spacing: 12) {
                        PlaylistCover(playlist: playlist).frame(width: 44, height: 44).clipShape(.rect(cornerRadius: 7))
                        Text(playlist.name).foregroundStyle(.primary)
                    }
                }
            }
        }
        .navigationTitle(songs.count == 1 ? "Add to a playlist" : "Add \(songs.count) songs")
        .navigationBarTitleDisplayMode(.inline)
        .themed()
        .toolbar { ToolbarItem(placement: .cancellationAction) { Button("Cancel") { dismiss() } } }
        .navigationDestination(isPresented: $making) { PlaylistEditor(songs: songs) { dismiss() } }
        .task { playlists = await store.load { try $0.playlists() } ?? [] }
    }
}

/// What can be done with songs chosen in a list.
struct SelectionActions: ToolbarContent {
    @Environment(Store.self) private var store
    let songs: [Song]

    var body: some ToolbarContent {
        ToolbarItemGroup(placement: .bottomBar) {
            Button("Play", systemImage: "play.fill") { store.play(songs) }
            Button("Play Next", systemImage: "text.line.first.and.arrowtriangle.forward") { store.playNext(songs) }
            Button("Add to Queue", systemImage: "text.line.last.and.arrowtriangle.forward") { store.enqueue(songs) }
            Menu {
                Button("Add to a playlist", systemImage: "music.note.list") { store.sheet = .addToPlaylist(songs) }
                Button("Favourite", systemImage: "star") {
                    let ids = songs.map(\.id)
                    store.run { core in for id in ids where !core.isFavorite(id: id) { _ = try core.toggleFavorite(id: id) } }
                    store.message = "Added to Favourites"
                }
                if songs.contains(where: \.fromServer) {
                    Button("Keep offline", systemImage: "arrow.down.circle") { store.keepOffline(songs.filter(\.fromServer), keep: true) }
                    Button("Save to this iPhone", systemImage: "square.and.arrow.down") { store.saveToPhone(songs.filter(\.fromServer)) }
                }
                if !songs.contains(where: \.fromServer) {
                    Button("Edit Tags", systemImage: "tag") { store.sheet = .tags(songs.map(\.id)) }
                }
            } label: { Image(systemName: "ellipsis.circle") }
        }
    }
}
