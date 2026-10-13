import SwiftUI
import UniformTypeIdentifiers

/// A row of cards that scrolls sideways and stops at each one.
struct Shelf<Content: View>: View {
    @ViewBuilder let content: () -> Content

    var body: some View {
        ScrollView(.horizontal) {
            LazyHStack(alignment: .top, spacing: 12) { content() }
                .scrollTargetLayout()
                .padding(.vertical, 6)
        }
        .scrollTargetBehavior(.viewAligned)
        .scrollIndicators(.hidden)
        .contentMargins(.horizontal, 16, for: .scrollContent)
    }
}

/// Needle reading the music folder, on glass, while it works.
struct ScanPill: View {
    @Environment(Store.self) private var store

    var body: some View {
        if store.scan.running {
            HStack(spacing: 8) {
                ProgressView().controlSize(.small)
                Text(store.scan.imported > 0 ? "Reading your music… \(count(Int(store.scan.imported), "song"))" : "Reading your music…")
                    .font(.footnote.weight(.medium))
            }
            .padding(.horizontal, 14).padding(.vertical, 9)
            .glassEffect(.regular, in: .capsule)
            .padding(.top, 4)
        }
    }
}

// MARK: - Home

struct HomeView: View {
    @Environment(Store.self) private var store
    @State private var home: Home?
    @State private var playlists: [Playlist] = []
    @State private var importing = false

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 0) {
                ScanPill().frame(maxWidth: .infinity)
                if let home {
                    if home.recent.isEmpty && home.added.isEmpty && home.mostPlayed.isEmpty {
                        empty
                    } else {
                        picks(home)
                        shelf("Recently Played", home.recent, route: nil)
                        shelf("Recently Added", home.added, route: .albums)
                        shelf("Most Played", home.mostPlayed, route: nil)
                        if !playlists.isEmpty {
                            SectionTitle(title: "Playlists", route: .playlists)
                            Shelf {
                                ForEach(playlists) { PlaylistTile(playlist: $0) }
                            }
                        }
                    }
                }
            }
            .padding(.bottom, 24)
        }
        .navigationTitle("Home")
        .toolbar {
            ToolbarItem(placement: .topBarTrailing) {
                NavigationLink(value: Route.settings) { Image(systemName: "gearshape") }
            }
        }
        .fileImporter(isPresented: $importing, allowedContentTypes: [.audio, .folder], allowsMultipleSelection: true) { result in
            if case .success(let urls) = result { store.importFiles(urls) }
        }
        .task(id: store.libraryVersion) {
            home = await store.load { try $0.home() }
            playlists = await store.load { try $0.playlists() } ?? []
        }
        .refreshable { store.rescan() }
    }

    /// The big cards at the top: the most played albums, or the newest when nothing has played.
    @ViewBuilder
    private func picks(_ home: Home) -> some View {
        let most = home.mostPlayed.prefix(3).map { ($0, "Most played") }
        let recent = home.recent.prefix(2).map { ($0, "Recently played") }
        let added = home.added.prefix(3).map { ($0, "New in your library") }
        var seen = Set<String>()
        let cards = (most + recent + added).filter { seen.insert($0.0.key).inserted }.prefix(5)
        Shelf {
            ForEach(Array(cards), id: \.0.key) { album, kicker in
                BigCard(album: album, kicker: kicker)
            }
        }
        .padding(.top, 8)
    }

    @ViewBuilder
    private func shelf(_ title: String, _ albums: [Album], route: Route?) -> some View {
        if !albums.isEmpty {
            SectionTitle(title: title, route: route)
            Shelf {
                ForEach(albums) { AlbumTile(album: $0) }
            }
        }
    }

    private var empty: some View {
        VStack(spacing: 16) {
            ContentUnavailableView {
                Label("No music yet", systemImage: "music.note.house")
            } description: {
                Text("Import songs from the Files app, or put them in Files › On My iPhone › Needle. Music servers and your computer are in Sources.")
            }
            Button { importing = true } label: {
                Label("Import from Files", systemImage: "folder").padding(.horizontal, 8).padding(.vertical, 4)
            }
            .buttonStyle(.glassProminent)
        }
        .padding(.top, 60)
    }
}

struct PlaylistTile: View {
    let playlist: Playlist

