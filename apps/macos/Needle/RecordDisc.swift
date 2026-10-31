import SwiftUI

/// The big player's Record look, as on Android (and so on every system): the whole cover, round,
/// turning once in 12 seconds while `spinning`, with fine dark grooves and a soft sheen over it,
/// the label's hole in the middle, and the cream arm resting on it at the top right, as on
/// Needle's icon.
struct RecordDisc<Label: View>: View {
    let side: CGFloat
    let spinning: Bool
    @ViewBuilder let label: () -> Label

    private static var cream: Color { Color(red: 0xF2 / 255, green: 0xE8 / 255, blue: 0xD6 / 255) }

    var body: some View {
        ZStack {
            // The shadow: a soft dark glow behind the record, a little lower.
            Circle()
                .fill(RadialGradient(stops: [.init(color: .black.opacity(0.55), location: 0.55), .init(color: .clear, location: 1)],
                                     center: .center, startRadius: 0, endRadius: side * 0.48))
                .frame(width: side * 0.96, height: side * 0.96)
                .offset(y: 18)
            RecordSpin(on: spinning, period: 12) {
                label()
                    .frame(width: side, height: side)
                    .clipShape(Circle())
            }
            // Round grooves look the same at any turn: they stay, with the sheen.
            grooves
            // The label's hole.
            Circle().fill(.black.opacity(0.55)).frame(width: 34, height: 34)
            Circle().fill(Color(red: 0x0F / 255, green: 0x0E / 255, blue: 0x0D / 255)).frame(width: 10, height: 10)
            arm
        }
        .frame(width: side, height: side)
        .accessibilityHidden(true)
    }

    /// Fourteen fine dark rings from near the edge in to the label, and a diagonal sheen.
    private var grooves: some View {
        Canvas { context, size in
            let r = size.width / 2
            let middle = CGPoint(x: r, y: r)
            var k: CGFloat = 0.95
            while k > 0.36 {
                let ring = CGRect(x: middle.x - r * k, y: middle.y - r * k, width: 2 * r * k, height: 2 * r * k)
                context.stroke(Path(ellipseIn: ring), with: .color(.black.opacity(0.16)), lineWidth: 1.2)
                k -= 0.045
            }
            context.fill(Path(ellipseIn: CGRect(origin: .zero, size: size)),
                         with: .linearGradient(Gradient(colors: [.white.opacity(0.14), .clear, .white.opacity(0.08)]),
                                               startPoint: .zero, endPoint: CGPoint(x: size.width, y: size.height)))
        }
        .frame(width: side, height: side)
        .allowsHitTesting(false)
    }

    /// The arm: cream, from its pivot at the top right to the outer grooves, with its shadow.
    private var arm: some View {
        // Drawn with room around the record: the pivot reaches a little past its edge.
        Canvas { context, size in
            let w = side
            context.translateBy(x: (size.width - side) / 2, y: (size.height - side) / 2)
            let pivot = CGPoint(x: w * 0.96, y: w * 0.04)
            let tip = CGPoint(x: w * 0.73, y: w * 0.30)
            var shadow = Path()
            shadow.move(to: CGPoint(x: pivot.x, y: pivot.y + 6))
            shadow.addLine(to: CGPoint(x: tip.x, y: tip.y + 6))
            context.stroke(shadow, with: .color(.black.opacity(0.3)), style: StrokeStyle(lineWidth: w * 0.035, lineCap: .round))
            var line = Path()
            line.move(to: pivot)
            line.addLine(to: tip)
            context.stroke(line, with: .color(Self.cream), style: StrokeStyle(lineWidth: w * 0.03, lineCap: .round))
            context.fill(Path(ellipseIn: CGRect(x: pivot.x - w * 0.045, y: pivot.y - w * 0.045, width: w * 0.09, height: w * 0.09)), with: .color(Self.cream))
            context.fill(Path(ellipseIn: CGRect(x: pivot.x - w * 0.018, y: pivot.y - w * 0.018, width: w * 0.036, height: w * 0.036)),
                         with: .color(Color(red: 0x1B / 255, green: 0x18 / 255, blue: 0x16 / 255)))
        }
        .frame(width: side * 1.2, height: side * 1.2)
        .allowsHitTesting(false)
    }
}

/// Turns its content slowly, as a record on a turntable, while `on`.
struct RecordSpin<Content: View>: View {
    let on: Bool
    /// Seconds for one turn.
    var period: Double = 12
    /// Frames a second, at most.
    var rate: Double = 30
    @ViewBuilder let content: () -> Content
    /// When it began turning this time; nil while still.
    @State private var start: Date?
    /// The turn before this time.
    @State private var held: Double = 0

    var body: some View {
        TimelineView(.animation(minimumInterval: 1 / rate, paused: !on)) { context in
            // From the state alone, so the frame on which `on` changes does not jump.
            let turned = held + (start.map { max(0, context.date.timeIntervalSince($0)) } ?? 0) * 360 / period
            content().rotationEffect(.degrees(turned))
        }
        .onChange(of: on, initial: true) { _, now in
            if now {
                if start == nil { start = Date() }
            } else if let began = start {
                held += Date().timeIntervalSince(began) * 360 / period
                start = nil
            }
        }
    }
}
