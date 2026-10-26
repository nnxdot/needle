import SwiftUI

/// Search, with its field on the tab bar. Chips for All, Songs, Albums, Artists, and Lyrics;
/// under All, the best match large, then Songs, Albums, Artists, the lines of lyrics that hold
/// the words, songs from your server, and rules to try. Genres to browse while it is empty.
struct SearchView: View {
    @Environment(Store.self) private var store
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

    enum Kind: String, CaseIterable { case all = "All", songs = "Songs", albums = "Albums", artists = "Artists", lyrics = "Lyrics" }

    var body: some View {
        let words = query.trimmingCharacters(in: .whitespaces)
        ScrollView {
            VStack(alignment: .leading, spacing: 0) {
                if !hints.isEmpty { suggestions }
                if words.isEmpty {
                    rules
                    if !genres.isEmpty {
                        SectionTitle(title: "Browse").padding(.bottom, 10)
                        GenreTiles(genres: genres)
                    }
                } else {
                    chips.padding(.top, 10)
                    if let error {
                        Text(error).font(.needle(.subheadline)).foregroundStyle(.red).padding()
                        rules
                    } else if let found {
                        if nothing(found) {
                            ContentUnavailableView.search(text: query).padding(.top, 40)
                            rules
                        } else {
                            results(found, words: words)
                        }
                    }
                }
            }
            .padding(.bottom, 24)
        }
        .navigationTitle("Search")
        .themed()
        .searchable(text: $query, prompt: "Songs, albums, artists, lyrics")
        #if DEBUG
        // Test builds: -search <words> searches for them.
        .onAppear { if query.isEmpty, let words = UserDefaults.standard.string(forKey: "search") { query = words } }
        #endif
        .task(id: store.libraryVersion) { genres = await store.load { try $0.genres() } ?? [] }
        .task(id: "\(query)|\(kind)|\(store.ratingVersion)|\(store.libraryVersion)") {
            let typed = query
            hints = await store.load { $0.suggest(input: typed, cursor: UInt32(typed.utf16.count)) } ?? []
            guard !words.isEmpty else { found = nil; songs = []; server = []; error = nil; return }
            try? await Task.sleep(for: .milliseconds(200))
            guard !Task.isCancelled else { return }
            let limit: UInt32 = kind == .lyrics ? 50 : kind == .all ? 12 : 60
            let next = await store.load { try $0.find(query: words, limit: limit) }
            // A rule that does not read: its error, as the Songs page shows it.
            let problem: String? = await store.load { core -> String? in
                do { _ = try core.search(query: words); return nil } catch { return Store.words(error) }
            } ?? nil
            let every = kind == .songs ? (await store.load { try $0.search(query: words) } ?? []) : []
            guard !Task.isCancelled else { return }
            found = next
            songs = every
            error = (next.map(nothing) ?? true) ? problem : nil
            guard kind == .all else { server = []; return }
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

    private func nothing(_ found: SearchFound) -> Bool {
        switch kind {
        case .all: found.best == nil && found.songs.isEmpty && found.albums.isEmpty && found.artists.isEmpty && found.lyrics.isEmpty && server.isEmpty
        case .songs: songs.isEmpty && found.songs.isEmpty
        case .albums: found.albums.isEmpty
        case .artists: found.artists.isEmpty
        case .lyrics: found.lyrics.isEmpty
        }
    }

    // MARK: Parts

    private var suggestions: some View {
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

    private var chips: some View {
        ScrollView(.horizontal) {
            HStack(spacing: 8) {
                ForEach(Kind.allCases, id: \.self) { chip in
                    let on = kind == chip
                    Button { kind = chip } label: {
                        Text(chip.rawValue)
                            .font(.needle(.subheadline).weight(.semibold))
                            .padding(.horizontal, 14).padding(.vertical, 7)
                            .foregroundStyle(on ? store.colors.onAccent : .primary)
                            .background(on ? AnyShapeStyle(store.colors.accent) : AnyShapeStyle(.white.opacity(0.1)), in: .capsule)
                    }
                    .buttonStyle(.plain)
                    .accessibilityAddTraits(on ? .isSelected : [])
                }
            }
            .padding(.horizontal)
        }
        .scrollIndicators(.hidden)
        .sensoryFeedback(.selection, trigger: kind) { _, _ in store.app.haptics }
    }

    @ViewBuilder
    private func results(_ found: SearchFound, words: String) -> some View {
        switch kind {
        case .all:
            if let best = found.best { bestMatch(best).padding(.horizontal).padding(.top, 14) }
            if !found.songs.isEmpty {
                SectionTitle(title: "Songs")
                songRows(Array(found.songs.prefix(5)))
            }
            albumShelf(found.albums)
            artistShelf(found.artists)
            if !found.lyrics.isEmpty {
                SectionTitle(title: "In the lyrics")
                lyricRows(Array(found.lyrics.prefix(3)), words: words)
            }
            if !server.isEmpty {
                SectionTitle(title: "On your server")
                LazyVStack(spacing: 10) {
                    ForEach(Array(server.prefix(30).enumerated()), id: \.element.id) { i, song in
                        SongRow(song: song) { let g = generation; store.core.playServerSongs(generation: g, start: UInt32(i)); store.played() }
                    }
                }
                .padding(.horizontal).padding(.top, 8)
            }
            rules
        case .songs: songRows(songs.isEmpty ? found.songs : Array(songs.prefix(200)))
        case .albums:
            AlbumGrid(albums: found.albums).padding(.top, 12)
        case .artists:
            LazyVStack(alignment: .leading, spacing: 4) {
                ForEach(found.artists, id: \.name) { artist in
                    NavigationLink(value: Route.artist(artist.name)) {
                        HStack(spacing: 14) {
                            Artwork(path: artist.artwork, seed: artist.name, size: 160).frame(width: 52, height: 52).clipShape(.circle)
                            VStack(alignment: .leading, spacing: 2) {
                                Text(artist.name).lineLimit(1)
                                Text(count(Int(artist.songs), "song")).font(.needle(.subheadline)).foregroundStyle(.secondary)
                            }
                            Spacer()
                        }
                        .contentShape(.rect)
                    }
                    .buttonStyle(.plain)
                }
            }
            .padding(.horizontal).padding(.top, 12)
        case .lyrics:
            lyricRows(found.lyrics, words: words).padding(.top, 8)
        }
    }

    /// The best match, large: its cover or photo, what it is, its name, and Play.
    private func bestMatch(_ best: SearchBest) -> some View {
        let (name, detail, label, artwork): (String, String, String, String?) = switch best.kind {
        case "album": (best.album?.title ?? "", best.album?.artist ?? "", "Album", best.album?.artwork)
        case "artist": (best.artist?.name ?? "", count(Int(best.artist?.songs ?? 0), "song"), "Artist", best.artist?.artwork)
        default: (best.song?.title ?? "", best.song?.artist ?? "", "Song", best.song?.artwork)
        }
        return HStack(spacing: 16) {
            Artwork(path: artwork, seed: name + detail, size: 300)
                .frame(width: 104, height: 104)
                .clipShape(best.kind == "artist" ? AnyShape(.circle) : AnyShape(.rect(cornerRadius: 10)))
                .shadow(color: .black.opacity(0.3), radius: 8, y: 4)
            VStack(alignment: .leading, spacing: 6) {
                Text(label.uppercased()).font(.needle(.caption2).weight(.bold)).kerning(0.6).foregroundStyle(.secondary)
                Text(name).font(.needleTitle(size: 22)).lineLimit(2)
                Text(detail).font(.needle(.subheadline)).foregroundStyle(.secondary).lineLimit(1)
                Button { play(best) } label: {
                    Label("Play", systemImage: "play.fill").font(.needle(.subheadline).weight(.semibold)).padding(.horizontal, 6)
                }
                .buttonStyle(.glassProminent)
                .foregroundStyle(store.colors.onAccent)
            }
            Spacer(minLength: 0)
        }
        .padding(14)
        .background(.white.opacity(0.06), in: .rect(cornerRadius: 18))
        .contentShape(.rect)
        .onTapGesture { open(best) }
        .accessibilityElement(children: .combine)
        .accessibilityAddTraits(.isButton)
        .accessibilityAction { open(best) }
        .accessibilityAction(named: "Play") { play(best) }
    }

    private func songRows(_ list: [Song]) -> some View {
        LazyVStack(spacing: 10) {
            ForEach(Array(list.enumerated()), id: \.element.id) { i, song in
                SongRow(song: song) { store.play(list, from: i) }
            }
        }
        .padding(.horizontal).padding(.top, 8)
    }

    @ViewBuilder
    private func albumShelf(_ albums: [Album]) -> some View {
        if !albums.isEmpty {
            SectionTitle(title: "Albums")
            Shelf { ForEach(albums) { AlbumTile(album: $0) } }
        }
    }

    @ViewBuilder
    private func artistShelf(_ artists: [Artist]) -> some View {
        if !artists.isEmpty {
            SectionTitle(title: "Artists")
            Shelf { ForEach(artists, id: \.name) { ArtistBubble(name: $0.name, artwork: $0.artwork) } }
        }
    }

    /// Lines of lyrics that hold the words, the words lit; a tap plays the song from the start.
    private func lyricRows(_ hits: [LyricHit], words: String) -> some View {
        LazyVStack(alignment: .leading, spacing: 4) {
            ForEach(Array(hits.enumerated()), id: \.offset) { _, hit in
                Button { store.play([hit.song]) } label: {
                    VStack(alignment: .leading, spacing: 3) {
                        Text(lit(hit.line, words)).font(.needle(.title3))
                        Text("\(hit.song.title) · \(hit.song.artist)").font(.needle(.subheadline)).foregroundStyle(.secondary)
                    }
                    .padding(.vertical, 8)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .contentShape(.rect)
                }
                .buttonStyle(Pressable())
                .accessibilityLabel("\(hit.line), from \(hit.song.title) by \(hit.song.artist)")
            }
        }
        .padding(.horizontal).padding(.top, 6)
    }

    /// The line, with each place it holds the words lit in the accent.
    private func lit(_ line: String, _ words: String) -> AttributedString {
        var text = AttributedString(line)
        guard !words.isEmpty else { return text }
        var from = line.startIndex
        while let range = line.range(of: words, options: .caseInsensitive, range: from..<line.endIndex) {
            if let start = AttributedString.Index(range.lowerBound, within: text),
               let end = AttributedString.Index(range.upperBound, within: text) {
                text[start..<end].foregroundColor = store.colors.accent
                text[start..<end].font = .needle(.title3).weight(.semibold)
            }
            from = range.upperBound
        }
        return text
    }

    /// Try a rule: a tap puts it in the search field.
    private var rules: some View {
        let examples = store.core.ruleExamples()
        let pairs = stride(from: 0, to: examples.count - 1, by: 2).map { (examples[$0], examples[$0 + 1]) }
        return VStack(alignment: .leading, spacing: 0) {
            SectionTitle(title: "Try a rule")
            ScrollView(.horizontal) {
                HStack(spacing: 8) {
                    ForEach(pairs, id: \.1) { name, rule in
                        Button { query = rule; kind = .all } label: {
                            Text(name).font(.needle(.subheadline).weight(.medium))
                        }
                        .buttonStyle(.glass)
                        .accessibilityHint("Searches for \(rule)")
                    }
                }
                .padding(.horizontal)
            }
            .scrollIndicators(.hidden)
            .padding(.top, 10)
        }
    }

    // MARK: Actions

    private func open(_ best: SearchBest) {
        switch best.kind {
        case "album": if let album = best.album { store.opening = .album(album) }
        case "artist": if let artist = best.artist { store.opening = .artist(artist.name) }
        default: if let song = best.song { store.play([song]) }
        }
    }

    private func play(_ best: SearchBest) {
        switch best.kind {
        case "album": if let album = best.album { store.playAlbum(album) }
        case "artist": if let artist = best.artist { store.playArtist(artist.name) }
        default: if let song = best.song { store.play([song]) }
        }
    }
}