    var body: some View {
        NavigationLink(value: Route.playlist(playlist)) {
            VStack(alignment: .leading, spacing: 3) {
                PlaylistCover(playlist: playlist)
                    .frame(width: 150, height: 150)
                    .clipShape(.rect(cornerRadius: 14))
                    .padding(.bottom, 5)
                Text(playlist.name).font(.subheadline.weight(.medium)).lineLimit(1)
                Text(count(Int(playlist.songs), "song")).font(.footnote).foregroundStyle(.secondary)
            }
            .frame(width: 150)
        }
        .buttonStyle(Pressable())
    }
}

/// A playlist's picture, or a mosaic of four of its covers.
struct PlaylistCover: View {
    let playlist: Playlist

    var body: some View {
        if playlist.artwork == nil || playlist.mosaic.count < 4 {
            Artwork(path: playlist.artwork, seed: playlist.name, size: 400)
                .overlay {
                    if playlist.smart {
                        Image(systemName: "gearshape.2.fill").font(.title).foregroundStyle(.white.opacity(0.85))
                    }
                }
        } else {
            Grid(horizontalSpacing: 0, verticalSpacing: 0) {
                GridRow {
                    Artwork(path: playlist.mosaic[0], seed: "", size: 200)
                    Artwork(path: playlist.mosaic[1], seed: "", size: 200)
                }
                GridRow {
                    Artwork(path: playlist.mosaic[2], seed: "", size: 200)
                    Artwork(path: playlist.mosaic[3], seed: "", size: 200)
                }
            }
        }
    }
}

// MARK: - Library

struct LibraryView: View {
    @Environment(Store.self) private var store
    @State private var added: [Album] = []

    private let rows: [(String, String, Route)] = [
        ("Playlists", "music.note.list", .playlists),
        ("Artists", "music.microphone", .artists),
        ("Albums", "square.stack", .albums),
        ("Songs", "music.note", .songs),
        ("Genres", "guitars", .genres),
        ("Favourites", "star", .favorites),
        ("Recently Added", "clock", .recent),
    ]

    var body: some View {
        ScrollView {
            VStack(spacing: 0) {
                ScanPill()
                VStack(spacing: 0) {
                    ForEach(Array(rows.enumerated()), id: \.offset) { i, row in
                        NavigationLink(value: row.2) {
                            HStack(spacing: 14) {
                                Image(systemName: row.1).font(.title3).foregroundStyle(Color.accentColor).frame(width: 28)
                                Text(row.0).font(.body)
                                Spacer()
                                Image(systemName: "chevron.right").font(.footnote.weight(.bold)).foregroundStyle(.tertiary)
                            }
                            .padding(.horizontal, 16).padding(.vertical, 14)
                            .contentShape(.rect)
                        }
                        .buttonStyle(.plain)
                        if i < rows.count - 1 { Divider().padding(.leading, 58) }
                    }
                }
                .glassEffect(.regular, in: .rect(cornerRadius: 24))
                .padding(.horizontal)
                .padding(.top, 8)
                if !added.isEmpty {
                    SectionTitle(title: "Recently Added")
                    AlbumGrid(albums: Array(added.prefix(12))).padding(.top, 10)
                }
            }
            .padding(.bottom, 24)
        }
        .navigationTitle("Library")
        .refreshable { store.rescan() }
        .task(id: store.libraryVersion) {
            added = await store.load { try $0.home().added } ?? []
        }
    }
}

struct AlbumGrid: View {
    let albums: [Album]

    var body: some View {
        LazyVGrid(columns: [GridItem(.flexible(), spacing: 14), GridItem(.flexible(), spacing: 14)], spacing: 18) {
            ForEach(albums) { AlbumTile(album: $0, width: nil) }
        }
        .padding(.horizontal)
    }
}

struct AlbumsView: View {
    @Environment(Store.self) private var store
    @State private var albums: [Album]?
    @State private var filter = ""

    var body: some View {
        ScrollView {
            AlbumGrid(albums: shown).padding(.vertical, 8)
        }
        .overlay { if albums == nil { ProgressView() } else if albums?.isEmpty == true { ContentUnavailableView("No albums", systemImage: "square.stack") } }
        .navigationTitle("Albums")
        .searchable(text: $filter, placement: .navigationBarDrawer, prompt: "Find in Albums")
        .task(id: store.libraryVersion) { albums = await store.load { try $0.albums() } ?? [] }
    }

