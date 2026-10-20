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
        // The cards' shadows go past the row; cut off there, they made a band across the page.
        .scrollClipDisabled()
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
        .themed()
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
        ("Folders", "folder", .folders),
        ("History", "clock.arrow.circlepath", .history),
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
        .themed()
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
        .themed()
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
        .themed()
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
    @State private var making = false

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
                ContentUnavailableView("No playlists", systemImage: "music.note.list", description: Text("Make one with +, or bring yours from Needle on your computer with a sync file."))
            }
        }
        .navigationTitle("Playlists")
        .themed()
        .toolbar {
            ToolbarItem(placement: .topBarTrailing) {
                Button { making = true } label: { Image(systemName: "plus") }
            }
        }
        .sheet(isPresented: $making) { NavigationStack { PlaylistEditor {} } }
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
        .themed()
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
    var body: some View { PagedSongs(title: "Songs", query: "") }
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

/// An album, after Apple Music's: the cover across the top fades into a page of its colour,
/// with the album's name, Play and Shuffle over the fade, the songs on the colour, and more
/// by the artist at the foot.
struct AlbumView: View {
    @Environment(Store.self) private var store
    let album: Album
    @State private var songs: [Song]?
    @State private var more: [Album] = []
    @State private var colors: PageColors?
    @State private var selecting = false
    @State private var selected = Set<String>()
    @State private var pastHeader = false

    init(album: Album) {
        self.album = album
        _colors = State(initialValue: PageColors.cached(album.artwork, seed: album.title + album.artist))
    }

    var body: some View {
        let look = store.app.coverColors ? colors ?? .plain(store.colors) : .plain(store.colors)
        List(selection: selecting ? $selected : nil) {
            header(look)
                .listRowInsets(EdgeInsets())
                .listRowSeparator(.hidden)
                .listRowBackground(Color.clear)
                .selectionDisabled()
            ForEach(Array((songs ?? []).enumerated()), id: \.element.id) { i, song in
                SongRow(song: song, number: Int(song.trackNumber > 0 ? song.trackNumber : Int64(i + 1))) {
                    store.play(songs ?? [], from: i)
                }
                .tag(song.id)
                .listRowBackground(Color.clear)
                .listRowSeparatorTint(look.line)
            }
            footer(look)
                .listRowInsets(EdgeInsets())
                .listRowSeparator(.hidden)
                .listRowBackground(Color.clear)
                .selectionDisabled()
        }
        .listStyle(.plain)
        .ignoresSafeArea(edges: .top)
        .scrollEdgeEffectStyle(.soft, for: .top)
        .onScrollGeometryChange(for: Bool.self) { $0.contentOffset.y > $0.containerSize.width * 0.8 } action: { _, past in
            withAnimation(.easeOut(duration: 0.2)) { pastHeader = past }
        }
        .paged(look)
        // The name moves into the bar once the cover has scrolled away.
        .navigationTitle(pastHeader ? album.title : "")
        .navigationBarTitleDisplayMode(.inline)
        .environment(\.editMode, .constant(selecting ? .active : .inactive))
        .toolbar {
            SelectToolbar(selecting: $selecting, selected: $selected, songs: songs ?? [], tint: look.ink)
            if !selecting {
                ToolbarItem(placement: .topBarTrailing) {
                    Menu { AlbumMenu(album: album) } label: { Image(systemName: "ellipsis") }
                        .tint(look.ink)
                }
            }
        }
        .task(id: store.libraryVersion) {
            let (key, artist) = (album.key, album.artist)
            songs = await store.load { try $0.albumSongs(key: key) } ?? []
            more = (await store.load { try $0.artistAlbums(name: artist) } ?? []).filter { $0.key != key }
        }
        .task { colors = await PageColors.of(album.artwork, seed: album.title + album.artist) }
    }

    private func header(_ look: PageColors) -> some View {
        VStack(spacing: 0) {
            Artwork(path: album.artwork, seed: album.title + album.artist, size: 1200)
                .aspectRatio(1, contentMode: .fit)
                .frame(maxWidth: .infinity)
                .overlay(alignment: .bottom) { Fade(color: look.page).frame(height: 230) }
                .modifier(Stretchy())
            VStack(spacing: 5) {
                Text(album.title).font(.title.bold()).multilineTextAlignment(.center)
                // A button, not a link: a link in a list row is laid out as a row, at the left
                // with an arrow.
                Button { store.opening = .artist(album.artist) } label: {
                    Text(album.artist).font(.title3.weight(.medium)).foregroundStyle(look.ink.opacity(0.82)).multilineTextAlignment(.center)
                }
                .buttonStyle(.borderless)
                .disabled(isPlaceholderArtist(album.artist))
                Text(details).font(.footnote.weight(.medium)).foregroundStyle(look.muted)
                if let songs { PlayButtons(songs: songs, look: look).padding(.top, 10) }
            }
            .padding(.horizontal, 22)
            // Over the foot of the fading cover.
            .padding(.top, -86)
            .padding(.bottom, 10)
        }
    }

