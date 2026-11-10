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
    /// An album opened from its tile: the page grows out of that tile's cover (its zoom id).
    case albumFrom(Album, String)
    case albumOf(String)
    case artist(String)
    case playlist(Playlist)
    case genre(String)
    /// The songs a rule finds (Fix my library's cards: songs with no genre, …): title, rule.
    case rule(String, String)
    case folder(String)
    case albums, artists, songs, playlists, genres, favorites, recent, folders
    case history, stats, wrapped, crate, playerButtons, numbers, genreMap, fonts(Bool)
    case settings, settingsLibrary, settingsPlayback, settingsSound, settingsAppearance, settingsScrobbling, settingsDiscord
    case plugins, plugin(String), server(String), computer, sources, sync, whatsNew, about
    case fixLibrary, duplicates, missingCovers, albumProblems, tidy, importing
    case themeEditor(String?)
}

extension View {
    /// Opens the screens of [`Route`] from any navigation stack.
    func routes() -> some View {
        navigationDestination(for: Route.self) { route in
            Self.page(route).modifier(ZoomScope())
        }
    }

    @ViewBuilder
    fileprivate static func page(_ route: Route) -> some View {
            switch route {
            // Opened another way (Search, a menu, a link): it simply comes in.
            case .album(let album): AlbumView(album: album)
            case .albumFrom(let album, let source): AlbumView(album: album).modifier(CoverZoom(id: source))
            case .albumOf(let id): AlbumOfView(songId: id)
            case .artist(let name): ArtistView(name: name)
            case .playlist(let playlist): PlaylistView(playlist: playlist)
            case .genre(let name): GenreView(name: name)
            case .rule(let title, let query): PagedSongs(title: title, query: query)
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
            case .wrapped: YearReviewView()
            case .crate: RecordCrateView()
            case .playerButtons: PlayerButtonsView()
            case .numbers: NumbersView()
            case .genreMap: GenreMapView()
            case .fonts(let titles): FontsView(titles: titles)
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

/// The page a route opens, as the root of a stack (an iPad sidebar's entry, a column of
/// Settings).
struct RoutePage: View {
    let route: Route

    var body: some View {
        EmptyView.page(route)
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
    @Environment(\.wide) private var wide
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
                            Image(systemName: "chevron.right").font(.needle(.subheadline).weight(.bold)).foregroundStyle(.secondary)
                        }
                    }
                    .buttonStyle(.plain)
                } else {
                    Text(title)
                }
            }
            .font(.needleTitle(size: 22))
            .accessibilityAddTraits(.isHeader)
            if let detail {
                Text(detail).font(.needle(.subheadline)).foregroundStyle(.secondary)
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(.horizontal, Tiles.margin(wide))
        .padding(.top, wide ? 28 : 22)
    }
}

/// An album as a square cover with its name under it; its page grows out of this cover.
struct AlbumTile: View {
    @Environment(Store.self) private var store
    @Environment(\.zoomScope) private var scope
    @Environment(\.wide) private var wide
    let album: Album
    /// Its width on a shelf (nil in a grid, where the grid sizes it); wider in a wide window.
    var width: CGFloat? = Tiles.phoneShelf
    /// Which shelf or grid on the page holds it: an album can be on two shelves at once.
    var shelf = "grid"
    /// The pointer is over the tile (an iPad's): Play shows on its cover, as on the Mac.
    @State private var hovered = false

