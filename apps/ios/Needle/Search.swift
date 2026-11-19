import SwiftUI

/// Search, with its field where the system puts it (the tab bar on an iPhone). Before typing:
/// Recent, Rules (one line), and Browse (the genres with the most songs). While typing: chips for All, Songs, Albums, Artists,
/// and Lyrics with their counts, pinned at the top; under All, the top result on a card in its
/// cover's colour, then Songs, Albums, Artists, the lines of lyrics that hold the words, and
/// songs from your server. The letters that match are lit.
struct SearchView: View {
    @Environment(Store.self) private var store
    @Environment(WindowState.self) private var window
    @Environment(\.wide) private var wide
    @Environment(\.zoomScope) private var scope
    @State private var query = ""
    @State private var kind = Kind.all
    @State private var found: SearchFound?
    /// Every song that matches, for the Songs chip.
    @State private var songs: [Song] = []
    @State private var error: String?
    @State private var genres: [Genre] = []
    @State private var hints: [Suggestion] = []
    @State private var server: [Song] = []
    @State private var generation: UInt64 = 0
    /// The rules to try, read once (name, rule, name, rule…).
    @State private var examples: [String] = []
    /// What was looked for before, the newest first.
    @State private var recent: [RecentSearch] = []
    /// Read again after a change to the list (remembered, dropped, cleared).
    @State private var recentStamp = 0
    /// The words searched for are a rule: no letters are lit.
    @State private var rule = false
    /// The recent tile under the pointer (an iPad's): its × shows.
    @State private var hoveredRecent: String?
    /// The lit glass glides from the chip chosen before to the one chosen now.
    @Namespace private var chipGlass
    /// The page's width: Top Result and the songs beside it share it.
    @State private var pageWidth: CGFloat = 0

    enum Kind: String, CaseIterable { case all = "All", songs = "Songs", albums = "Albums", artists = "Artists", lyrics = "Lyrics" }

