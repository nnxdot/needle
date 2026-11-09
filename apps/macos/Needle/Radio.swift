import SwiftUI

/// Radio, as on the iPhone: your station (from the song you play most), quick mixes of the whole
/// library, and a station from each song you play most (that song first, then songs that flow
/// from it). Every tile takes its colour from a cover: the music is the colour. Tiles are plain
/// fills, not glass.
struct RadioPage: View {
    private var store: Store { .shared }
    @State private var data: RadioData?

    private var seeds: [Ranked]? { data?.seeds }
    private var home: HomeData? { data?.home }

    var body: some View {
        ScrollView { content }
            .scrollContentBackground(.hidden)
            .task(id: store.libraryVersion) { data = await RadioData.load(store) }
    }

    private var content: some View {
        VStack(alignment: .leading, spacing: 30) {
            VStack(alignment: .leading, spacing: 2) {
                Text("Radio").font(store.display(28))
                Text("Stations that start with a song and flow on from it, and mixes of your whole library.")
                    .font(.needle(.callout)).foregroundStyle(store.colors.ink2)
            }
            StationHero(seed: seeds?.first, loaded: seeds != nil) { yourStation() }
            mixes
            stations
        }
        .padding(28)
        .frame(maxWidth: 1100, alignment: .leading)
    }

    /// Mixes of the whole library, each in the colour of a cover it plays from.
    private var mixes: some View {
        let covers = tileCovers()
        let (mostPlayed, loved, added, top, recent) = (covers[0], covers[1], covers[2], covers[3], covers[4])
        return VStack(alignment: .leading, spacing: 12) {
            ShelfTitle(title: "Quick mixes", detail: nil, all: nil)
            LazyVGrid(columns: [GridItem(.adaptive(minimum: 220), spacing: 12)], spacing: 12) {
                MixTile(title: "Shuffle all", detail: count(Int(data?.songs ?? 0), "song"), symbol: "shuffle",
                        cover: mostPlayed.path, seed: mostPlayed.seed) {
                    let reason = "Shuffle all"
                    Task { _ = await store.ask { try $0.playView(query: "", playlist: nil, filter: "", start: 0, shuffle: true, reason: reason) } }
                }
                let favourites = data?.favourites ?? 0
                MixTile(title: "Favorites", detail: favourites > 0 ? "\(count(favourites, "song")), shuffled" : "Star songs to fill it",
                        symbol: "heart.fill", cover: loved.path, seed: loved.seed) {
                    let reason = "From your favorites"
                    Task { _ = await store.ask { try $0.playView(query: Route.favorites.base, playlist: nil, filter: "", start: 0, shuffle: true, reason: reason) } }
                }
                .disabled(favourites == 0)
                MixTile(title: "New to you", detail: "Added lately, shuffled", symbol: "sparkles",
                        cover: added.path, seed: added.seed) { newToYou() }
                MixTile(title: "Your listening", detail: "Top songs and artists", symbol: "chart.bar.fill",
                        cover: top.path, seed: top.seed) { store.navigate(.history) }
                MixTile(title: "Genre map", detail: "Click a genre to play it", symbol: "circle.hexagongrid.fill",
                        cover: recent.path, seed: recent.seed) { store.navigate(.genreMap) }
            }
        }
    }

    @ViewBuilder
    private var stations: some View {
        if let seeds, !seeds.isEmpty {
            VStack(alignment: .leading, spacing: 10) {
                ShelfTitle(title: "Stations from your favorites", detail: "Each starts with the song, then plays songs that flow from it.", all: nil)
                PagedRow(tile: 156, spacing: 18, middle: 78) {
                    LazyHStack(alignment: .top, spacing: 18) {
                        ForEach(Array(seeds.prefix(12).enumerated()), id: \.offset) { _, seed in StationTile(seed: seed) }
                    }
                    .padding(.horizontal, 2)
                    .padding(.bottom, 4)
                }
            }
        } else if seeds != nil {
            ContentUnavailableView {
                Label("Stations come from what you play", systemImage: "dot.radiowaves.left.and.right")
            } description: {
                Text("Play some music, and a station for each song you play most shows here. Any song's menu can start one too: Start Radio.")
            }
            .frame(maxWidth: .infinity)
        }
    }

    /// The quick mixes' covers, in the tiles' order (Shuffle all, Favorites, New to you, Your
    /// listening, Genre map): each its own album's while the library has enough of them.
    private func tileCovers() -> [(path: String?, seed: String)] {
        typealias Pick = (path: String?, seed: String)
        let album = { (a: AlbumSummary) -> Pick in (a.artwork, a.album + a.artist) }
        let all = ((home?.mostPlayed ?? []) + (home?.recent ?? []) + (home?.added ?? []) + (home?.favorites ?? [])).map(album)
        let song = (seeds ?? []).prefix(1).map { (path: $0.artwork, seed: $0.album + $0.artist) }
        let wishes: [[Pick]] = [
            (home?.mostPlayed ?? []).map(album) + all,
            (home?.favorites ?? []).map(album) + all,
            (home?.added ?? []).map(album) + all,
            song + all,
            (home?.recent ?? []).map(album) + all,
        ]
        var used: Set<String> = []
        // Favorites and New to you first: their covers say the most about them.
        var picks = Array(repeating: Pick(path: nil, seed: "radio"), count: wishes.count)
        for i in [1, 2, 3, 0, 4] {
            let wish = wishes[i]
            let pick = wish.first { !used.contains($0.path ?? $0.seed) } ?? wish.first ?? Pick(path: nil, seed: ["shuffle", "favorites", "new", "listening", "genres"][i])
            used.insert(pick.path ?? pick.seed)
            picks[i] = pick
        }
        return picks
    }

