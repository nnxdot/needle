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
    var best: Best?
    var songs: [Track]
    var albums: [AlbumSummary]
    var artists: [ArtistSummary]
    var lyrics: [Line]

    var isEmpty: Bool { best == nil && songs.isEmpty && albums.isEmpty && artists.isEmpty && lyrics.isEmpty }
}

/// The Search page: what typing in search finds on a page that is not a list. Chips for All,
/// Songs, Albums, Artists, and Lyrics; under All, the best match large, then Songs, Albums,
/// Artists, the lines of lyrics that hold the words, and rules to try.
struct SearchPage: View {
    private var store: Store { .shared }
    @State private var found: FoundData?
    @State private var lyricsOnly = false
    @State private var error: String?

    var body: some View {
        let query = store.search.trimmingCharacters(in: .whitespaces)
        ScrollView {
            VStack(alignment: .leading, spacing: 26) {
                VStack(alignment: .leading, spacing: 14) {
                    Text(query.isEmpty ? "Search" : "“\(query)”").font(store.display(28)).lineLimit(1)
                    chips(query)
                }
                if query.isEmpty {
                    Text("Type in the search field: songs, albums, artists, and the words of lyrics.")
                        .foregroundStyle(store.colors.ink2)
                    rules
                } else if let error {
                    Text(error).foregroundStyle(store.colors.danger)
                    rules
                } else if let found {
                    if found.isEmpty || (lyricsOnly && found.lyrics.isEmpty) {
                        Text("Nothing matches").font(.needle(.title3)).foregroundStyle(store.colors.ink2)
                        rules
                    } else if lyricsOnly {
                        lyricLines(found.lyrics, query: query)
                    } else {
                        all(found, query: query)
                    }
                }
            }
            .padding(28)
            .frame(maxWidth: .infinity, alignment: .leading)
        }
        .task(id: "\(query)|\(lyricsOnly)|\(store.libraryVersion)") {
            guard !query.isEmpty else { found = nil; error = nil; return }
            try? await Task.sleep(for: .milliseconds(150))
            guard !Task.isCancelled else { return }
            let limit: UInt32 = lyricsOnly ? 50 : 12
            let next = JSON.decode(FoundData.self, await store.load { try $0.findJson(query: query, limit: limit) })
            guard !Task.isCancelled else { return }
            found = next
            // A rule that does not read shows its error, as the Songs page does.
            error = (next?.isEmpty ?? true) ? store.core.checkQuery(query: query).error : nil
        }
    }

    // MARK: Chips

    private func chips(_ query: String) -> some View {
        HStack(spacing: 8) {
            chip("All", on: !lyricsOnly) { lyricsOnly = false }
            // Songs, Albums, and Artists open those pages, filtered by the search.
            chip("Songs", on: false) { store.navigate(.songs) }
            chip("Albums", on: false) { store.navigate(.albums) }
            chip("Artists", on: false) { store.navigate(.artists) }
            chip("Lyrics", on: lyricsOnly) { lyricsOnly = true }
        }
        .disabled(query.isEmpty)
    }

