import SwiftUI

/// Radio: stations Needle makes from a song (that song first, then songs that flow from it), from
/// the songs played most, and quick mixes of the whole library.
struct RadioView: View {
    @Environment(Store.self) private var store
    @State private var seeds: [Ranked]?
    @State private var songs: UInt32 = 0
    @State private var favorites = 0
    /// A cover for each quick mix, which gives the tile its colour (the music is the colour).
    @State private var covers: [String: String] = [:]

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 0) {
                hero
                mixes
                if let seeds, !seeds.isEmpty {
                    SectionTitle(title: "Stations from your favorites")
                    Text("Each starts with the song, then plays songs that flow from it.")
                        .font(.needle(.footnote)).foregroundStyle(.secondary)
                        .padding(.horizontal).padding(.top, 2)
                    Shelf {
                        ForEach(Array(seeds.prefix(12).enumerated()), id: \.offset) { _, seed in
                            StationCard(seed: seed)
                        }
                    }
                } else if seeds != nil {
                    ContentUnavailableView {
                        Label("Stations come from what you play", systemImage: "dot.radiowaves.left.and.right")
                    } description: {
                        Text("Play some music, and a station for each song you play most shows here. Any song's menu can start one too: Start Radio.")
                    }
                    .padding(.top, 30)
                }
            }
            .padding(.bottom, 24)
        }
        .navigationTitle("Radio")
        .themed()
        .toolbar {
            ToolbarItem(placement: .topBarTrailing) {
                NavigationLink(value: Route.settings) { Image(systemName: "gearshape") }
            }
        }
        .task(id: store.libraryVersion) {
            seeds = await store.load { try $0.stats(days: 90).topSongs.filter { $0.songId != nil } } ?? []
            songs = await store.load { try $0.songCount() } ?? 0
            let loved = await store.load { try $0.favorites() } ?? []
            favorites = loved.count
            covers = await store.load { core -> [String: String] in
                var found: [String: String] = [:]
                let home = try? core.home()
                found["shuffle"] = home?.mostPlayed.first?.artwork ?? home?.recent.first?.artwork
                found["new"] = home?.added.first?.artwork
                found["listening"] = (try? core.stats(days: 30))?.topAlbums.first?.artwork ?? home?.recent.dropFirst().first?.artwork
                found["genres"] = (try? core.genres())?.first { $0.artwork != nil }?.artwork
                return found
            } ?? [:]
            covers["favorites"] = loved.first { $0.artwork != nil }?.artwork
        }
    }

    /// Your station: from the song played most lately, or any song when nothing has played yet.
    private var hero: some View {
        let seed = seeds?.first
        return Button { yourStation() } label: {
            ZStack(alignment: .bottomLeading) {
                Artwork(path: seed?.artwork, seed: (seed?.album ?? "") + (seed?.artist ?? "Needle"), size: 800)
                    .frame(height: 220)
                    .frame(maxWidth: .infinity)
                    .clipped()
                    .overlay {
                        LinearGradient(colors: [.clear, .black.opacity(0.75)], startPoint: .center, endPoint: .bottom)
                    }
                HStack(alignment: .bottom) {
                    VStack(alignment: .leading, spacing: 3) {
                        Text("YOUR STATION").font(.needle(.caption).weight(.bold)).kerning(0.6).foregroundStyle(.white.opacity(0.8))
                        Text(seed.map { "From \($0.title)" } ?? "From anything in your library")
                            .font(.needle(.title2).bold()).foregroundStyle(.white).lineLimit(2)
                        Text(seed.map { $0.artist } ?? "Needle picks a song to start")
                            .font(.needle(.subheadline)).foregroundStyle(.white.opacity(0.8)).lineLimit(1)
                    }
                    Spacer()
                    Image(systemName: "play.fill").font(.needle(.title3)).foregroundStyle(.black)
                        .frame(width: 52, height: 52)
                        .background(.white, in: .circle)
                }
                .padding(18)
            }
            .clipShape(.soft(Corner.large))
            .shadow(color: .black.opacity(0.3), radius: 14, y: 8)
            .padding(.horizontal)
            .padding(.top, 8)
        }
        .buttonStyle(Pressable())
        .accessibilityLabel(seed.map { "Your station, from \($0.title) by \($0.artist)" } ?? "Your station")
        .accessibilityHint("Starts a radio station")
    }

    /// Mixes of the whole library, two to a row (more on a phone on its side).
    private var mixes: some View {
        VStack(alignment: .leading, spacing: 0) {
            SectionTitle(title: "Quick mixes")
            LazyVGrid(columns: [GridItem(.adaptive(minimum: 150), spacing: 12)], spacing: 12) {
                MixTile(title: "Shuffle all", detail: count(Int(songs), "song"), symbol: "shuffle", cover: covers["shuffle"]) {
                    store.playIDs({ try $0.songs().map(\.id) }, shuffled: true)
                }
                MixTile(title: "Favorites", detail: favorites > 0 ? "\(count(favorites, "song")), shuffled" : "Heart songs to fill it", symbol: "heart.fill", cover: covers["favorites"]) {
                    store.playIDs({ try $0.favorites().map(\.id) }, shuffled: true)
                }
                .disabled(favorites == 0)
                MixTile(title: "New to you", detail: "Added lately, shuffled", symbol: "sparkles", cover: covers["new"]) {
                    store.playIDs({ core in try core.home().added.prefix(12).flatMap { try core.albumSongs(key: $0.key).map(\.id) } }, shuffled: true)
                }
                NavigationLink(value: Route.stats) {
                    MixTileLabel(title: "Your listening", detail: "Top songs and artists", symbol: "chart.bar.fill", cover: covers["listening"])
                }
                .buttonStyle(Pressable())
                NavigationLink(value: Route.genreMap) {
                    MixTileLabel(title: "Genre map", detail: "Tap a genre to play it", symbol: "circle.hexagongrid.fill", cover: covers["genres"])
                }
                .buttonStyle(Pressable())
            }
            .padding(.horizontal)
            .padding(.top, 10)
        }
    }

    private func yourStation() {
        store.run { core in
            let top = try core.stats(days: 30).topSongs.compactMap(\.songId)
            let any = try core.songs().randomElement()?.id
            // The first that can start one (a song whose file is gone cannot).
            for id in top + [any].compactMap({ $0 }) where (try? core.startRadio(id: id)) != nil { return }
            throw NeedleError.Failed(message: "Add music first")
        }
        store.playedWhenReady()
    }
}

