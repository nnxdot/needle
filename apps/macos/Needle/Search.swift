import SwiftUI

/// What a search finds, as the core gives it (`find_json`; the phones' record is `SearchFound`).
struct FoundData: Decodable {
    struct Best: Decodable {
        var kind: String
        var track: Track?
        var album: AlbumSummary?
        var artist: ArtistSummary?
    }
    struct Line: Decodable {
        var track: Track
        var line: String
    }
    /// How many of each match in all, not only the rows given.
    struct Counts: Decodable {
        var songs: Int
        var albums: Int
        var artists: Int
        var lyrics: Int
    }
    var best: Best?
    var songs: [Track]
    var albums: [AlbumSummary]
    var artists: [ArtistSummary]
    var lyrics: [Line]
    var counts: Counts?

    var isEmpty: Bool { best == nil && songs.isEmpty && albums.isEmpty && artists.isEmpty && lyrics.isEmpty }

    /// How many a chip finds in all (never fewer than the rows given).
    func count(_ chip: SearchChip) -> Int {
        switch chip {
        case .all: 0
        case .songs: max(counts?.songs ?? 0, songs.count)
        case .albums: max(counts?.albums ?? 0, albums.count)
        case .artists: max(counts?.artists ?? 0, artists.count)
        case .lyrics: max(counts?.lyrics ?? 0, lyrics.count)
        }
    }
}

/// Something looked for before, for Search's Recent list (`recent_searches_json`). `kind` is
/// "words", "song", "album", or "artist"; `key` is what remember and forget take.
struct RecentFind: Decodable, Identifiable {
    var kind: String
    var key: String
    var track: Track?
    var album: AlbumSummary?
    var artist: ArtistSummary?
    var id: String { kind + "|" + key }
}

/// Search's chips.
enum SearchChip: String, CaseIterable {
    case all = "All", songs = "Songs", albums = "Albums", artists = "Artists", lyrics = "Lyrics"
}

/// What Search remembers for its Recent list: the words, when Return is pressed in the field,
/// and each result opened or played from Search.
@MainActor
enum SearchMemory {
    static func remember(_ kind: String, _ key: String) {
        let key = kind == "words" ? key.trimmingCharacters(in: .whitespaces) : key
        guard !key.isEmpty else { return }
        Task { _ = await Store.shared.load { try $0.rememberSearch(kind: kind, key: key) } }
    }

    static func song(_ track: Track) { remember("song", track.id) }
    static func album(_ album: AlbumSummary) { remember("album", album.key) }
    static func artist(_ name: String) { remember("artist", name) }
}

/// `text` with the letters that match `words` (without case) in the accent and bold.
@MainActor
func litText(_ text: String, _ words: [String]) -> AttributedString {
    var out = AttributedString(text)
    let accent = Store.shared.colors.accent
    for word in words where !word.isEmpty {
        var from = text.startIndex
        while from < text.endIndex,
              let range = text.range(of: word, options: [.caseInsensitive, .diacriticInsensitive], range: from..<text.endIndex) {
            if let start = AttributedString.Index(range.lowerBound, within: out),
               let end = AttributedString.Index(range.upperBound, within: out) {
                out[start..<end].foregroundColor = accent
                out[start..<end].inlinePresentationIntent = .stronglyEmphasized
            }
            from = range.upperBound
        }
    }
    return out
}

/// The Search page: what typing in search finds on a page that is not a list. Before typing:
/// Recent, Rules, and Browse (genres). While typing: chips (All, Songs, Albums, Artists,
/// Lyrics, each with its count) that stay at the top, and under All the top result, Songs,
/// Albums, Artists, and the lines of lyrics that hold the words.
struct SearchPage: View {
    private var store: Store { .shared }
    @State private var found: FoundData?
    /// The search and chip `found` is for ("words|Songs").
    @State private var foundFor = ""
    @State private var chip: SearchChip = .all
    @State private var error: String?
    /// The words to light: the search's words, or none for a rule.
    @State private var words: [String] = []
    @State private var recent: [RecentFind] = []
    @State private var recentVersion = 0
    @State private var genres: [Genre] = []
    @State private var width: CGFloat = 900
    @Namespace private var chipSpace

    /// The content is too narrow for two columns.
    private var narrow: Bool { width - 56 < 640 }

