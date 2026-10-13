import SwiftUI

/// What can be done with chosen songs: the song table's right-click menu, and the song
/// playing's.
struct SongMenu: View {
    @Environment(Store.self) private var store
    let tracks: [Track]
    var route: Route? = nil
    var arrangeable = false
    var playlistID: String? = nil
    /// The menu of the song playing: no Play, no queue.
    var playing = false

    var body: some View {
        let many = tracks.count > 1
        let ids = tracks.map(\.id)
        if let first = tracks.first {
            if !playing {
                Button(many ? "Play \(tracks.count) Tracks" : "Play") { store.play(tracks) }
                Button(many ? "Play \(tracks.count) Next" : "Play Next") { store.playNext(ids) }
                Button(many ? "Add \(tracks.count) to Queue" : "Add to Queue") { store.enqueue(ids) }
                Divider()
            }
            let favorite = tracks.allSatisfy(\.favorite)
            Button(favorite ? "Remove from Favorites" : "Add to Favorites") { store.toggleFavorite(tracks) }
            Menu("Rate") {
                ForEach(0...5, id: \.self) { stars in
                    Button(stars == 0 ? "No stars" : String(repeating: "★", count: stars)) { store.rate(ids, stars) }
                }
            }
            Menu("Add to Playlist") {
                Button("New Playlist from Selection…") {
                    var playlist = PlaylistModel.new()
                    playlist.trackIds = ids
                    store.editor = playlist
                }
                let lists = store.playlists.filter { !$0.smart }
                if !lists.isEmpty { Divider() }
                ForEach(lists, id: \.id) { list in
                    Button(list.name) { store.addToPlaylist(list, ids) }
                }
            }
            if arrangeable, let playlistID {
                Button(many ? "Remove \(tracks.count) from This Playlist" : "Remove from This Playlist") {
                    Task {
                        if let n = await store.ask({ try $0.removeSongsFromPlaylist(id: playlistID, ids: ids) }) {
                            store.notify("Took \(count(Int(n), "song")) out of the playlist.")
                            store.libraryChanged()
                        }
                    }
                }
            }
            let commands = store.plugins.filter(\.enabled).flatMap { p in p.commands.filter(\.forSongs).map { (p.id, $0) } }
            if !commands.isEmpty {
                Menu("Plugins") {
                    ForEach(Array(commands.enumerated()), id: \.offset) { _, pair in
                        Button(pair.1.title) { store.core.runPluginCommand(plugin: pair.0, command: pair.1.id, songs: ids) }
                    }
                }
            }
            let streamed = tracks.filter(\.streamed)
            if !streamed.isEmpty {
                Divider()
                let kept = streamed.allSatisfy { store.core.keptState(id: $0.id).first == true }
                Button(kept ? "Stop Keeping on This Computer" : "Keep on This Computer") { keep(streamed.map(\.id), !kept) }
                Button(streamed.count == 1 ? "Save to My Music" : "Save \(streamed.count) to My Music") { save(streamed.map(\.id)) }
            }
            if !many {
                Divider()
                Button("Go to Album") { store.navigate(first.albumRoute) }
                Button("Go to Artist") { store.navigate(.artist(first.artist)) }
                Button("Start Radio") { store.startRadio(first) }
                if !first.streamed {
                    Button("Edit Tags…") { store.tagging = ids }
                    Button("Split into Stems") {
                        store.focused = first
                        store.panel = .details
                        store.set(["show_inspector": true])
                        let id = first.id
                        store.run { try $0.splitStems(id: id) }
                    }
                    Divider()
                    Button("Show in Finder") { store.reveal(first.filePath) }
                    Button("Copy File Path") { store.copy(first.filePath) }
                }
            } else {
                Button("Edit Tags on \(tracks.count) Tracks…") { store.tagging = ids }
                Divider()
                Text("\(tracks.count) tracks selected")
            }
        }
    }

    private func keep(_ ids: [String], _ on: Bool) {
        Task {
            if let n = await store.ask({ try $0.keepOffline(ids: ids, keep: on) }) {
                store.notify(on ? "Keeping \(count(Int(n), "song")) on this computer." : "Stopped keeping \(count(Int(n), "song")).")
            }
        }
    }

    /// Saves songs from a music server as files in the first music folder.
    private func save(_ ids: [String]) {
        guard let folder = (try? store.core.folders())?.first else {
            store.fail("Add a music folder in Settings › Library first, so Needle knows where to save songs.")
            return
        }
        store.notify("Saving \(count(ids.count, "song"))…")
        Task {
            if let n = await store.ask({ try $0.saveToMusic(ids: ids, folder: folder) }) {
                store.notify("Saved \(count(Int(n), "song")) to \(folder).")
            }
        }
    }
}

/// What can be done with an album or an artist: their tiles' right-click menu.
struct GroupMenu: View {
    @Environment(Store.self) private var store
    let route: Route
    let title: String

    var body: some View {
        Button("Play") { play(shuffle: false) }
        Button("Shuffle") { play(shuffle: true) }
        Button("Play Next") { queue(next: true) }
        Button("Add to Queue") { queue(next: false) }
        Menu("Add to Playlist") {
            ForEach(store.playlists.filter { !$0.smart }, id: \.id) { list in
                Button(list.name) {
                    Task {
                        if let ids = await ids() { store.addToPlaylist(list, ids) }
                    }
                }
            }
        }
        Divider()
        switch route {
        case .album(_, let artist, _):
            Button("Open Album") { store.navigate(route) }
            Button("Go to Artist") { store.navigate(.artist(artist)) }
        case .artist(let name):
            Button("Open Artist") { store.navigate(route) }
            Button("Artist Radio") { store.artistRadio(name) }
        default: EmptyView()
        }
    }

    private var rule: String {
        if case .album(_, _, let query) = route { return query }
        return route.base
    }

    private func play(shuffle: Bool) {
        let (rule, reason) = (rule, route.reason(search: "", playlistName: nil))
        Task { _ = await store.ask { try $0.playView(query: rule, playlist: nil, filter: "", start: 0, shuffle: shuffle, reason: reason) } }
    }

    private func ids() async -> [String]? {
        let rule = rule
        guard let json = await store.ask({ try $0.viewPage(query: rule, playlist: nil, filter: "", offset: 0, size: 5000) }) else { return nil }
        return JSON.decode(TracksPage.self, json)?.tracks.filter { !$0.missing }.map(\.id)
    }

    private func queue(next: Bool) {
        Task {
            guard let ids = await ids() else { return }
            next ? store.playNext(ids) : store.enqueue(ids)
        }
    }
}