    @ViewBuilder
    private func footer(_ look: PageColors) -> some View {
        VStack(alignment: .leading, spacing: 0) {
            Text(summary).font(.footnote).foregroundStyle(look.muted)
                .padding(.horizontal).padding(.top, 14)
            if !more.isEmpty, !isPlaceholderArtist(album.artist) {
                SectionTitle(title: "More by \(album.artist)", route: .artist(album.artist))
                ScrollView(.horizontal, showsIndicators: false) {
                    LazyHStack(alignment: .top, spacing: 14) {
                        ForEach(more) { AlbumTile(album: $0, width: 150) }
                    }
                    .padding(.horizontal)
                    .padding(.vertical, 10)
                }
            }
        }
        .padding(.bottom, 30)
    }

    private var details: String {
        var parts: [String] = []
        if album.year > 0 { parts.append(String(album.year)) }
        // The format, when every song has the same one.
        if let songs, let format = songs.first?.format, !format.isEmpty, songs.allSatisfy({ $0.format == format }) {
            parts.append(format.uppercased())
        }
        return parts.joined(separator: " · ")
    }

    private var summary: String {
        "\(count(Int(album.songs), "song")), \(clock(album.duration))"
    }
}

/// A picture's foot fading into the page's colour.
struct Fade: View {
    let color: Color

    var body: some View {
        LinearGradient(stops: [
            .init(color: color.opacity(0), location: 0),
            .init(color: color.opacity(0.55), location: 0.45),
            .init(color: color.opacity(0.9), location: 0.78),
            .init(color: color, location: 1),
        ], startPoint: .top, endPoint: .bottom)
        .allowsHitTesting(false)
    }
}

/// An artist, after Apple Music's: their picture across the top fades into a page of its
/// colour, with the name large over it and Play, Shuffle, and Radio under it; then the newest
/// album in its own box, the top songs, and the albums.
struct ArtistView: View {
    @Environment(Store.self) private var store
    let name: String
    @State private var top: [Song] = []
    @State private var albums: [Album]?
    @State private var photo: String?
    @State private var logo: String?
    /// A wide picture of them from fanart.tv, for the top of the page before their photo.
    @State private var banner: String?
    @State private var colors: PageColors?
    @State private var pastHeader = false

    init(name: String) {
        self.name = name
        _colors = State(initialValue: PageColors.cached(key: "artist:" + name))
    }

    /// Not a real artist: songs without one. Nothing is looked up for it.
    private var placeholder: Bool { isPlaceholderArtist(name) }

