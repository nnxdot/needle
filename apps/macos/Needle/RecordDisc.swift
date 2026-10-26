import SwiftUI

/// The big player's Record look: a black record with its grooves, the cover as its label, the
/// light on it, and the arm resting on it, as on Needle's icon. It turns while `spinning`.
struct RecordDisc<Label: View>: View {
    let side: CGFloat
    let spinning: Bool
    @ViewBuilder let label: () -> Label

    var body: some View {
        ZStack {
            // One turn in eight seconds: slow, so the label can still be seen.
            RecordSpin(on: spinning, period: 8) {
                ZStack {
                    Circle().fill(RadialGradient(colors: [Color(white: 0.13), Color(white: 0.05)], center: .center, startRadius: 0, endRadius: side / 2))
                    // The grooves: fine rings, a few stronger, between the label and the edge.
                    ForEach(0..<16, id: \.self) { i in
                        Circle()
                            .stroke(.white.opacity(i % 4 == 0 ? 0.08 : 0.035), lineWidth: i % 4 == 0 ? 1.4 : 0.8)
                            .frame(width: side * (0.46 + Double(i) * 0.031), height: side * (0.46 + Double(i) * 0.031))
                    }
                    label()
                        .frame(width: side * 0.4, height: side * 0.4)
                        .clipShape(Circle())
                        .overlay(Circle().stroke(.black.opacity(0.35), lineWidth: 2))
                    Circle().fill(Color(white: 0.85)).frame(width: side * 0.022, height: side * 0.022)
                }
                .frame(width: side, height: side)
            }
            // The light on the record stays where it is while the record turns.
            Circle()
                .fill(AngularGradient(colors: [.clear, .white.opacity(0.09), .clear, .clear, .white.opacity(0.06), .clear, .clear], center: .center))
                .frame(width: side, height: side)
                .allowsHitTesting(false)
            arm
        }
        .frame(width: side, height: side)
        .shadow(color: .black.opacity(0.45), radius: side * 0.06, y: side * 0.04)
        .accessibilityHidden(true)
    }

    /// The tonearm: its pivot beyond the record's top right, resting near the outer grooves.
    private var arm: some View {
        Canvas { context, size in
            let s = size.width
            let pivot = CGPoint(x: s * 0.97, y: s * 0.05)
            let rest = CGPoint(x: s * 0.74, y: s * 0.44)
            var line = Path()
            line.move(to: pivot)
            line.addLine(to: rest)
            context.stroke(line, with: .color(Color(red: 0.949, green: 0.91, blue: 0.839)), style: StrokeStyle(lineWidth: s * 0.022, lineCap: .round))
            let head = CGRect(x: rest.x - s * 0.025, y: rest.y - s * 0.012, width: s * 0.05, height: s * 0.05)
            context.fill(Path(roundedRect: head, cornerRadius: s * 0.008), with: .color(Color(red: 0.949, green: 0.91, blue: 0.839)))
            let knob = CGRect(x: pivot.x - s * 0.05, y: pivot.y - s * 0.05, width: s * 0.1, height: s * 0.1)
            context.fill(Path(ellipseIn: knob), with: .color(Color(red: 0.949, green: 0.91, blue: 0.839)))
            context.fill(Path(ellipseIn: knob.insetBy(dx: s * 0.03, dy: s * 0.03)), with: .color(Color(red: 0.106, green: 0.094, blue: 0.086)))
        }
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
