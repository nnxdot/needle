import Charts
import SwiftUI

/// Songs' loud and quiet shapes, as the core measures them (about a second for a song not
/// measured before), kept while Needle runs.
@MainActor
enum Waveforms {
    private static var kept: [String: [Float]] = [:]

    static func heights(_ id: String, store: Store) async -> [Float] {
        if let known = kept[id] { return known }
        let core = store.core
        var found = await Task.detached { (try? core.waveform(id: id, count: 64)) ?? [] }.value
        // A song measured as silence (or not read) keeps the plain line, not a row of dots.
        if (found.max() ?? 0) < 0.02 { found = [] }
        kept[id] = found
        if kept.count > 200 { kept.removeAll() }
        return found
    }
}

/// The seek bar as the song's real shape: a bar for each part, the played ones lit.
struct WaveBars: View {
    let heights: [Float]
    /// How far the song is, 0 to 1.
    let part: Double
    let held: Bool

    var body: some View {
        Canvas { context, size in
            let count = heights.count
            guard count > 0 else { return }
            let step = size.width / CGFloat(count)
            for (i, height) in heights.enumerated() {
                let h = max(3, CGFloat(height) * size.height)
                let rect = CGRect(x: CGFloat(i) * step + step * 0.16, y: (size.height - h) / 2, width: step * 0.68, height: h)
                let played = (Double(i) + 0.5) / Double(count) <= part
                context.fill(Path(roundedRect: rect, cornerRadius: step * 0.34),
                             with: .color(.white.opacity(played ? (held ? 0.95 : 0.85) : 0.25)))
            }
        }
    }
}

/// Light that moves with the music, in the cover's colours: soft glows that swell with the
/// bass, the middle, and the highs, and a ring of thin bars around the middle. Still (one soft
/// glow) with Reduce motion.
struct Visualizer: View {
    @Environment(Store.self) private var store
    @Environment(\.accessibilityReduceMotion) private var systemReduceMotion
    @State private var levels = Levels()

    /// The bands as they move: rising at once, falling slowly.
    final class Levels {
        var bands = [Float](repeating: 0, count: 40)
        func follow(_ now: [Float]) {
            guard now.count == bands.count else { return }
            for i in bands.indices {
                bands[i] = now[i] > bands[i] ? now[i] : bands[i] * 0.86 + now[i] * 0.14
            }
        }
        func average(_ range: Range<Int>) -> CGFloat {
            CGFloat(bands[range].reduce(0, +) / Float(range.count))
        }
    }

    var body: some View {
        let still = systemReduceMotion || store.app.reduceMotion
        let tint = store.coverColor ?? store.colors.accent
        let palette = [tint, store.colors.accent, tint.opacity(0.7)]
        TimelineView(.animation(minimumInterval: 1 / 30, paused: still)) { timeline in
            let t = timeline.date.timeIntervalSinceReferenceDate
            Canvas { context, size in
                if !still { levels.follow(store.core.spectrum(bands: 40)) }
                let center = CGPoint(x: size.width / 2, y: size.height / 2)
                let side = min(size.width, size.height)
                let bass = levels.average(0..<5), middle = levels.average(8..<22), high = levels.average(25..<40)
                context.addFilter(.blur(radius: side * 0.09))
                for (i, energy) in [bass, middle, high].enumerated() {
                    let angle = t * (0.07 + Double(i) * 0.03) + Double(i) * 2.1
                    let reach = side * (0.12 + 0.06 * CGFloat(i))
                    let at = CGPoint(x: center.x + cos(angle) * reach, y: center.y + sin(angle) * reach * 0.8)
                    let radius = side * (0.24 + 0.3 * energy) * (i == 0 ? 1.2 : 1)
                    context.fill(Path(ellipseIn: CGRect(x: at.x - radius, y: at.y - radius, width: radius * 2, height: radius * 2)),
                                 with: .color(palette[i].opacity(0.35 + 0.55 * Double(energy))))
                }
            }
            .overlay {
                Canvas { context, size in
                    let center = CGPoint(x: size.width / 2, y: size.height / 2)
                    let side = min(size.width, size.height)
                    let inner = side * (0.22 + 0.03 * levels.average(0..<5))
                    let count = levels.bands.count * 2
                    for i in 0..<count {
                        let band = levels.bands[i < levels.bands.count ? i : count - 1 - i]
                        let angle = Double(i) / Double(count) * 2 * .pi - .pi / 2
                        let length = side * 0.014 + side * 0.18 * CGFloat(band)
                        var bar = Path()
                        bar.move(to: CGPoint(x: center.x + cos(angle) * inner, y: center.y + sin(angle) * inner))
                        bar.addLine(to: CGPoint(x: center.x + cos(angle) * (inner + length), y: center.y + sin(angle) * (inner + length)))
                        context.stroke(bar, with: .color(.white.opacity(0.25 + 0.6 * Double(band))), style: StrokeStyle(lineWidth: 2.4, lineCap: .round))
                    }
                }
            }
        }
        .background(.black)
        .accessibilityHidden(true)
    }
}

