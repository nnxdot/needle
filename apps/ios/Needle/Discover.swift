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
        // Kept only once there is a shape: none yet (a server song not wholly here) is asked again.
        if !found.isEmpty { kept[id] = found }
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

/// Light that moves with the music, as on Android: a bright glow in the middle that swells with
/// the bass, and 96 rays all round, each as long as its band is loud, in the colours of the
/// cover playing. A square, drawn in the cover's place; still with Reduce motion.
struct Visualizer: View {
    @Environment(Store.self) private var store
    @Environment(\.accessibilityReduceMotion) private var systemReduceMotion

    /// The bands drawn, as on Windows and Android: 48, mirrored down both sides.
    private static let bands = 48

    var body: some View {
        let (inner, outer) = Self.lights(UIColor(store.coverColor ?? store.colors.accent))
        TimelineView(.animation(minimumInterval: 1 / 30, paused: systemReduceMotion || store.app.reduceMotion)) { timeline in
            Canvas { g, size in
                // Drawn anew each frame of the timeline.
                _ = timeline.date
                // Straight from the core: it eases the spectrum itself (quick up, slow down).
                let bands = store.core.spectrum(bands: UInt32(Self.bands))
                guard bands.count == Self.bands else { return }
                func average(_ range: Range<Int>) -> CGFloat {
                    min(max(CGFloat(bands[range].reduce(0, +) / Float(range.count)), 0), 1)
                }
                let bass = average(0..<6), level = average(0..<Self.bands)
                let c = CGPoint(x: size.width / 2, y: size.height / 2)
                let r = min(size.width, size.height) / 2
                // The glow, as on Android: larger and brighter with the bass.
                let glow = r * (0.55 + 0.45 * bass)
                g.fill(Path(ellipseIn: CGRect(x: c.x - glow, y: c.y - glow, width: glow * 2, height: glow * 2)),
                       with: .radialGradient(Gradient(colors: [inner.opacity(0.25 + 0.55 * bass), outer.opacity(0.12 + 0.3 * level), .clear]),
                                             center: c, startRadius: 0, endRadius: glow))
                // The rays: 96 all round, the low bands at the top, each as long as its band
                // is loud.
                let base = r * 0.34
                let rays = Self.bands * 2
                let stroke = max(2, 2 * .pi * base / CGFloat(rays) * 0.55)
                for i in 0..<rays {
                    let band = i < Self.bands ? i : rays - 1 - i
                    let v = min(max(CGFloat(bands[band]), 0), 1)
                    let angle = CGFloat(i) / CGFloat(rays) * 2 * .pi - .pi / 2
                    let length = base + r * 0.62 * (0.06 + v)
                    var ray = Path()
                    ray.move(to: CGPoint(x: c.x + cos(angle) * base, y: c.y + sin(angle) * base))
                    ray.addLine(to: CGPoint(x: c.x + cos(angle) * length, y: c.y + sin(angle) * length))
                    let color = inner.mix(with: outer, by: Double(band) / Double(Self.bands)).opacity(0.55 + 0.45 * Double(v))
                    g.stroke(ray, with: .color(color), style: StrokeStyle(lineWidth: stroke, lineCap: .round))
                }
            }
        }
        .aspectRatio(1, contentMode: .fit)
        .accessibilityHidden(true)
    }

    /// The light's two colours from the cover's, as Android's cover colours are: the cover's
    /// own hue made bright, and a hue a sixth of the way round. A grey cover stays grey.
    static func lights(_ cover: UIColor) -> (Color, Color) {
        var h: CGFloat = 0, s: CGFloat = 0, b: CGFloat = 0, a: CGFloat = 0
        cover.getHue(&h, saturation: &s, brightness: &b, alpha: &a)
        let saturation = s < 0.08 ? s : min(max(s, 0.45), 0.75)
        let inner = UIColor(hue: h, saturation: saturation, brightness: 0.95, alpha: 1)
        let outer = UIColor(hue: (h + 1 / 6).truncatingRemainder(dividingBy: 1), saturation: saturation, brightness: 0.85, alpha: 1)
        return (Color(uiColor: inner), Color(uiColor: outer))
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
            CoverBackdrop(path: song?.artwork, seed: (song?.album ?? "") + (song?.artist ?? ""), moving: false)
                .ignoresSafeArea()
            Visualizer()
                .padding(24)
                .frame(maxWidth: .infinity, maxHeight: .infinity)
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
