import SwiftUI

/// Songs' loud and quiet shapes, as the core measures them (about a second for a song not
/// measured before), kept while Needle runs.
@MainActor
enum Waveforms {
    private static var kept: [String: [Float]] = [:]

    static func heights(_ id: String, store: Store) async -> [Float] {
        if let known = kept[id] { return known }
        let core = store.core
        var found = await Task.detached { (try? core.waveform(id: id, count: 140)) ?? [] }.value
        // A song measured as silence (or not read) keeps the plain slider, not a row of dots.
        if (found.max() ?? 0) < 0.02 { found = [] }
        // Kept only once there is a shape: none yet (a server song not wholly here) is asked again.
        if !found.isEmpty { kept[id] = found }
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

/// Light that moves with the music, as on Windows and Android: a soft glow in the middle that
/// swells with the bass, in the music's colours, and 96 rays all round, each as long as its
/// band is loud. A square, drawn in the cover's place; still with Reduce motion.
struct Visualizer: View {
    private var store: Store { .shared }

    /// The bands drawn, as on Windows and Android: 48, mirrored down both sides.
    private static let bands = 48

    var body: some View {
        let (inner, outer) = (store.colors.accent, store.colors.glow)
        TimelineView(.animation(minimumInterval: 1 / 30, paused: !store.motion)) { _ in
            Canvas { g, size in
                // Straight from the core: it eases the spectrum itself (quick up, slow down).
                let bands = store.core.spectrum(bands: UInt32(Self.bands))
                guard bands.count == Self.bands else { return }
                func average(_ range: Range<Int>) -> CGFloat {
                    min(max(CGFloat(bands[range].reduce(0, +) / Float(range.count)), 0), 1)
                }
                let bass = average(0..<6), level = average(0..<Self.bands)
                let c = CGPoint(x: size.width / 2, y: size.height / 2)
                let r = min(size.width, size.height) / 2
                // The glow: rings from wide and faint to small and bright, a soft light that
                // swells with the bass.
                let glow = r * (0.55 + 0.45 * bass)
                for i in 0..<14 {
                    let t = CGFloat(i) / 14
                    let radius = glow * (1 - 0.85 * t)
                    let alpha = (0.015 + 0.03 * level) + (0.02 + 0.05 * bass) * t
                    g.fill(Path(ellipseIn: CGRect(x: c.x - radius, y: c.y - radius, width: radius * 2, height: radius * 2)),
                           with: .color(outer.mix(with: inner, by: Double(t)).opacity(Double(alpha))))
                }
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
}

/// The big player's seek bar as a wave, a line, or a thick bar (Settings › Appearance › Seek
/// bar), as on Android and Windows: what is still to play is a straight bar; what has played is
/// the same in the lit colour, or, for Wave, a line that waves while the music plays (flat while
/// it rests, and with Reduce motion).
struct StyledSeekLine: View {
    /// "wave", "line", or "thick".
    let style: String
    /// Where the song is, 0 to 1.
    let fraction: Double
    /// The wave moves: the music plays and motion is allowed.
    let moving: Bool
    let lit: Color
    let dim: Color

    var body: some View {
        TimelineView(.animation(minimumInterval: 1 / 30, paused: !(moving && style == "wave"))) { context in
            Canvas { g, size in
                let thick: CGFloat = style == "thick" ? 10 : style == "wave" ? 3 : 4
                let middle = size.height / 2
                let split = size.width * min(max(fraction, 0), 1)
                // What is still to play.
                let rest = max(0, split - thick / 2)
                g.fill(Path(roundedRect: CGRect(x: rest, y: middle - thick / 2, width: size.width - rest, height: thick), cornerRadius: thick / 2),
                       with: .color(dim))
                guard split > 0 else { return }
                if style == "wave" {
                    // About 34 points a wave, 3.5 high, moving at 6 radians a second.
                    let amplitude: CGFloat = moving ? 3.5 : 0
                    let phase = moving ? (context.date.timeIntervalSinceReferenceDate * 6).truncatingRemainder(dividingBy: 2 * .pi) : 0
                    var line = Path()
                    var x: CGFloat = 0
                    while x <= split {
                        let point = CGPoint(x: x, y: middle + amplitude * CGFloat(sin(Double(x) / 34 * 2 * .pi - phase)))
                        if x == 0 { line.move(to: point) } else { line.addLine(to: point) }
                        x += 1
                    }
                    g.stroke(line, with: .color(lit), style: StrokeStyle(lineWidth: thick, lineCap: .round, lineJoin: .round))
                } else {
                    g.fill(Path(roundedRect: CGRect(x: 0, y: middle - thick / 2, width: max(split, thick), height: thick), cornerRadius: thick / 2),
                           with: .color(lit))
                }
            }
        }
        .accessibilityHidden(true)
    }
}
