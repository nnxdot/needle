import SwiftUI

/// Home: what played lately, what is new, and what is loved most.
struct HomePage: View {
    @Environment(Store.self) private var store
    @State private var home: HomeData?

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 30) {
                Text(greeting).font(store.display(34))
                NowCard()
                if let home {
                    Shelf(title: "Jump back in", detail: "Albums you played last", albums: home.recent)
                    Shelf(title: "New in your library", detail: "Added most recently", albums: home.added)
                    Shelf(title: "On repeat", detail: "Your most played albums", albums: home.mostPlayed)
                    Shelf(title: "Favorites", detail: "Albums with the songs you love", albums: home.favorites)
                    if !home.artists.isEmpty {
                        VStack(alignment: .leading, spacing: 10) {
                            ShelfTitle(title: "Artists you play most", detail: nil, all: .artists)
                            ScrollView(.horizontal, showsIndicators: false) {
                                LazyHStack(alignment: .top, spacing: 20) {
                                    ForEach(home.artists) { artist in ArtistTile(artist: artist, side: 132) }
                                }
                                .padding(.horizontal, 2)
                            }
                        }
                    }
                }
            }
            .padding(28)
        }
        .scrollContentBackground(.hidden)
        .task(id: store.libraryVersion) {
            home = JSON.decode(HomeData.self, await store.load { try $0.homeJson() })
        }
    }

    private var greeting: String {
        switch Calendar.current.component(.hour, from: Date()) {
        case 5..<12: "Good morning"
        case 12..<18: "Good afternoon"
        default: "Good evening"
        }
    }
}

/// The song playing, large, at the top of Home.
private struct NowCard: View {
    @Environment(Store.self) private var store

    var body: some View {
        if let p = store.playback, p.hasSong {
            let colors = store.colors
            HStack(spacing: 18) {
                Button { store.big = true } label: { Cover(path: p.artwork, seed: p.album + p.artist, side: 96, corner: 8) }
                    .buttonStyle(.plain)
                VStack(alignment: .leading, spacing: 4) {
                    Text(p.playing ? "Playing now" : "Paused").font(.caption.weight(.semibold)).foregroundStyle(colors.accent)
                    Text(p.title).font(.title2.weight(.semibold)).lineLimit(1)
                    Text("\(p.artist) · \(p.album)").foregroundStyle(colors.ink2).lineLimit(1)
                }
                Spacer()
                Button { store.toggle() } label: { Label(p.playing ? "Pause" : "Resume", systemImage: p.playing ? "pause.fill" : "play.fill") }
                    .glassButton(prominent: true)
                    .controlSize(.large)
            }
            .padding(16)
            .background {
                LinearGradient(colors: [colors.glow.opacity(0.35), colors.raised.opacity(0.6)], startPoint: .leading, endPoint: .trailing)
                    .clipShape(RoundedRectangle(cornerRadius: 16, style: .continuous))
            }
        }
    }
}

struct ShelfTitle: View {
    @Environment(Store.self) private var store
    let title: String
    let detail: String?
    let all: Route?

    var body: some View {
        HStack(alignment: .firstTextBaseline) {
            VStack(alignment: .leading, spacing: 2) {
                Text(title).font(.title3.weight(.semibold))
                if let detail { Text(detail).font(.callout).foregroundStyle(store.colors.ink2) }
            }
            Spacer()
            if let all { Button("See all") { store.navigate(all) }.buttonStyle(.link) }
        }
    }
}

/// A row of album covers.
struct Shelf: View {
    let title: String
    let detail: String
    let albums: [AlbumSummary]

    var body: some View {
        if !albums.isEmpty {
            VStack(alignment: .leading, spacing: 10) {
                ShelfTitle(title: title, detail: detail, all: .albums)
                ScrollView(.horizontal, showsIndicators: false) {
                    LazyHStack(alignment: .top, spacing: 18) {
                        ForEach(albums) { album in AlbumTile(album: album, side: 156) }
                    }
                    .padding(.horizontal, 2)
                    .padding(.bottom, 4)
                }
            }
        }
    }
}