    private var words: String { query.trimmingCharacters(in: .whitespaces) }
    /// The words to light in names: none for a rule.
    private var match: String? { rule ? nil : words }
    /// The row the page's parts share, inside its margins.
    private var row: CGFloat { pageWidth - 2 * Tiles.margin(wide) }
    /// A wide window with room for two columns.
    private var twoColumns: Bool { wide && row >= 640 }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 0) {
                if words.isEmpty {
                    recentPart
                    rules
                    browse
                } else if let error {
                    Text(error).font(.needle(.subheadline)).foregroundStyle(.red).padding()
                    rules
                } else if let found {
                    if nothing(found) {
                        nothingFound
                        rules
                        recentPart
                    } else {
                        results(found)
                    }
                }
            }
            .padding(.bottom, 24)
        }
        .onGeometryChange(for: CGFloat.self) { $0.size.width } action: { pageWidth = $0 }
        // The typing hints and the chips stay at the top while the results scroll.
        .safeAreaBar(edge: .top) {
            VStack(alignment: .leading, spacing: 0) {
                if !hints.isEmpty { suggestions }
                if !words.isEmpty { chips.padding(.top, 8) }
            }
            .padding(.bottom, hints.isEmpty && words.isEmpty ? 0 : 6)
        }
        .navigationTitle("Search")
        .themed()
        .searchable(text: $query, prompt: "Songs, albums, artists, lyrics")
        // The keyboard's Search key: the words are kept in Recent (never on each letter).
        .onSubmit(of: .search) { if !words.isEmpty { remember("words", words) } }
        #if DEBUG
        // Test builds: -search <words> searches for them.
        .onAppear {
            if query.isEmpty, let words = UserDefaults.standard.string(forKey: "search") { query = words }
            // -search-kind lyrics|songs|albums|artists: that chip.
            if let chip = UserDefaults.standard.string(forKey: "search-kind"), let chosen = Kind(rawValue: chip.capitalized) { kind = chosen }
            // -seed-recent: a few recent searches, for pictures of the page before typing.
            if CommandLine.arguments.contains("-seed-recent") { seedRecent() }
        }
        #endif
        .task(id: store.libraryVersion) {
            genres = await store.load { try $0.genres() } ?? []
            if examples.isEmpty { examples = await store.load { $0.ruleExamples() } ?? [] }
        }
        .task(id: "\(recentStamp)|\(words.isEmpty)|\(store.libraryVersion)") {
            let next = await store.load { try $0.recentSearches(limit: 8) } ?? recent
            withFade(.easeOut(duration: 0.18)) { recent = next }
        }
        .task(id: "\(query)|\(kind)|\(store.ratingVersion)|\(store.libraryVersion)") {
            let typed = query
            // No hints for an empty field: the Rules chips are on the page then (as on Android).
            hints = typed.trimmingCharacters(in: .whitespaces).isEmpty ? []
                : await store.load { $0.suggest(input: typed, cursor: UInt32(typed.utf16.count)) } ?? []
            let words = self.words
            guard !words.isEmpty else { found = nil; songs = []; server = []; error = nil; return }
            try? await Task.sleep(for: .milliseconds(200))
            guard !Task.isCancelled else { return }
            let limit: UInt32 = kind == .lyrics ? 50 : kind == .all || kind == .songs ? 12 : 60
            // One search for each query, with the counts for the chips; the Songs chip also
            // asks for every song that matches. A rule that does not read gives its error here,
            // as the Songs page shows it. The old results stay until the new ones are here.
            let (core, chip) = (store.core, kind)
            let outcome = await Task.detached { () -> Result<(SearchFound, [Song], Bool), Error> in
                Result {
                    let next = try core.find(query: words, limit: limit)
                    return (next, chip == .songs ? try core.search(query: words) : [], core.isRule(input: words))
                }
            }.value
            guard !Task.isCancelled else { return }
            withFade(.easeOut(duration: 0.18)) {
                switch outcome {
                case .success(let (next, every, isRule)):
                    found = next
                    songs = every
                    rule = isRule
                    error = nil
                case .failure(let problem):
                    found = nil
                    songs = []
                    error = Store.words(problem)
                }
            }
            guard kind == .all else { server = []; return }
            // Music servers that search as you type (an experimental switch on the server).
            let asked = await store.load { $0.searchServers(query: words) } ?? 0
            generation = asked
            server = []
            for _ in 0..<12 where asked > 0 {
                try? await Task.sleep(for: .milliseconds(500))
                guard !Task.isCancelled else { return }
                let heard = await store.load { $0.serverResults(generation: asked) } ?? []
                if !heard.isEmpty { withFade(.easeOut(duration: 0.18)) { server = heard }; break }
            }
        }
    }

    private func nothing(_ found: SearchFound) -> Bool {
        switch kind {
        case .all: found.best == nil && found.songs.isEmpty && found.albums.isEmpty && found.artists.isEmpty && found.lyrics.isEmpty && server.isEmpty
        case .songs: songs.isEmpty && found.songs.isEmpty
        case .albums: found.albums.isEmpty
        case .artists: found.artists.isEmpty
        case .lyrics: found.lyrics.isEmpty
        }
    }

    /// How many a chip finds in all; nil before the first search is back.
    private func total(_ chip: Kind) -> Int? {
        guard let found else { return nil }
        return switch chip {
        case .all: nil
        case .songs: Int(found.songCount)
        case .albums: Int(found.albumCount)
        case .artists: Int(found.artistCount)
        case .lyrics: Int(found.lyricCount)
        }
    }

    // MARK: Before typing

    /// Recent: rows on a phone, one line of up to 4 tiles in a wide window.
    @ViewBuilder
    private var recentPart: some View {
        if !recent.isEmpty {
            header("Recent") {
                Button("Clear") { clearRecent() }
                    .font(.needle(.subheadline)).foregroundStyle(.secondary)
                    .buttonStyle(.plain)
                    .accessibilityLabel("Clear recent searches")
            }
            if wide {
                HStack(spacing: 12) {
                    ForEach(recent.prefix(4), id: \.id) { recentTile($0) }
                    // Fewer than 4: the tiles keep the size they have with 4.
                    ForEach(0..<max(0, 4 - recent.count), id: \.self) { _ in Color.clear.frame(height: 1).frame(maxWidth: .infinity) }
                }
                .padding(.horizontal, Tiles.margin(wide))
            } else {
                LazyVStack(spacing: 2) {
                    ForEach(recent, id: \.id) { recentRow($0) }
                }
                .padding(.horizontal, Tiles.margin(wide))
            }
        }
    }

    /// A recent search's name, its second line (long on a phone, short on a tile), and picture.
    private func describe(_ item: RecentSearch, short: Bool) -> (name: String, detail: String) {
        switch item.kind {
        case "song": (item.song?.title ?? item.key, short ? "Song" : "Song · \(item.song?.artist ?? "")")
        case "album": (item.album?.shownTitle ?? item.key, short ? "Album" : "Album · \(item.album?.shownArtist ?? "")")
        case "artist": (item.artist?.name ?? item.key, "Artist")
        default: (item.key, short ? "Search" : "You searched for this")
        }
    }

    @ViewBuilder
    private func recentPicture(_ item: RecentSearch, side: CGFloat) -> some View {
        switch item.kind {
        case "song": Artwork(path: item.song?.artwork, seed: (item.song?.album ?? "") + (item.song?.artist ?? ""), size: 120)
                .frame(width: side, height: side).clipShape(.soft(Corner.smallCover))
        case "album": Artwork(path: item.album?.artwork, seed: (item.album?.title ?? "") + (item.album?.artist ?? ""), size: 120)
                .frame(width: side, height: side).clipShape(.soft(Corner.smallCover))
        case "artist": Artwork(path: item.artist?.artwork, seed: item.key, size: 120)
                .frame(width: side, height: side).clipShape(.circle)
        default:
            Image(systemName: "clock.arrow.circlepath")
                .font(.system(size: side * 0.45)).foregroundStyle(.secondary)
                .frame(width: side, height: side)
                .background(store.colors.card, in: .soft(Corner.smallCover))
        }
    }

    private func recentRow(_ item: RecentSearch) -> some View {
        let (name, detail) = describe(item, short: false)
        return HStack(spacing: 12) {
            Button { openRecent(item) } label: {
                HStack(spacing: 12) {
                    recentPicture(item, side: 44)
                    VStack(alignment: .leading, spacing: 2) {
                        Text(name).lineLimit(1)
                        Text(detail).font(.needle(.subheadline)).foregroundStyle(.secondary).lineLimit(1)
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                }
                .padding(.vertical, 4)
                .contentShape(.rect)
            }
            .buttonStyle(Pressable())
            .accessibilityLabel("\(name), \(detail)")
            .accessibilityAction(named: "Remove from Recent") { forget(item) }
            Button { forget(item) } label: {
                Image(systemName: "xmark").font(.needle(.subheadline)).foregroundStyle(.secondary)
                    .frame(width: 44, height: 44).contentShape(.rect)
            }
            .buttonStyle(.plain)
            .accessibilityLabel("Remove \(name) from Recent")
        }
    }

    private func recentTile(_ item: RecentSearch) -> some View {
        let (name, detail) = describe(item, short: true)
        let hovered = hoveredRecent == item.id
        return Button { openRecent(item) } label: {
            HStack(spacing: 10) {
                recentPicture(item, side: 40)
                VStack(alignment: .leading, spacing: 1) {
                    Text(name).font(.needle(.subheadline).weight(.medium)).lineLimit(1)
                    Text(detail).font(.needle(.footnote)).foregroundStyle(.secondary).lineLimit(1)
                }
                .frame(maxWidth: .infinity, alignment: .leading)
            }
            .padding(8)
            .background(store.colors.card, in: .soft(Corner.middle))
            .contentShape(.soft(Corner.middle))
        }
        .buttonStyle(Pressable())
        .accessibilityLabel("\(name), \(detail)")
        .accessibilityAction(named: "Remove from Recent") { forget(item) }
        .frame(maxWidth: .infinity)
        // The × under the pointer; a long press gives Remove too.
        .overlay(alignment: .topTrailing) {
            if hovered {
                Button { forget(item) } label: {
                    Image(systemName: "xmark").font(.system(size: 10, weight: .bold))
                        .frame(width: 22, height: 22)
                        .background(.regularMaterial, in: .circle)
                }
                .buttonStyle(.plain)
                .offset(x: 6, y: -6)
                .transition(.opacity)
                .accessibilityLabel("Remove \(name) from Recent")
            }
        }
        .onHover { over in withFade(.easeOut(duration: 0.15)) { hoveredRecent = over ? item.id : (hoveredRecent == item.id ? nil : hoveredRecent) } }
        .hoverEffect(.lift)
        .contextMenu { Button("Remove from Recent", systemImage: "xmark") { forget(item) } }
    }

    // MARK: Parts

    /// A section's title with something at the end of its line ("Clear", "All 24 ›").
    private func header<Trailing: View>(_ title: String, inset: Bool = true, @ViewBuilder trailing: () -> Trailing) -> some View {
        HStack(alignment: .firstTextBaseline) {
            Text(title).font(.needleTitle(size: 22)).accessibilityAddTraits(.isHeader)
            Spacer(minLength: 8)
            trailing()
        }
        .padding(.horizontal, inset ? Tiles.margin(wide) : 0)
        .padding(.top, inset ? (wide ? 28 : 22) : 0)
        .padding(.bottom, 2)
    }

    /// "All 24 ›": chooses that chip.
    private func allOf(_ n: Int, _ chip: Kind, _ what: String) -> some View {
        Button { kind = chip } label: {
            Text("All \(n.formatted()) ›").font(.needle(.subheadline)).foregroundStyle(.secondary)
        }
        .buttonStyle(.plain)
        .accessibilityLabel("All \(count(n, what))")
    }

    private var suggestions: some View {
        ScrollView(.horizontal) {
            GlassEffectContainer(spacing: 4) {
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
            }
            .padding(.horizontal, Tiles.margin(wide))
        }
        .scrollIndicators(.hidden)
        .padding(.top, 8)
    }

    private var chips: some View {
        ScrollView(.horizontal) {
            // Glass chips, the chosen one in the accent; the lit glass glides to the chip chosen.
            // The container's spacing is under the gap, so the chips stay apart.
            GlassEffectContainer(spacing: 4) {
                HStack(spacing: 8) {
                    ForEach(Kind.allCases, id: \.self) { chip in
                        let on = kind == chip
                        let n = total(chip)
                        // None found: dimmed and not pressable, but still there (nothing jumps).
                        let empty = n == 0 && !on
                        // The results change at once; only the lit glass glides (below).
                        Button { kind = chip } label: {
                            HStack(spacing: 5) {
                                Text(chip.rawValue).font(.needle(.subheadline).weight(.semibold))
                                if let n {
                                    Text(n.formatted()).font(.needle(.caption).monospacedDigit()).opacity(0.62)
                                        .contentTransition(.numericText())
                                }
                            }
                            .padding(.horizontal, 14).padding(.vertical, 7)
                            .foregroundStyle(on ? store.colors.onAccent : .primary)
                            .glassEffect(on ? .regular.tint(store.colors.accent).interactive() : .regular.interactive(), in: .capsule)
                            .glassEffectID(on ? "chosen" : chip.rawValue, in: chipGlass)
                            .contentShape(.capsule)
                        }
                        .buttonStyle(.plain)
                        .disabled(empty)
                        .opacity(empty ? 0.4 : 1)
                        .accessibilityLabel(n.map { "\(chip.rawValue), \($0)" } ?? chip.rawValue)
                        .accessibilityAddTraits(on ? .isSelected : [])
                    }
                }
            }
            .motion(.smooth(duration: 0.22), value: kind)
            .padding(.horizontal, Tiles.margin(wide))
            // Room for the glass's edge and its spring when pressed.
            .padding(.vertical, 2)
        }
        .scrollIndicators(.hidden)
        // A chip is only ever chosen by a press.
        .sensoryFeedback(.selection, trigger: kind) { _, _ in store.app.haptics }
    }

    @ViewBuilder
    private func results(_ found: SearchFound) -> some View {
        switch kind {
        case .all: everything(found)
        case .songs:
            let list = songs.isEmpty ? found.songs : Array(songs.prefix(200))
            let n = max(Int(found.songCount), list.count)
            HStack {
                Text(count(n, "song")).font(.needle(.subheadline)).foregroundStyle(.secondary)
                Spacer()
                Button { store.playSearch(words, shuffled: true) } label: {
                    Label("Shuffle", systemImage: "shuffle")
                }
                .buttonStyle(CardPill(prominent: true))
                .accessibilityLabel("Shuffle \(count(n, "song"))")
            }
            .padding(.horizontal, Tiles.margin(wide)).padding(.top, 14)
            songRows(list, showAlbum: true)
        case .albums:
            LazyVGrid(columns: [GridItem(.adaptive(minimum: Tiles.least(wide)), spacing: Tiles.gap(wide), alignment: .top)], spacing: wide ? 26 : 18) {
                ForEach(found.albums) { album in
                    AlbumTile(album: album, width: nil, shelf: "search-grid", match: match)
                        .simultaneousGesture(TapGesture().onEnded { remember("album", album.key) })
                }
            }
            .padding(.horizontal, Tiles.margin(wide))
            .padding(.top, 12)
        case .artists:
            LazyVStack(alignment: .leading, spacing: 4) {
                ForEach(found.artists, id: \.name) { artist in
                    NavigationLink(value: Route.artist(artist.name)) {
                        HStack(spacing: 14) {
                            Artwork(path: artist.artwork, seed: artist.name, size: 160).frame(width: 52, height: 52).clipShape(.circle)
                            VStack(alignment: .leading, spacing: 2) {
                                Text(litMatches(artist.name, match, accent: store.colors.accent)).lineLimit(1)
                                Text(count(Int(artist.songs), "song")).font(.needle(.subheadline)).foregroundStyle(.secondary)
                            }
                            Spacer()
                        }
                        .contentShape(.rect)
                    }
                    .buttonStyle(Pressable())
                    .simultaneousGesture(TapGesture().onEnded { remember("artist", artist.name) })
                }
            }
            .padding(.horizontal, Tiles.margin(wide)).padding(.top, 12)
        case .lyrics:
            lyricRows(found.lyrics).padding(.horizontal, Tiles.margin(wide)).padding(.top, 8)
        }
    }

    /// The All chip: the top result, Songs, Albums, Artists, In the lyrics, and On your server.
    @ViewBuilder
    private func everything(_ found: SearchFound) -> some View {
        let five = Array(found.songs.prefix(5))
        let songsTitle = header("Songs", inset: false) {
            if found.songCount > 0 { allOf(Int(found.songCount), .songs, "song") }
        }
        if twoColumns, let best = found.best, !five.isEmpty {
            // A wide window with room: the top result at the left (about 38% of the row), the
            // songs beside it in short rows. Under 640 points, one column.
            HStack(alignment: .top, spacing: 28) {
                topResult(best, tall: true)
                    .frame(width: min(max(row * 0.38, 300), 380))
                VStack(alignment: .leading, spacing: 4) {
                    songsTitle
                    LazyVStack(spacing: 2) {
                        ForEach(Array(five.enumerated()), id: \.element.id) { i, song in
                            SongRow(song: song, stacked: true, match: match) { playRow(five, i) }
                        }
                    }
                }
                .frame(maxWidth: .infinity)
            }
            .padding(.horizontal, Tiles.margin(wide))
            .padding(.top, 18)
        } else {
            if let best = found.best {
                topResult(best, tall: false)
                    .frame(maxWidth: wide ? 520 : .infinity, alignment: .leading)
                    .padding(.horizontal, Tiles.margin(wide)).padding(.top, 14)
            }
            if !five.isEmpty {
                songsTitle.padding(.horizontal, Tiles.margin(wide)).padding(.top, wide ? 28 : 22)
                songRows(five)
            }
        }
        if !found.albums.isEmpty {
            header("Albums") { allOf(Int(found.albumCount), .albums, "album") }
            Shelf {
                ForEach(found.albums) { album in
                    AlbumTile(album: album, shelf: "search-albums", match: match)
                        .simultaneousGesture(TapGesture().onEnded { remember("album", album.key) })
                }
            }
        }
        let artists = found.artists
        let lyrics = Array(found.lyrics.prefix(3))
        if twoColumns, !artists.isEmpty, !lyrics.isEmpty {
            // Artists and In the lyrics share one line.
            HStack(alignment: .top, spacing: 28) {
                VStack(alignment: .leading, spacing: 8) {
                    header("Artists", inset: false) { EmptyView() }
                    ScrollView(.horizontal) {
                        HStack(alignment: .top, spacing: 16) { artistBubbles(artists) }.padding(.vertical, 6)
                    }
                    .scrollIndicators(.hidden)
                    .scrollClipDisabled()
                }
                .frame(width: min(max(row * 0.38, 300), 380))
                VStack(alignment: .leading, spacing: 4) {
                    header("In the lyrics", inset: false) { allOf(Int(found.lyricCount), .lyrics, "song") }
                    lyricRows(lyrics)
                }
                .frame(maxWidth: .infinity)
            }
            .padding(.horizontal, Tiles.margin(wide))
            .padding(.top, 28)
        } else {
            if !artists.isEmpty {
                SectionTitle(title: "Artists")
                Shelf { artistBubbles(artists) }
            }
            if !lyrics.isEmpty {
                header("In the lyrics") { allOf(Int(found.lyricCount), .lyrics, "song") }
                lyricRows(lyrics).padding(.horizontal, Tiles.margin(wide)).padding(.top, 6)
            }
        }
        if !server.isEmpty {
            SectionTitle(title: "On your server")
            LazyVStack(spacing: 10) {
                ForEach(Array(server.prefix(30).enumerated()), id: \.element.id) { i, song in
                    SongRow(song: song, match: match) { let g = generation; store.core.playServerSongs(generation: g, start: UInt32(i)); store.played() }
                }
            }
            .padding(.horizontal, Tiles.margin(wide)).padding(.top, 8)
        }
    }

    private func artistBubbles(_ artists: [Artist]) -> some View {
        ForEach(artists, id: \.name) { artist in
            ArtistBubble(name: artist.name, artwork: artist.artwork, match: match)
                .simultaneousGesture(TapGesture().onEnded { remember("artist", artist.name) })
        }
    }

    /// The top result on a card: its cover blurred behind a veil, so the card takes the cover's
    /// colour, with a thin glass edge. Album and artist: Play and Shuffle; a song: Play and Play
    /// next. A press on the card opens the album or the artist, or plays the song. `tall`: the
    /// cover above the words (a wide window's column); else the cover at the left.
    private func topResult(_ best: SearchBest, tall: Bool) -> some View {
        let card = TopCard(best: best)
        let source = best.album.map { CoverSource.id($0, shelf: "search-top", in: scope) }
        let cover = Artwork(path: card.artwork, seed: card.seed, size: 300)
            .frame(width: tall ? 116 : 96, height: tall ? 116 : 96)
            .clipShape(card.round ? AnyShape(.circle) : AnyShape(.soft(Corner.cover + 2)))
            .modifier(CoverSource(id: source ?? ""))
            .shadow(color: .black.opacity(0.36), radius: 10, y: 8)
            .accessibilityHidden(true)
        let text = VStack(alignment: .leading, spacing: 2) {
            Text("TOP RESULT · \(card.label.uppercased())")
                .font(.needle(.caption2).weight(.bold)).kerning(0.9)
                .foregroundStyle(store.colors.accent)
            Text(litMatches(card.name, match, accent: store.colors.accent))
                .font(.needleTitle(size: tall ? 23 : 20)).lineLimit(2)
                .padding(.top, 2)
            if !card.detail.isEmpty {
                Text(card.detail).font(.needle(.footnote)).opacity(0.78).lineLimit(1)
            }
            HStack(spacing: 8) {
                Button { play(best) } label: { Label("Play", systemImage: "play.fill") }
                    .buttonStyle(CardPill(prominent: true))
                if card.shuffles {
                    Button { play(best, shuffled: true) } label: { Label("Shuffle", systemImage: "shuffle") }
                        .buttonStyle(CardPill(prominent: false))
                } else {
                    Button { playNext(best) } label: {
                        Label("Play next", systemImage: "text.line.first.and.arrowtriangle.forward")
                    }
                    .buttonStyle(CardPill(prominent: false))
                }
            }
            .padding(.top, tall ? 14 : 11)
        }
        return Group {
            if tall {
                VStack(alignment: .leading, spacing: 14) { cover; text }
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(18)
            } else {
                HStack(spacing: 14) { cover; text; Spacer(minLength: 0) }
                    .padding(14)
            }
        }
        .background { TopCardBackdrop(path: card.artwork, seed: card.seed) }
        .clipShape(.soft(Corner.large))
        .overlay { RoundedRectangle(cornerRadius: Corner.large, style: .continuous).strokeBorder(.white.opacity(store.colors.dark ? 0.16 : 0.9), lineWidth: 1) }
        .shadow(color: .black.opacity(store.colors.dark ? 0.34 : 0.18), radius: 13, y: 10)
        .contentShape(.soft(Corner.large))
        .onTapGesture { open(best, from: source) }
        .hoverEffect(.lift)
        // One element for VoiceOver: a press opens it; Play and the other button are actions.
        .accessibilityElement(children: .ignore)
        .accessibilityLabel("Top result, \(card.label), \(card.name)\(card.detail.isEmpty ? "" : ", \(card.detail)")")
        .accessibilityAddTraits(.isButton)
        .accessibilityAction { open(best, from: source) }
        .accessibilityAction(named: "Play") { play(best) }
        .accessibilityAction(named: card.shuffles ? "Shuffle" : "Play next") {
            if card.shuffles { play(best, shuffled: true) } else { playNext(best) }
        }
    }

    private func playNext(_ best: SearchBest) {
        guard let song = best.song else { return }
        store.playNext([song])
        remember("song", song.id)
    }

    private func songRows(_ list: [Song], showAlbum: Bool = false) -> some View {
        LazyVStack(spacing: 10) {
            ForEach(Array(list.enumerated()), id: \.element.id) { i, song in
                SongRow(song: song, match: match, showAlbum: showAlbum) { playRow(list, i) }
            }
        }
        .padding(.horizontal, Tiles.margin(wide)).padding(.top, 8)
    }

    /// Lines of lyrics that hold the words, the words lit; a tap plays the song from the start.
    private func lyricRows(_ hits: [LyricHit]) -> some View {
        LazyVStack(alignment: .leading, spacing: 4) {
            ForEach(Array(hits.enumerated()), id: \.offset) { _, hit in
                Button { store.play([hit.song]); remember("song", hit.song.id) } label: {
                    VStack(alignment: .leading, spacing: 5) {
                        Text(litLine(hit.line)).font(.needle(.title3))
                        HStack(spacing: 7) {
                            Artwork(path: hit.song.artwork, seed: hit.song.album + hit.song.artist, size: 60)
                                .frame(width: 18, height: 18).clipShape(.soft(4))
                            Text("\(hit.song.title) · \(hit.song.artist)").lineLimit(1)
                        }
                        .font(.needle(.subheadline)).foregroundStyle(.secondary)
                    }
                    .padding(.vertical, 8)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .contentShape(.rect)
                }
                .buttonStyle(Pressable())
                .accessibilityLabel("\(hit.line), from \(hit.song.title) by \(hit.song.artist)")
            }
        }
    }

    /// A line of lyrics in quotes, with each place it holds the words lit.
    private func litLine(_ line: String) -> AttributedString {
        AttributedString("“") + litMatches(line, words, accent: store.colors.accent) + AttributedString("”")
    }

    /// Nothing matches: a round sign, the words, and what to try.
    private var nothingFound: some View {
        VStack(spacing: 4) {
            Image(systemName: "magnifyingglass")
                .font(.system(size: 26, weight: .medium)).foregroundStyle(.secondary)
                .frame(width: 64, height: 64)
                .background(store.colors.card, in: .circle)
                .padding(.bottom, 10)
                .accessibilityHidden(true)
            Text("Nothing for “\(words)”").font(.needle(.headline)).multilineTextAlignment(.center)
            Text("Look at the spelling. Or search with a rule.")
                .font(.needle(.subheadline)).foregroundStyle(.secondary).multilineTextAlignment(.center)
        }
        .frame(maxWidth: .infinity)
        .padding(.horizontal, 30)
        .padding(.top, 40)
        .accessibilityElement(children: .combine)
    }

    /// Browse: only the genres with the most songs, 6 on a phone (three lines of two), two lines
    /// (8 at most) in a wide window; "All N ›" opens the Genres page when there are more.
    @ViewBuilder
    private var browse: some View {
        if !genres.isEmpty {
            // In a wide window as many across as the Genres page fits (tiles of 200 or more).
            let across = wide ? min(4, max(2, Int((row + 16) / 216))) : 2
            let shown = Array(genres.sorted { $0.songs > $1.songs }.prefix(wide ? 2 * across : 6))
            header("Browse") {
                if genres.count > shown.count {
                    NavigationLink(value: Route.genres) {
                        Text("All \(genres.count.formatted()) ›").font(.needle(.subheadline)).foregroundStyle(.secondary)
                    }
                    .buttonStyle(.plain)
                    .accessibilityLabel("All \(count(genres.count, "genre"))")
                }
            }
            GenreTiles(genres: shown, across: across).padding(.top, 10)
        }
    }

    /// Rules: a tap puts the rule in the search field. A wide window also shows the rule.
    private var rules: some View {
        let pairs = stride(from: 0, to: examples.count - 1, by: 2).map { (examples[$0], examples[$0 + 1]) }
        return VStack(alignment: .leading, spacing: 0) {
            if !pairs.isEmpty {
                SectionTitle(title: "Rules")
                ScrollView(.horizontal) {
                    GlassEffectContainer(spacing: 4) {
                        HStack(spacing: 8) {
                            ForEach(pairs, id: \.1) { name, rule in
                                Button { query = rule; kind = .all } label: {
                                    HStack(spacing: 8) {
                                        Text(name).font(.needle(.subheadline).weight(.medium))
                                        if wide {
                                            Text(rule).font(.system(.caption, design: .monospaced)).foregroundStyle(.secondary)
                                        }
                                    }
                                }
                                .buttonStyle(.glass)
                                .accessibilityHint("Searches for \(rule)")
                            }
                        }
                    }
                    .padding(.horizontal, Tiles.margin(wide))
                }
                .scrollIndicators(.hidden)
                .padding(.top, 10)
            }
        }
    }

    // MARK: Actions

    private func playRow(_ list: [Song], _ i: Int) {
        store.play(list, from: i)
        if list.indices.contains(i) { remember("song", list[i].id) }
    }

    private func open(_ best: SearchBest, from source: String?) {
        switch best.kind {
        case "album":
            if let album = best.album {
                remember("album", album.key)
                window.opening = source.map { .albumFrom(album, $0) } ?? .album(album)
            }
        case "artist":
            if let artist = best.artist { remember("artist", artist.name); window.opening = .artist(artist.name) }
        default:
            if let song = best.song { store.play([song]); remember("song", song.id) }
        }
    }

    private func play(_ best: SearchBest, shuffled: Bool = false) {
        switch best.kind {
        case "album": if let album = best.album { store.playAlbum(album, shuffled: shuffled); remember("album", album.key) }
        case "artist": if let artist = best.artist { store.playArtist(artist.name, shuffled: shuffled); remember("artist", artist.name) }
        default: if let song = best.song { store.play([song]); remember("song", song.id) }
        }
    }

    private func openRecent(_ item: RecentSearch) {
        switch item.kind {
        case "song": if let song = item.song { store.play([song]) }
        case "album": if let album = item.album { window.opening = .album(album) }
        case "artist": window.opening = .artist(item.key)
        default: query = item.key; kind = .all
        }
        // It comes to the top of Recent.
        if item.kind != "words" { remember(item.kind, item.key) }
    }

    /// Keeps a search or a result opened from Search in Recent.
    private func remember(_ kind: String, _ key: String) {
        Task {
            _ = await store.ask { try $0.rememberSearch(kind: kind, key: key) }
            recentStamp += 1
        }
    }

    /// The ×: drops one from Recent.
    private func forget(_ item: RecentSearch) {
        let (kind, key) = (item.kind, item.key)
        withMotion(.smooth(duration: 0.22)) { recent.removeAll { $0.id == item.id } }
        Task {
            _ = await store.ask { try $0.forgetSearch(kind: kind, key: key) }
            recentStamp += 1
        }
    }

    /// Clear: every recent search goes, with no question first.
    private func clearRecent() {
        withFade(.easeOut(duration: 0.18)) { recent = [] }
        Task {
            _ = await store.ask { try $0.clearSearches() }
            recentStamp += 1
        }
    }

    #if DEBUG
    /// Test builds (-seed-recent): words, an album, an artist, and a song from the library, as
    /// recent searches.
    private func seedRecent() {
        Task {
            guard let found = await store.load({ try $0.find(query: "a", limit: 4) }) else { return }
            var items: [(String, String)] = [("words", "late night")]
            if let song = found.songs.first { items.append(("song", song.id)) }
            if let album = found.albums.first { items.append(("album", album.key)) }
            if let artist = found.artists.first { items.append(("artist", artist.name)) }
            for (kind, key) in items { _ = await store.ask { try $0.rememberSearch(kind: kind, key: key) } }
            recentStamp += 1
        }
    }
    #endif
}