/// A station from one song: its cover, and a tap starts it.
private struct StationCard: View {
    @Environment(Store.self) private var store
    let seed: Ranked

    var body: some View {
        Button {
            guard let id = seed.songId else { return }
            store.run { try $0.startRadio(id: id) }
            store.playedWhenReady()
        } label: {
            VStack(alignment: .leading, spacing: 3) {
                Artwork(path: seed.artwork, seed: seed.album + seed.artist, size: 400)
                    .frame(width: 150, height: 150)
                    .clipShape(.soft(Corner.cover))
                    .overlay(alignment: .bottomLeading) {
                        // A plain label in the page's raised colour: it is part of the cover.
                        Label("Radio", systemImage: "dot.radiowaves.left.and.right")
                            .font(.needle(.caption2).weight(.bold))
                            .foregroundStyle(store.colors.text)
                            .padding(.horizontal, 8).padding(.vertical, 4)
                            .background(store.colors.card.opacity(0.92), in: .capsule)
                            .padding(8)
                    }
                    .shadow(color: .black.opacity(0.3), radius: 10, y: 6)
                    .padding(.bottom, 5)
                Text(seed.title).font(.needle(.subheadline).weight(.medium)).lineLimit(1)
                Text(seed.artist).font(.needle(.footnote)).foregroundStyle(.secondary).lineLimit(1)
            }
            .frame(width: 150)
            .contentShape(.rect)
        }
        .buttonStyle(Pressable())
        .accessibilityLabel("\(seed.title) Radio, by \(seed.artist)")
    }
}

/// A mix to start with one tap.
private struct MixTile: View {
    let title: String
    let detail: String
    let symbol: String
    let cover: String?
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            MixTileLabel(title: title, detail: detail, symbol: symbol, cover: cover)
        }
        .buttonStyle(Pressable())
    }
}

/// A mix's tile: plain, in the page's raised colour (content, not glass), its sign in the colour
/// of a cover from the mix (the music is the colour), or the accent before one is known.
private struct MixTileLabel: View {
    @Environment(Store.self) private var store
    let title: String
    let detail: String
    let symbol: String
    let cover: String?
    @State private var look: PageColors?

    init(title: String, detail: String, symbol: String, cover: String?) {
        self.title = title
        self.detail = detail
        self.symbol = symbol
        self.cover = cover
        _look = State(initialValue: cover.flatMap { PageColors.cached($0, seed: title) })
    }

    var body: some View {
        let colors = store.colors
        VStack(alignment: .leading, spacing: 10) {
            Image(systemName: symbol).font(.needle(.title3).weight(.semibold))
                .foregroundStyle(look?.ink ?? colors.onAccent)
                .frame(width: 40, height: 40)
                .background((look?.page ?? colors.accent).gradient, in: .soft(Corner.small))
            VStack(alignment: .leading, spacing: 1) {
                Text(title).font(.needle(.subheadline).weight(.semibold)).lineLimit(1)
                Text(detail).font(.needle(.caption)).foregroundStyle(.secondary).lineLimit(1)
            }
        }
        .padding(14)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(colors.card, in: .soft(Corner.middle))
        .contentShape(.rect)
        .task(id: cover) {
            guard let cover else { return }
            let found = await PageColors.of(cover, seed: title)
            if !Task.isCancelled, found != look { withFade { look = found } }
        }
    }
}
