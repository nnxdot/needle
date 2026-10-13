import SwiftUI

/// What a song list shows, loaded away from the window: a page of up to 1,000 songs at a time.
@MainActor @Observable
final class SongsModel {
    var tracks: [Track] = []
    var total = 0
    var offset = 0
    var loading = true
    var error: String?
    /// What the search does, in words, when it is a rule.
    var explanation: String?
    /// The rule behind the list, and the playlist of picked songs it shows (if it is one).
    var rule = ""
    var playlistID: String?
    var playlist: PlaylistModel?
    @ObservationIgnored private var generation = 0

    nonisolated static let pageSize = 1000

    func load(_ store: Store, _ route: Route) async {
        generation += 1
        let mine = generation
        if case .playlist(let id) = route {
            playlist = store.playlistModel(id)
            playlistID = playlist?.smart == false ? id : nil
            rule = store.expression(for: route, playlistRule: playlist?.query)
        } else {
            playlist = nil
            playlistID = nil
            rule = store.expression(for: route)
        }
        let check = store.core.checkQuery(query: rule)
        explanation = check.explanation
        if let problem = check.error, playlistID == nil {
            error = problem
            loading = false
            return
        }
        let (rule, playlist, filter, offset) = (rule, playlistID, store.search.trimmingCharacters(in: .whitespaces), offset)
        loading = true
        let result = await Task.detached { [core = store.core] in
            Result { try core.viewPage(query: rule, playlist: playlist, filter: filter, offset: UInt32(offset), size: UInt32(Self.pageSize)) }
        }.value
        guard mine == generation else { return }
        loading = false
        switch result {
        case .success(let json):
            if let page = JSON.decode(TracksPage.self, json) {
                tracks = page.tracks
                total = page.total
                error = nil
            }
        case .failure(let failure):
            error = failure.localizedDescription
        }
    }
}

/// A page of songs: Songs, Favorites, an album, a playlist, a folder, a music server.
struct SongsPage: View {
    @Environment(Store.self) private var store
    let route: Route
    @State private var model = SongsModel()

    var body: some View {
        VStack(spacing: 0) {
            SongsHeader(route: route, model: model)
            if case .folder(let path) = route { FolderStrip(path: path) }
            if let error = model.error, !model.tracks.isEmpty {
                Label("\(error) · showing the last results", systemImage: "exclamationmark.triangle")
                    .font(.callout).foregroundStyle(store.colors.danger)
                    .frame(maxWidth: .infinity, alignment: .leading).padding(.horizontal, 24).padding(.bottom, 6)
            }
            if !store.search.isEmpty { ServerSongs() }
            content
            if model.total > SongsModel.pageSize { pager }
        }
        .task(id: Key(route: route, search: store.search, sort: store.sortField, ascending: store.sortAscending,
                      offset: model.offset, version: store.libraryVersion)) {
            try? await Task.sleep(for: .milliseconds(store.search.isEmpty ? 0 : 150))
            if Task.isCancelled { return }
            await model.load(store, route)
        }
        .onChange(of: store.search) { _, _ in model.offset = 0 }
    }

    private struct Key: Hashable {
        let route: Route
        let search: String
        let sort: String?
        let ascending: Bool
        let offset: Int
        let version: Int
    }

    @ViewBuilder
    private var content: some View {
        if let error = model.error, model.tracks.isEmpty {
            Problem(title: "That rule needs a fix", detail: error, steps: [.clearSearch])
        } else if model.loading && model.tracks.isEmpty {
            Skeleton()
        } else if model.tracks.isEmpty {
            empty
        } else {
            SongTable(model: model, route: route)
        }
    }