/// An album: its cover (with a play button on hover), name, and artist.
struct AlbumTile: View {
    @Environment(Store.self) private var store
    let album: AlbumSummary
    let side: CGFloat
    @State private var hover = false

    var body: some View {
        VStack(alignment: .leading, spacing: 5) {
            ZStack(alignment: .bottomTrailing) {
                Cover(path: album.artwork, seed: album.album + album.artist, side: side, corner: 8)
                    .shadow(color: .black.opacity(hover ? 0.3 : 0.15), radius: hover ? 12 : 6, y: 4)
                if hover {
                    Button { play() } label: {
                        Image(systemName: "play.fill").font(.system(size: 14, weight: .bold)).foregroundStyle(.white)
                            .frame(width: 34, height: 34)
                            .background(store.colors.accent, in: Circle())
                    }
                    .buttonStyle(.plain)
                    .padding(8)
                    .transition(.scale.combined(with: .opacity))
                }
            }
            Text(album.title).font(.callout.weight(.medium)).lineLimit(1)
            Text(album.year > 0 ? "\(album.artist) · \(album.year)" : album.artist)
                .font(.caption).foregroundStyle(store.colors.ink2).lineLimit(1)
        }
        .frame(width: side)
        .contentShape(Rectangle())
        .onHover { h in withAnimation(.snappy(duration: 0.15)) { hover = h } }
        .onTapGesture { store.navigate(album.route) }
        .contextMenu { GroupMenu(route: album.route, title: album.title) }
    }

    private func play() {
        let (rule, reason) = (album.query, "From the album \(album.album)")
        Task { _ = await store.ask { try $0.playView(query: rule, playlist: nil, filter: "", start: 0, shuffle: false, reason: reason) } }
    }
}

/// An artist: a round photo and their name.
struct ArtistTile: View {
    @Environment(Store.self) private var store
    let artist: ArtistSummary
    let side: CGFloat

    var body: some View {
        VStack(spacing: 6) {
            ArtistPhoto(name: artist.name, side: side)
                .shadow(color: .black.opacity(0.15), radius: 6, y: 3)
            Text(artist.title).font(.callout.weight(.medium)).lineLimit(1)
            Text(artist.albums == 1 ? count(artist.tracks, "track") : "\(artist.albums) albums · \(count(artist.tracks, "track"))")
                .font(.caption).foregroundStyle(store.colors.ink2).lineLimit(1)
        }
        .frame(width: side)
        .contentShape(Rectangle())
        .onTapGesture { store.navigate(.artist(artist.name)) }
        .contextMenu { GroupMenu(route: .artist(artist.name), title: artist.title) }
    }
}

/// Every album, as covers; the search narrows them.
struct AlbumsPage: View {
    @Environment(Store.self) private var store
    @State private var albums: [AlbumSummary]?

    var body: some View {
        TileGrid(title: "Albums", summary: albums.map { count($0.count, "album") }) {
            ForEach(albums ?? []) { AlbumTile(album: $0, side: 172) }
        }
        .task(id: "\(store.search)\(store.libraryVersion)") {
            try? await Task.sleep(for: .milliseconds(store.search.isEmpty ? 0 : 150))
            let q = store.search.trimmingCharacters(in: .whitespaces)
            albums = JSON.decode([AlbumSummary].self, await store.load { try $0.albumsJson(query: q) }) ?? []
        }
    }
}

/// Every artist, as photos; the search narrows them.
struct ArtistsPage: View {
    @Environment(Store.self) private var store
    @State private var artists: [ArtistSummary]?