    var body: some View {
        let source = CoverSource.id(album, shelf: shelf, in: scope)
        NavigationLink(value: Route.albumFrom(album, source)) {
            VStack(alignment: .leading, spacing: 3) {
                Artwork(path: album.artwork, seed: album.title + album.artist, size: 400)
                    .aspectRatio(1, contentMode: .fit)
                    .clipShape(.soft(Corner.cover))
                    .modifier(CoverSource(id: source))
                    .shadow(color: .black.opacity(0.35), radius: 10, y: 6)
                    .overlay(alignment: .bottomTrailing) {
                        if hovered {
                            Button { store.playAlbum(album) } label: {
                                Image(systemName: "play.fill")
                                    .font(.system(size: 15, weight: .bold))
                                    .foregroundStyle(store.colors.onAccent)
                                    .offset(x: 1)
                                    .frame(width: 40, height: 40)
                                    .background(store.colors.accent, in: .circle)
                                    .shadow(color: .black.opacity(0.3), radius: 6, y: 3)
                            }
                            .buttonStyle(Pressable())
                            .padding(10)
                            .transition(.opacity)
                            .accessibilityLabel("Play \(album.title)")
                        }
                    }
                    .padding(.bottom, 5)
                Text(album.title).font(.needle(.subheadline).weight(.medium)).lineLimit(1)
                Text(album.artist).font(.needle(.footnote)).foregroundStyle(.secondary).lineLimit(1)
            }
            .frame(width: width == Tiles.phoneShelf ? Tiles.shelf(wide) : width)
            .contentShape(.rect)
        }
        .buttonStyle(Pressable())
        .hoverEffect(.lift)
        .onHover { over in withFade(.easeOut(duration: 0.15)) { hovered = over } }
        .contextMenu { AlbumMenu(album: album) }
        .draggableAlbum(album)
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
        Button("Add to Playlist", systemImage: "music.note.list") { withSongs { store.sheet = .addToPlaylist($0) } }
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

/// A song in a list: its cover (or its number on an album), its name, and a menu. In a wide
/// window, a row of a table, as on the Mac: the number or cover, the title, the artist and the
/// album in columns, the time, and the heart.
struct SongRow: View {
    @Environment(Store.self) private var store
    @Environment(\.wide) private var wide
    let song: Song
    var number: Int?
    var showCover = true
    /// On an album's page: the album's artist, so a song's own artist shows only where it
    /// differs (in a wide window's table).
    var albumArtist: String?
    let play: () -> Void

    var body: some View {
        if wide { tableRow } else { phoneRow }
    }

    /// A row of the wide window's table.
    private var tableRow: some View {
        let playing = store.playingID == song.id
        let lit = AnyShapeStyle(store.colors.accent)
        let onAlbum = number != nil
        return Button(action: play) {
            HStack(spacing: 14) {
                if let number {
                    Group {
                        if playing { Image(systemName: "waveform").symbolEffect(.variableColor.iterative, isActive: store.isPlaying) }
                        else { Text("\(number)") }
                    }
                    .font(.needle(.subheadline).monospacedDigit()).foregroundStyle(playing ? lit : AnyShapeStyle(.secondary))
                    .frame(width: 28)
                } else if showCover {
                    Artwork(path: song.artwork, seed: song.album + song.artist, size: 120)
                        .frame(width: 40, height: 40)
                        .clipShape(.soft(Corner.smallCover))
                }
                HStack(spacing: 5) {
                    Text(song.title).foregroundStyle(playing ? lit : AnyShapeStyle(.primary)).lineLimit(1)
                    if song.fromServer && store.app.serverMark {
                        Image(systemName: "server.rack").font(.needle(.caption2)).foregroundStyle(.secondary)
                            .accessibilityLabel("On your music server")
                    }
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                // The artist: always in a list, on an album only where it is not the album's.
                if !onAlbum || albumArtist != nil {
                    Text(onAlbum && song.artist == albumArtist ? "" : song.artist)
                        .foregroundStyle(.secondary).lineLimit(1)
                        .frame(maxWidth: .infinity, alignment: .leading)
                }
                if !onAlbum {
                    Text(song.album).foregroundStyle(.secondary).lineLimit(1)
                        .frame(maxWidth: .infinity, alignment: .leading)
                }
                Text(clock(song.duration)).font(.needle(.subheadline).monospacedDigit()).foregroundStyle(.secondary)
                    .frame(width: 56, alignment: .trailing)
                if !song.onComputer { HeartButton(song: song) }
                Menu {
                    SongMenu(song: song)
                } label: {
                    Image(systemName: "ellipsis").frame(width: 36, height: 36).contentShape(.rect)
                }
                .foregroundStyle(.secondary)
                .accessibilityLabel("More for \(song.title)")
            }
            .font(.needle(.subheadline))
            .frame(minHeight: 44)
            .contentShape(.rect)
        }
        .buttonStyle(.plain)
        // One line under each row, from the row's own edge, whatever its first column holds.
        .alignmentGuide(.listRowSeparatorLeading) { _ in 0 }
        .hoverEffect(.highlight)
        .contextMenu { SongMenu(song: song) }
        .draggableSongs([song])
    }

    @ViewBuilder
    private var phoneRow: some View {
        let playing = store.playingID == song.id
        // Settings › Appearance › Song rows: Compact fits more on the screen.
        let compact = store.app.density == "compact"
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
                    .font(.needle(.subheadline).monospacedDigit()).foregroundStyle(playing ? lit : AnyShapeStyle(.secondary))
                    .frame(width: 24)
                } else if showCover {
                    Artwork(path: song.artwork, seed: song.album + song.artist, size: 120)
                        .frame(width: compact ? 36 : 48, height: compact ? 36 : 48)
                        .clipShape(.soft(compact ? Corner.smallCover : Corner.cover))
                }
                VStack(alignment: .leading, spacing: compact ? 0 : 2) {
                    Text(song.title).foregroundStyle(playing ? lit : AnyShapeStyle(.primary)).lineLimit(1)
                    if number == nil {
                        HStack(spacing: 4) {
                            if song.fromServer && store.app.serverMark { Image(systemName: "server.rack").font(.needle(.caption2)) }
                            Text(song.artist).lineLimit(1)
                        }
                        .font(.needle(.subheadline)).foregroundStyle(.secondary)
                    }
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                Menu {
                    SongMenu(song: song)
                } label: {
                    Image(systemName: "ellipsis").frame(width: 44, height: compact ? 36 : 44).contentShape(.rect)
                }
                .foregroundStyle(.secondary)
                .accessibilityLabel("More for \(song.title)")
            }
            .contentShape(.rect)
        }
        .buttonStyle(.plain)
        .contextMenu { SongMenu(song: song) }
    }
}

/// The heart of a song's row in a wide window: a favourite or not, at one press.
struct HeartButton: View {
    @Environment(Store.self) private var store
    let song: Song
    /// Pressed here, before the list is read again.
    @State private var chosen: Bool?
    @State private var presses = 0