    @ViewBuilder
    private var empty: some View {
        let search = store.search.trimmingCharacters(in: .whitespaces)
        if !search.isEmpty {
            Problem(title: "Nothing matches “\(search)”", detail: "Try fewer words, or a rule such as  artist contains \"Nick\".", steps: [.clearSearch, .palette])
        } else {
            switch route {
            case .favorites: Problem(title: "No favorites yet", detail: "Press the heart on any track, or drag songs onto Favorites.", steps: [.songs])
            case .folder: Problem(title: "No songs in this folder", detail: "Needle lists music it has scanned. Check for changes in Settings › Library if you added files here.", steps: [.songs])
            case .recent: Problem(title: "Nothing added in the last 30 days", detail: "New files in your music folders appear here automatically.", steps: [.addFolder])
            case .source(_, let name): Problem(title: "No songs from \(name) yet", detail: "Needle is getting the list of songs, or the server has none. Check Settings › Plugins.", steps: [.songs])
            case .playlist: Problem(title: "This playlist is empty", detail: "Drag songs onto the playlist in the sidebar, or right-click them and choose Add to Playlist.", steps: [.songs])
            case .songs: Problem(title: "Your library is empty", detail: "Add a folder with music, or bring your library over from iTunes, Spotify, or Last.fm.", steps: [.addFolder, .importing])
            default: Problem(title: "Nothing to show yet", detail: "Your songs are one click away.", steps: [.songs])
            }
        }
    }

    private var pager: some View {
        HStack {
            Text("\((model.offset + 1).formatted())–\((model.offset + model.tracks.count).formatted()) of \(model.total.formatted()) tracks")
                .font(.callout).foregroundStyle(store.colors.ink2)
            Spacer()
            Button { model.offset = max(0, model.offset - SongsModel.pageSize) } label: { Image(systemName: "chevron.left") }
                .disabled(model.offset == 0).help("Previous 1,000")
            Button { model.offset += SongsModel.pageSize } label: { Image(systemName: "chevron.right") }
                .disabled(model.offset + model.tracks.count >= model.total).help("Next 1,000")
        }
        .buttonStyle(.borderless)
        .padding(.horizontal, 24)
        .padding(.vertical, 8)
        .overlay(alignment: .top) { Divider() }
    }
}

/// The top of a song list: its cover and name, what it holds, and what to do with it.
struct SongsHeader: View {
    @Environment(Store.self) private var store
    let route: Route
    let model: SongsModel
    @State private var confirmDelete = false
    @State private var blendWith: [String]?

    var body: some View {
        let colors = store.colors
        let album = albumTrack
        let playlist = store.pagePlaylist
        let big = album != nil || playlist != nil
        HStack(alignment: .bottom, spacing: 22) {
            if let playlist {
                PlaylistArt(playlist: playlist, side: 180).shadow(color: .black.opacity(0.25), radius: 18, y: 8)
            } else if let album {
                Cover(path: album.artwork, seed: album.album + album.displayAlbumArtist, side: 180, corner: 10)
                    .shadow(color: colors.glow.opacity(0.35), radius: 26, y: 10)
            }
            VStack(alignment: .leading, spacing: 4) {
                if case .folder(let path) = route { Breadcrumbs(path: path) }
                Text(title)
                    .font(big ? store.display(40) : store.display(28))
                    .lineLimit(2)
                    .foregroundStyle(colors.ink)
                if let album {
                    Button(album.displayAlbumArtist) { store.navigate(.artist(album.displayAlbumArtist)) }
                        .buttonStyle(.link)
                        .font(.title3.weight(.medium))
                }
                Text(summary(album)).font(.callout).foregroundStyle(colors.ink2)
                if let description = playlist?.description, !description.isEmpty {
                    Text(description).font(.callout).foregroundStyle(colors.ink2).lineLimit(2)
                }
                if let rule = ruleLine {
                    Text(rule).font(.callout).foregroundStyle(colors.accent).lineLimit(1).truncationMode(.middle)
                }
                if big { buttons.padding(.top, 10) }
            }
            Spacer(minLength: 0)
            if !big { buttons }
        }
        .padding(.horizontal, 24)
        .padding(.top, big ? 26 : 18)
        .padding(.bottom, big ? 20 : 12)
        .background(alignment: .top) {
            if big {
                LinearGradient(colors: [colors.glow.opacity(colors.dark ? 0.28 : 0.2), .clear], startPoint: .top, endPoint: .bottom)
                    .ignoresSafeArea()
            }
        }
        .confirmationDialog("Delete this playlist?", isPresented: $confirmDelete) {
            Button("Delete Playlist", role: .destructive) { deletePlaylist() }
        } message: { Text("Your music files are untouched.") }
    }