    private func chip(_ title: String, on: Bool, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Text(title)
                .font(.needle(.callout).weight(.medium))
                .padding(.horizontal, 14)
                .padding(.vertical, 6)
                .foregroundStyle(on ? store.colors.accentInk : store.colors.ink)
                .background(on ? store.colors.accent : store.colors.raised, in: Capsule())
        }
        .buttonStyle(.plain)
        .accessibilityAddTraits(on ? .isSelected : [])
    }

    // MARK: All

    @ViewBuilder
    private func all(_ found: FoundData, query: String) -> some View {
        HStack(alignment: .top, spacing: 30) {
            if let best = found.best { bestMatch(best) }
            if !found.songs.isEmpty {
                VStack(alignment: .leading, spacing: 8) {
                    Text("Songs").font(.needle(.title3).weight(.semibold))
                    VStack(spacing: 2) {
                        ForEach(Array(found.songs.prefix(5).enumerated()), id: \.element.id) { i, track in
                            SearchSongRow(track: track) { store.play(Array(found.songs.prefix(5)), from: i, reason: "Matches “\(query)”") }
                        }
                    }
                }
                .frame(maxWidth: .infinity, alignment: .leading)
            }
        }
        if !found.albums.isEmpty {
            VStack(alignment: .leading, spacing: 10) {
                ShelfTitle(title: "Albums", detail: nil, all: .albums)
                PagedRow(tile: 156, spacing: 18, middle: 78) {
                    LazyHStack(alignment: .top, spacing: 18) {
                        ForEach(found.albums) { AlbumTile(album: $0, side: 156) }
                    }
                    .padding(.horizontal, 2)
                }
            }
        }
        if !found.artists.isEmpty {
            VStack(alignment: .leading, spacing: 10) {
                ShelfTitle(title: "Artists", detail: nil, all: .artists)
                PagedRow(tile: 132, spacing: 20, middle: 66) {
                    LazyHStack(alignment: .top, spacing: 20) {
                        ForEach(found.artists) { ArtistTile(artist: $0, side: 132) }
                    }
                    .padding(.horizontal, 2)
                }
            }
        }
        if !found.lyrics.isEmpty {
            VStack(alignment: .leading, spacing: 10) {
                Text("In the lyrics").font(.needle(.title3).weight(.semibold))
                lyricLines(Array(found.lyrics.prefix(3)), query: query)
            }
        }
        rules
    }

    /// The best match, large: its cover or photo, what it is, its name, and Play.
    private func bestMatch(_ best: FoundData.Best) -> some View {
        let colors = store.colors
        let (name, detail, kind): (String, String, String) = switch best.kind {
        case "album": (best.album?.title ?? "", best.album?.artist ?? "", "Album")
        case "artist": (best.artist?.title ?? "", count(best.artist?.tracks ?? 0, "song"), "Artist")
        default: (best.track?.title ?? "", best.track?.artist ?? "", "Song")
        }
        return VStack(alignment: .leading, spacing: 12) {
            Group {
                if best.kind == "artist", let artist = best.artist {
                    ArtistPhoto(name: artist.name, side: 120)
                } else {
                    Cover(path: best.album?.artwork ?? best.track?.artwork, seed: name + detail, side: 120, corner: 8)
                }
            }
            .shadow(color: .black.opacity(0.25), radius: 10, y: 5)
            VStack(alignment: .leading, spacing: 3) {
                Text(name).font(store.display(26)).lineLimit(2)
                HStack(spacing: 8) {
                    Text(kind.uppercased()).font(.needle(.caption).weight(.bold)).kerning(0.6)
                        .padding(.horizontal, 8).padding(.vertical, 3)
                        .background(colors.raisedHover, in: Capsule())
                    Text(detail).foregroundStyle(colors.ink2).lineLimit(1)
                }
            }
            Button { play(best) } label: { Label("Play", systemImage: "play.fill") }
                .glassButton(prominent: true)
                .controlSize(.large)
        }
        .padding(20)
        .frame(width: 340, alignment: .leading)
        .background(colors.raised, in: RoundedRectangle(cornerRadius: 14, style: .continuous))
        .contentShape(Rectangle())
        .onTapGesture { open(best) }
        .accessibilityElement(children: .combine)
        .accessibilityAddTraits(.isButton)
        .accessibilityAction { open(best) }
        .accessibilityAction(named: "Play") { play(best) }
    }

    /// Lines of lyrics that hold the words, the words lit; a click plays the song from the start.
    private func lyricLines(_ lines: [FoundData.Line], query: String) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            ForEach(Array(lines.enumerated()), id: \.offset) { _, hit in
                LyricHitRow(hit: hit, text: lit(hit.line, query)) {
                    store.play([hit.track], reason: "Its lyrics hold “\(query)”")
                }
            }
        }
    }

    /// The line, with each place it holds the words lit in the accent.
    private func lit(_ line: String, _ query: String) -> AttributedString {
        var text = AttributedString(line)
        let words = query.lowercased()
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

    /// Try a rule: a click puts it in the search field.
    private var rules: some View {
        let examples = store.core.ruleExamples()
        let pairs = stride(from: 0, to: examples.count - 1, by: 2).map { (examples[$0], examples[$0 + 1]) }
        return VStack(alignment: .leading, spacing: 10) {
            Text("Try a rule").font(.needle(.title3).weight(.semibold))
            HStack(spacing: 8) {
                ForEach(pairs, id: \.1) { name, rule in
                    Button { store.search = rule } label: {
                        Text(name).font(.needle(.callout))
                            .padding(.horizontal, 12).padding(.vertical, 6)
                            .background(store.colors.raised, in: Capsule())
                    }
                    .buttonStyle(.plain)
                    .help(rule)
                }
            }
        }
    }

    // MARK: Actions

    private func open(_ best: FoundData.Best) {
        switch best.kind {
        case "album": if let album = best.album { store.navigate(album.route) }
        case "artist": if let artist = best.artist { store.navigate(.artist(artist.name)) }
        default: if let track = best.track { store.play([track], reason: "Matches “\(store.search)”") }
        }
    }

    private func play(_ best: FoundData.Best) {
        switch best.kind {
        case "album":
            guard let album = best.album else { return }
            let reason = "From the album \(album.album)"
            Task { _ = await store.ask { try $0.playView(query: album.query, playlist: nil, filter: "", start: 0, shuffle: false, reason: reason) } }
        case "artist":
            guard let artist = best.artist else { return }
            let (rule, reason) = (Route.artist(artist.name).base, "From \(artist.name)")
            Task { _ = await store.ask { try $0.playView(query: rule, playlist: nil, filter: "", start: 0, shuffle: false, reason: reason) } }
        default:
            if let track = best.track { store.play([track], reason: "Matches “\(store.search)”") }
        }
    }
}