    var body: some View {
        let on = chosen ?? (song.rating >= 4)
        Button {
            let id = song.id
            chosen = !on
            if !on { presses += 1 }
            Task {
                if let now = await store.ask({ try $0.toggleFavorite(id: id) }) {
                    chosen = now
                    store.ratingsChanged()
                } else {
                    chosen = nil
                }
            }
        } label: {
            Image(systemName: on ? "heart.fill" : "heart")
                .foregroundStyle(on ? AnyShapeStyle(store.colors.accent) : AnyShapeStyle(.tertiary))
                .contentTransition(.symbolEffect(.replace))
                .pressBounce(presses)
                .frame(width: 36, height: 36)
                .contentShape(.rect)
        }
        .buttonStyle(.plain)
        .hoverEffect(.highlight)
        .accessibilityLabel(on ? "Remove from Favorites" : "Add to Favorites")
        .onChange(of: song.rating) { _, _ in chosen = nil }
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
            Button("Add to Playlist", systemImage: "music.note.list") { store.sheet = .addToPlaylist([song]) }
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
                Button("Save to This \(Device.name)", systemImage: "square.and.arrow.down") { store.saveToPhone([song]) }
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
        .readableWidth(1180)
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
    @Environment(\.wide) private var wide
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
                // The container's spacing under the gap: Play and Shuffle never melt together.
                GlassEffectContainer(spacing: 4) {
                    HStack(spacing: 12) {
                        // Two pills of their own size at the left in a wide window.
                        Button { playAll(shuffled: false) } label: {
                            Label("Play", systemImage: "play.fill").frame(maxWidth: wide ? nil : .infinity).padding(.vertical, 6).padding(.horizontal, wide ? 14 : 0)
                        }
                        .buttonStyle(.glassProminent)
                        .foregroundStyle(store.colors.onAccent)
                        Button { playAll(shuffled: true) } label: {
                            Label("Shuffle", systemImage: "shuffle").frame(maxWidth: wide ? nil : .infinity).padding(.vertical, 6).padding(.horizontal, wide ? 14 : 0)
                        }
                        .buttonStyle(.glass)
                        if wide { Spacer(minLength: 0) }
                    }
                }
                .font(.needle(.headline))
                .listRowSeparator(.hidden).listRowBackground(Color.clear).selectionDisabled()
            }
            ForEach(Array(songs.enumerated()), id: \.element.id) { i, song in
                SongRow(song: song) { store.playSearch(query, from: i) }
                    .tag(song.id)
                    .onAppear { if i == songs.count - 20 || i == songs.count - 1 { Task { await more() } } }
            }
        }
        .listStyle(.plain)
        .readableWidth(1180)
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
    @Environment(\.wide) private var wide
    let songs: [Song]
    /// On a page in its picture's colours: Play in the page's ink, its words in the page's colour.
    var look: PageColors?