    private var title: String {
        if case .playlist = route { return store.pagePlaylist?.name ?? "Playlist" }
        if route == .songs && !store.search.isEmpty { return "Results" }
        return route.title
    }

    /// An album page shows its first song's cover and details.
    private var albumTrack: Track? {
        if case .album = route { return model.tracks.first }
        return nil
    }

    private func summary(_ album: Track?) -> String {
        if model.loading && model.tracks.isEmpty { return "Loading…" }
        var text = model.total == 1 ? "1 track" : "\(model.total.formatted()) tracks"
        if model.total > 0 && model.total <= model.tracks.count {
            text += " · " + longTime(model.tracks.reduce(0) { $0 + $1.duration })
        }
        guard let album else { return text }
        return [album.year > 0 ? String(album.year) : nil, text, album.quality].compactMap { $0 }.joined(separator: " · ")
    }

    private var ruleLine: String? {
        if let rule = model.playlist?.query { return "Smart playlist · \(rule)" }
        guard !store.search.isEmpty, let e = model.explanation, e.hasPrefix("Matches rule") else { return nil }
        return e.replacingOccurrences(of: "Matches rule: ", with: "Rule · ")
    }

    private var canPlay: Bool { !model.tracks.isEmpty && model.error == nil }

    private var buttons: some View {
        HStack(spacing: 8) {
            Button { store.playPage(rule: model.rule, playlist: model.playlistID) } label: { Label("Play", systemImage: "play.fill") }
                .glassButton(prominent: true)
                .disabled(!canPlay)
            Button { store.playPage(rule: model.rule, playlist: model.playlistID, shuffle: true) } label: { Label("Shuffle", systemImage: "shuffle") }
                .glassButton()
                .disabled(!canPlay)
            if case .album = route, !model.tracks.isEmpty, model.tracks.allSatisfy(\.streamed) {
                let ids = model.tracks.map(\.id)
                let kept = model.tracks.allSatisfy { store.core.keptState(id: $0.id).first == true }
                Button { keep(ids, !kept) } label: { Image(systemName: kept ? "pin.fill" : "pin") }
                    .glassButton()
                    .help(kept ? "Stop keeping this album on this computer" : "Keep this album on this computer, to play without the network")
            }
            if let playlist = model.playlist {
                if !playlist.smart {
                    Button { store.addingTo = playlist } label: { Label("Add songs", systemImage: "plus") }.glassButton()
                }
                Button { store.editor = playlist } label: { Label("Edit", systemImage: "pencil") }.glassButton()
                Menu {
                    Button("Export as M3U8…") { exportPlaylist(playlist) }
                    Divider()
                    Button("Delete Playlist…", role: .destructive) { confirmDelete = true }
                } label: { Image(systemName: "ellipsis") }
                    .menuIndicator(.hidden)
                    .fixedSize()
                    .glassButton()
            } else {
                Button { saveAsPlaylist() } label: { Image(systemName: "text.badge.plus") }
                    .glassButton()
                    .help("Save as a playlist")
            }
        }
        .controlSize(.large)
    }

    private func keep(_ ids: [String], _ on: Bool) {
        Task {
            if let n = await store.ask({ try $0.keepOffline(ids: ids, keep: on) }) {
                store.notify(on ? "Downloading \(count(Int(n), "song")) to keep on this computer…" : "Stopped keeping \(count(Int(n), "song")).")
                store.libraryChanged()
            }
        }
    }

    /// Save what the page shows: a smart playlist of its rule, or the songs shown (or selected).
    private func saveAsPlaylist() {
        var playlist = PlaylistModel.new()
        let rule = model.rule.trimmingCharacters(in: .whitespaces)
        playlist.query = rule.isEmpty ? nil : rule
        let selected = model.tracks.filter { store.selectedIDs.contains($0.id) }
        playlist.trackIds = (selected.count > 1 ? selected : model.tracks).map(\.id)
        playlist.name = store.search.trimmingCharacters(in: .whitespaces)
        store.editor = playlist
    }