    var body: some View {
        TileGrid(title: "Artists", summary: artists.map { count($0.count, "artist") }) {
            ForEach(artists ?? []) { ArtistTile(artist: $0, side: 160) }
        }
        .task(id: "\(store.search)\(store.libraryVersion)") {
            try? await Task.sleep(for: .milliseconds(store.search.isEmpty ? 0 : 150))
            let q = store.search.trimmingCharacters(in: .whitespaces)
            artists = JSON.decode([ArtistSummary].self, await store.load { try $0.artistsJson(query: q) }) ?? []
        }
    }
}

/// A page of tiles, under its title.
struct TileGrid<Content: View>: View {
    @Environment(Store.self) private var store
    let title: String
    let summary: String?
    @ViewBuilder let content: Content

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 18) {
                VStack(alignment: .leading, spacing: 2) {
                    Text(store.search.isEmpty ? title : "\(title) matching “\(store.search)”").font(store.display(28))
                    Text(summary ?? "Loading…").font(.callout).foregroundStyle(store.colors.ink2)
                }
                LazyVGrid(columns: [GridItem(.adaptive(minimum: 160, maximum: 220), spacing: 22, alignment: .top)], spacing: 26) {
                    content
                }
            }
            .padding(28)
        }
    }
}

/// An artist: their photo, their albums, and their radio.
struct ArtistPage: View {
    @Environment(Store.self) private var store
    let name: String
    @State private var albums: [AlbumSummary]?
    @State private var top: [Song] = []
    @State private var similar: [String]?

    var body: some View {
        let colors = store.colors
        ScrollView {
            VStack(alignment: .leading, spacing: 24) {
                HStack(alignment: .bottom, spacing: 22) {
                    ArtistPhoto(name: name, side: 170, online: true)
                        .shadow(color: colors.glow.opacity(0.35), radius: 24, y: 10)
                    VStack(alignment: .leading, spacing: 6) {
                        Text(name.isEmpty ? "Unknown artist" : name).font(store.display(42)).lineLimit(2)
                        if let albums {
                            Text("\(count(albums.count, "album")) · \(count(albums.reduce(0) { $0 + $1.tracks }, "track"))")
                                .foregroundStyle(colors.ink2)
                        }
                        HStack(spacing: 8) {
                            Button { play(shuffle: false) } label: { Label("Play", systemImage: "play.fill") }.glassButton(prominent: true)
                            Button { play(shuffle: true) } label: { Label("Shuffle", systemImage: "shuffle") }.glassButton()
                            Button { store.artistRadio(name) } label: { Label("Radio", systemImage: "dot.radiowaves.left.and.right") }.glassButton()
                            Menu {
                                if let similar {
                                    if similar.isEmpty { Text("No other artists are measured yet.") }
                                    ForEach(similar, id: \.self) { other in
                                        Button(other) { store.blend(name, other) }
                                    }
                                } else {
                                    Text("Listening for similar artists…")
                                }
                            } label: { Label("Blend", systemImage: "circle.lefthalf.filled") }
                                .glassButton()
                                .fixedSize()
                                .help("Blend with another artist")
                        }
                        .controlSize(.large)
                        .padding(.top, 8)
                    }
                }
                .padding(.top, 8)
                if !top.isEmpty {
                    VStack(alignment: .leading, spacing: 8) {
                        Text("Top songs").font(.title3.weight(.semibold))
                        LazyVGrid(columns: [GridItem(.adaptive(minimum: 300), spacing: 12)], alignment: .leading, spacing: 6) {
                            ForEach(Array(top.prefix(10).enumerated()), id: \.element.id) { index, song in
                                HStack(spacing: 10) {
                                    Cover(path: song.artwork, seed: song.album + name, side: 38, corner: 4)
                                    VStack(alignment: .leading, spacing: 1) {
                                        Text(song.title).lineLimit(1)
                                        Text(song.album).font(.caption).foregroundStyle(colors.ink2).lineLimit(1)
                                    }
                                    Spacer()
                                    Text(clock(song.duration)).font(.caption).monospacedDigit().foregroundStyle(colors.ink3)
                                }
                                .padding(6)
                                .contentShape(Rectangle())
                                .onTapGesture(count: 2) { store.play(top.map(\.asTrack), from: index, reason: "From \(name)") }
                                .contextMenu { SongMenu(tracks: [song.asTrack]) }
                            }
                        }
                    }
                }
                VStack(alignment: .leading, spacing: 12) {
                    Text("Albums").font(.title3.weight(.semibold))
                    LazyVGrid(columns: [GridItem(.adaptive(minimum: 160, maximum: 220), spacing: 22, alignment: .top)], spacing: 26) {
                        ForEach(albums ?? []) { AlbumTile(album: $0, side: 172) }
                    }
                }
            }
            .padding(28)
        }
        .background(alignment: .top) {
            LinearGradient(colors: [colors.glow.opacity(colors.dark ? 0.25 : 0.18), .clear], startPoint: .top, endPoint: .center)
                .frame(height: 360)
                .ignoresSafeArea()
        }
        .task(id: "\(name)\(store.libraryVersion)") {
            albums = JSON.decode([AlbumSummary].self, await store.load { try $0.artistAlbumsJson(name: name) }) ?? []
            top = await store.load { try $0.artistSongs(name: name) } ?? []
            similar = await store.load { $0.similarArtists(artist: name) } ?? []
        }
    }