    var body: some View {
        let query = store.search.trimmingCharacters(in: .whitespaces)
        VStack(alignment: .leading, spacing: 0) {
            // The title and the chips stay at the top while the results scroll.
            VStack(alignment: .leading, spacing: 14) {
                Text(query.isEmpty ? "Search" : "“\(query)”").font(store.display(28)).tracking(store.displayTracking(28)).lineLimit(1)
                if !query.isEmpty { chips }
            }
            .padding(.horizontal, 28)
            .padding(.top, 28)
            .padding(.bottom, 12)
            ScrollView {
                ZStack(alignment: .topLeading) {
                    VStack(alignment: .leading, spacing: 26) {
                        content(query)
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .id(query.isEmpty ? "" : foundFor)
                    // New results fade in over the old ones, which stay until then.
                    .transition(.opacity)
                }
                .padding(.horizontal, 28)
                .padding(.top, 12)
                .padding(.bottom, 28)
            }
        }
        .onGeometryChange(for: CGFloat.self) { $0.size.width } action: { width = $0 }
        #if DEBUG
        // Test builds: `--search-chip songs` chooses that chip once results show.
        .onChange(of: found == nil) { _, none in
            let arguments = CommandLine.arguments
            guard !none, let i = arguments.firstIndex(of: "--search-chip"), arguments.indices.contains(i + 1),
                  let wanted = SearchChip.allCases.first(where: { $0.rawValue.lowercased() == arguments[i + 1] }) else { return }
            select(wanted)
        }
        #endif
        .task(id: "\(query)|\(chip)|\(store.libraryVersion)") {
            guard !query.isEmpty else {
                found = nil; error = nil; foundFor = ""
                if chip != .all { chip = .all }
                return
            }
            try? await Task.sleep(for: .milliseconds(150))
            guard !Task.isCancelled else { return }
            let limit: UInt32 = chip == .all ? 12 : 200
            let next = JSON.decode(FoundData.self, await store.load { try $0.findJson(query: query, limit: limit) })
            guard !Task.isCancelled else { return }
            let check = store.core.checkQuery(query: query)
            let plain = check.explanation?.hasPrefix("Title, artist") ?? false
            withAnimation(.easeOut(duration: 0.18)) {
                found = next
                foundFor = "\(query)|\(chip)"
                words = plain ? query.split(whereSeparator: \.isWhitespace).map(String.init) : []
                // A rule that does not read shows its error, as the Songs page does.
                error = (next?.isEmpty ?? true) ? check.error : nil
            }
            // The chip chosen finds nothing for these words: back to All.
            if chip != .all, let next, !next.isEmpty, next.count(chip) == 0 { chip = .all }
        }
        .task(id: "\(query.isEmpty)|\(recentVersion)|\(store.libraryVersion)") {
            let json = await store.load { try $0.recentSearchesJson(limit: 8) }
            recent = JSON.decode([RecentFind].self, json) ?? []
        }
        .task(id: store.libraryVersion) {
            let all = await store.load { try $0.genres() } ?? []
            genres = Array(all.sorted { $0.songs > $1.songs }.prefix(8))
        }
    }

    @ViewBuilder
    private func content(_ query: String) -> some View {
        if query.isEmpty {
            if !recent.isEmpty { recentSection }
            rules
            if !genres.isEmpty { browse }
        } else if let error {
            Text(error).foregroundStyle(store.colors.danger)
            rules
        } else if let found {
            if found.isEmpty {
                nothing(query)
            } else {
                switch chip {
                case .all: all(found, query: query)
                case .songs: songsPage(found, query: query)
                case .albums: albumsPage(found)
                case .artists: artistsPage(found)
                case .lyrics: lyricLines(found.lyrics, query: query)
                }
            }
        }
    }

    // MARK: Chips

    private var chips: some View {
        HStack(spacing: 8) {
            ForEach(SearchChip.allCases, id: \.self) { c in
                chipButton(c, n: c == .all ? nil : found?.count(c))
            }
        }
    }

    private func chipButton(_ c: SearchChip, n: Int?) -> some View {
        let colors = store.colors
        let on = chip == c
        let empty = n == 0
        return Button { select(c) } label: {
            HStack(spacing: 5) {
                Text(c.rawValue).font(.needle(.callout).weight(on ? .semibold : .medium))
                if let n { Text(n.formatted()).font(.needle(.caption)).monospacedDigit().opacity(0.62) }
            }
            .padding(.horizontal, 14)
            .padding(.vertical, 6)
            .foregroundStyle(on ? colors.accentInk : colors.ink)
            .background {
                if on {
                    Capsule().fill(colors.accent).matchedGeometryEffect(id: "lit", in: chipSpace)
                } else {
                    Capsule().fill(colors.raised)
                }
            }
            .contentShape(Capsule())
        }
        .buttonStyle(.plain)
        .opacity(empty ? 0.4 : 1)
        .disabled(empty)
        .accessibilityLabel(n.map { "\(c.rawValue), \($0)" } ?? c.rawValue)
        .accessibilityAddTraits(on ? .isSelected : [])
    }

    private func select(_ c: SearchChip) {
        // The lit chip glides to the one chosen.
        withAnimation(store.motion ? .snappy(duration: 0.25) : nil) { chip = c }
    }

    // MARK: All

    @ViewBuilder
    private func all(_ found: FoundData, query: String) -> some View {
        let songs = Array(found.songs.prefix(5))
        let songList = VStack(alignment: .leading, spacing: 8) {
            head("Songs", all: found.count(.songs), chip: .songs)
            VStack(spacing: 2) {
                ForEach(Array(songs.enumerated()), id: \.element.id) { i, track in
                    SearchSongRow(track: track, title: litText(track.title, words), detail: track.artist) {
                        SearchMemory.song(track)
                        store.play(songs, from: i, reason: "Matches “\(query)”")
                    }
                }
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        if narrow {
            if let best = found.best { topResult(best, query: query) }
            if !songs.isEmpty { songList }
        } else {
            HStack(alignment: .top, spacing: 28) {
                if let best = found.best { topResult(best, query: query) }
                if !songs.isEmpty { songList }
            }
        }
        if !found.albums.isEmpty {
            VStack(alignment: .leading, spacing: 10) {
                head("Albums", all: found.count(.albums), chip: .albums)
                PagedRow(tile: 156, spacing: 18, middle: 78) {
                    HStack(alignment: .top, spacing: 18) {
                        ForEach(found.albums) { album in
                            AlbumTile(album: album, side: 156, place: "search", words: words) { SearchMemory.album(album) }
                        }
                    }
                    .padding(.horizontal, 2)
                    .padding(.bottom, 6)
                }
            }
        }
        let artists = artistShelf(found.artists)
        let lyrics = VStack(alignment: .leading, spacing: 10) {
            head("In the lyrics", all: found.count(.lyrics), chip: .lyrics)
            lyricLines(Array(found.lyrics.prefix(3)), query: query)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        // Artists and the lyrics share a line when there is room for both.
        if !narrow && !found.artists.isEmpty && !found.lyrics.isEmpty {
            HStack(alignment: .top, spacing: 28) {
                artists.frame(width: 292, alignment: .leading)
                lyrics
            }
        } else {
            if !found.artists.isEmpty { artists }
            if !found.lyrics.isEmpty { lyrics }
        }
    }

    private func artistShelf(_ artists: [ArtistSummary]) -> some View {
        VStack(alignment: .leading, spacing: 10) {
            head("Artists", all: nil, chip: nil)
            PagedRow(tile: 112, spacing: 16, middle: 56) {
                HStack(alignment: .top, spacing: 16) {
                    ForEach(artists) { artist in
                        ArtistTile(artist: artist, side: 112, words: words) { SearchMemory.artist(artist.name) }
                    }
                }
                .padding(.horizontal, 2)
                .padding(.bottom, 6)
            }
        }
    }

    /// A part's title, and "All N ›" that chooses its chip.
    private func head(_ title: String, all n: Int?, chip c: SearchChip?) -> some View {
        HStack(alignment: .firstTextBaseline) {
            Text(title).font(.needle(.title3).weight(.semibold))
            Spacer()
            if let n, n > 0, let c {
                Button("All \(n.formatted()) ›") { select(c) }
                    .buttonStyle(.plain)
                    .font(.needle(.callout))
                    .foregroundStyle(store.colors.ink2)
                    .accessibilityLabel("All \(n.formatted()) \(title.lowercased())")
            }
        }
    }

    // MARK: Top result

    /// The best match on a card that takes its cover's colour: the cover blurred behind a veil,
    /// with a thin glass edge. Wide: the cover above the text; narrow: beside it.
    private func topResult(_ best: FoundData.Best, query: String) -> some View {
        let colors = store.colors
        let (name, detail, kind) = describe(best)
        let second: String = best.kind == "song" ? "Play next" : "Shuffle"
        let picture = Group {
            if best.kind == "artist", let artist = best.artist {
                ArtistPhoto(name: artist.name, side: narrow ? 96 : 116)
            } else if let album = best.album {
                Cover(path: album.artwork, seed: album.album + album.artist, side: narrow ? 96 : 116, corner: 10)
                    .modifier(TravelingCover(id: album.query, on: store.travelingTile == "search-top|" + album.query))
            } else {
                Cover(path: best.track?.artwork, seed: (best.track?.album ?? "") + (best.track?.artist ?? ""), side: narrow ? 96 : 116, corner: 10)
            }
        }
        .shadow(color: .black.opacity(0.36), radius: 10, y: 8)
        let text = VStack(alignment: .leading, spacing: 2) {
            Text("Top result · \(kind)".uppercased())
                .font(.needle(.caption).weight(.bold)).kerning(1)
                .foregroundStyle(colors.accent)
            Text(litText(name, words)).font(store.display(narrow ? 20 : 23, weight: .heavy)).lineLimit(2)
                .padding(.top, 2)
            if !detail.isEmpty {
                Text(detail).font(.needle(.callout)).foregroundStyle(colors.ink.opacity(0.78)).lineLimit(1)
            }
            HStack(spacing: 8) {
                Button { play(best, shuffle: false) } label: { Label("Play", systemImage: "play.fill") }
                    .glassButton(prominent: true)
                if best.kind == "song" {
                    Button { playNext(best) } label: { Label("Play next", systemImage: "text.line.first.and.arrowtriangle.forward") }
                        .glassButton()
                } else {
                    Button { play(best, shuffle: true) } label: { Label("Shuffle", systemImage: "shuffle") }
                        .glassButton()
                }
            }
            .controlSize(.large)
            .padding(.top, narrow ? 9 : 12)
        }
        let shape = RoundedRectangle(cornerRadius: Radius.large, style: .continuous)
        return Group {
            if narrow {
                HStack(spacing: 14) { picture; text; Spacer(minLength: 0) }
                    .padding(14)
                    .frame(maxWidth: .infinity, alignment: .leading)
            } else {
                VStack(alignment: .leading, spacing: 14) { picture; text }
                    .padding(18)
                    .frame(width: 292, alignment: .leading)
            }
        }
        .background {
            ZStack {
                colors.raised
                Group {
                    if best.kind == "artist", let artist = best.artist {
                        ArtistPhoto(name: artist.name, side: 240)
                    } else {
                        Cover(path: best.album?.artwork ?? best.track?.artwork, seed: name + detail, side: 240, corner: 0)
                    }
                }
                .scaleEffect(1.6)
                .blur(radius: 28)
                .saturation(1.5)
                .opacity(0.8)
                LinearGradient(colors: [colors.canvas.opacity(colors.dark ? 0.38 : 0.46), colors.canvas.opacity(0.8)],
                               startPoint: .top, endPoint: .bottom)
            }
            .allowsHitTesting(false)
        }
        .clipShape(shape)
        .overlay(shape.strokeBorder(Color.white.opacity(colors.dark ? 0.16 : 0.9), lineWidth: 1))
        .shadow(color: .black.opacity(colors.dark ? 0.34 : 0.18), radius: 13, y: 10)
        .contentShape(shape)
        .onTapGesture { open(best) }
        .accessibilityElement(children: .combine)
        .accessibilityAddTraits(.isButton)
        .accessibilityLabel("Top result, \(kind), \(name)")
        .accessibilityAction { open(best) }
        .accessibilityAction(named: "Play") { play(best, shuffle: false) }
        .accessibilityAction(named: second) {
            if best.kind == "song" { playNext(best) } else { play(best, shuffle: true) }
        }
    }

    /// The name, the detail line, and the kind ("Album") of the best match.
    private func describe(_ best: FoundData.Best) -> (String, String, String) {
        switch best.kind {
        case "album":
            let album = best.album
            var parts = [album?.artist ?? ""]
            if let year = album?.year, year > 0 { parts.append(String(year)) }
            parts.append(count(album?.tracks ?? 0, "song"))
            return (album?.title ?? "", parts.filter { !$0.isEmpty }.joined(separator: " · "), "Album")
        case "artist":
            let artist = best.artist
            return (artist?.title ?? "", "\(count(artist?.tracks ?? 0, "song")) · \(count(artist?.albums ?? 0, "album"))", "Artist")
        default:
            let track = best.track
            return (track?.title ?? "", [track?.artist ?? "", track?.album ?? ""].filter { !$0.isEmpty }.joined(separator: " · "), "Song")
        }
    }

    // MARK: Chip pages

    /// "N songs" and Shuffle (all that match), then up to 200 rows.
    @ViewBuilder
    private func songsPage(_ found: FoundData, query: String) -> some View {
        let total = found.count(.songs)
        HStack {
            Text(count(total, "song")).font(.needle(.callout)).foregroundStyle(store.colors.ink2)
            Spacer()
            Button { shuffleAll(query) } label: { Label("Shuffle", systemImage: "shuffle") }
                .glassButton(prominent: true)
        }
        LazyVStack(spacing: 2) {
            ForEach(Array(found.songs.enumerated()), id: \.element.id) { i, track in
                SearchSongRow(track: track, title: litText(track.title, words),
                              detail: [track.artist, track.album].filter { !$0.isEmpty }.joined(separator: " · ")) {
                    SearchMemory.song(track)
                    store.play(found.songs, from: i, reason: "Matches “\(query)”")
                }
            }
            if total > found.songs.count { showAll(total, "Songs", .songs) }
        }
    }

    @ViewBuilder
    private func albumsPage(_ found: FoundData) -> some View {
        let total = found.count(.albums)
        LazyVGrid(columns: [GridItem(.adaptive(minimum: 160, maximum: 200), spacing: 22, alignment: .top)], alignment: .leading, spacing: 26) {
            ForEach(found.albums) { album in
                AlbumTile(album: album, side: 160, place: "search-albums", words: words) { SearchMemory.album(album) }
            }
        }
        if total > found.albums.count { showAll(total, "Albums", .albums) }
    }

    @ViewBuilder
    private func artistsPage(_ found: FoundData) -> some View {
        let total = found.count(.artists)
        LazyVGrid(columns: [GridItem(.adaptive(minimum: 140, maximum: 180), spacing: 20, alignment: .top)], alignment: .leading, spacing: 24) {
            ForEach(found.artists) { artist in
                ArtistTile(artist: artist, side: 132, words: words) { SearchMemory.artist(artist.name) }
            }
        }
        if total > found.artists.count { showAll(total, "Artists", .artists) }
    }

    /// The last row when there are more than the page shows: that page, with the search.
    private func showAll(_ total: Int, _ title: String, _ route: Route) -> some View {
        Button {
            let text = store.search
            store.navigate(route)
            store.search = text
        } label: {
            Text("Show all \(total.formatted()) in \(title)")
                .font(.needle(.callout).weight(.medium))
                .foregroundStyle(store.colors.accent)
                .padding(.vertical, 10)
                .frame(maxWidth: .infinity, alignment: .leading)
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .padding(.horizontal, 8)
    }

    /// Lines of lyrics that hold the words, the words lit; a click plays the song from the start.
    private func lyricLines(_ lines: [FoundData.Line], query: String) -> some View {
        let lit = words.isEmpty ? [query] : words
        return VStack(alignment: .leading, spacing: 4) {
            ForEach(Array(lines.enumerated()), id: \.offset) { _, hit in
                LyricHitRow(hit: hit, text: litText(hit.line, lit)) {
                    SearchMemory.song(hit.track)
                    store.play([hit.track], reason: "Its lyrics hold “\(query)”")
                }
            }
        }
    }

    // MARK: Before typing, and nothing found

    private var recentSection: some View {
        let columns = Array(repeating: GridItem(.flexible(), spacing: 12), count: narrow ? 2 : 4)
        return VStack(alignment: .leading, spacing: 10) {
            HStack(alignment: .firstTextBaseline) {
                Text("Recent").font(.needle(.title3).weight(.semibold))
                Spacer()
                Button("Clear") {
                    recent = []
                    Task {
                        _ = await store.load { try $0.clearSearches() }
                        recentVersion += 1
                    }
                }
                .buttonStyle(.plain)
                .font(.needle(.callout))
                .foregroundStyle(store.colors.ink2)
                .accessibilityLabel("Clear recent searches")
            }
            LazyVGrid(columns: columns, alignment: .leading, spacing: 12) {
                ForEach(recent.prefix(4)) { item in
                    RecentTile(item: item, open: { openRecent(item) }) {
                        recent.removeAll { $0.id == item.id }
                        let (kind, key) = (item.kind, item.key)
                        Task {
                            _ = await store.load { try $0.forgetSearch(kind: kind, key: key) }
                            recentVersion += 1
                        }
                    }
                }
            }
        }
    }

    private func openRecent(_ item: RecentFind) {
        switch item.kind {
        case "words": store.search = item.key
        case "song":
            guard let track = item.track else { return }
            SearchMemory.song(track)
            store.play([track], reason: "From your recent searches")
        case "album":
            guard let album = item.album else { return }
            SearchMemory.album(album)
            store.navigate(album.route)
        case "artist":
            guard let artist = item.artist else { return }
            SearchMemory.artist(artist.name)
            store.navigate(.artist(artist.name))
        default: break
        }
    }

    /// The genres with the most songs, as colour tiles, two lines of them; a click opens the
    /// genre's songs. The Mac has no page of all genres, so no "All".
    private var browse: some View {
        let across = narrow ? 2 : 4
        let columns = Array(repeating: GridItem(.flexible(), spacing: 12), count: across)
        return VStack(alignment: .leading, spacing: 10) {
            Text("Browse").font(.needle(.title3).weight(.semibold))
            LazyVGrid(columns: columns, alignment: .leading, spacing: 12) {
                ForEach(genres.prefix(across * 2), id: \.name) { genre in
                    GenreTile(name: genre.name, artwork: genre.artwork, songs: Int(genre.songs))
                }
            }
        }
    }

    /// The core's rules as chips on one line, with the rule itself in mono when there is room; the
    /// line scrolls sideways when it does not fit. A click puts the rule in the search field.
    private var rules: some View {
        let examples = store.core.ruleExamples()
        let pairs = stride(from: 0, to: examples.count - 1, by: 2).map { (examples[$0], examples[$0 + 1]) }
        return VStack(alignment: .leading, spacing: 10) {
            Text("Rules").font(.needle(.title3).weight(.semibold))
            ScrollView(.horizontal, showsIndicators: false) {
                HStack(spacing: 8) {
                    ForEach(pairs, id: \.1) { name, rule in
                        Button { store.search = rule } label: {
                            HStack(spacing: 8) {
                                Text(name).font(.needle(.callout).weight(.medium))
                                if !narrow {
                                    Text(rule).font(.system(size: 11.5, design: .monospaced)).foregroundStyle(store.colors.ink2)
                                }
                            }
                            .padding(.horizontal, 13).padding(.vertical, 6)
                            .overlay(Capsule().strokeBorder(store.colors.line))
                            .contentShape(Capsule())
                        }
                        .buttonStyle(.plain)
                        .help(rule)
                        .accessibilityLabel("\(name): \(rule)")
                    }
                }
                .padding(.vertical, 1)
            }
            // The chips scroll out to the page's edges, not to its margin.
            .scrollClipDisabled()
        }
    }

    @ViewBuilder
    private func nothing(_ query: String) -> some View {
        VStack(spacing: 6) {
            Image(systemName: "magnifyingglass").font(.system(size: 26, weight: .medium))
                .foregroundStyle(store.colors.ink2)
                .frame(width: 64, height: 64)
                .background(store.colors.raised, in: Circle())
                .padding(.bottom, 8)
            Text("Nothing for “\(query)”").font(.needle(.title3).weight(.bold)).multilineTextAlignment(.center)
            Text("Look at the spelling. Or search with a rule.").foregroundStyle(store.colors.ink2)
        }
        .frame(maxWidth: .infinity)
        .padding(.top, 24)
        .accessibilityElement(children: .combine)
        rules
        if !recent.isEmpty { recentSection }
    }

    // MARK: Actions

    private func open(_ best: FoundData.Best) {
        switch best.kind {
        case "album":
            guard let album = best.album else { return }
            SearchMemory.album(album)
            // The cover flies to the album's page, as from an album tile.
            store.travelingTile = "search-top|" + album.query
            withAnimation(store.motion ? .smooth(duration: 0.3) : nil) { store.navigate(album.route) }
        case "artist":
            guard let artist = best.artist else { return }
            SearchMemory.artist(artist.name)
            store.navigate(.artist(artist.name))
        default: play(best, shuffle: false)
        }
    }

    private func play(_ best: FoundData.Best, shuffle: Bool) {
        switch best.kind {
        case "album":
            guard let album = best.album else { return }
            SearchMemory.album(album)
            let (rule, reason) = (album.query, "From the album \(album.album)")
            Task { _ = await store.ask { try $0.playView(query: rule, playlist: nil, filter: "", start: 0, shuffle: shuffle, reason: reason) } }
        case "artist":
            guard let artist = best.artist else { return }
            SearchMemory.artist(artist.name)
            let (rule, reason) = (Route.artist(artist.name).base, "From \(artist.name)")
            Task { _ = await store.ask { try $0.playView(query: rule, playlist: nil, filter: "", start: 0, shuffle: shuffle, reason: reason) } }
        default:
            guard let track = best.track else { return }
            SearchMemory.song(track)
            store.play([track], reason: "Matches “\(store.search)”")
        }
    }

    private func playNext(_ best: FoundData.Best) {
        guard let track = best.track else { return }
        SearchMemory.song(track)
        store.playNext([track.id])
    }

    /// Every song that matches, shuffled.
    private func shuffleAll(_ query: String) {
        let reason = "Matches “\(query)”"
        Task {
            guard let n = await store.ask({ try $0.playView(query: query, playlist: nil, filter: "", start: 0, shuffle: true, reason: reason) }) else { return }
            if n == 0 { store.notify("Nothing here can be played. The files may be unavailable.") }
        }
    }
}

/// A song in Search: its cover, title above the second line, and its length; Play shows on the
/// cover under the pointer.
private struct SearchSongRow: View {
    private var store: Store { .shared }
    let track: Track
    let title: AttributedString
    let detail: String
    let play: () -> Void
    @State private var hover = false

    var body: some View {
        HStack(spacing: 12) {
            ZStack {
                Cover(path: track.artwork, seed: track.album + track.artist, side: 38, corner: 6)
                if hover {
                    Image(systemName: "play.fill").font(.system(size: 13, weight: .bold)).foregroundStyle(.white)
                        .frame(width: 38, height: 38)
                        .background(.black.opacity(0.35), in: RoundedRectangle(cornerRadius: 6, style: .continuous))
                }
            }
            VStack(alignment: .leading, spacing: 1) {
                Text(title).lineLimit(1)
                Text(detail).font(.needle(.caption)).foregroundStyle(store.colors.ink2).lineLimit(1)
            }
            Spacer()
            Text(clock(track.duration)).font(.needle(.caption)).monospacedDigit().foregroundStyle(store.colors.ink3)
        }
        .padding(.horizontal, 8)
        .padding(.vertical, 4)
        .background(hover ? Color.primary.opacity(0.07) : .clear, in: RoundedRectangle(cornerRadius: Radius.small, style: .continuous))
        .contentShape(Rectangle())
        .onHover { hover = $0 }
        // One click plays at once: no double-click gesture beside it, which made SwiftUI hold
        // every single click for the double-click time.
        .onTapGesture(perform: play)
        .accessibilityElement(children: .combine)
        .accessibilityAddTraits(.isButton)
        .accessibilityAction(named: "Play") { play() }
    }
}

/// A line of lyrics that holds the words, the song under it; a click plays the song.
private struct LyricHitRow: View {
    private var store: Store { .shared }
    let hit: FoundData.Line
    let text: AttributedString
    let play: () -> Void
    @State private var hover = false

    var body: some View {
        VStack(alignment: .leading, spacing: 3) {
            Text(text).font(.needle(.title3))
            HStack(spacing: 7) {
                Cover(path: hit.track.artwork, seed: hit.track.album + hit.track.artist, side: 18, corner: 4)
                Text("\(hit.track.title) · \(hit.track.artist)").font(.needle(.callout)).foregroundStyle(store.colors.ink2).lineLimit(1)
            }
        }
        .padding(.vertical, 8)
        .padding(.horizontal, 12)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(hover ? Color.primary.opacity(0.06) : .clear, in: RoundedRectangle(cornerRadius: Radius.small, style: .continuous))
        .contentShape(Rectangle())
        .onHover { hover = $0 }
        .onTapGesture(perform: play)
        .accessibilityElement(children: .combine)
        .accessibilityAddTraits(.isButton)
        .accessibilityLabel("\(hit.line), from \(hit.track.title) by \(hit.track.artist)")
        .accessibilityAction { play() }
    }
}

/// A recent search: its picture (round for an artist, a clock for words), its name, and what
/// it is; × under the pointer drops it.
private struct RecentTile: View {
    private var store: Store { .shared }
    let item: RecentFind
    let open: () -> Void
    let forget: () -> Void
    @State private var hover = false

    var body: some View {
        let colors = store.colors
        let (name, detail) = describe()
        HStack(spacing: 10) {
            picture
            VStack(alignment: .leading, spacing: 1) {
                Text(name).font(.needle(.callout).weight(.medium)).lineLimit(1)
                Text(detail).font(.needle(.caption)).foregroundStyle(colors.ink2).lineLimit(1)
            }
            Spacer(minLength: 0)
            Button(action: forget) {
                Image(systemName: "xmark").font(.system(size: 10, weight: .bold)).foregroundStyle(colors.ink2)
                    .frame(width: 22, height: 22)
                    .background(colors.raisedHover, in: Circle())
            }
            .buttonStyle(.plain)
            .opacity(hover ? 1 : 0)
            .allowsHitTesting(hover)
            .accessibilityLabel("Remove \(name) from Recent")
        }
        .padding(8)
        .background(hover ? colors.raisedHover : colors.raised, in: RoundedRectangle(cornerRadius: 12, style: .continuous))
        .contentShape(Rectangle())
        .onHover { hover = $0 }
        .onTapGesture(perform: open)
        .accessibilityElement(children: .combine)
        .accessibilityAddTraits(.isButton)
        .accessibilityLabel("\(name), \(detail)")
        .accessibilityAction { open() }
        .accessibilityAction(named: "Remove") { forget() }
    }

    @ViewBuilder
    private var picture: some View {
        switch item.kind {
        case "artist":
            ArtistPhoto(name: item.key, side: 40)
        case "album":
            Cover(path: item.album?.artwork, seed: (item.album?.album ?? "") + (item.album?.artist ?? ""), side: 40, corner: 6)
        case "song":
            Cover(path: item.track?.artwork, seed: (item.track?.album ?? "") + (item.track?.artist ?? ""), side: 40, corner: 6)
        default:
            Image(systemName: "clock").font(.system(size: 17)).foregroundStyle(store.colors.ink2)
                .frame(width: 40, height: 40)
                .background(store.colors.raisedHover, in: RoundedRectangle(cornerRadius: 6, style: .continuous))
        }
    }

    private func describe() -> (String, String) {
        switch item.kind {
        case "artist": return (item.artist?.title ?? item.key, "Artist")
        case "album": return (item.album?.title ?? "", "Album · \(item.album?.artist ?? "")")
        case "song": return (item.track?.title ?? "", "Song · \(item.track?.artist ?? "")")
        default: return (item.key, "You searched for this")
        }
    }
}

/// A genre as a colour tile with one of its covers in the corner; a click opens its songs.
private struct GenreTile: View {
    private var store: Store { .shared }
    let name: String
    let artwork: String?
    let songs: Int
    @State private var hover = false

    var body: some View {
        Button(action: open) {
            ZStack(alignment: .topLeading) {
                GenreLayout.tone(name, dark: false)
                LinearGradient(colors: [.clear, .black.opacity(0.35)], startPoint: .topLeading, endPoint: .bottomTrailing)
                Cover(path: artwork, seed: name, side: 52, corner: 6)
                    .shadow(color: .black.opacity(0.4), radius: 5, y: 4)
                    .rotationEffect(.degrees(22))
                    .offset(x: 8, y: 12)
                    .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .bottomTrailing)
                Text(name).font(.needle(.body).weight(.bold)).foregroundStyle(.white).lineLimit(2)
                    .padding(.horizontal, 12).padding(.vertical, 10)
                    .padding(.trailing, 36)
            }
            .frame(height: 72)
            .clipShape(RoundedRectangle(cornerRadius: 12, style: .continuous))
            .overlay(RoundedRectangle(cornerRadius: 12, style: .continuous).strokeBorder(.white.opacity(hover ? 0.4 : 0.18), lineWidth: 1))
            .contentShape(RoundedRectangle(cornerRadius: 12, style: .continuous))
        }
        .buttonStyle(.plain)
        .onHover { hover = $0 }
        .help("\(name): \(count(songs, "song"))")
        .accessibilityLabel("\(name), \(count(songs, "song"))")
    }

    /// The genre's songs: the Songs page with the genre's rule in the search field.
    private func open() {
        let rule = "genre = \(Rule.quote(name))"
        store.navigate(.songs)
        store.search = rule
    }
}