    private func exportPlaylist(_ playlist: PlaylistModel) {
        let panel = NSSavePanel()
        panel.nameFieldStringValue = "\(playlist.name).m3u8"
        guard panel.runModal() == .OK, let path = panel.url?.path else { return }
        let id = playlist.id
        store.job { try $0.exportPlaylist(id: id, path: path); return "Playlist exported." }
    }

    private func deletePlaylist() {
        guard let id = model.playlist?.id else { return }
        store.run { try $0.deletePlaylist(id: id) }
        store.navigate(.songs)
        store.notify("Playlist deleted. Your music files are untouched.")
        Task { try? await Task.sleep(for: .milliseconds(200)); store.reloadPlaylists() }
    }
}

/// Folders › Music › Albums: each part opens that folder.
struct Breadcrumbs: View {
    @Environment(Store.self) private var store
    let path: String
    @State private var roots: [String] = []

    var body: some View {
        HStack(spacing: 4) {
            Button("Folders") { store.navigate(.folders) }.buttonStyle(.link)
            ForEach(parts, id: \.self) { part in
                Image(systemName: "chevron.right").font(.caption2).foregroundStyle(.secondary)
                Button((part as NSString).lastPathComponent) { store.navigate(.folder(part)) }
                    .buttonStyle(.link)
                    .disabled(part == path)
            }
        }
        .font(.callout)
        .task { roots = (try? store.core.folders()) ?? [] }
    }

    /// The folders from the music folder this one is in, down to it.
    private var parts: [String] {
        guard let root = roots.filter({ path.hasPrefix($0) }).max(by: { $0.count < $1.count }) else { return [path] }
        var list = [root]
        var at = root
        for piece in path.dropFirst(root.count).split(separator: "/") {
            at = (at as NSString).appendingPathComponent(String(piece))
            list.append(at)
        }
        return list
    }
}

/// A folder's own folders, above its songs.
struct FolderStrip: View {
    @Environment(Store.self) private var store
    let path: String
    @State private var folders: [Folder] = []

    var body: some View {
        if !folders.isEmpty {
            ScrollView(.horizontal, showsIndicators: false) {
                HStack(spacing: 10) {
                    ForEach(folders, id: \.path) { folder in
                        Button { store.navigate(.folder(folder.path)) } label: {
                            HStack(spacing: 8) {
                                if let art = folder.artwork {
                                    Cover(path: art, seed: folder.name, side: 28)
                                } else {
                                    Image(systemName: "folder.fill").foregroundStyle(store.colors.accent).frame(width: 28)
                                }
                                VStack(alignment: .leading, spacing: 0) {
                                    Text(folder.name).lineLimit(1)
                                    Text(count(Int(folder.songs), "song")).font(.caption).foregroundStyle(.secondary)
                                }
                            }
                            .padding(.vertical, 6).padding(.horizontal, 10)
                            .background(store.colors.raised, in: RoundedRectangle(cornerRadius: 8))
                        }
                        .buttonStyle(.plain)
                    }
                }
                .padding(.horizontal, 24)
            }
            .padding(.bottom, 10)
        }
        Color.clear.frame(height: 0)
            .task(id: path) { folders = await store.load { try $0.subfolders(path: path) } ?? [] }
    }
}

/// Songs a music server found for the search that are not in the library.
struct ServerSongs: View {
    @Environment(Store.self) private var store
    @State private var found: [Song] = []
    @State private var generation: UInt64 = 0

