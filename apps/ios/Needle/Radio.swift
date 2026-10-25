import SwiftUI

/// Radio: stations Needle makes from a song (that song first, then songs that flow from it), from
/// the songs played most, and quick mixes of the whole library.
struct RadioView: View {
    @Environment(Store.self) private var store
    @State private var seeds: [Ranked]?
    @State private var songs: UInt32 = 0
    @State private var favourites = 0

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 0) {
                hero
                mixes
                if let seeds, !seeds.isEmpty {
                    SectionTitle(title: "Stations from your favourites")
                    Text("Each starts with the song, then plays songs that flow from it.")
                        .font(.footnote).foregroundStyle(.secondary)
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
            favourites = await store.load { try $0.favorites().count } ?? 0
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
                        Text("YOUR STATION").font(.caption.weight(.bold)).kerning(0.6).foregroundStyle(.white.opacity(0.8))
                        Text(seed.map { "From \($0.title)" } ?? "From anything in your library")
                            .font(.title2.bold()).foregroundStyle(.white).lineLimit(2)
                        Text(seed.map { $0.artist } ?? "Needle picks a song to start")
                            .font(.subheadline).foregroundStyle(.white.opacity(0.8)).lineLimit(1)
                    }
                    Spacer()
                    Image(systemName: "play.fill").font(.title3).foregroundStyle(.black)
                        .frame(width: 52, height: 52)
                        .background(.white, in: .circle)
                }
                .padding(18)
            }
            .clipShape(.rect(cornerRadius: 24))
            .shadow(color: .black.opacity(0.3), radius: 14, y: 8)
            .padding(.horizontal)
            .padding(.top, 8)
        }
        .buttonStyle(Pressable())
        .accessibilityLabel(seed.map { "Your station, from \($0.title) by \($0.artist)" } ?? "Your station")
        .accessibilityHint("Starts a radio station")
    }

    /// Mixes of the whole library, two to a row.
    private var mixes: some View {
        VStack(alignment: .leading, spacing: 0) {
            SectionTitle(title: "Quick mixes")
            LazyVGrid(columns: [GridItem(.flexible(), spacing: 12), GridItem(.flexible(), spacing: 12)], spacing: 12) {
                MixTile(title: "Shuffle all", detail: count(Int(songs), "song"), symbol: "shuffle", tint: .orange) {
                    store.playIDs({ try $0.songs().map(\.id) }, shuffled: true)
                }
                MixTile(title: "Favorites", detail: favourites > 0 ? "\(count(favourites, "song")), shuffled" : "Star songs to fill it", symbol: "star.fill", tint: .pink) {
                    store.playIDs({ try $0.favorites().map(\.id) }, shuffled: true)
                }
                .disabled(favourites == 0)
                MixTile(title: "New to you", detail: "Added lately, shuffled", symbol: "sparkles", tint: .blue) {
                    store.playIDs({ core in try core.home().added.prefix(12).flatMap { try core.albumSongs(key: $0.key).map(\.id) } }, shuffled: true)
                }
                NavigationLink(value: Route.stats) {
                    MixTileLabel(title: "Your listening", detail: "Top songs and artists", symbol: "chart.bar.fill", tint: .purple)
                }
                .buttonStyle(Pressable())
                NavigationLink(value: Route.moodMap) {
                    MixTileLabel(title: "Mood map", detail: "Draw a circle to play it", symbol: "circle.hexagongrid.fill", tint: .teal)
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
                    .clipShape(.rect(cornerRadius: 14))
                    .overlay(alignment: .bottomLeading) {
                        Label("Radio", systemImage: "dot.radiowaves.left.and.right")
                            .font(.caption2.weight(.bold))
                            .padding(.horizontal, 8).padding(.vertical, 4)
                            .glassEffect(.regular, in: .capsule)
                            .padding(8)
                    }
                    .shadow(color: .black.opacity(0.3), radius: 10, y: 6)
                    .padding(.bottom, 5)
                Text(seed.title).font(.subheadline.weight(.medium)).lineLimit(1)
                Text(seed.artist).font(.footnote).foregroundStyle(.secondary).lineLimit(1)
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
    let tint: Color
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            MixTileLabel(title: title, detail: detail, symbol: symbol, tint: tint)
        }
        .buttonStyle(Pressable())
    }
}

private struct MixTileLabel: View {
    let title: String
    let detail: String
    let symbol: String
    let tint: Color

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            Image(systemName: symbol).font(.title3.weight(.semibold)).foregroundStyle(.white)
                .frame(width: 40, height: 40)
                .background(tint.gradient, in: .rect(cornerRadius: 11))
            VStack(alignment: .leading, spacing: 1) {
                Text(title).font(.subheadline.weight(.semibold)).lineLimit(1)
                Text(detail).font(.caption).foregroundStyle(.secondary).lineLimit(1)
            }
        }
        .padding(14)
        .frame(maxWidth: .infinity, alignment: .leading)
        .glassEffect(.regular, in: .rect(cornerRadius: 20))
        .contentShape(.rect)
    }
}
