import SwiftUI

extension Song: Identifiable {
    /// A song on a music server (not a file on this phone).
    var fromServer: Bool { id.hasPrefix("src-") }
    /// A song on the computer, while the phone shows the computer's player.
    var onComputer: Bool { id.hasPrefix(Store.pcPrefix) }
}
extension Album: Identifiable { public var id: String { key } }
extension Artist: Identifiable { public var id: String { name } }
extension Playlist: Identifiable {}
extension Genre: Identifiable { public var id: String { name } }

/// The places a screen can open.
/// The core's name for songs without an artist.
let unknownArtist = "Unknown artist"

/// Not a real artist (none, or the core's name for none): it has no page.
func isPlaceholderArtist(_ name: String) -> Bool {
    name.trimmingCharacters(in: .whitespaces).isEmpty || name == unknownArtist
}

enum Route: Hashable {
    case album(Album)
    case albumOf(String)
    case artist(String)
    case playlist(Playlist)
    case genre(String)
    case folder(String)
    case albums, artists, songs, playlists, genres, favorites, recent, folders
    case history, stats, wrapped
    case settings, settingsLibrary, settingsPlayback, settingsSound, settingsAppearance, settingsScrobbling, settingsDiscord
    case plugins, plugin(String), server(String), computer, sources, sync, whatsNew, about
    case fixLibrary, duplicates, missingCovers, albumProblems, tidy, importing
    case themeEditor(String?)
}