/// The visualizer over the whole screen (parties, a TV through AirPlay): the song's name at
/// its foot, Play/Pause, and Done. The screen stays on.
struct VisualizerScreen: View {
    @Environment(Store.self) private var store
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        let song = store.playback?.current
        ZStack(alignment: .bottom) {
            Visualizer().ignoresSafeArea()
            HStack(spacing: 14) {
                VStack(alignment: .leading, spacing: 2) {
                    Text(song?.title ?? "").font(.needle(.headline)).lineLimit(1)
                    Text(song?.artist ?? "").font(.needle(.subheadline)).foregroundStyle(.secondary).lineLimit(1)
                }
                Spacer()
                Button { store.toggle() } label: {
                    Image(systemName: store.playback?.playing == true ? "pause.fill" : "play.fill").font(.needle(.title2)).frame(width: 50, height: 50)
                }
                .buttonStyle(.glass)
                .buttonBorderShape(.circle)
                .accessibilityLabel(store.playback?.playing == true ? "Pause" : "Play")
            }
            .padding(20)
        }
        .overlay(alignment: .topTrailing) {
            Button("Done") { dismiss() }
                .buttonStyle(.glass)
                .padding(20)
        }
        .preferredColorScheme(.dark)
        .onAppear { UIApplication.shared.isIdleTimerDisabled = true }
        .onDisappear { UIApplication.shared.isIdleTimerDisabled = false }
    }
}

/// Your library in numbers: how much music, in what formats and quality, from which decades
/// and genres, the artists with the most songs, and the albums to fix.
struct NumbersView: View {
    @Environment(Store.self) private var store
    @State private var numbers: LibraryNumbers?

    var body: some View {
        ScrollView {
            if let n = numbers {
                VStack(alignment: .leading, spacing: 0) {
                    LazyVGrid(columns: [GridItem(.flexible(), spacing: 10), GridItem(.flexible(), spacing: 10)], spacing: 10) {
                        tile(Int(n.songs).formatted(), "songs")
                        tile(Int(n.albums).formatted(), "albums")
                        tile(Int(n.artists).formatted(), "artists")
                        tile(hours(n.duration), "of music")
                        if n.size > 0 { tile(ByteCountFormatter.string(fromByteCount: n.size, countStyle: .file), "on this iPhone") }
                        if n.serverSongs > 0 { tile(Int(n.serverSongs).formatted(), "from your server") }
                    }
                    .padding(.horizontal)
                    .padding(.top, 8)
                    SectionTitle(title: "Quality")
                    VStack(spacing: 12) {
                        share("Lossless", n.lossless, of: n.songs)
                        share("Hi-res", n.hires, of: n.songs)
                    }
                    .padding(.horizontal).padding(.top, 8)
                    if !n.formats.isEmpty { SectionTitle(title: "Formats"); bars(n.formats) }
                    if !n.decades.isEmpty {
                        SectionTitle(title: "Decades")
                        Chart(n.decades, id: \.name) { decade in
                            BarMark(x: .value("Decade", decade.name), y: .value("Songs", decade.count))
                                .foregroundStyle(store.colors.accent.gradient)
                                .cornerRadius(4)
                        }
                        .chartYAxis(.hidden)
                        .frame(height: 150)
                        .padding(.horizontal).padding(.top, 8)
                    }
                    if !n.genres.isEmpty { SectionTitle(title: "Genres"); bars(n.genres) }
                    if !n.artistsBySongs.isEmpty { SectionTitle(title: "Artists with the most songs"); bars(n.artistsBySongs) }
                    shelf("Albums without a cover", n.albumsWithoutCover, n.withoutCover)
                    shelf("Albums in low quality", n.lowQualityAlbums, n.lowQuality, detail: "Every song lossy and under 192 kbit/s")
                }
                .padding(.bottom, 24)
            } else {
                ProgressView().padding(.top, 80)
            }
        }
        .navigationTitle("Library in numbers")
        .themed()
        .task(id: store.libraryVersion) { numbers = await store.load { try $0.libraryNumbers() } }
    }

