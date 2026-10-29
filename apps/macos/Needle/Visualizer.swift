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

/// Light that moves with the music, in the cover's colours: soft glows that swell with the
/// bass, the middle, and the highs, and a ring of thin bars around the middle. Still (one soft
/// glow) with Reduce motion.
struct Visualizer: View {
    private var store: Store { .shared }
    @State private var levels = Levels()

    /// The bands as the core gives them (it eases them itself: quick to rise, slow to fall).
    final class Levels {
        var bands = [Float](repeating: 0, count: 48)
        func follow(_ now: [Float]) {
            guard now.count == bands.count else { return }
            bands = now
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