/// A song in Search's short list: its cover, name, and artist; Play shows under the pointer.
private struct SearchSongRow: View {
    private var store: Store { .shared }
    let track: Track
    let play: () -> Void
    @State private var hover = false

    var body: some View {
        HStack(spacing: 10) {
            ZStack {
                Cover(path: track.artwork, seed: track.album + track.artist, side: 40, corner: 5)
                if hover {
                    Image(systemName: "play.fill").font(.system(size: 13, weight: .bold)).foregroundStyle(.white)
                        .frame(width: 40, height: 40)
                        .background(.black.opacity(0.35), in: RoundedRectangle(cornerRadius: 5, style: .continuous))
                }
            }
            VStack(alignment: .leading, spacing: 1) {
                Text(track.title).lineLimit(1)
                Text(track.artist).font(.needle(.caption)).foregroundStyle(store.colors.ink2).lineLimit(1)
            }
            Spacer()
            Text(clock(track.duration)).font(.needle(.caption)).monospacedDigit().foregroundStyle(store.colors.ink3)
        }
        .padding(.horizontal, 8)
        .padding(.vertical, 4)
        .background(hover ? Color.primary.opacity(0.07) : .clear, in: RoundedRectangle(cornerRadius: 7, style: .continuous))
        .contentShape(Rectangle())
        .onHover { hover = $0 }
        .onTapGesture(count: 2, perform: play)
        .onTapGesture { if hover { play() } }
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
            Text("\(hit.track.title) · \(hit.track.artist)").font(.needle(.callout)).foregroundStyle(store.colors.ink2)
        }
        .padding(.vertical, 8)
        .padding(.horizontal, 12)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(hover ? Color.primary.opacity(0.06) : .clear, in: RoundedRectangle(cornerRadius: 8, style: .continuous))
        .contentShape(Rectangle())
        .onHover { hover = $0 }
        .onTapGesture(perform: play)
        .accessibilityElement(children: .combine)
        .accessibilityAddTraits(.isButton)
        .accessibilityLabel("\(hit.line), from \(hit.track.title) by \(hit.track.artist)")
        .accessibilityAction { play() }
    }
}