extension View {
    /// Opens the screens of [`Route`] from any navigation stack.
    func routes() -> some View {
        navigationDestination(for: Route.self) { route in
            switch route {
            case .album(let album): AlbumView(album: album)
            case .albumOf(let id): AlbumOfView(songId: id)
            case .artist(let name): ArtistView(name: name)
            case .playlist(let playlist): PlaylistView(playlist: playlist)
            case .genre(let name): GenreView(name: name)
            case .folder(let path): FoldersView(path: path)
            case .folders: FoldersView(path: nil)
            case .albums: AlbumsView()
            case .artists: ArtistsView()
            case .songs: SongsView()
            case .playlists: PlaylistsView()
            case .genres: GenresView()
            case .favorites: FavoritesView()
            case .recent: RecentSongsView()
            case .history: HistoryView()
            case .stats: StatsView()
            case .wrapped: WrappedView()
            case .settings: SettingsView()
            case .settingsLibrary: LibrarySettingsView()
            case .settingsPlayback: PlaybackSettingsView()
            case .settingsSound: SoundView()
            case .settingsAppearance: AppearanceView()
            case .settingsScrobbling: ScrobblingView()
            case .settingsDiscord: DiscordView()
            case .plugins: PluginsView()
            case .plugin(let id): PluginView(id: id)
            case .server(let id): ServerView(plugin: id)
            case .computer: ComputerView()
            case .sources: SourcesView()
            case .sync: SyncView()
            case .whatsNew: WhatsNewView()
            case .about: AboutView()
            case .fixLibrary: FixLibraryView()
            case .duplicates: DuplicatesView()
            case .missingCovers: MissingCoversView()
            case .albumProblems: AlbumProblemsView()
            case .tidy: TidyView()
            case .importing: ImportView()
            case .themeEditor(let id): ThemeEditorView(id: id)
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
    /// A short grey line under the title.
    var detail: String?

    var body: some View {
        VStack(alignment: .leading, spacing: 2) {
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
            .accessibilityAddTraits(.isHeader)
            if let detail {
                Text(detail).font(.subheadline).foregroundStyle(.secondary)
            }
        }
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
        Button("Play Next", systemImage: "text.line.first.and.arrowtriangle.forward") { withSongs(store.playNext) }
        Button("Add to Queue", systemImage: "text.line.last.and.arrowtriangle.forward") { withSongs(store.enqueue) }
        Button("Add to a Playlist", systemImage: "music.note.list") { withSongs { store.sheet = .addToPlaylist($0) } }
        Button("Edit Tags", systemImage: "tag") {
            withSongs { songs in
                let own = songs.filter { !$0.fromServer }.map(\.id)
                if own.isEmpty { store.message = "These songs are on your music server: edit their tags there." } else { store.sheet = .tags(own) }
            }
        }
        Divider()
        Button("Go to Artist", systemImage: "music.microphone") { store.opening = .artist(album.artist) }
            .disabled(isPlaceholderArtist(album.artist))
    }

    private func withSongs(_ then: @escaping ([Song]) -> Void) {
        let key = album.key
        Task { if let songs = await store.load({ try $0.albumSongs(key: key) }), !songs.isEmpty { then(songs) } }
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
        let playing = store.playingID == song.id
        // The app's accent, not the tint: on a page in its cover's colours the tint is the
        // text's own colour, and the playing song did not stand out.
        let lit = AnyShapeStyle(store.colors.accent)
        Button(action: play) {
            HStack(spacing: 12) {
                if let number {
                    Group {
                        if playing { Image(systemName: "waveform").symbolEffect(.variableColor.iterative, isActive: store.isPlaying) }
                        else { Text("\(number)") }
                    }
                    .font(.subheadline.monospacedDigit()).foregroundStyle(playing ? lit : AnyShapeStyle(.secondary))
                    .frame(width: 24)
                } else if showCover {
                    Artwork(path: song.artwork, seed: song.album + song.artist, size: 120)
                        .frame(width: 48, height: 48)
                        .clipShape(.rect(cornerRadius: 8))
                }
                VStack(alignment: .leading, spacing: 2) {
                    Text(song.title).foregroundStyle(playing ? lit : AnyShapeStyle(.primary)).lineLimit(1)
                    if number == nil {
                        HStack(spacing: 4) {
                            if song.fromServer && store.app.serverMark { Image(systemName: "server.rack").font(.caption2) }
                            Text(song.artist).lineLimit(1)
                        }
                        .font(.subheadline).foregroundStyle(.secondary)
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
    var inPlayer = false

    var body: some View {
        if !song.onComputer {
            if !inPlayer {
                Button("Play Next", systemImage: "text.line.first.and.arrowtriangle.forward") { store.playNext([song]) }
                Button("Add to Queue", systemImage: "text.line.last.and.arrowtriangle.forward") { store.enqueue([song]) }
            }
            Button("Add to a Playlist", systemImage: "music.note.list") { store.sheet = .addToPlaylist([song]) }
            Button("Start Radio", systemImage: "dot.radiowaves.left.and.right") {
                let id = song.id
                store.run { try $0.startRadio(id: id) }
                store.playedWhenReady()
            }
            Menu {
                ForEach((0...5).reversed(), id: \.self) { stars in
                    Button {
                        let id = song.id
                        store.rate(id, stars: stars)
                        store.message = stars == 0 ? "Stars taken off" : "Rated \(count(stars, "star"))"
                    } label: {
                        Text(stars == 0 ? "No stars" : String(repeating: "★", count: stars))
                    }
                    .accessibilityLabel(stars == 0 ? "No stars" : count(stars, "star"))
                }
            } label: {
                Label("Rate", systemImage: "star")
            }
            Divider()
            Button("Go to Album", systemImage: "square.stack") { store.opening = .albumOf(song.id) }
            Button("Go to Artist", systemImage: "music.microphone") { store.opening = .artist(song.artist) }
                .disabled(isPlaceholderArtist(song.artist))
            Divider()
            if song.fromServer {
                Button("Keep Offline", systemImage: "arrow.down.circle") { store.keepOffline([song], keep: true) }
                Button("Save to This iPhone", systemImage: "square.and.arrow.down") { store.saveToPhone([song]) }
            } else {
                Button("Edit Tags", systemImage: "tag") { store.sheet = .tags([song.id]) }
                Button("Identify by Sound", systemImage: "waveform.badge.magnifyingglass") { store.sheet = .identify(song.id) }
            }
            Button("Time the Lyrics", systemImage: "text.badge.checkmark") { store.sheet = .timing(song) }
            if !store.songCommands.isEmpty {
                Menu {
                    ForEach(store.songCommands) { command in
                        Button(command.command.title) { store.runCommand(command, on: [song]) }
                    }
                } label: {
                    Label("Plugins", systemImage: "puzzlepiece.extension")
                }
            }
        }
    }
}

/// Opens the album a song is on.
struct AlbumOfView: View {
    @Environment(Store.self) private var store
    let songId: String
    @State private var album: Album?
    @State private var looked = false

    var body: some View {
        Group {
            if let album { AlbumView(album: album) }
            else if looked { ContentUnavailableView("Album not found", systemImage: "square.stack") }
            else { ProgressView() }
        }
        .task {
            let id = songId
            album = await store.load { try $0.albumOf(songId: id) } ?? nil
            looked = true
        }
    }
}

/// A list of songs, loaded away from the screen whenever the library changes, with a Select
/// mode for acting on several at once.
struct SongList: View {
    @Environment(Store.self) private var store
    let title: String
    let load: @Sendable (Needle) throws -> [Song]
    @State private var songs: [Song]?
    @State private var selecting = false
    @State private var selected = Set<String>()

    var body: some View {
        List(selection: selecting ? $selected : nil) {
            if let songs, !songs.isEmpty {
                PlayButtons(songs: songs).listRowSeparator(.hidden).listRowBackground(Color.clear).selectionDisabled()
                ForEach(Array(songs.enumerated()), id: \.element.id) { i, song in
                    SongRow(song: song) { store.play(songs, from: i) }.tag(song.id)
                }
            } else if songs != nil {
                ContentUnavailableView("No songs", systemImage: "music.note", description: Text("Add music in Sources."))
                    .listRowBackground(Color.clear)
            }
        }
        .listStyle(.plain)
        .navigationTitle(title)
        .themed()
        .environment(\.editMode, .constant(selecting ? .active : .inactive))
        .toolbar { SelectToolbar(selecting: $selecting, selected: $selected, songs: songs ?? []) }
        .task(id: "\(store.libraryVersion)|\(store.ratingVersion)") { songs = await store.load(load) ?? [] }
        .overlay { if songs == nil { ProgressView() } }
    }
}

/// Select and Done, and what to do with the songs chosen.
struct SelectToolbar: ToolbarContent {
    @Binding var selecting: Bool
    @Binding var selected: Set<String>
    let songs: [Song]
    /// On a page in its picture's colours, the words in the page's ink.
    var tint: Color?

    var body: some ToolbarContent {
        ToolbarItem(placement: .topBarTrailing) {
            Group {
                if selecting {
                    Button("Done") { selecting = false; selected = [] }
                } else if !songs.isEmpty {
                    Button("Select") { selecting = true }
                }
            }
            .tint(tint)
        }
        if selecting && !selected.isEmpty {
            SelectionActions(songs: songs.filter { selected.contains($0.id) })
        }
    }
}

/// Every song matching a search (all of them with an empty one), a page at a time, for large
/// libraries.
struct PagedSongs: View {
    @Environment(Store.self) private var store
    let title: String
    let query: String
    @State private var songs: [Song] = []
    @State private var total: UInt32?
    @State private var selecting = false
    @State private var selected = Set<String>()
    private let page: UInt32 = 200

    var body: some View {
        List(selection: selecting ? $selected : nil) {
            if !songs.isEmpty {
                HStack(spacing: 12) {
                    Button { playAll(shuffled: false) } label: {
                        Label("Play", systemImage: "play.fill").frame(maxWidth: .infinity).padding(.vertical, 6)
                    }
                    .buttonStyle(.glassProminent)
                    Button { playAll(shuffled: true) } label: {
                        Label("Shuffle", systemImage: "shuffle").frame(maxWidth: .infinity).padding(.vertical, 6)
                    }
                    .buttonStyle(.glass)
                }
                .font(.headline)
                .listRowSeparator(.hidden).listRowBackground(Color.clear).selectionDisabled()
            }
            ForEach(Array(songs.enumerated()), id: \.element.id) { i, song in
                SongRow(song: song) { store.playSearch(query, from: i) }
                    .tag(song.id)
                    .onAppear { if i == songs.count - 20 || i == songs.count - 1 { Task { await more() } } }
            }
        }
        .listStyle(.plain)
        .navigationTitle(title)
        .themed()
        .environment(\.editMode, .constant(selecting ? .active : .inactive))
        .toolbar { SelectToolbar(selecting: $selecting, selected: $selected, songs: songs) }
        .overlay {
            if total == nil { ProgressView() }
            else if total == 0 { ContentUnavailableView("No songs", systemImage: "music.note", description: Text("Add music in Sources.")) }
        }
        .task(id: store.libraryVersion) {
            let (q, want) = (query, max(Int(page), songs.count))
            total = await store.load { try $0.searchSongsCount(query: q) } ?? 0
            // As many as were shown, so the list stays where it was scrolled: in pages of at
            // most 1,000, the core's most at a time.
            var all: [Song] = []
            while all.count < want {
                let (offset, size) = (UInt32(all.count), UInt32(min(1000, want - all.count)))
                let next = await store.load { try $0.searchSongsPage(query: q, offset: offset, size: size) } ?? []
                all += next
                if next.count < Int(size) { break }
            }
            songs = all
        }
    }

    private func more() async {
        guard let total, songs.count < Int(total) else { return }
        let (q, offset, size) = (query, UInt32(songs.count), page)
        let next = await store.load { try $0.searchSongsPage(query: q, offset: offset, size: size) } ?? []
        if UInt32(songs.count) == offset { songs += next }
    }

    private func playAll(shuffled: Bool) {
        store.playSearch(query, shuffled: shuffled)
    }
}

/// Play and Shuffle, side by side, as glass buttons.
struct PlayButtons: View {
    @Environment(Store.self) private var store
    let songs: [Song]
    /// On a page in its picture's colours: Play in the page's ink, its words in the page's colour.
    var look: PageColors?

    var body: some View {
        GlassEffectContainer(spacing: 12) {
            HStack(spacing: 12) {
                Button { store.play(songs) } label: {
                    Label("Play", systemImage: "play.fill").frame(maxWidth: .infinity).padding(.vertical, 6)
                        .foregroundStyle(look?.page ?? .white)
                }
                .buttonStyle(.glassProminent)
                .tint(look?.ink)
                Button { store.shufflePlay(songs) } label: {
                    Label("Shuffle", systemImage: "shuffle").frame(maxWidth: .infinity).padding(.vertical, 6)
                }
                .buttonStyle(.glass)
            }
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

/// A short message at the top of the screen, on glass, that goes after a few seconds.
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
                .task(id: store.messageCount) {
                    try? await Task.sleep(for: .seconds(2.5))
                    withAnimation { store.message = nil }
                }
        }
    }
}