    private var shown: [Album] {
        let all = albums ?? []
        guard !filter.isEmpty else { return all }
        return all.filter { $0.title.localizedCaseInsensitiveContains(filter) || $0.artist.localizedCaseInsensitiveContains(filter) }
    }
}

struct ArtistsView: View {
    @Environment(Store.self) private var store
    @State private var artists: [Artist]?
    @State private var filter = ""

    var body: some View {
        List(shown) { artist in
            NavigationLink(value: Route.artist(artist.name)) {
                HStack(spacing: 14) {
                    Artwork(path: artist.artwork, seed: artist.name, size: 160)
                        .frame(width: 52, height: 52).clipShape(.circle)
                    VStack(alignment: .leading, spacing: 2) {
                        Text(artist.name).lineLimit(1)
                        Text(count(Int(artist.albums), "album") + " · " + count(Int(artist.songs), "song"))
                            .font(.subheadline).foregroundStyle(.secondary)
                    }
                }
            }
        }
        .listStyle(.plain)
        .overlay { if artists == nil { ProgressView() } }
        .navigationTitle("Artists")
        .searchable(text: $filter, placement: .navigationBarDrawer, prompt: "Find in Artists")
        .task(id: store.libraryVersion) { artists = await store.load { try $0.artists() } ?? [] }
    }

    private var shown: [Artist] {
        let all = artists ?? []
        return filter.isEmpty ? all : all.filter { $0.name.localizedCaseInsensitiveContains(filter) }
    }
}

struct PlaylistsView: View {
    @Environment(Store.self) private var store
    @State private var playlists: [Playlist]?

    var body: some View {
        List(playlists ?? []) { playlist in
            NavigationLink(value: Route.playlist(playlist)) {
                HStack(spacing: 14) {
                    PlaylistCover(playlist: playlist).frame(width: 56, height: 56).clipShape(.rect(cornerRadius: 8))
                    VStack(alignment: .leading, spacing: 2) {
                        Text(playlist.name).lineLimit(1)
                        Text(playlist.smart ? "Smart playlist" : count(Int(playlist.songs), "song"))
                            .font(.subheadline).foregroundStyle(.secondary)
                    }
                }
            }
        }
        .listStyle(.plain)
        .overlay {
            if playlists == nil { ProgressView() }
            else if playlists?.isEmpty == true {
                ContentUnavailableView("No playlists", systemImage: "music.note.list", description: Text("Playlists made in Needle on your computer come over with a sync file or a music server."))
            }
        }
        .navigationTitle("Playlists")
        .task(id: store.libraryVersion) { playlists = await store.load { try $0.playlists() } ?? [] }
    }
}

struct GenresView: View {
    @Environment(Store.self) private var store
    @State private var genres: [Genre]?

    var body: some View {
        ScrollView {
            GenreTiles(genres: genres ?? []).padding(.vertical, 8)
        }
        .overlay { if genres == nil { ProgressView() } }
        .navigationTitle("Genres")
        .task(id: store.libraryVersion) { genres = await store.load { try $0.genres() } ?? [] }
    }
}

/// Genres as coloured tiles, with their name on glass.
struct GenreTiles: View {
    let genres: [Genre]

    var body: some View {
        LazyVGrid(columns: [GridItem(.flexible(), spacing: 12), GridItem(.flexible(), spacing: 12)], spacing: 12) {
            ForEach(genres) { genre in
                NavigationLink(value: Route.genre(genre.name)) {
                    Artwork(path: genre.artwork, seed: genre.name, size: 300)
                        .frame(height: 104)
                        .frame(maxWidth: .infinity)
                        .overlay(alignment: .bottomLeading) {
                            Text(genre.name).font(.subheadline.bold()).lineLimit(1)
                                .padding(.horizontal, 12).padding(.vertical, 7)
                                .glassEffect(.regular, in: .rect(cornerRadius: 12))
                                .padding(8)
                        }
                        .clipShape(.rect(cornerRadius: 18))
                }
                .buttonStyle(Pressable())
            }
        }
        .padding(.horizontal)
    }
}

struct SongsView: View {
    var body: some View { SongList(title: "Songs") { try $0.songs() } }
}