    private func play(shuffle: Bool) {
        let (rule, reason) = (Route.artist(name).base, "From \(name)")
        Task { _ = await store.ask { try $0.playView(query: rule, playlist: nil, filter: "", start: 0, shuffle: shuffle, reason: reason) } }
    }
}

/// The music folders, as they are on disk.
struct FoldersPage: View {
    @Environment(Store.self) private var store
    @State private var roots: [String] = []
    @State private var inside: [String: [Folder]] = [:]

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 20) {
                VStack(alignment: .leading, spacing: 2) {
                    Text("Folders").font(store.display(28))
                    Text("Your music folders, as they are on disk.").foregroundStyle(store.colors.ink2)
                }
                if roots.isEmpty {
                    Text("Add a music folder to browse it here.").foregroundStyle(.secondary)
                    Button { store.addFolder() } label: { Label("Add music folder", systemImage: "folder.badge.plus") }.glassButton(prominent: true)
                }
                ForEach(roots, id: \.self) { root in
                    VStack(alignment: .leading, spacing: 10) {
                        Button { store.navigate(.folder(root)) } label: {
                            Label((root as NSString).lastPathComponent, systemImage: "folder.fill").font(.title3.weight(.semibold))
                        }
                        .buttonStyle(.plain)
                        Text(root).font(.caption).foregroundStyle(.secondary)
                        LazyVGrid(columns: [GridItem(.adaptive(minimum: 140, maximum: 180), spacing: 18, alignment: .top)], spacing: 20) {
                            ForEach(inside[root] ?? [], id: \.path) { folder in
                                VStack(alignment: .leading, spacing: 4) {
                                    ZStack {
                                        if let art = folder.artwork {
                                            Cover(path: art, seed: folder.name, side: 140, corner: 8)
                                        } else {
                                            RoundedRectangle(cornerRadius: 8).fill(store.colors.raised).frame(width: 140, height: 140)
                                                .overlay { Image(systemName: "folder").font(.largeTitle).foregroundStyle(store.colors.ink3) }
                                        }
                                    }
                                    Text(folder.name).font(.callout.weight(.medium)).lineLimit(1)
                                    Text(count(Int(folder.songs), "song")).font(.caption).foregroundStyle(.secondary)
                                }
                                .frame(width: 140)
                                .contentShape(Rectangle())
                                .onTapGesture { store.navigate(.folder(folder.path)) }
                            }
                        }
                    }
                }
            }
            .padding(28)
        }
        .task(id: store.libraryVersion) {
            roots = await store.load { try $0.folders() } ?? []
            for root in roots {
                inside[root] = await store.load { try $0.subfolders(path: root) } ?? []
            }
        }
    }
}
