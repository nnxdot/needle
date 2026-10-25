import SwiftUI

/// Search, with its field on the tab bar: songs, albums, and artists as you type, or rules
/// such as "rating is at least 4", as on the desktop. Genres to browse while it is empty.
struct SearchView: View {
    @Environment(Store.self) private var store
    @State private var query = ""
    @State private var songs: [Song] = []
    @State private var albums: [Album] = []
    @State private var artists: [Artist] = []
    @State private var genres: [Genre] = []
    @State private var hints: [Suggestion] = []
    @State private var server: [Song] = []
    @State private var generation: UInt64 = 0

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 0) {
                if !hints.isEmpty {
                    ScrollView(.horizontal) {
                        HStack {
                            ForEach(Array(hints.prefix(10).enumerated()), id: \.offset) { _, hint in
                                Button { query = apply(hint, to: query) } label: {
                                    VStack(alignment: .leading, spacing: 0) {
                                        Text(hint.label).font(.needle(.subheadline).weight(.medium))
                                        if !hint.detail.isEmpty { Text(hint.detail).font(.needle(.caption2)).foregroundStyle(.secondary) }
                                    }
                                }
                                .buttonStyle(.glass)
                            }
                        }
                        .padding(.horizontal)
                    }
                    .scrollIndicators(.hidden)
                    .padding(.top, 8)
                }
                if query.trimmingCharacters(in: .whitespaces).isEmpty {
                    if !genres.isEmpty {
                        SectionTitle(title: "Browse").padding(.bottom, 10)
                        GenreTiles(genres: genres)
                    }
                } else if songs.isEmpty && albums.isEmpty && artists.isEmpty && server.isEmpty {
                    ContentUnavailableView.search(text: query).padding(.top, 60)
                } else {
                    results
                }
            }
            .padding(.bottom, 24)
        }
        .navigationTitle("Search")
        .themed()
        .searchable(text: $query, prompt: "Songs, albums, artists")
        .task(id: store.libraryVersion) { genres = await store.load { try $0.genres() } ?? [] }
        .task(id: "\(query)|\(store.ratingVersion)") {
            let words = query.trimmingCharacters(in: .whitespaces)
            let typed = query
            hints = await store.load { $0.suggest(input: typed, cursor: UInt32(typed.utf16.count)) } ?? []
            guard !words.isEmpty else { songs = []; albums = []; artists = []; server = []; return }
            try? await Task.sleep(for: .milliseconds(220))
            guard !Task.isCancelled else { return }
            async let foundSongs = store.load { try $0.search(query: words) }
            async let foundAlbums = store.load { core in
                try core.albums().filter { $0.title.localizedCaseInsensitiveContains(words) || $0.artist.localizedCaseInsensitiveContains(words) }
            }
            async let foundArtists = store.load { core in
                try core.artists().filter { $0.name.localizedCaseInsensitiveContains(words) }
            }
            let (s, a, r) = await (foundSongs, foundAlbums, foundArtists)
            guard !Task.isCancelled else { return }
            songs = s ?? []
            albums = Array((a ?? []).prefix(20))
            artists = Array((r ?? []).prefix(10))
            // Music servers that search as you type (an experimental switch on the server).
            let asked = await store.load { $0.searchServers(query: words) } ?? 0
            generation = asked
            server = []
            for _ in 0..<12 where asked > 0 {
                try? await Task.sleep(for: .milliseconds(500))
                guard !Task.isCancelled else { return }
                let heard = await store.load { $0.serverResults(generation: asked) } ?? []
                if !heard.isEmpty { server = heard; break }
            }
        }
    }

    @ViewBuilder
    private var results: some View {
        if !artists.isEmpty {
            SectionTitle(title: "Artists")
            Shelf {
                ForEach(artists) { artist in
                    NavigationLink(value: Route.artist(artist.name)) {
                        VStack(spacing: 6) {
                            Artwork(path: artist.artwork, seed: artist.name, size: 240)
                                .frame(width: 96, height: 96).clipShape(.circle)
                            Text(artist.name).font(.needle(.footnote).weight(.medium)).lineLimit(1)
                        }
                        .frame(width: 100)
                    }
                    .buttonStyle(Pressable())
                }
            }
        }
        if !albums.isEmpty {
            SectionTitle(title: "Albums")
            Shelf {
                ForEach(albums) { AlbumTile(album: $0) }
            }
        }
        if !server.isEmpty {
            SectionTitle(title: "On your server")
            LazyVStack(spacing: 10) {
                ForEach(Array(server.prefix(30).enumerated()), id: \.element.id) { i, song in
                    SongRow(song: song) { let g = generation; store.core.playServerSongs(generation: g, start: UInt32(i)); store.played() }
                }
            }
            .padding(.horizontal)
            .padding(.top, 8)
        }
        if !songs.isEmpty {
            SectionTitle(title: "Songs")
            LazyVStack(spacing: 10) {
                ForEach(Array(songs.prefix(100).enumerated()), id: \.element.id) { i, song in
                    SongRow(song: song) { store.play(songs, from: i) }
                }
            }
            .padding(.horizontal)
            .padding(.top, 8)
        }
    }
}
