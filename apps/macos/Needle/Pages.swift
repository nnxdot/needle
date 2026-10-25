import SwiftUI

/// Home: what played lately, what is new, and what is loved most.
struct HomePage: View {
    private var store: Store { .shared }
    @State private var home: HomeData?
    /// What played on this day one and five years ago.
    @State private var days: [PastDay] = []

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
                    ForEach(days) { day in
                        Shelf(title: day.title, detail: day.detail, albums: day.albums)
                    }
                    if !home.artists.isEmpty {
                        VStack(alignment: .leading, spacing: 10) {
                            ShelfTitle(title: "Artists you play most", detail: nil, all: .artists)
                            PagedRow(tile: 132, spacing: 20, middle: 66) {
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
            let offset = Int32(TimeZone.current.secondsFromGMT())
            days = JSON.decode([PastDay].self, await store.load { try $0.onThisDayJson(offset: offset) }) ?? []
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
    private var store: Store { .shared }

    var body: some View {
        if let p = store.playback, p.hasSong {
            let colors = store.colors
            HStack(spacing: 18) {
                Button { store.big = true } label: { Cover(path: p.artwork, seed: p.album + p.artist, side: 96, corner: 8) }
                    .buttonStyle(.plain)
                VStack(alignment: .leading, spacing: 4) {
                    Text(p.playing ? "Playing now" : "Paused").font(.needle(.caption).weight(.semibold)).foregroundStyle(colors.accent)
                    Text(p.title).font(.needle(.title2).weight(.semibold)).lineLimit(1)
                    Text("\(p.artist) · \(p.album)").foregroundStyle(colors.ink2).lineLimit(1)
                }
                .frame(minWidth: 0)
                Spacer(minLength: 8)
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
    private var store: Store { .shared }
    let title: String
    let detail: String?
    let all: Route?

    var body: some View {
        HStack(alignment: .firstTextBaseline) {
            VStack(alignment: .leading, spacing: 2) {
                Text(title).font(.needle(.title3).weight(.semibold))
                if let detail { Text(detail).font(.needle(.callout)).foregroundStyle(store.colors.ink2) }
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
                PagedRow(tile: 156, spacing: 18, middle: 78) {
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

/// A sideways row of tiles with an arrow at each end while the pointer is over it, only
/// toward more: a click moves one page, as many whole tiles as the row shows (at least one).
struct PagedRow<Content: View>: View {
    private var store: Store { .shared }
    /// A tile's width, and the room between two.
    let tile: CGFloat
    let spacing: CGFloat
    /// The middle of the covers, from the row's top: the arrows are centred on it.
    let middle: CGFloat
    @ViewBuilder let content: Content
    @State private var position = ScrollPosition(edge: .leading)
    @State private var offset: CGFloat = 0
    @State private var extent: CGFloat = 0
    @State private var visible: CGFloat = 0
    @State private var hover = false

    var body: some View {
        ScrollView(.horizontal, showsIndicators: false) { content }
            .scrollPosition($position)
            .onScrollGeometryChange(for: [CGFloat].self) { g in
                [g.contentOffset.x, g.contentSize.width, g.containerSize.width]
            } action: { _, now in
                offset = now[0]; extent = now[1]; visible = now[2]
            }
            .overlay(alignment: .topLeading) {
                if hover && offset > 1 { arrow(-1).padding(.leading, 6).offset(y: middle - 28) }
            }
            .overlay(alignment: .topTrailing) {
                if hover && offset < extent - visible - 1 { arrow(1).padding(.trailing, 6).offset(y: middle - 28) }
            }
            .animation(.easeOut(duration: 0.15), value: hover)
            .onHover { hover = $0 }
    }

    private func arrow(_ way: CGFloat) -> some View {
        Button { page(way) } label: {
            Image(systemName: way < 0 ? "chevron.left" : "chevron.right")
                .font(.system(size: 15, weight: .semibold))
                .foregroundStyle(store.colors.ink)
                .frame(width: 30, height: 56)
                .contentShape(Capsule())
        }
        .buttonStyle(.plain)
        .pane(Capsule(), interactive: true)
        .shadow(color: .black.opacity(0.18), radius: 6, y: 2)
        .help(way < 0 ? "Back" : "More")
        .accessibilityLabel(way < 0 ? "Scroll back" : "Scroll for more")
        .transition(.opacity)
    }

    /// One page on, from the tile at the edge now, kept within the row.
    private func page(_ way: CGFloat) {
        let step = tile + spacing
        let tiles = max(1, ((visible + spacing) / step).rounded(.down))
        let first = (offset / step).rounded()
        let target = min(max((first + way * tiles) * step, 0), max(extent - visible, 0))
        withAnimation(store.motion ? .smooth(duration: 0.35) : nil) { position.scrollTo(x: target) }
    }
}

/// An album: its cover (with a play button on hover), name, and artist.
struct AlbumTile: View {
    private var store: Store { .shared }
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
                        Image(systemName: "play.fill").font(.system(size: 14, weight: .bold)).foregroundStyle(store.colors.accentInk)
                            .frame(width: 34, height: 34)
                            .background(store.colors.accent, in: Circle())
                    }
                    .buttonStyle(.plain)
                    .padding(8)
                    .transition(.scale.combined(with: .opacity))
                }
            }
            Text(album.title).font(.needle(.callout).weight(.medium)).lineLimit(1)
            Text(album.year > 0 ? "\(album.artist) · \(album.year)" : album.artist)
                .font(.needle(.caption)).foregroundStyle(store.colors.ink2).lineLimit(1)
        }
        .frame(width: side)
        .contentShape(Rectangle())
        .onHover { h in withAnimation(.snappy(duration: 0.15)) { hover = h } }
        .onTapGesture { store.navigate(album.route) }
        .accessibilityElement(children: .combine)
        .accessibilityAddTraits(.isButton)
        .accessibilityAction { store.navigate(album.route) }
        .accessibilityAction(named: "Play") { play() }
        .contextMenu { GroupMenu(route: album.route, title: album.title) }
    }

    private func play() {
        let (rule, reason) = (album.query, "From the album \(album.album)")
        Task { _ = await store.ask { try $0.playView(query: rule, playlist: nil, filter: "", start: 0, shuffle: false, reason: reason) } }
    }
}

/// An artist: a round photo and their name.
struct ArtistTile: View {
    private var store: Store { .shared }
    let artist: ArtistSummary
    let side: CGFloat

    var body: some View {
        VStack(spacing: 6) {
            ArtistPhoto(name: artist.name, side: side)
                .shadow(color: .black.opacity(0.15), radius: 6, y: 3)
            Text(artist.title).font(.needle(.callout).weight(.medium)).lineLimit(1)
            Text(artist.albums == 1 ? count(artist.tracks, "track") : "\(artist.albums) albums · \(count(artist.tracks, "track"))")
                .font(.needle(.caption)).foregroundStyle(store.colors.ink2).lineLimit(1)
        }
        .frame(width: side)
        .contentShape(Rectangle())
        .onTapGesture { store.navigate(.artist(artist.name)) }
        .accessibilityElement(children: .combine)
        .accessibilityAddTraits(.isButton)
        .accessibilityAction { store.navigate(.artist(artist.name)) }
        .contextMenu { GroupMenu(route: .artist(artist.name), title: artist.title) }
    }
}

/// Every album, as covers; the search narrows them.
struct AlbumsPage: View {
    private var store: Store { .shared }
    @State private var albums: [AlbumSummary]?
    /// The albums as a record crate to flip through, not a grid.
    @AppStorage("albumsAsCrate") private var crate = false

    var body: some View {
        Group {
            if crate {
                RecordCrate(albums: albums ?? [])
            } else {
                TileGrid(title: "Albums", summary: albums.map { count($0.count, "album") }) {
                    ForEach(albums ?? []) { AlbumTile(album: $0, side: 172) }
                }
            }
        }
        .overlay(alignment: .topTrailing) {
            Picker("View", selection: $crate) {
                Image(systemName: "square.grid.2x2").help("Grid").tag(false)
                Image(systemName: "square.stack.3d.down.forward").help("Record crate").tag(true)
            }
            .pickerStyle(.segmented)
            .labelsHidden()
            .fixedSize()
            .padding(.top, 34)
            .padding(.trailing, 28)
            .accessibilityLabel("Show albums as")
        }
        .task(id: "\(store.search)\(store.libraryVersion)") {
            try? await Task.sleep(for: .milliseconds(store.search.isEmpty ? 0 : 150))
            guard !Task.isCancelled else { return }
            let q = store.search.trimmingCharacters(in: .whitespaces)
            let found = JSON.decode([AlbumSummary].self, await store.load { try $0.albumsJson(query: q) }) ?? []
            guard !Task.isCancelled else { return }
            albums = found
        }
    }
}

/// Every artist, as photos, in the order chosen (by name with an A–Z bar at the side, most
/// played, most songs, or recently added), and only album artists when asked; the search
/// narrows them.
struct ArtistsPage: View {
    private var store: Store { .shared }
    @State private var artists: [ArtistSummary]?

    var body: some View {
        let s = store.settings
        let letters = s.artistOrder == "name" ? Self.letters(artists ?? []) : []
        ScrollViewReader { reader in
            ScrollView {
                VStack(alignment: .leading, spacing: 18) {
                    HStack(alignment: .firstTextBaseline) {
                        VStack(alignment: .leading, spacing: 2) {
                            Text(store.search.isEmpty ? "Artists" : "Artists matching “\(store.search)”").font(store.display(28))
                            Text(artists.map { count($0.count, "artist") } ?? "Loading…").font(.needle(.callout)).foregroundStyle(store.colors.ink2)
                        }
                        Spacer()
                        Menu {
                            Picker("Sort by", selection: Binding(get: { s.artistOrder }, set: { store.set(["artist_order": $0]) })) {
                                Text("A–Z").tag("name")
                                Text("Most played").tag("most_played")
                                Text("Most songs").tag("most_songs")
                                Text("Newest").tag("recently_added")
                            }
                            .pickerStyle(.inline)
                            Divider()
                            Toggle("Only album artists", isOn: Binding(get: { s.albumArtistsOnly }, set: { store.set(["album_artists_only": $0]) }))
                        } label: {
                            Label(Self.orderName(s.artistOrder), systemImage: "arrow.up.arrow.down")
                        }
                        .menuStyle(.button)
                        .fixedSize()
                        .padding(.trailing, letters.isEmpty ? 0 : 22)
                        .help("Sort artists")
                    }
                    LazyVGrid(columns: [GridItem(.adaptive(minimum: 160, maximum: 220), spacing: 22, alignment: .top)], spacing: 26) {
                        ForEach(artists ?? []) { artist in
                            ArtistTile(artist: artist, side: 160).id("artist:" + artist.name)
                        }
                    }
                    .padding(.trailing, letters.isEmpty ? 0 : 22)
                }
                .padding(28)
            }
            .overlay(alignment: .trailing) {
                if letters.count > 1 {
                    LetterBar(letters: letters) { letter in
                        guard let first = artists?.first(where: { $0.letter == letter }) else { return }
                        withAnimation(store.motion ? .easeOut(duration: 0.25) : nil) { reader.scrollTo("artist:" + first.name, anchor: .top) }
                    }
                    .padding(.trailing, 6)
                }
            }
        }
        .task(id: "\(store.search)\(store.libraryVersion)\(s.artistOrder)\(s.albumArtistsOnly)") {
            try? await Task.sleep(for: .milliseconds(store.search.isEmpty ? 0 : 150))
            guard !Task.isCancelled else { return }
            let q = store.search.trimmingCharacters(in: .whitespaces)
            let found = JSON.decode([ArtistSummary].self, await store.load { try $0.artistsJson(query: q) }) ?? []
            guard !Task.isCancelled else { return }
            artists = found
        }
    }

    /// The letters the artists file under, in their order, each once.
    private static func letters(_ artists: [ArtistSummary]) -> [String] {
        var seen: [String] = []
        for artist in artists where !artist.letter.isEmpty && !seen.contains(artist.letter) {
            seen.append(artist.letter)
        }
        return seen
    }

    private static func orderName(_ order: String) -> String {
        switch order {
        case "most_played": "Most played"
        case "most_songs": "Most songs"
        case "recently_added": "Newest"
        default: "A–Z"
        }
    }
}

/// The A–Z bar: the letters down the page's side; a click, or a drag along it, goes to the
/// first artist under that letter.
struct LetterBar: View {
    private var store: Store { .shared }
    let letters: [String]
    let go: (String) -> Void
    @State private var last: String?

    var body: some View {
        VStack(spacing: 0) {
            ForEach(letters, id: \.self) { letter in
                Text(letter)
                    .font(.system(size: 10, weight: .semibold))
                    .foregroundStyle(last == letter ? store.colors.accent : store.colors.ink2)
                    .frame(width: 18, height: 15)
            }
        }
        .padding(.vertical, 6)
        .background(Capsule().fill(store.colors.raised.opacity(0.6)))
        .contentShape(Rectangle())
        .gesture(DragGesture(minimumDistance: 0).onChanged { value in
            let index = Int((value.location.y - 6) / 15)
            guard letters.indices.contains(index), letters[index] != last else { return }
            last = letters[index]
            go(letters[index])
        }.onEnded { _ in last = nil })
        .accessibilityElement(children: .ignore)
        .accessibilityLabel("Go to a letter")
        .accessibilityAdjustableAction { direction in
            let at = last.flatMap { letters.firstIndex(of: $0) } ?? -1
            let next = direction == .increment ? min(at + 1, letters.count - 1) : max(at - 1, 0)
            last = letters[next]
            go(letters[next])
        }
    }
}

/// A page of tiles, under its title.
struct TileGrid<Content: View>: View {
    private var store: Store { .shared }
    let title: String
    let summary: String?
    @ViewBuilder let content: Content

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 18) {
                VStack(alignment: .leading, spacing: 2) {
                    Text(store.search.isEmpty ? title : "\(title) matching “\(store.search)”").font(store.display(28))
                    Text(summary ?? "Loading…").font(.needle(.callout)).foregroundStyle(store.colors.ink2)
                }
                LazyVGrid(columns: [GridItem(.adaptive(minimum: 160, maximum: 220), spacing: 22, alignment: .top)], spacing: 26) {
                    content
                }
            }
            .padding(28)
        }
    }
}

/// An artist, after Apple Music's: their picture tall across the top, under the bar's and
/// sidebar's glass too, fading into a page (the whole window) of its colour; their logo (or
/// name) in the middle near its foot, and under it a large Play between Shuffle, Radio, and
/// Blend; then the newest album beside the top songs, and the albums.
struct ArtistPage: View {
    private var store: Store { .shared }
    @Environment(\.panelInset) private var panelInset
    let name: String
    @State private var albums: [AlbumSummary]?
    @State private var top: [Song] = []
    @State private var similar: [String]?
    @State private var photo: String?
    @State private var logo: String?
    /// A wide picture of them from fanart.tv, for the top of the page before their photo.
    @State private var hero: String?
    /// Goes up when one of their pictures is chosen or reset, to read them again.
    @State private var pictures = 0

    var body: some View {
        let colors = store.colors
        ScrollView {
            VStack(alignment: .leading, spacing: 30) {
                banner
                VStack(alignment: .leading, spacing: 32) {
                    HStack(alignment: .top, spacing: 36) {
                        if let latest { featured(latest) }
                        if !top.isEmpty { topSongs }
                    }
                    VStack(alignment: .leading, spacing: 12) {
                        Text("Albums").font(.needle(.title3).weight(.semibold))
                        LazyVGrid(columns: [GridItem(.adaptive(minimum: 160, maximum: 220), spacing: 22, alignment: .top)], spacing: 26) {
                            ForEach(albums ?? []) { AlbumTile(album: $0, side: 172) }
                        }
                    }
                }
                .padding(.horizontal, 32)
                .padding(.trailing, panelInset)
            }
            .padding(.bottom, 28)
        }
        // Under the bar's glass (and the side panel's: see MainWindow.runsUnder).
        .ignoresSafeArea(edges: .top)
        .foregroundStyle(colors.ink)
        .task(id: "\(name)\(store.libraryVersion)\(pictures)") {
            let name = name
            // What the library has first: it never waits on the web.
            albums = JSON.decode([AlbumSummary].self, await store.load { try $0.artistAlbumsJson(name: name) }) ?? []
            top = await store.load { try $0.artistSongs(name: name) } ?? []
            // The colour at once, from their first album; the picture's when it comes (a first
            // visit downloads it).
            store.setPageTint(await Covers.pageColor(albums?.first?.artwork, seed: name), for: .artist(name))
            guard !placeholder else { return }
            // Looked up side by side, and let go of when the page closes.
            async let wide = store.load { $0.artistBanner(name: name) } ?? nil
            async let portrait = Photos.find(name, online: store.settings.onlineMedia, store: store)
            (hero, photo) = await (wide, portrait)
            if !Task.isCancelled, let picture = hero ?? photo {
                store.setPageTint(await Covers.pageColor(picture, seed: name), for: .artist(name))
            }
        }
        // Similar artists measure the whole library: once for the page, not on each change of
        // the library (a play, a heart), and not before the pictures.
        .task(id: name) {
            let name = name
            similar = await store.load { $0.similarArtists(artist: name) } ?? []
        }
        // The logo on its own: it shows as soon as it is found, not after the pictures.
        .task(id: "\(name)\(store.libraryVersion)\(pictures)") {
            guard !placeholder else { return }
            let name = name
            let found = await store.load { $0.artistLogo(name: name) } ?? nil
            if !Task.isCancelled { logo = found }
        }
    }

    /// Not a real artist: songs without one. Nothing is looked up for it.
    private var placeholder: Bool { name.isEmpty || name == "Unknown artist" }

    /// The newest album, for the box beside the top songs: only among albums with a year (with
    /// none, there is no box).
    private var latest: AlbumSummary? {
        albums?.filter { $0.year > 0 }.max { ($0.year, $0.album) < ($1.year, $1.album) }
    }

    private var banner: some View {
        let colors = store.colors
        return Color.clear
            .frame(maxWidth: .infinity)
            // Two thirds of the window, as in Apple Music, but never short.
            .containerRelativeFrame(.vertical) { height, _ in max(460, height * 0.66) }
            .overlay {
                Group {
                    // A wide picture of them, or their photo.
                    if let picture = hero ?? photo {
                        Artwork(path: picture, seed: name, size: 1800)
                            .overlay {
                                LinearGradient(stops: [
                                    .init(color: colors.canvas.opacity(0), location: 0.4),
                                    .init(color: colors.canvas.opacity(0.7), location: 0.78),
                                    .init(color: colors.canvas, location: 1),
                                ], startPoint: .top, endPoint: .bottom)
                            }
                            // No background extension here: it blurred the photo's side
                            // beside the side panel.
                    } else {
                        // No picture: their first album's cover, blurred across the top.
                        GeometryReader { geo in
                            HeaderBackdrop(path: albums?.first?.artwork, seed: name, height: geo.size.height)
                        }
                    }
                }
                .allowsHitTesting(false)
            }
            .clipped()
            .overlay(alignment: .bottom) {
                VStack(spacing: 18) {
                    // Their logo when fanart.tv has one, as Apple Music shows artists' own.
                    if let logo {
                        // Logos are mostly white: on a light page, the logo takes the page's ink.
                        Logo(path: logo, ink: colors.dark ? nil : colors.ink)
                            .frame(maxWidth: 420, maxHeight: 140)
                            .shadow(color: colors.canvas.opacity(0.5), radius: 18)
                            .accessibilityLabel(name)
                    } else {
                        Text(name.isEmpty ? "Unknown artist" : name)
                            .font(store.display(64))
                            .tracking(-1.2)
                            .multilineTextAlignment(.center)
                            .lineLimit(2)
                            .minimumScaleFactor(0.5)
                            .shadow(color: colors.canvas.opacity(0.35), radius: 14)
                    }
                    HStack(spacing: 20) {
                        round("shuffle", "Shuffle") { play(shuffle: true) }
                        Button { play(shuffle: false) } label: {
                            Image(systemName: "play.fill")
                                .font(.system(size: 30, weight: .bold))
                                .foregroundStyle(colors.canvas)
                                .offset(x: 2)
                                .frame(width: 76, height: 76)
                                .background(colors.ink, in: Circle())
                                .shadow(color: .black.opacity(0.25), radius: 12, y: 5)
                                .contentShape(Circle())
                        }
                        .buttonStyle(.plain)
                        .help("Play")
                        round("dot.radiowaves.left.and.right", "Radio") { store.artistRadio(name) }
                        Menu {
                            if let similar {
                                if similar.isEmpty { Text("No other artists are measured yet.") }
                                ForEach(similar, id: \.self) { other in
                                    Button(other) { store.blend(name, other) }
                                }
                            } else {
                                Text("Listening for similar artists…")
                            }
                        } label: {
                            Image(systemName: "circle.lefthalf.filled").font(.system(size: 15, weight: .semibold))
                        }
                        .menuStyle(.button)
                        .menuIndicator(.hidden)
                        .buttonBorderShape(.circle)
                        .glassButton()
                        .controlSize(.extraLarge)
                        .help("Blend with another artist")
                        if !placeholder {
                            ArtistPicturesMenu(name: name) {
                                Photos.forget(name)
                                pictures += 1
                            }
                        }
                    }
                    // Glass in the page's own ink, as Apple Music's.
                    .tint(colors.ink)
                }
                .padding(.horizontal, 32)
                .padding(.bottom, 6)
                .padding(.trailing, panelInset)
            }
    }

    /// A small round button of glass beside Play.
    private func round(_ symbol: String, _ help: String, _ action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Image(systemName: symbol).font(.system(size: 15, weight: .semibold))
        }
        .buttonBorderShape(.circle)
        .glassButton()
        .controlSize(.extraLarge)
        .help(help)
    }

    /// The newest album, as Apple Music's "Latest Release": its cover, then when, what, and
    /// how many songs.
    private func featured(_ album: AlbumSummary) -> some View {
        let colors = store.colors
        return VStack(alignment: .leading, spacing: 12) {
            Text("Latest Release").font(.needle(.title3).weight(.semibold))
            HStack(alignment: .center, spacing: 18) {
                Cover(path: album.artwork, seed: album.album + album.artist, side: 190, corner: 10)
                    .shadow(color: .black.opacity(0.25), radius: 12, y: 6)
                VStack(alignment: .leading, spacing: 4) {
                    if album.year > 0 {
                        Text(String(album.year)).font(.needle(.callout).weight(.semibold)).foregroundStyle(colors.ink2)
                    }
                    Text(album.title).font(store.display(20, weight: .semibold)).lineLimit(3)
                    Text(count(album.tracks, "song")).foregroundStyle(colors.ink2)
                    Button { playAlbum(album) } label: { Label("Play", systemImage: "play.fill") }
                        .glassButton()
                        .tint(store.isPaged ? colors.ink : colors.accent)
                        .padding(.top, 6)
                }
                .frame(maxWidth: 220, alignment: .leading)
            }
            .contentShape(Rectangle())
            .onTapGesture { store.navigate(album.route) }
            .accessibilityAddTraits(.isButton)
            .accessibilityAction { store.navigate(album.route) }
            .contextMenu { GroupMenu(route: album.route, title: album.title) }
        }
    }

    private var topSongs: some View {
        let colors = store.colors
        return VStack(alignment: .leading, spacing: 8) {
            Text("Top Songs").font(.needle(.title3).weight(.semibold))
            LazyVGrid(columns: [GridItem(.adaptive(minimum: 280), spacing: 18)], alignment: .leading, spacing: 4) {
                ForEach(Array(top.prefix(8).enumerated()), id: \.element.id) { index, song in
                    HStack(spacing: 12) {
                        Cover(path: song.artwork, seed: song.album + name, side: 44, corner: 5)
                        VStack(alignment: .leading, spacing: 1) {
                            Text(song.title).lineLimit(1)
                            Text(song.album).font(.needle(.caption)).foregroundStyle(colors.ink2).lineLimit(1)
                        }
                        Spacer()
                        Text(clock(song.duration)).font(.needle(.caption)).monospacedDigit().foregroundStyle(colors.ink3)
                    }
                    .padding(.vertical, 5)
                    .overlay(alignment: .bottom) { Rectangle().fill(colors.lineSoft).frame(height: 1).padding(.leading, 56) }
                    .contentShape(Rectangle())
                    .onTapGesture(count: 2) { store.play(top.map(\.asTrack), from: index, reason: "From \(name)") }
                    .contextMenu { SongMenu(tracks: [song.fullTrack]) }
                }
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    private func play(shuffle: Bool) {
        let (rule, reason) = (Route.artist(name).base, "From \(name)")
        Task { _ = await store.ask { try $0.playView(query: rule, playlist: nil, filter: "", start: 0, shuffle: shuffle, reason: reason) } }
    }

    private func playAlbum(_ album: AlbumSummary) {
        let (rule, reason) = (album.query, "From the album \(album.album)")
        Task { _ = await store.ask { try $0.playView(query: rule, playlist: nil, filter: "", start: 0, shuffle: false, reason: reason) } }
    }
}

/// The music folders, as they are on disk.
struct FoldersPage: View {
    private var store: Store { .shared }
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
                            Label((root as NSString).lastPathComponent, systemImage: "folder.fill").font(.needle(.title3).weight(.semibold))
                        }
                        .buttonStyle(.plain)
                        Text(root).font(.needle(.caption)).foregroundStyle(.secondary)
                        LazyVGrid(columns: [GridItem(.adaptive(minimum: 140, maximum: 180), spacing: 18, alignment: .top)], spacing: 20) {
                            ForEach(inside[root] ?? [], id: \.path) { folder in
                                VStack(alignment: .leading, spacing: 4) {
                                    ZStack {
                                        if let art = folder.artwork {
                                            Cover(path: art, seed: folder.name, side: 140, corner: 8)
                                        } else {
                                            RoundedRectangle(cornerRadius: 8).fill(store.colors.raised).frame(width: 140, height: 140)
                                                .overlay { Image(systemName: "folder").font(.needle(.largeTitle)).foregroundStyle(store.colors.ink3) }
                                        }
                                    }
                                    Text(folder.name).font(.needle(.callout).weight(.medium)).lineLimit(1)
                                    Text(count(Int(folder.songs), "song")).font(.needle(.caption)).foregroundStyle(.secondary)
                                }
                                .frame(width: 140)
                                .contentShape(Rectangle())
                                .onTapGesture { store.navigate(.folder(folder.path)) }
                                .accessibilityElement(children: .combine)
                                .accessibilityAddTraits(.isButton)
                                .accessibilityAction { store.navigate(.folder(folder.path)) }
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

