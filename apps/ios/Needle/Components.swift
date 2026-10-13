import SwiftUI

extension Song: Identifiable {}
extension Album: Identifiable { public var id: String { key } }
extension Artist: Identifiable { public var id: String { name } }
extension Playlist: Identifiable {}
extension Genre: Identifiable { public var id: String { name } }

/// The places a screen can open.
enum Route: Hashable {
    case album(Album)
    case artist(String)
    case playlist(Playlist)
    case genre(String)
    case albums, artists, songs, playlists, genres, favorites, recent
    case settings
}

extension View {
    /// Opens the screens of [`Route`] from any navigation stack.
    func routes() -> some View {
        navigationDestination(for: Route.self) { route in
            switch route {
            case .album(let album): AlbumView(album: album)
            case .artist(let name): ArtistView(name: name)
            case .playlist(let playlist): PlaylistView(playlist: playlist)
            case .genre(let name): GenreView(name: name)
            case .albums: AlbumsView()
            case .artists: ArtistsView()
            case .songs: SongsView()
            case .playlists: PlaylistsView()
            case .genres: GenresView()
            case .favorites: FavoritesView()
            case .recent: RecentSongsView()
            case .settings: SettingsView()
            }
        }
    }
}

func clock(_ seconds: Double) -> String {
    guard seconds.isFinite, seconds >= 0 else { return "0:00" }
    let s = Int(seconds)
    return s >= 3600 ? String(format: "%d:%02d:%02d", s / 3600, s / 60 % 60, s % 60) : String(format: "%d:%02d", s / 60, s % 60)
}

func count(_ n: Int, _ one: String, _ many: String? = nil) -> String {
    n == 1 ? "1 \(one)" : "\(n.formatted()) \(many ?? one + "s")"
}

/// A section's title, with an arrow when it opens a full list.
struct SectionTitle: View {
    let title: String
    var route: Route?

    var body: some View {
        Group {
            if let route {
                NavigationLink(value: route) {
                    HStack(spacing: 4) {
                        Text(title)
                        Image(systemName: "chevron.right").font(.subheadline.weight(.bold)).foregroundStyle(.secondary)
                    }
                }
                .buttonStyle(.plain)
            } else {
                Text(title)
            }
        }
        .font(.title2.bold())
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(.horizontal)
        .padding(.top, 22)
    }
}

/// An album as a square cover with its name under it.
struct AlbumTile: View {
    let album: Album
    var width: CGFloat? = 150

    var body: some View {
        NavigationLink(value: Route.album(album)) {
            VStack(alignment: .leading, spacing: 3) {
                Artwork(path: album.artwork, seed: album.title + album.artist, size: 400)
                    .aspectRatio(1, contentMode: .fit)
                    .clipShape(.rect(cornerRadius: 14))
                    .shadow(color: .black.opacity(0.35), radius: 10, y: 6)
                    .padding(.bottom, 5)
                Text(album.title).font(.subheadline.weight(.medium)).lineLimit(1)
                Text(album.artist).font(.footnote).foregroundStyle(.secondary).lineLimit(1)
            }
            .frame(width: width)
            .contentShape(.rect)
        }
        .buttonStyle(Pressable())
        .contextMenu { AlbumMenu(album: album) }
    }
}

struct AlbumMenu: View {
    @Environment(Store.self) private var store
    let album: Album

    var body: some View {
        Button("Play", systemImage: "play.fill") { store.playAlbum(album) }
        Button("Shuffle", systemImage: "shuffle") { store.playAlbum(album, shuffled: true) }
        Button("Play Next", systemImage: "text.line.first.and.arrowtriangle.forward") {
            let key = album.key
            Task { if let songs = await store.load({ try $0.albumSongs(key: key) }) { store.playNext(songs) } }
        }
        Button("Add to Queue", systemImage: "text.line.last.and.arrowtriangle.forward") {
            let key = album.key
            Task { if let songs = await store.load({ try $0.albumSongs(key: key) }) { store.enqueue(songs) } }
        }
    }
}

/// The large card at the top of Home: a cover with its name on a pane of glass.
struct BigCard: View {
    @Environment(Store.self) private var store
    let album: Album
    let kicker: String

    var body: some View {
        NavigationLink(value: Route.album(album)) {
            Artwork(path: album.artwork, seed: album.title + album.artist, size: 800)
                .frame(width: 270, height: 340)
                .overlay(alignment: .topLeading) {
                    Text(kicker.uppercased())
                        .font(.caption.weight(.bold)).kerning(0.6)
                        .padding(.horizontal, 10).padding(.vertical, 5)
                        .glassEffect(.regular, in: .capsule)
                        .padding(14)
                }
                .overlay(alignment: .bottom) {
                    HStack(spacing: 10) {
                        VStack(alignment: .leading, spacing: 2) {
                            Text(album.title).font(.headline).lineLimit(1)
                            Text(album.artist).font(.subheadline).foregroundStyle(.secondary).lineLimit(1)
                        }
                        .frame(maxWidth: .infinity, alignment: .leading)
                        Button { store.playAlbum(album) } label: {
                            Image(systemName: "play.fill").font(.body).frame(width: 30, height: 30)
                        }
                        .buttonStyle(.glass)
                        .buttonBorderShape(.circle)
                    }
                    .padding(.leading, 16).padding(.trailing, 8).padding(.vertical, 8)
                    .glassEffect(.regular, in: .rect(cornerRadius: 22))
                    .padding(10)
                }
                .clipShape(.rect(cornerRadius: 28))
                .shadow(color: .black.opacity(0.45), radius: 18, y: 12)
        }
        .buttonStyle(Pressable())
        .contextMenu { AlbumMenu(album: album) }
    }
}