    var body: some View {
        Group {
            if !found.isEmpty {
                VStack(alignment: .leading, spacing: 6) {
                    HStack {
                        Text("On your server").font(.headline)
                        Text("Not in your library yet. Playing one fetches it.").font(.caption).foregroundStyle(.secondary)
                    }
                    ScrollView(.horizontal, showsIndicators: false) {
                        HStack(spacing: 10) {
                            ForEach(Array(found.enumerated()), id: \.element.id) { index, song in
                                HStack(spacing: 8) {
                                    Cover(path: song.artwork, seed: song.album, side: 36)
                                    VStack(alignment: .leading) {
                                        Text(song.title).lineLimit(1)
                                        Text(song.artist).font(.caption).foregroundStyle(.secondary).lineLimit(1)
                                    }
                                    .frame(maxWidth: 180, alignment: .leading)
                                    Button { store.core.playServerSongs(generation: generation, start: UInt32(index)) } label: { Image(systemName: "play.fill") }
                                        .buttonStyle(.borderless)
                                }
                                .padding(6)
                                .background(store.colors.raised, in: RoundedRectangle(cornerRadius: 8))
                            }
                        }
                    }
                }
                .padding(.horizontal, 24)
                .padding(.bottom, 8)
            }
        }
        .task(id: store.search) {
            found = []
            let text = store.search.trimmingCharacters(in: .whitespaces)
            guard text.count >= 2, !store.servers.isEmpty else { return }
            try? await Task.sleep(for: .milliseconds(400))
            if Task.isCancelled { return }
            let core = store.core
            generation = core.searchServers(query: text)
            for _ in 0..<6 {
                try? await Task.sleep(for: .milliseconds(600))
                if Task.isCancelled { return }
                let songs = core.serverResults(generation: generation)
                if !songs.isEmpty { found = songs; break }
            }
        }
    }
}

/// A way out of an empty page.
enum Step {
    case clearSearch, palette, songs, addFolder, importing

    var label: (String, String) {
        switch self {
        case .clearSearch: ("Clear search", "xmark")
        case .palette: ("Search everything", "command")
        case .songs: ("Browse songs", "music.note")
        case .addFolder: ("Add music folder", "folder.badge.plus")
        case .importing: ("Import a library", "square.and.arrow.down")
        }
    }
}

struct Problem: View {
    @Environment(Store.self) private var store
    let title: String
    let detail: String
    let steps: [Step]

    var body: some View {
        VStack(spacing: 10) {
            Image(systemName: "magnifyingglass").font(.system(size: 28)).foregroundStyle(store.colors.ink3)
            Text(title).font(.title3.weight(.medium)).multilineTextAlignment(.center)
            Text(detail).foregroundStyle(store.colors.ink2).multilineTextAlignment(.center).frame(maxWidth: 460)
            HStack {
                ForEach(Array(steps.enumerated()), id: \.offset) { i, step in
                    let (label, icon) = step.label
                    Button { run(step) } label: { Label(label, systemImage: icon) }
                        .glassButton(prominent: i == 0)
                }
            }
            .padding(.top, 6)
        }
        .padding(40)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    private func run(_ step: Step) {
        switch step {
        case .clearSearch: store.search = ""
        case .palette: store.palette = true
        case .songs: store.navigate(.songs)
        case .addFolder: store.addFolder()
        case .importing: store.navigate(.importing)
        }
    }
}

/// Rows that breathe while a long list loads.
struct Skeleton: View {
    @Environment(Store.self) private var store
    @State private var on = false

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            ForEach(0..<9, id: \.self) { i in
                HStack(spacing: 12) {
                    RoundedRectangle(cornerRadius: 4).fill(store.colors.raised).frame(width: 36, height: 36)
                    VStack(alignment: .leading, spacing: 7) {
                        RoundedRectangle(cornerRadius: 3).fill(store.colors.raised).frame(width: 160 + CGFloat(i % 3) * 50, height: 10)
                        RoundedRectangle(cornerRadius: 3).fill(store.colors.raised.opacity(0.7)).frame(width: 110 + CGFloat(i % 4) * 30, height: 8)
                    }
                }
            }
            Spacer()
        }
        .padding(24)
        .frame(maxWidth: .infinity, alignment: .leading)
        .opacity(on ? 1 : 0.45)
        .onAppear {
            guard store.motion else { return }
            withAnimation(.easeInOut(duration: 0.7).repeatForever()) { on = true }
        }
    }
}
