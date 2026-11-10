import PhotosUI
import SwiftUI

/// A playlist: its songs to play, take out, and move; its name, words, rule, and picture.
/// A server's read-only playlist (another person's, or a server's smart one) only plays: its
/// name and songs stay as the server has them. A change that went to a copy (the playlist
/// changed on the server meanwhile, or left Needle) moves the page to the copy.
struct PlaylistView: View {
    @Environment(Store.self) private var store
    @Environment(\.dismiss) private var dismiss
    @Environment(\.wide) private var wide
    @State private var pageWidth: CGFloat = 0
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
    /// Scrolled past the picture and name: the bar shows the name.
    @State private var pastHeader = false
    /// The playlist shown, when a change moved the page to a copy (else `playlist`'s).
    @State private var shownID: String?
    /// It left the library while its page was open (a sync took it away).
    @State private var gone = false
    /// Deleted from this page: the page goes, without saying it was removed.
    @State private var deleted = false
    /// Why it could not be read, when it could not.
    @State private var problem: String?
    @State private var tries = 0

    private var id: String { shownID ?? playlist.id }
    private var name: String { detail?.name ?? current?.name ?? playlist.name }
    private var readOnly: Bool { detail?.readOnly ?? current?.readOnly ?? playlist.readOnly }

    var body: some View {
        Group {
            if gone {
                ContentUnavailableView("This playlist was removed.", systemImage: "music.note.list")
            } else if detail == nil, let problem {
                LoadProblem(title: "Could not open “\(name)”", words: problem) { tries += 1 }
            } else {
                list
            }
        }
        .navigationTitle(pastHeader && !gone ? name : "")
        .navigationBarTitleDisplayMode(.inline)
        .themed()
        // Stars too: a smart playlist's rule can be by them.
        .task(id: "\(id)|\(store.libraryVersion)|\(store.ratingVersion)|\(tries)") { await reload() }
    }