extension RecentSearch {
    var id: String { "\(kind)|\(key)" }
}

/// What the top result's card shows.
private struct TopCard {
    let name: String
    let detail: String
    let label: String
    let artwork: String?
    let seed: String
    let round: Bool
    /// Album and artist: Play and Shuffle. A song: Play and Play next.
    let shuffles: Bool

    init(best: SearchBest) {
        shuffles = best.kind == "album" || best.kind == "artist"
        switch best.kind {
        case "album":
            let album = best.album
            name = album?.shownTitle ?? ""
            detail = [album?.shownArtist, (album?.year ?? 0) > 0 ? "\(album?.year ?? 0)" : nil, album.map { count(Int($0.songs), "song") }]
                .compactMap { $0 }.filter { !$0.isEmpty }.joined(separator: " · ")
            label = "Album"
            artwork = album?.artwork
            seed = (album?.title ?? "") + (album?.artist ?? "")
            round = false
        case "artist":
            let artist = best.artist
            name = artist?.name ?? ""
            detail = artist.map { "\(count(Int($0.songs), "song")) · \(count(Int($0.albums), "album"))" } ?? ""
            label = "Artist"
            artwork = artist?.artwork
            seed = name
            round = true
        default:
            let song = best.song
            name = song?.title ?? ""
            detail = [song?.artist, song?.album].compactMap { $0 }.filter { !$0.trimmingCharacters(in: .whitespaces).isEmpty }.joined(separator: " · ")
            label = "Song"
            artwork = song?.artwork
            seed = (song?.album ?? "") + (song?.artist ?? "")
            round = false
        }
    }
}