/// A song in a list: its cover (or its number on an album), its name, and a menu.
struct SongRow: View {
    @Environment(Store.self) private var store
    let song: Song
    var number: Int?
    var showCover = true
    let play: () -> Void

    var body: some View {
        let playing = store.playback?.current?.id == song.id
        Button(action: play) {
            HStack(spacing: 12) {
                if let number {
                    Group {
                        if playing { Image(systemName: "waveform").symbolEffect(.variableColor.iterative, isActive: store.playback?.playing == true) }
                        else { Text("\(number)") }
                    }
                    .font(.subheadline.monospacedDigit()).foregroundStyle(playing ? Color.accentColor : .secondary)
                    .frame(width: 24)
                } else if showCover {
                    Artwork(path: song.artwork, seed: song.album + song.artist, size: 120)
                        .frame(width: 48, height: 48)
                        .clipShape(.rect(cornerRadius: 8))
                }
                VStack(alignment: .leading, spacing: 2) {
                    Text(song.title).foregroundStyle(playing ? Color.accentColor : .primary).lineLimit(1)
                    if number == nil {
                        Text(song.artist).font(.subheadline).foregroundStyle(.secondary).lineLimit(1)
                    }
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                Menu {
                    SongMenu(song: song)
                } label: {
                    Image(systemName: "ellipsis").frame(width: 32, height: 32).contentShape(.rect)
                }
                .foregroundStyle(.secondary)
            }
            .contentShape(.rect)
        }
        .buttonStyle(.plain)
        .contextMenu { SongMenu(song: song) }
    }
}

struct SongMenu: View {
    @Environment(Store.self) private var store
    let song: Song

    var body: some View {
        Button("Play Next", systemImage: "text.line.first.and.arrowtriangle.forward") { store.playNext([song]) }
        Button("Add to Queue", systemImage: "text.line.last.and.arrowtriangle.forward") { store.enqueue([song]) }
        Button("Start Radio", systemImage: "dot.radiowaves.left.and.right") {
            let id = song.id
            store.run { try $0.startRadio(id: id) }
        }
        Button("Favourite", systemImage: "star") {
            let id = song.id
            store.run { core in
                let on = try core.toggleFavorite(id: id)
                Task { @MainActor in store.message = on ? "Added to Favourites" : "Removed from Favourites" }
            }
        }
        Divider()
        NavigationLink(value: Route.artist(song.artist)) { Label("Go to Artist", systemImage: "music.microphone") }
    }
}

/// A list of songs, loaded away from the screen whenever the library changes.
struct SongList: View {
    @Environment(Store.self) private var store
    let title: String
    let load: @Sendable (Needle) throws -> [Song]
    @State private var songs: [Song]?

    var body: some View {
        List {
            if let songs, !songs.isEmpty {
                PlayButtons(songs: songs).listRowSeparator(.hidden).listRowBackground(Color.clear)
                ForEach(Array(songs.enumerated()), id: \.element.id) { i, song in
                    SongRow(song: song) { store.play(songs, from: i) }
                }
            } else if songs != nil {
                ContentUnavailableView("No songs", systemImage: "music.note", description: Text("Add music in Sources."))
                    .listRowBackground(Color.clear)
            }
        }
        .listStyle(.plain)
        .navigationTitle(title)
        .task(id: store.libraryVersion) { songs = await store.load(load) ?? [] }
        .overlay { if songs == nil { ProgressView() } }
    }
}

/// Play and Shuffle, side by side, as glass buttons.
struct PlayButtons: View {
    @Environment(Store.self) private var store
    let songs: [Song]

    var body: some View {
        HStack(spacing: 12) {
            Button { store.play(songs) } label: {
                Label("Play", systemImage: "play.fill").frame(maxWidth: .infinity).padding(.vertical, 6)
            }
            .buttonStyle(.glassProminent)
            Button { store.shufflePlay(songs) } label: {
                Label("Shuffle", systemImage: "shuffle").frame(maxWidth: .infinity).padding(.vertical, 6)
            }
            .buttonStyle(.glass)
        }
        .font(.headline)
        .padding(.vertical, 6)
    }
}

/// Springs down a little while held, as Liquid Glass controls do.
struct Pressable: ButtonStyle {
    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .scaleEffect(configuration.isPressed ? 0.96 : 1)
            .animation(.spring(response: 0.35, dampingFraction: 0.6), value: configuration.isPressed)
    }
}

/// A short message at the bottom of the screen, on glass, that goes after a few seconds.
struct Toast: View {
    @Environment(Store.self) private var store

    var body: some View {
        if let text = store.message {
            Text(text)
                .font(.subheadline.weight(.medium))
                .padding(.horizontal, 18).padding(.vertical, 12)
                .glassEffect(.regular, in: .capsule)
                .padding(.top, 8)
                .transition(.move(edge: .top).combined(with: .opacity))
                .task(id: text) {
                    try? await Task.sleep(for: .seconds(2.5))
                    withAnimation { store.message = nil }
                }
        }
    }
}
