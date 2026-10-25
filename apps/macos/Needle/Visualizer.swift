import SwiftUI

/// Songs' loud and quiet shapes, as the core measures them (about a second for a song not
/// measured before), kept while Needle runs.
@MainActor
enum Waveforms {
    private static var kept: [String: [Float]] = [:]

    static func heights(_ id: String, store: Store) async -> [Float] {
        if let known = kept[id] { return known }
        let core = store.core
        let found = await Task.detached { core.waveformHeights(id: id, count: 140) }.value
        kept[id] = found
        if kept.count > 200 { kept.removeAll() }
        return found
    }
}

/// The seek bar as the song's real shape: a bar for each part, the played ones lit. A click
/// or a drag goes there.
struct WaveformSeek: View {
    let heights: [Float]
    /// Where the song is, 0 to 1.
    let fraction: Double
    let lit: Color
    let dim: Color
    /// The place under the pointer while it is held (0 to 1), then nil when let go.
    let scrub: (Double?) -> Void

    var body: some View {
        GeometryReader { geo in
            Canvas { context, size in
                let count = heights.count
                guard count > 0 else { return }
                let step = size.width / CGFloat(count)
                for (i, height) in heights.enumerated() {
                    let h = max(2, CGFloat(height) * size.height)
                    let rect = CGRect(x: CGFloat(i) * step + step * 0.18, y: (size.height - h) / 2, width: step * 0.64, height: h)
                    let played = (Double(i) + 0.5) / Double(count) <= fraction
                    context.fill(Path(roundedRect: rect, cornerRadius: step * 0.32), with: .color(played ? lit : dim))
                }
            }
            .contentShape(Rectangle())
            .gesture(DragGesture(minimumDistance: 0)
                .onChanged { value in scrub(min(max(0, value.location.x / max(geo.size.width, 1)), 1)) }
                .onEnded { _ in scrub(nil) })
        }
    }
}

/// Light that moves with the music, in the cover's colours: soft glows that swell with the
/// bass, the middle, and the highs, and a ring of thin bars around the middle. Still (one soft
/// glow) with Reduce motion.
struct Visualizer: View {
    private var store: Store { .shared }
    @State private var levels = Levels()

    /// The bands as they move: rising at once, falling slowly.
    final class Levels {
        var bands = [Float](repeating: 0, count: 48)
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
        let colors = store.colors
        let palette = [colors.glow, colors.accent, colors.glow.opacity(0.8)]
        TimelineView(.animation(minimumInterval: 1 / 30, paused: !store.motion)) { timeline in
            let t = timeline.date.timeIntervalSinceReferenceDate
            Canvas { context, size in
                if store.motion { levels.follow(store.core.spectrum(bands: 48)) }
                let center = CGPoint(x: size.width / 2, y: size.height / 2)
                let side = min(size.width, size.height)
                let bass = levels.average(0..<6), middle = levels.average(10..<26), high = levels.average(30..<48)
                // The glows, slowly turning about the middle.
                context.addFilter(.blur(radius: side * 0.08))
                for (i, energy) in [bass, middle, high].enumerated() {
                    let angle = t * (0.07 + Double(i) * 0.03) + Double(i) * 2.1
                    let reach = side * (0.12 + 0.06 * CGFloat(i))
                    let at = CGPoint(x: center.x + cos(angle) * reach, y: center.y + sin(angle) * reach * 0.7)
                    let radius = side * (0.22 + 0.28 * energy) * (i == 0 ? 1.2 : 1)
                    context.fill(Path(ellipseIn: CGRect(x: at.x - radius, y: at.y - radius, width: radius * 2, height: radius * 2)),
                                 with: .color(palette[i].opacity(0.35 + 0.55 * Double(energy))))
                }
            }
            .overlay {
                // The ring of bars, sharp, over the glows.
                Canvas { context, size in
                    let center = CGPoint(x: size.width / 2, y: size.height / 2)
                    let side = min(size.width, size.height)
                    let bass = levels.average(0..<6)
                    let inner = side * (0.2 + 0.03 * bass)
                    let count = levels.bands.count * 2
                    for i in 0..<count {
                        // Mirrored: the lows at the top and bottom, the highs at the sides.
                        let band = levels.bands[i < levels.bands.count ? i : count - 1 - i]
                        let angle = Double(i) / Double(count) * 2 * .pi - .pi / 2
                        let length = side * 0.012 + side * 0.16 * CGFloat(band)
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