/// Behind the top result: its cover, large and blurred, under a veil in the page's colour (dark
/// in the dark looks, light in Day). Reduce Transparency: the plain card colour.
private struct TopCardBackdrop: View {
    @Environment(Store.self) private var store
    @Environment(\.accessibilityReduceTransparency) private var reduceTransparency
    let path: String?
    let seed: String

    var body: some View {
        let colors = store.colors
        ZStack {
            colors.card
            if !reduceTransparency {
                Artwork(path: path, seed: seed, size: 200)
                    .scaleEffect(1.6)
                    .blur(radius: 26, opaque: true)
                    .saturation(1.5)
                    .opacity(0.8)
                LinearGradient(colors: [colors.page.opacity(colors.dark ? 0.38 : 0.46), colors.page.opacity(0.8)], startPoint: .top, endPoint: .bottom)
            }
        }
        .allowsHitTesting(false)
        .accessibilityHidden(true)
    }
}

/// A small round button of the top result and the Songs chip: the accent (Play) or a soft
/// fill (Shuffle, Play next).
private struct CardPill: ButtonStyle {
    @Environment(Store.self) private var store
    let prominent: Bool

    func makeBody(configuration: Configuration) -> some View {
        let colors = store.colors
        configuration.label
            .labelStyle(CardPillLabel())
            .font(.needle(.subheadline).weight(.semibold))
            .lineLimit(1)
            .padding(.leading, 11).padding(.trailing, 14)
            .frame(minHeight: 34)
            .foregroundStyle(prominent ? colors.onAccent : colors.text)
            .background(prominent ? AnyShapeStyle(colors.accent) : AnyShapeStyle(colors.text.opacity(0.1)), in: .capsule)
            .overlay { if !prominent { Capsule().strokeBorder(colors.text.opacity(0.1), lineWidth: 1) } }
            .contentShape(.capsule)
            .scaleEffect(configuration.isPressed && !Motion.reduced ? 0.96 : 1)
            .opacity(configuration.isPressed && Motion.reduced ? 0.7 : 1)
            .animation(Pressable.motion(pressed: configuration.isPressed), value: configuration.isPressed)
    }
}

private struct CardPillLabel: LabelStyle {
    func makeBody(configuration: Configuration) -> some View {
        HStack(spacing: 4) { configuration.icon.imageScale(.small); configuration.title }
    }
}
