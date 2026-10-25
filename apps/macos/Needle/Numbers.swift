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

/// What was playing on this day some years ago.
struct OnThisDay: Decodable, Identifiable {
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
                        .font(.callout).foregroundStyle(colors.ink2)
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
            Text(value).font(.title2.weight(.semibold)).monospacedDigit()
            Text(label).font(.callout).foregroundStyle(store.colors.ink2)
        }
        .padding(14)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(store.colors.raised, in: RoundedRectangle(cornerRadius: 10, style: .continuous))
    }

    private func section<Content: View>(_ title: String, @ViewBuilder _ content: () -> Content) -> some View {
        VStack(alignment: .leading, spacing: 10) {
            Text(title).font(.title3.weight(.semibold))
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

/// A song on the mood map: `x` dark (0) to bright (1), `y` calm (0) to lively (1).
struct MoodPoint: Decodable, Hashable {
    var id: String
    var x: Double
    var y: Double
    var mood: String
}

/// Every measured song as a dot, by dark ↔ bright and calm ↔ lively; drag a circle over an
/// area to play the songs in it.
struct MoodMapPage: View {
    private var store: Store { .shared }
    @State private var points: [MoodPoint]?
    /// The circle being drawn: its centre and edge, in the map's 0-to-1 places.
    @State private var circle: (CGPoint, CGPoint)?

    var body: some View {
        let colors = store.colors
        VStack(alignment: .leading, spacing: 14) {
            VStack(alignment: .leading, spacing: 2) {
                Text("Mood map").font(store.display(28))
                Text(points.map { $0.isEmpty ? "No songs measured yet" : "\(count($0.count, "song")) · drag a circle to play an area" } ?? "Loading…")
                    .font(.callout).foregroundStyle(colors.ink2)
            }
            if let points, points.isEmpty {
                Text("Needle measures each song's sound first. Turn on Settings › Library › Measure songs for radio, and the map fills in as it goes.")
                    .foregroundStyle(colors.ink2)
                    .frame(maxWidth: 520, alignment: .leading)
                Spacer()
            } else {
                GeometryReader { geo in
                    let side = min(geo.size.width, geo.size.height) - 40
                    map(points ?? [], side: side)
                        .frame(width: side, height: side)
                        .frame(maxWidth: .infinity, maxHeight: .infinity)
                }
            }
        }
        .padding(28)
        .task(id: store.libraryVersion) {
            points = JSON.decode([MoodPoint].self, await store.load { try $0.moodMapJson() }) ?? []
        }
    }

    private func map(_ points: [MoodPoint], side: CGFloat) -> some View {
        let colors = store.colors
        return ZStack {
            RoundedRectangle(cornerRadius: 16, style: .continuous).fill(colors.raised.opacity(0.6))
            Canvas { context, size in
                for p in points {
                    let at = CGPoint(x: p.x * size.width, y: (1 - p.y) * size.height)
                    // Colour by place: dark and calm deep blue, bright and lively warm.
                    let color = Color(hue: 0.62 - 0.55 * p.x, saturation: 0.55 + 0.3 * p.y, brightness: 0.55 + 0.4 * p.x)
                    context.fill(Path(ellipseIn: CGRect(x: at.x - 2.5, y: at.y - 2.5, width: 5, height: 5)), with: .color(color.opacity(0.85)))
                }
                if let (center, edge) = circle {
                    let c = CGPoint(x: center.x * size.width, y: center.y * size.height)
                    let r = hypot((edge.x - center.x) * size.width, (edge.y - center.y) * size.height)
                    let ring = Path(ellipseIn: CGRect(x: c.x - r, y: c.y - r, width: r * 2, height: r * 2))
                    context.fill(ring, with: .color(colors.accent.opacity(0.15)))
                    context.stroke(ring, with: .color(colors.accent), lineWidth: 2)
                }
            }
            .padding(10)
            .contentShape(Rectangle())
            .gesture(DragGesture(minimumDistance: 0)
                .onChanged { value in
                    let inner = side - 20
                    let place = { (p: CGPoint) in CGPoint(x: min(max(0, (p.x - 10) / inner), 1), y: min(max(0, (p.y - 10) / inner), 1)) }
                    circle = (place(value.startLocation), place(value.location))
                }
                .onEnded { _ in play() })
            // The axes' words.
            VStack {
                Text("Lively")
                Spacer()
                Text("Calm")
            }
            .font(.caption.weight(.semibold)).foregroundStyle(colors.ink3)
            .padding(.vertical, -22)
            .allowsHitTesting(false)
            HStack {
                Text("Dark").rotationEffect(.degrees(-90)).fixedSize()
                Spacer()
                Text("Bright").rotationEffect(.degrees(90)).fixedSize()
            }
            .font(.caption.weight(.semibold)).foregroundStyle(colors.ink3)
            .padding(.horizontal, -30)
            .allowsHitTesting(false)
        }
        .accessibilityElement(children: .ignore)
        .accessibilityLabel("Mood map of \(count(points.count, "song"))")
        .accessibilityAction(named: "Play calm songs") { play(at: CGPoint(x: 0.5, y: 0.85)) }
        .accessibilityAction(named: "Play lively songs") { play(at: CGPoint(x: 0.5, y: 0.15)) }
        .accessibilityAction(named: "Play dark songs") { play(at: CGPoint(x: 0.15, y: 0.5)) }
        .accessibilityAction(named: "Play bright songs") { play(at: CGPoint(x: 0.85, y: 0.5)) }
    }

    /// Plays the songs in the circle drawn (a click: a small circle around it).
    private func play() {
        guard let (center, edge) = circle else { return }
        let radius = max(0.06, hypot(edge.x - center.x, edge.y - center.y))
        play(at: center, radius: radius)
    }

    private func play(at center: CGPoint, radius: Double = 0.15) {
        // The map's y runs from lively at the top; the core's from calm (0) to lively (1).
        let (x, y, r) = (Float(center.x), Float(1 - center.y), Float(radius))
        Task {
            let played = await store.ask { try $0.playMoodArea(x: x, y: y, radius: r) } ?? 0
            store.notify(played > 0 ? "Playing \(count(Int(played), "song")) from the mood map" : "No songs there yet")
        }
    }
}