    var body: some View {
        let look = store.app.coverColors ? colors ?? .plain(store.colors) : .plain(store.colors)
        ScrollView {
            VStack(alignment: .leading, spacing: 0) {
                header(look)
                if let latest { featured(latest, look) }
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
        .ignoresSafeArea(edges: .top)
        .scrollEdgeEffectStyle(.soft, for: .top)
        .onScrollGeometryChange(for: Bool.self) { $0.contentOffset.y > 450 } action: { _, past in
            withAnimation(.easeOut(duration: 0.2)) { pastHeader = past }
        }
        .paged(look)
        .navigationTitle(pastHeader ? name : "")
        .navigationBarTitleDisplayMode(.inline)
        .task(id: store.libraryVersion) {
            let name = name
            top = await store.load { try $0.artistSongs(name: name) } ?? []
            albums = await store.load { try $0.artistAlbums(name: name) } ?? []
            // The colour at once, from their first album; the picture's when it comes (a first
            // visit downloads it).
            if colors == nil { colors = await PageColors.of(albums?.first?.artwork, seed: name) }
            guard !placeholder else { return }
            async let wide = store.load { $0.artistBanner(name: name) }
            async let portrait = store.load { $0.artistPhoto(name: name) }
            (banner, photo) = (await wide ?? nil, await portrait ?? nil)
            colors = await PageColors.of(picture, seed: name)
            if let colors { PageColors.keep(colors, as: "artist:" + name) }
        }
        // The logo on its own: it shows as soon as it is found, not after the pictures.
        .task(id: store.libraryVersion) {
            guard !placeholder else { return }
            let name = name
            let found = await store.load { $0.artistLogo(name: name) } ?? nil
            if !Task.isCancelled { logo = found }
        }
    }

    /// All their songs, for Play and Shuffle (the page lists only the top ones).
    private func playAll(shuffled: Bool) {
        store.playArtist(name, shuffled: shuffled)
    }

    /// At the top: a wide picture of them, or their photo, or else their first album's cover.
    private var picture: String? {
        banner ?? photo ?? albums?.first?.artwork
    }

    /// The newest album, for the box under the name.
    private var latest: Album? {
        albums?.max { ($0.year, $0.title) < ($1.year, $1.title) }
    }

    /// As Apple Music's: the picture tall across the top, the logo (or name) in the middle
    /// near its foot, and under it a large Play between Shuffle and Radio.
    private func header(_ look: PageColors) -> some View {
        // The page's width, whatever the picture's shape: a picture laid out at its own width
        // made the whole page wider than the screen.
        Color.clear
            .frame(maxWidth: .infinity)
            .frame(height: 590)
            .overlay { Artwork(path: picture, seed: name, size: 1400) }
            .clipped()
            .overlay(alignment: .bottom) { Fade(color: look.page).frame(height: 330) }
            .modifier(Stretchy())
            .overlay(alignment: .bottom) {
                VStack(spacing: 18) {
                    // Their logo when fanart.tv has one, as Apple Music shows artists' own.
                    if let logo {
                        // Logos are mostly white: on a light page, the logo takes the page's ink.
                        Logo(path: logo, ink: look.dark ? nil : look.ink)
                            .frame(maxWidth: 300, maxHeight: 130)
                            .shadow(color: look.page.opacity(0.5), radius: 16)
                            .accessibilityLabel(name)
                    } else {
                        Text(name)
                            .font(.system(size: 46, weight: .heavy))
                            .tracking(-1)
                            .multilineTextAlignment(.center)
                            .lineLimit(2)
                            .minimumScaleFactor(0.5)
                            .shadow(color: look.page.opacity(0.4), radius: 12)
                    }
                    HStack(spacing: 26) {
                        Button { playAll(shuffled: true) } label: {
                            Image(systemName: "shuffle").font(.title3.weight(.semibold)).frame(width: 34, height: 34)
                        }
                        .buttonStyle(.glass)
                        .buttonBorderShape(.circle)
                        .accessibilityLabel("Shuffle")
                        Button { playAll(shuffled: false) } label: {
                            Image(systemName: "play.fill")
                                .font(.system(size: 30, weight: .bold))
                                .foregroundStyle(look.page)
                                .offset(x: 2)
                                .frame(width: 74, height: 74)
                                .background(look.ink, in: .circle)
                                .shadow(color: .black.opacity(0.2), radius: 10, y: 4)
                        }
                        .buttonStyle(Pressable())
                        .accessibilityLabel("Play")
                        Button {
                            let name = name
                            store.run { $0.startArtistRadio(name: name) }
                            store.message = "Starting radio from \(name)"
                        } label: {
                            Image(systemName: "dot.radiowaves.left.and.right").font(.title3.weight(.semibold)).frame(width: 34, height: 34)
                        }
                        .buttonStyle(.glass)
                        .buttonBorderShape(.circle)
                        .accessibilityLabel("Radio")
                    }
                    .disabled(top.isEmpty)
                }
                .padding(.horizontal, 24)
                .padding(.bottom, 4)
            }
    }

    private func featured(_ album: Album, _ look: PageColors) -> some View {
        NavigationLink(value: Route.album(album)) {
            HStack(spacing: 16) {
                Artwork(path: album.artwork, seed: album.title + album.artist, size: 300)
                    .frame(width: 100, height: 100)
                    .clipShape(.rect(cornerRadius: 10))
                    .shadow(color: .black.opacity(0.3), radius: 8, y: 4)
                VStack(alignment: .leading, spacing: 3) {
                    if album.year > 0 {
                        Text(String(album.year)).font(.subheadline).foregroundStyle(look.muted)
                    }
                    Text(album.title).font(.headline).lineLimit(2)
                    Text(count(Int(album.songs), "Song")).font(.subheadline).foregroundStyle(look.muted)
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                Button { store.playAlbum(album) } label: {
                    Image(systemName: "play.fill").frame(width: 30, height: 30)
                }
                .buttonStyle(.glass)
                .buttonBorderShape(.circle)
                .accessibilityLabel("Play \(album.title)")
            }
            .padding(14)
            .glassEffect(.regular.tint(look.ink.opacity(0.05)).interactive(), in: .rect(cornerRadius: 28))
            .contentShape(.rect)
        }
        .buttonStyle(Pressable())
        .contextMenu { AlbumMenu(album: album) }
        .padding(.horizontal)
        .padding(.top, 22)
    }
}