    /// Your station: from the song it shows (played most lately), else another played most,
    /// or any song when nothing has played yet.
    private func yourStation() {
        store.notify("Tuning in…")
        let shown = seeds?.first.flatMap { s in s.songId.map { ($0, s.title) } }
        Task {
            let started = await store.ask { core -> (String, UInt32) in
                let top = try core.stats(days: 30).topSongs.compactMap { s in s.songId.map { ($0, s.title) } }
                let any = try core.songs().randomElement().map { ($0.id, $0.title) }
                // The first that can start one (a song whose file is gone cannot).
                var why: Error = NeedleError.Failed(message: "Add music first")
                for (id, title) in [shown].compactMap({ $0 }) + top + [any].compactMap({ $0 }) {
                    do { return (title, try core.radioFrom(id: id)) } catch { why = error }
                }
                throw why
            }
            if case let (title, n)? = started { store.notify("Radio from “\(title)”: \(count(Int(n), "song")).") }
        }
    }

    /// The albums added last (twelve), their songs shuffled.
    private func newToYou() {
        Task {
            let ids = await store.ask { core in
                try core.home().added.prefix(12).flatMap { try core.albumSongs(key: $0.key).map(\.id) }.shuffled()
            }
            guard let ids else { return }
            if ids.isEmpty { store.notify("Add music first."); return }
            store.run { _ = try $0.playSongs(ids: ids, start: 0, reason: "New to you") }
        }
    }
}

/// What the Radio page shows, read away from the window.
struct RadioData {
    /// The songs played most in 90 days that are in the library: each starts a station.
    var seeds: [Ranked]
    var songs: UInt32
    var favourites: Int
    var home: HomeData?

    static func load(_ store: Store) async -> RadioData {
        RadioData(seeds: await store.load { try $0.stats(days: 90).topSongs.filter { $0.songId != nil } } ?? [],
                  songs: await store.load { try $0.songCount() } ?? 0,
                  favourites: await store.load { try $0.favorites().count } ?? 0,
                  home: JSON.decode(HomeData.self, await store.load { try $0.homeJson() }))
    }
}

/// Your station: the cover of the song it starts from, large, with its name and a Play button.
struct StationHero: View {
    private var store: Store { .shared }
    let seed: Ranked?
    /// The songs played most are read: before that, no "anything in your library" flashes.
    let loaded: Bool
    let start: () -> Void
    @State private var hover = false

    var body: some View {
        Button(action: start) {
            ZStack(alignment: .bottomLeading) {
                Artwork(path: seed?.artwork, seed: (seed?.album ?? "") + (seed?.artist ?? "Needle"), size: 1000)
                    .frame(height: 220)
                    .frame(maxWidth: .infinity)
                    .overlay {
                        LinearGradient(colors: [.clear, .black.opacity(0.75)], startPoint: .center, endPoint: .bottom)
                    }
                HStack(alignment: .bottom) {
                    VStack(alignment: .leading, spacing: 3) {
                        Text("YOUR STATION").font(.needle(.caption).weight(.bold)).kerning(0.6).foregroundStyle(.white.opacity(0.8))
                        Text(seed.map { "From \($0.title)" } ?? (loaded ? "From anything in your library" : " "))
                            .font(.needle(.title2).bold()).foregroundStyle(.white).lineLimit(2)
                        Text(seed?.artist ?? (loaded ? "Needle picks a song to start" : " "))
                            .font(.needle(.subheadline)).foregroundStyle(.white.opacity(0.8)).lineLimit(1)
                    }
                    Spacer()
                    Image(systemName: "play.fill").font(.system(size: 18, weight: .bold)).foregroundStyle(store.colors.accentInk)
                        .frame(width: 48, height: 48)
                        .background(store.colors.accent, in: Circle())
                        .shadow(color: .black.opacity(0.3), radius: 8, y: 3)
                        .scaleEffect(hover && store.motion ? 1.06 : 1)
                }
                .padding(20)
            }
            .clipShape(RoundedRectangle(cornerRadius: Radius.large, style: .continuous))
            .shadow(color: .black.opacity(hover ? 0.32 : 0.22), radius: hover ? 18 : 12, y: hover ? 8 : 6)
            .contentShape(RoundedRectangle(cornerRadius: Radius.large, style: .continuous))
        }
        .buttonStyle(.plain)
        .onHover { h in withAnimation(store.motion ? .snappy(duration: 0.15) : nil) { hover = h } }
        .accessibilityLabel(seed.map { "Your station, from \($0.title) by \($0.artist)" } ?? "Your station")
        .accessibilityHint("Starts a radio station")
    }
}

