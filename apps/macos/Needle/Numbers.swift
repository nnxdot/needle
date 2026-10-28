import Charts
import SwiftUI

/// A name and how many, as the core writes a pair (["FLAC", 120]).
struct NamedCount: Decodable, Hashable, Identifiable {
    var name: String
    var count: Int
    var id: String { name }

    init(from decoder: Decoder) throws {
        var pair = try decoder.unkeyedContainer()
        name = try pair.decode(String.self)
        count = try pair.decode(Int.self)
    }
}

/// What the library is made of.
struct NumbersData: Decodable {
    var songs: Int
    var albums: Int
    var artists: Int
    var duration: Double
    var size: Int64
    var serverSongs: Int
    var formats: [NamedCount]
    var lossless: Int
    var hires: Int
    var decades: [NamedCount]
    var genres: [NamedCount]
    var artistsBySongs: [NamedCount]
    var albumsWithoutCover: Int
    var lowQualityAlbums: Int
    var withoutCover: [AlbumSummary]
    var lowQuality: [AlbumSummary]
}

/// What was playing on this day some years ago (the core's `OnThisDay`, with the Mac's
/// album summaries).
struct PastDay: Decodable, Identifiable {
    var yearsAgo: Int
    var day: Int
    var albums: [AlbumSummary]
    var listens: Int
    var id: Int { yearsAgo }

    var title: String { yearsAgo == 1 ? "A year ago today" : "\(yearsAgo) years ago today" }
    var detail: String {
        let date = Date(timeIntervalSince1970: TimeInterval(day)).formatted(date: .long, time: .omitted)
        return "\(date) · \(count(listens, "song"))"
    }
}

/// Your library in numbers: how much music, in what formats and quality, from which decades
/// and genres, the artists with the most songs, and the albums to fix.
struct NumbersPage: View {
    private var store: Store { .shared }
    @State private var numbers: NumbersData?

    var body: some View {
        let colors = store.colors
        ScrollView {
            VStack(alignment: .leading, spacing: 30) {
                VStack(alignment: .leading, spacing: 2) {
                    Text("Your library in numbers").font(store.display(28))
                    Text(numbers.map { "\(count($0.songs, "song")) · \(hours($0.duration))" } ?? "Counting…")
                        .font(.needle(.callout)).foregroundStyle(colors.ink2)
                }
                if let n = numbers {
                    HStack(spacing: 14) {
                        tile("\(n.songs.formatted())", "songs")
                        tile("\(n.albums.formatted())", "albums")
                        tile("\(n.artists.formatted())", "artists")
                        tile(n.size > 0 ? ByteCountFormatter.string(fromByteCount: n.size, countStyle: .file) : "—", "on this Mac")
                        if n.serverSongs > 0 { tile("\(n.serverSongs.formatted())", "from your server") }
                    }
                    HStack(alignment: .top, spacing: 36) {
                        section("Quality") {
                            share("Lossless", n.lossless, of: n.songs)
                            share("Hi-res", n.hires, of: n.songs)
                        }
                        section("Formats") { bars(n.formats) }
                    }
                    if !n.decades.isEmpty {
                        section("Decades") {
                            Chart(n.decades) { decade in
                                BarMark(x: .value("Decade", decade.name), y: .value("Songs", decade.count))
                                    .foregroundStyle(colors.accent.gradient)
                                    .cornerRadius(4)
                            }
                            .chartYAxis(.hidden)
                            .frame(height: 160)
                        }
                    }
                    HStack(alignment: .top, spacing: 36) {
                        section("Genres") { bars(n.genres) }
                        section("Artists with the most songs") { bars(n.artistsBySongs) }
                    }
                    shelf("Albums without a cover", n.albumsWithoutCover, n.withoutCover)
                    shelf("Albums in low quality", n.lowQualityAlbums, n.lowQuality, note: "Every song lossy and under 192 kbit/s.")
                }
            }
            .padding(28)
        }
        .task(id: store.libraryVersion) {
            numbers = JSON.decode(NumbersData.self, await store.load { try $0.libraryNumbersJson() })
        }
    }

    private func tile(_ value: String, _ label: String) -> some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(value).font(.needle(.title2).weight(.semibold)).monospacedDigit()
            Text(label).font(.needle(.callout)).foregroundStyle(store.colors.ink2)
        }
        .padding(14)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(store.colors.raised, in: RoundedRectangle(cornerRadius: 10, style: .continuous))
    }

    private func section<Content: View>(_ title: String, @ViewBuilder _ content: () -> Content) -> some View {
        VStack(alignment: .leading, spacing: 10) {
            Text(title).font(.needle(.title3).weight(.semibold))
            content()
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    /// A part of all the songs, as a line and a share.
    private func share(_ name: String, _ part: Int, of whole: Int) -> some View {
        let fraction = whole > 0 ? Double(part) / Double(whole) : 0
        return VStack(alignment: .leading, spacing: 4) {
            HStack {
                Text(name)
                Spacer()
                Text(fraction.formatted(.percent.precision(.fractionLength(0)))).monospacedDigit().foregroundStyle(store.colors.ink2)
            }
            meter(fraction)
        }
        .accessibilityElement(children: .combine)
    }

    private func bars(_ items: [NamedCount]) -> some View {
        let most = max(items.map(\.count).max() ?? 1, 1)
        return VStack(alignment: .leading, spacing: 8) {
            ForEach(items.prefix(10)) { item in
                VStack(alignment: .leading, spacing: 3) {
                    HStack {
                        Text(item.name).lineLimit(1)
                        Spacer()
                        Text(item.count.formatted()).monospacedDigit().foregroundStyle(store.colors.ink2)
                    }
                    meter(Double(item.count) / Double(most))
                }
                .accessibilityElement(children: .combine)
            }
        }
    }

    private func meter(_ fraction: Double) -> some View {
        GeometryReader { geo in
            ZStack(alignment: .leading) {
                Capsule().fill(store.colors.raised)
                Capsule().fill(store.colors.accent).frame(width: max(4, geo.size.width * fraction))
            }
        }
        .frame(height: 5)
        .accessibilityHidden(true)
    }

    @ViewBuilder
    private func shelf(_ title: String, _ total: Int, _ albums: [AlbumSummary], note: String? = nil) -> some View {
        if total > 0 {
            VStack(alignment: .leading, spacing: 10) {
                ShelfTitle(title: "\(title) · \(total.formatted())", detail: note, all: nil)
                PagedRow(tile: 140, spacing: 18, middle: 70) {
                    LazyHStack(alignment: .top, spacing: 18) {
                        ForEach(albums) { album in AlbumTile(album: album, side: 140) }
                    }
                    .padding(.horizontal, 2)
                    .padding(.bottom, 4)
                }
            }
        }
    }

    private func hours(_ seconds: Double) -> String {
        let hours = Int(seconds / 3600)
        return hours >= 48 ? "\((hours / 24).formatted()) days of music" : "\(hours.formatted()) hours of music"
    }
}