struct FavoritesView: View {
    var body: some View { SongList(title: "Favourites") { try $0.favorites() } }
}

struct RecentSongsView: View {
    var body: some View { SongList(title: "Recently Added") { try $0.recentlyAdded() } }
}

struct GenreView: View {
    let name: String
    var body: some View {
        let name = name
        SongList(title: name) { try $0.genreSongs(name: name) }
    }
}

// MARK: - An album, an artist, a playlist

struct AlbumView: View {
    @Environment(Store.self) private var store
    let album: Album
    @State private var songs: [Song]?

    var body: some View {
        List {
            VStack(spacing: 6) {
                Artwork(path: album.artwork, seed: album.title + album.artist, size: 900)
                    .aspectRatio(1, contentMode: .fit)
                    .frame(maxWidth: 270)
                    .clipShape(.rect(cornerRadius: 18))
                    .shadow(color: .black.opacity(0.45), radius: 22, y: 14)
                    .padding(.bottom, 12)
                Text(album.title).font(.title2.bold()).multilineTextAlignment(.center)
                NavigationLink(value: Route.artist(album.artist)) {
                    Text(album.artist).font(.title3).foregroundStyle(Color.accentColor)
                }
                .buttonStyle(.plain)
                Text(details).font(.footnote).foregroundStyle(.secondary)
                if let songs { PlayButtons(songs: songs).padding(.top, 8) }
            }
            .frame(maxWidth: .infinity)
            .listRowSeparator(.hidden)
            .listRowBackground(Color.clear)
            ForEach(Array((songs ?? []).enumerated()), id: \.element.id) { i, song in
                SongRow(song: song, number: Int(song.trackNumber > 0 ? song.trackNumber : Int64(i + 1))) {
                    store.play(songs ?? [], from: i)
                }
            }
        }
        .listStyle(.plain)
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            ToolbarItem(placement: .topBarTrailing) {
                Menu { AlbumMenu(album: album) } label: { Image(systemName: "ellipsis") }
            }
        }
        .task(id: store.libraryVersion) {
            let key = album.key
            songs = await store.load { try $0.albumSongs(key: key) } ?? []
        }
    }

    private var details: String {
        var parts: [String] = []
        if album.year > 0 { parts.append(String(album.year)) }
        parts.append(count(Int(album.songs), "song"))
        parts.append(clock(album.duration))
        return parts.joined(separator: " · ")
    }
}

struct ArtistView: View {
    @Environment(Store.self) private var store
    let name: String
    @State private var top: [Song] = []
    @State private var albums: [Album]?

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 0) {
                HStack(spacing: 12) {
                    Button { store.play(top) } label: {
                        Label("Play", systemImage: "play.fill").frame(maxWidth: .infinity).padding(.vertical, 6)
                    }
                    .buttonStyle(.glassProminent)
                    Button {
                        let name = name
                        store.run { $0.startArtistRadio(name: name) }
                        store.message = "Starting radio from \(name)"
                    } label: {
                        Label("Radio", systemImage: "dot.radiowaves.left.and.right").frame(maxWidth: .infinity).padding(.vertical, 6)
                    }
                    .buttonStyle(.glass)
                }
                .font(.headline)
                .padding(.horizontal)
                .padding(.top, 8)
                if !top.isEmpty {
                    SectionTitle(title: "Top Songs")
                    VStack(spacing: 10) {
                        ForEach(Array(top.prefix(5).enumerated()), id: \.element.id) { i, song in
                            SongRow(song: song) { store.play(top, from: i) }
                        }
                    }
                    .padding(.horizontal)
                    .padding(.top, 8)
                }
                if let albums, !albums.isEmpty {
                    SectionTitle(title: "Albums")
                    AlbumGrid(albums: albums).padding(.top, 10)
                }
            }
            .padding(.bottom, 24)
        }
        .navigationTitle(name)
        .task(id: store.libraryVersion) {
            let name = name
            top = await store.load { try $0.artistSongs(name: name) } ?? []
            albums = await store.load { try $0.artistAlbums(name: name) } ?? []
        }
    }
}

struct PlaylistView: View {
    let playlist: Playlist
    var body: some View {
        let id = playlist.id
        SongList(title: playlist.name) { try $0.playlistSongs(id: id) }
    }
}