    private func tile(_ value: String, _ label: String) -> some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(value).font(.needle(.title2).bold()).monospacedDigit().lineLimit(1).minimumScaleFactor(0.6)
            Text(label).font(.needle(.subheadline)).foregroundStyle(.secondary)
        }
        .padding(14)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(store.colors.card, in: .rect(cornerRadius: 14))
        .accessibilityElement(children: .combine)
    }

    private func share(_ name: String, _ part: UInt32, of whole: UInt32) -> some View {
        let fraction = whole > 0 ? Double(part) / Double(whole) : 0
        return VStack(alignment: .leading, spacing: 4) {
            HStack {
                Text(name)
                Spacer()
                Text(fraction.formatted(.percent.precision(.fractionLength(0)))).monospacedDigit().foregroundStyle(.secondary)
            }
            meter(fraction)
        }
        .accessibilityElement(children: .combine)
    }

    private func bars(_ items: [Count]) -> some View {
        let most = max(items.map(\.count).max() ?? 1, 1)
        return VStack(spacing: 10) {
            ForEach(items.prefix(10), id: \.name) { item in
                VStack(alignment: .leading, spacing: 3) {
                    HStack {
                        Text(item.name).lineLimit(1)
                        Spacer()
                        Text(Int(item.count).formatted()).monospacedDigit().foregroundStyle(.secondary)
                    }
                    meter(Double(item.count) / Double(most))
                }
                .accessibilityElement(children: .combine)
            }
        }
        .padding(.horizontal).padding(.top, 8)
    }

    private func meter(_ fraction: Double) -> some View {
        GeometryReader { geo in
            ZStack(alignment: .leading) {
                Capsule().fill(store.colors.card)
                Capsule().fill(store.colors.accent).frame(width: max(4, geo.size.width * fraction))
            }
        }
        .frame(height: 6)
        .accessibilityHidden(true)
    }

    @ViewBuilder
    private func shelf(_ title: String, _ total: UInt32, _ albums: [Album], detail: String? = nil) -> some View {
        if total > 0 {
            SectionTitle(title: "\(title) · \(Int(total).formatted())", detail: detail)
            Shelf { ForEach(albums) { AlbumTile(album: $0) } }
        }
    }

    private func hours(_ seconds: Double) -> String {
        let hours = Int(seconds / 3600)
        return hours >= 48 ? "\((hours / 24).formatted()) days" : "\(hours.formatted()) hours"
    }
}

/// Every measured song as a dot, by dark ↔ bright and calm ↔ lively; draw a circle with a
/// finger over an area to play the songs in it.
struct MoodMapView: View {
    @Environment(Store.self) private var store
    @State private var points: [MoodPoint]?
    /// The circle being drawn: its centre and edge, in the map's 0-to-1 places.
    @State private var circle: (CGPoint, CGPoint)?

    var body: some View {
        VStack(spacing: 14) {
            if let points, points.isEmpty {
                ContentUnavailableView("No songs measured yet", systemImage: "circle.hexagongrid",
                                       description: Text("Needle measures each song's sound for radio first. The map fills in as it goes."))
            } else {
                Text("Draw a circle to play that part of your music.")
                    .font(.needle(.subheadline)).foregroundStyle(.secondary)
                GeometryReader { geo in
                    let side = min(geo.size.width, geo.size.height)
                    map(points ?? [], side: side)
                        .frame(width: side, height: side)
                        .frame(maxWidth: .infinity, maxHeight: .infinity)
                }
                .padding(.horizontal, 30)
            }
        }
        .padding(.vertical)
        .navigationTitle("Mood map")
        .themed()
        .task(id: store.libraryVersion) { points = await store.load { try $0.moodMap() } ?? [] }
    }