/// A station from one song: its cover, and a click starts it.
struct StationTile: View {
    private var store: Store { .shared }
    let seed: Ranked
    @State private var hover = false
    @State private var glow: Color?

    var body: some View {
        Button { start() } label: {
            VStack(alignment: .leading, spacing: 5) {
                Cover(path: seed.artwork, seed: seed.album + seed.artist, side: 156, corner: 8)
                    .overlay(alignment: .bottomLeading) {
                        // A plain dark capsule over the cover, not glass: content is not glass.
                        Label("Radio", systemImage: "dot.radiowaves.left.and.right")
                            .font(.needle(.caption2).weight(.bold))
                            .foregroundStyle(.white)
                            .padding(.horizontal, 8).padding(.vertical, 4)
                            .background(.black.opacity(0.55), in: Capsule())
                            .padding(8)
                    }
                    .shadow(color: hover ? (glow ?? .black).opacity(0.55) : .black.opacity(0.15), radius: hover ? 16 : 6, y: hover ? 6 : 4)
                    .offset(y: hover && store.motion ? -2 : 0)
                Text(seed.title).font(.needle(.callout).weight(.medium)).lineLimit(1)
                Text(seed.artist).font(.needle(.caption)).foregroundStyle(store.colors.ink2).lineLimit(1)
            }
            .frame(width: 156, alignment: .leading)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .onHover { h in withAnimation(store.motion ? .snappy(duration: 0.15) : nil) { hover = h } }
        .task(id: hover) {
            guard hover, glow == nil else { return }
            glow = Color(nsColor: await Covers.pageColor(seed.artwork, seed: seed.album + seed.artist))
        }
        .help("\(seed.title) Radio")
        .accessibilityLabel("\(seed.title) Radio, by \(seed.artist)")
    }

    private func start() {
        guard let id = seed.songId else { return }
        let title = seed.title
        store.notify("Tuning in…")
        Task {
            if let n = await store.ask({ try $0.radioFrom(id: id) }) { store.notify("Radio from “\(title)”: \(count(Int(n), "song")).") }
        }
    }
}

/// A mix to start with one click: a plain fill in its cover's colour, the cover beside its words.
struct MixTile: View {
    private var store: Store { .shared }
    let title: String
    let detail: String
    let symbol: String
    let cover: String?
    let seed: String
    let action: () -> Void
    @Environment(\.isEnabled) private var enabled
    @State private var fill: Color?
    @State private var ink: Color = .white
    @State private var hover = false

    init(title: String, detail: String, symbol: String, cover: String?, seed: String, action: @escaping () -> Void) {
        self.title = title
        self.detail = detail
        self.symbol = symbol
        self.cover = cover
        self.seed = seed
        self.action = action
        // A colour found before (another visit, or a test picture) shows at once.
        if let known = Covers.knownPageColor(cover, seed: seed) {
            _fill = State(initialValue: Color(nsColor: known))
            _ink = State(initialValue: Self.ink(on: known))
        }
    }

    var body: some View {
        Button(action: action) {
            HStack(spacing: 12) {
                Cover(path: cover, seed: seed, side: 52, corner: 6)
                    .shadow(color: .black.opacity(0.2), radius: 4, y: 2)
                VStack(alignment: .leading, spacing: 2) {
                    Text(title).font(.needle(.body).weight(.semibold)).lineLimit(1)
                    Text(detail).font(.needle(.caption)).opacity(0.78).lineLimit(1)
                }
                Spacer(minLength: 4)
                Image(systemName: symbol).font(.system(size: 17, weight: .semibold)).opacity(0.9)
            }
            .foregroundStyle(ink)
            .padding(12)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background {
                RoundedRectangle(cornerRadius: Radius.medium, style: .continuous)
                    .fill(fill ?? store.colors.raised)
                    .brightness(hover && enabled ? 0.04 : 0)
            }
            .contentShape(RoundedRectangle(cornerRadius: Radius.medium, style: .continuous))
            .opacity(enabled ? 1 : 0.55)
        }
        .buttonStyle(.plain)
        .onHover { h in withAnimation(store.motion ? .easeOut(duration: 0.12) : nil) { hover = h } }
        .task(id: cover ?? seed) {
            let color = await Covers.pageColor(cover, seed: seed)
            fill = Color(nsColor: color)
            ink = Self.ink(on: color)
        }
        .accessibilityLabel("\(title), \(detail)")
    }

    /// White or near-black words, whichever reads better on the tile's colour.
    static func ink(on color: NSColor) -> Color {
        let tile = HSL(color)
        let white = HSL(h: 0, s: 0, l: 1), black = HSL(h: 0, s: 0, l: 0.1)
        return Palette.contrast(white, tile) >= Palette.contrast(black, tile) ? .white : Color.black.opacity(0.85)
    }
}