    private var list: some View {
        let songs = detail?.songs ?? []
        // Songs picked by a rule, or kept as the server has them: none taken out or moved here.
        let fixed = detail?.rule != nil || readOnly
        let beside = wide && pageWidth >= 600
        return List(selection: selecting ? $selected : nil) {
            Group {
                if beside { besideHeader(songs) } else { header(songs) }
            }
            .frame(maxWidth: .infinity)
            .listRowSeparator(.hidden)
            .listRowBackground(Color.clear)
            .selectionDisabled()
            // Each row keeps its own id (a song can be in a playlist twice), so the row taken
            // out or moved is that one.
            ForEach(Array(Store.rowIDs(songs).enumerated()), id: \.element.id) { i, row in
                SongRow(song: row.song) { store.play(songs, from: i) }
                    .tag(row.song.id)
                    // Beside the picture, the rows lie on the page and its glow.
                    .listRowBackground(beside ? Color.clear : nil)
            }
            .onDelete(perform: fixed ? nil : { rows in
                guard let detail else { return }
                // From the end, so the places before stay right.
                let taken = rows.sorted(by: >).map { (detail.positions[$0], detail.songs[$0].id) }
                let id = detail.id
                // Gone from the screen at once; the playlist is read again once the core is done.
                withMotion(.snappy) {
                    self.detail?.songs.remove(atOffsets: rows)
                    self.detail?.positions = Self.positions(detail.positions, without: rows)
                }
                Task {
                    let saves = await store.ask { core in
                        // A save that went to a copy: the rest go to the copy too.
                        var target = id
                        var saves: [PlaylistSaved] = []
                        for (at, song) in taken {
                            let saved = try core.removeFromPlaylist(id: target, index: at, song: song)
                            target = saved.copyId ?? saved.id
                            saves.append(saved)
                        }
                        return saves
                    }
                    if let saves { follow(saves) } else { await reload() }
                }
            })
            .onMove(perform: fixed ? nil : { from, to in
                guard let detail, let first = from.first else { return }
                let target = to > first ? to - 1 : to
                let (a, b) = (detail.positions[first], detail.positions[min(target, detail.positions.count - 1)])
                let (id, song) = (detail.id, detail.songs[first].id)
                self.detail?.songs.move(fromOffsets: from, toOffset: to)
                Task {
                    if let saved = await store.ask({ try $0.moveInPlaylist(id: id, from: a, to: b, song: song) }) {
                        follow([saved])
                    } else {
                        await reload()
                    }
                }
            })
        }
        .listStyle(.plain)
        .readableWidth(1180)
        .background(alignment: .top) {
            if beside, let shown = current ?? Optional(playlist) {
                HeaderGlow(path: shown.picture ?? shown.artwork, seed: shown.name, look: .plain(store.colors))
            }
        }
        .onGeometryChange(for: CGFloat.self) { $0.size.width } action: { pageWidth = $0 }
        .onScrollGeometryChange(for: Bool.self) { [beside] in $0.contentOffset.y > (beside ? 220 : 300) } action: { _, past in
            withFade(.easeOut(duration: 0.2)) { pastHeader = past }
        }
        .toolbar {
            if selecting {
                ToolbarItem(placement: .topBarLeading) { Button("Done") { selecting = false; selected = [] } }
                if !selected.isEmpty { SelectionActions(songs: songs.filter { selected.contains($0.id) }) }
            } else {
                ToolbarItem(placement: .topBarTrailing) {
                    Menu {
                        Button("Select", systemImage: "checkmark.circle") { selecting = true }
                        if !readOnly {
                            Button("Edit", systemImage: "pencil") { editing = true }
                        }
                        if detail?.rule == nil {
                            Button("Choose a picture", systemImage: "photo") { picking = true }
                            if detail?.artwork != nil {
                                Button("Use the songs' covers", systemImage: "square.grid.2x2") {
                                    let id = self.id
                                    Task {
                                        if await store.ask({ try $0.setPlaylistPicture(id: id, path: nil) }) != nil { store.libraryChanged() }
                                    }
                                }
                            }
                        }
                        Button("Export as a playlist file", systemImage: "square.and.arrow.up") { export() }
                        if !readOnly {
                            Divider()
                            Button("Delete Playlist", systemImage: "trash", role: .destructive) { deleting = true }
                        }
                    } label: { Image(systemName: "ellipsis") }
                }
            }
        }
        .environment(\.editMode, .constant(selecting ? .active : .inactive))
        .sheet(isPresented: $editing) {
            NavigationStack {
                PlaylistEditor(detail: detail, done: { Task { await reload() } }, saved: { follow([$0]) })
            }
        }
        .photosPicker(isPresented: $picking, selection: $picture, matching: .images)
        .onChange(of: picture) { _, item in
            guard let item else { return }
            Task {
                guard let data = try? await item.loadTransferable(type: Data.self) else { return }
                let url = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString + ".jpg")
                try? data.write(to: url)
                let (id, path) = (self.id, url.path)
                if await store.ask({ try $0.setPlaylistPicture(id: id, path: path) }) != nil { store.libraryChanged() }
                picture = nil
            }
        }
        .fileMover(isPresented: $moving, file: exported) { _ in }
        .confirmationDialog("Delete “\(name)”?", isPresented: $deleting, titleVisibility: .visible) {
            Button("Delete Playlist", role: .destructive) {
                let id = self.id
                Task {
                    deleted = true
                    if await store.ask({ try $0.deletePlaylist(id: id) }) != nil {
                        store.libraryChanged()
                        dismiss()
                    } else {
                        deleted = false
                    }
                }
            }
        } message: {
            Text(detail?.server ?? playlist.server ? "It is deleted on your music server too. The songs stay." : "The songs stay in your library.")
        }
    }

    /// The playlist's places of the rows left when `rows` are taken out: those after a row taken
    /// out move up one.
    /// The picture, the name, its words and rule, and Play and Shuffle, one under another.
    private func header(_ songs: [Song]) -> some View {
        VStack(spacing: 6) {
            PlaylistCover(playlist: current ?? playlist)
                .frame(width: 230, height: 230)
                .clipShape(.soft(Corner.cover))
                .shadow(color: .black.opacity(0.4), radius: 20, y: 12)
                .padding(.bottom, 10)
            Text(name).font(.needle(.title2).bold()).multilineTextAlignment(.center)
            about(songs, center: true)
            if !songs.isEmpty { PlayButtons(songs: songs).padding(.top, 8) }
        }
    }

    /// A wide window's header, as the Mac's: the picture at the left, the rest beside it.
    private func besideHeader(_ songs: [Song]) -> some View {
        HStack(alignment: .bottom, spacing: 28) {
            PlaylistCover(playlist: current ?? playlist)
                .frame(width: 220, height: 220)
                .clipShape(.soft(Corner.cover))
                .shadow(color: .black.opacity(0.35), radius: 24, y: 12)
            VStack(alignment: .leading, spacing: 5) {
                Text(detail?.rule != nil ? "Smart playlist" : "Playlist").font(.needle(.subheadline).weight(.semibold)).foregroundStyle(.secondary)
                Text(name).font(.needleTitle(size: 38, relativeTo: .largeTitle)).lineLimit(2).minimumScaleFactor(0.7)
                about(songs, center: false)
                if !songs.isEmpty { PlayButtons(songs: songs).padding(.top, 10) }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
        }
        .padding(.horizontal, 12)
        .padding(.top, 20)
        .padding(.bottom, 14)
    }

    /// Its words, its rule, the server's mark when it is read-only, and what it holds.
    @ViewBuilder
    private func about(_ songs: [Song], center: Bool) -> some View {
        if let words = detail?.description, !words.isEmpty {
            Text(words).font(.needle(.subheadline)).foregroundStyle(.secondary).multilineTextAlignment(center ? .center : .leading)
        }
        if let rule = detail?.rule {
            Label(rule, systemImage: "gearshape.2").font(.needle(.caption)).foregroundStyle(.secondary)
        }
        if readOnly {
            Label("Read-only on the server", systemImage: "server.rack").font(.needle(.caption)).foregroundStyle(.secondary)
        }
        Text(count(songs.count, "song") + " · " + clock(songs.reduce(0) { $0 + $1.duration })).font(.needle(.footnote)).foregroundStyle(.secondary)
    }

    private static func positions(_ positions: [UInt32], without rows: IndexSet) -> [UInt32] {
        let removed = rows.map { positions[$0] }
        return positions.enumerated().filter { !rows.contains($0.offset) }.map { _, at in
            at - UInt32(removed.filter { $0 < at }.count)
        }
    }

    /// After changes were saved: tells the person where they went, and moves the page to the
    /// copy (or the playlist's new id) when they went there.
    private func follow(_ saves: [PlaylistSaved]) {
        guard let last = saves.last else { return }
        // The save that went elsewhere says so (later ones went to the same place).
        store.told(saves.first { $0.copyName != nil || $0.note != nil } ?? last, done: nil)
        let target = last.copyId ?? last.id
        if target != id {
            current = nil
            detail = nil
            withFade { shownID = target }
        }
        // Lists elsewhere show the copy too; this page reads it again with the new version.
        store.libraryChanged()
    }

    private func reload() async {
        guard !deleted, !gone else { return }
        let id = self.id
        switch await store.attempt({ try $0.playlistDetail(id: id) }) {
        case .success(let read):
            detail = read
            problem = nil
        case .failure(let error):
            // Gone from the library (a server sync took it away): say so, and go back.
            if await store.load({ try $0.playlists().contains { $0.id == id } }) == false {
                guard !deleted else { return }
                withFade { gone = true }
                store.message = "This playlist was removed."
                try? await Task.sleep(for: .seconds(1.2))
                dismiss()
            } else {
                problem = error.words
            }
            return
        }
        // The whole list only when the picture changed (or the first time): it is costly.
        if current == nil || current?.picture != detail?.artwork || Int(current?.songs ?? 0) != detail?.songs.count
            || current?.artwork != (detail?.artwork ?? detail?.songs.first(where: { $0.artwork != nil })?.artwork) {
            current = await store.load { try $0.playlists().first { $0.id == id } } ?? nil
        }
    }

    private func export() {
        let name = self.name.replacingOccurrences(of: "/", with: "-")
        let path = FileManager.default.temporaryDirectory.appendingPathComponent(name + ".m3u8").path
        let id = self.id
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
    /// Where a change to a playlist went (a copy, when it did not go to the playlist itself).
    var saved: (PlaylistSaved) -> Void = { _ in }
    @State private var name = ""
    @State private var words = ""
    /// What the editor showed at first (name, words): only what the person changes is saved.
    @State private var before = ("", "")
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
                            .font(.needle(.body).monospaced())
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
                        if let problem { Text(problem).font(.needle(.caption)).foregroundStyle(.red) }
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
            before = (detail.name, detail.description)
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
                let (bn, bw) = before
                guard let result = await store.ask({ try $0.editPlaylist(id: id, name: n, description: w, rule: r, beforeName: bn, beforeDescription: bw) }) else { return }
                saved(result)
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
            // Only playlists that take songs: not smart ones, nor those read-only on the server.
            ForEach(playlists.filter { !$0.smart && !$0.readOnly }) { playlist in
                Button {
                    let (id, ids) = (playlist.id, songs.map(\.id))
                    Task {
                        if let saved = await store.ask({ try $0.addToPlaylist(id: id, songs: ids) }) {
                            // Never "Added to" a playlist the songs did not go to.
                            store.told(saved, done: "Added to “\(playlist.name)”")
                            store.libraryChanged()
                            dismiss()
                        }
                    }
                } label: {
                    HStack(spacing: 12) {
                        PlaylistCover(playlist: playlist).frame(width: 44, height: 44).clipShape(.soft(Corner.smallCover))
                        Text(playlist.name).foregroundStyle(.primary)
                    }
                }
            }
        }
        .navigationTitle(songs.count == 1 ? "Add to Playlist" : "Add \(songs.count) songs")
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
    @Environment(WindowState.self) private var window
    let songs: [Song]

    var body: some ToolbarContent {
        ToolbarItemGroup(placement: .bottomBar) {
            Button("Play", systemImage: "play.fill") { store.play(songs) }
            Button("Play Next", systemImage: "text.line.first.and.arrowtriangle.forward") { store.playNext(songs) }
            Button("Add to Queue", systemImage: "text.line.last.and.arrowtriangle.forward") { store.enqueue(songs) }
            Menu {
                Button("Add to Playlist", systemImage: "music.note.list") { window.sheet = .addToPlaylist(songs) }
                Button("Add to Favorites", systemImage: "heart") {
                    let ids = songs.map(\.id)
                    Task {
                        _ = await store.ask { core in for id in ids where !core.isFavorite(id: id) { _ = try core.toggleFavorite(id: id) } }
                        store.ratingsChanged()
                    }
                    store.message = "Added to Favorites"
                }
                if songs.contains(where: \.fromServer) {
                    Button("Keep offline", systemImage: "arrow.down.circle") { store.keepOffline(songs.filter(\.fromServer), keep: true) }
                    Button("Save to this \(Device.name)", systemImage: "square.and.arrow.down") { store.saveToPhone(songs.filter(\.fromServer)) }
                }
                if !songs.contains(where: \.fromServer) {
                    Button("Edit Tags", systemImage: "tag") { window.sheet = .tags(songs.map(\.id)) }
                }
            } label: { Image(systemName: "ellipsis.circle") }
        }
    }
}