    private func map(_ points: [MoodPoint], side: CGFloat) -> some View {
        ZStack {
            RoundedRectangle(cornerRadius: 18).fill(store.colors.card)
            Canvas { context, size in
                for p in points {
                    let at = CGPoint(x: CGFloat(p.x) * size.width, y: (1 - CGFloat(p.y)) * size.height)
                    let color = Color(hue: 0.62 - 0.55 * Double(p.x), saturation: 0.55 + 0.3 * Double(p.y), brightness: 0.55 + 0.4 * Double(p.x))
                    context.fill(Path(ellipseIn: CGRect(x: at.x - 2.5, y: at.y - 2.5, width: 5, height: 5)), with: .color(color.opacity(0.85)))
                }
                if let (center, edge) = circle {
                    let c = CGPoint(x: center.x * size.width, y: center.y * size.height)
                    let r = hypot((edge.x - center.x) * size.width, (edge.y - center.y) * size.height)
                    let ring = Path(ellipseIn: CGRect(x: c.x - r, y: c.y - r, width: r * 2, height: r * 2))
                    context.fill(ring, with: .color(store.colors.accent.opacity(0.15)))
                    context.stroke(ring, with: .color(store.colors.accent), lineWidth: 2)
                }
            }
            .padding(10)
            .contentShape(.rect)
            .gesture(DragGesture(minimumDistance: 0)
                .onChanged { value in
                    let inner = side - 20
                    let place = { (p: CGPoint) in CGPoint(x: min(max(0, (p.x - 10) / inner), 1), y: min(max(0, (p.y - 10) / inner), 1)) }
                    circle = (place(value.startLocation), place(value.location))
                }
                .onEnded { _ in play() })
            VStack { Text("Lively"); Spacer(); Text("Calm") }
                .font(.needle(.caption).weight(.semibold)).foregroundStyle(.secondary)
                .padding(.vertical, -22)
                .allowsHitTesting(false)
            HStack { Text("Dark").rotationEffect(.degrees(-90)).fixedSize(); Spacer(); Text("Bright").rotationEffect(.degrees(90)).fixedSize() }
                .font(.needle(.caption).weight(.semibold)).foregroundStyle(.secondary)
                .padding(.horizontal, -28)
                .allowsHitTesting(false)
        }
        .sensoryFeedback(.selection, trigger: circle == nil) { _, _ in store.app.haptics }
        .accessibilityElement(children: .ignore)
        .accessibilityLabel("Mood map of \(count(points.count, "song"))")
        .accessibilityAction(named: "Play calm songs") { play(at: CGPoint(x: 0.5, y: 0.85)) }
        .accessibilityAction(named: "Play lively songs") { play(at: CGPoint(x: 0.5, y: 0.15)) }
        .accessibilityAction(named: "Play dark songs") { play(at: CGPoint(x: 0.15, y: 0.5)) }
        .accessibilityAction(named: "Play bright songs") { play(at: CGPoint(x: 0.85, y: 0.5)) }
    }

    private func play() {
        guard let (center, edge) = circle else { return }
        play(at: center, radius: max(0.06, hypot(edge.x - center.x, edge.y - center.y)))
    }

    private func play(at center: CGPoint, radius: Double = 0.15) {
        // The map's y runs from lively at the top; the core's from calm (0) to lively (1).
        let (x, y, r) = (Float(center.x), Float(1 - center.y), Float(radius))
        Task {
            let songs = await store.ask { try $0.moodArea(x: x, y: y, radius: r, limit: 200) } ?? []
            store.message = songs.isEmpty ? "No songs there yet" : "Playing \(count(songs.count, "song")) from the mood map"
            if !songs.isEmpty { store.play(songs) }
        }
    }
}