    var body: some View {
        // In a wide window two pills of their own size, at the left, as on the Mac; on a phone
        // two halves of the row.
        let most: CGFloat? = wide ? nil : .infinity
        // The container's spacing under the gap: Play and Shuffle never melt together.
        GlassEffectContainer(spacing: 4) {
            HStack(spacing: 12) {
                Button { store.play(songs) } label: {
                    Label("Play", systemImage: "play.fill").frame(maxWidth: most).padding(.vertical, 6).padding(.horizontal, wide ? 14 : 0)
                        .foregroundStyle(look?.page ?? .white)
                }
                .buttonStyle(.glassProminent)
                .tint(look?.ink)
                Button { store.shufflePlay(songs) } label: {
                    Label("Shuffle", systemImage: "shuffle").frame(maxWidth: most).padding(.vertical, 6).padding(.horizontal, wide ? 14 : 0)
                }
                .buttonStyle(.glass)
                if wide { Spacer(minLength: 0) }
            }
        }
        .font(.needle(.headline))
        .padding(.vertical, 6)
    }
}

/// Springs down a little while held, as Liquid Glass controls do; with Reduce motion it only
/// dims a little.
/// A page (or a part of one) that could not be read: what went wrong, in Needle's words, and
/// Try Again. Shown instead of an empty page.
struct LoadProblem: View {
    let title: String
    let words: String
    let retry: () -> Void

    var body: some View {
        ContentUnavailableView {
            Label(title, systemImage: "exclamationmark.triangle")
        } description: {
            Text(words)
        } actions: {
            Button("Try Again", action: retry)
        }
    }
}

struct Pressable: ButtonStyle {
    func makeBody(configuration: Configuration) -> some View {
        let reduced = Motion.reduced
        return configuration.label
            .scaleEffect(configuration.isPressed && !reduced ? 0.96 : 1)
            .opacity(configuration.isPressed && reduced ? 0.7 : 1)
            .animation(reduced ? .easeOut(duration: 0.1) : .spring(response: 0.35, dampingFraction: 0.6), value: configuration.isPressed)
    }
}

/// A short message at the top of the screen, on glass, that goes after a few seconds.
struct Toast: View {
    @Environment(Store.self) private var store

    var body: some View {
        if let text = store.message {
            Text(text)
                .font(.needle(.subheadline).weight(.medium))
                .padding(.horizontal, 18).padding(.vertical, 12)
                .glassEffect(.regular, in: .capsule)
                .padding(.top, 8)
                .transition(Motion.transition(.move(edge: .top).combined(with: .opacity)))
                .task(id: store.messageCount) {
                    try? await Task.sleep(for: .seconds(2.5))
                    withFade { store.message = nil }
                }
        }
    }
}

private struct CoverSpaceKey: EnvironmentKey {
    static let defaultValue: Namespace.ID? = nil
}

extension EnvironmentValues {
    /// Album covers zoom from their tile into the album's page in this namespace: one for each
    /// tab (see `TabStack`).
    var coverSpace: Namespace.ID? {
        get { self[CoverSpaceKey.self] }
        set { self[CoverSpaceKey.self] = newValue }
    }

    /// The page a tile is on, made up when the page opens: the same album on two pages of one
    /// tab (an artist, its album, the artist again) are two sources, never one.
    @Entry var zoomScope = "root"
}

/// Gives a page opened in a tab a place of its own in the tab's zoom namespace.
struct ZoomScope: ViewModifier {
    @State private var scope = UUID().uuidString

    func body(content: Content) -> some View {
        content.environment(\.zoomScope, scope)
    }
}

/// A tab's navigation stack, with its own namespace for covers that zoom into their album's
/// page (a namespace shared by the tabs zoomed from a tile of another tab).
struct TabStack<Root: View>: View {
    @Binding var path: NavigationPath
    @ViewBuilder let root: () -> Root
    @Namespace private var covers

    var body: some View {
        NavigationStack(path: $path) { root().routes() }
            .environment(\.coverSpace, covers)
    }
}

/// An album tile's cover, where the album's page zooms from.
struct CoverSource: ViewModifier {
    @Environment(\.coverSpace) private var space
    let id: String

    /// The zoom id of an album's cover on a shelf (or grid) of a page.
    static func id(_ album: Album, shelf: String, in scope: String = "root") -> String {
        "\(scope)|\(shelf)|\(album.key)"
    }

    func body(content: Content) -> some View {
        if let space { content.matchedTransitionSource(id: id, in: space) } else { content }
    }
}

/// An album's page grows out of the cover chosen (the zoom transition); with Reduce motion
/// (Needle's or the iPhone's) it simply comes in.
struct CoverZoom: ViewModifier {
    @Environment(Store.self) private var store
    @Environment(\.coverSpace) private var space
    @Environment(\.accessibilityReduceMotion) private var systemReduceMotion
    let id: String

    func body(content: Content) -> some View {
        if let space, !systemReduceMotion, !store.app.reduceMotion {
            content.navigationTransition(.zoom(sourceID: id, in: space))
        } else {
            content
        }
    }
}
