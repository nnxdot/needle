import SwiftUI

/// Two of the player's tools, as sheets from its foot, after the phone mockups: the sleep timer
/// (a moon, lit while it runs, with the time left) and speed and pitch. Both are the core's;
/// this is only their look.

/// "24:12", or "1:02:05".
func sleepClock(_ seconds: Double) -> String {
    let s = Int(max(0, seconds).rounded(.up))
    return s >= 3600 ? String(format: "%d:%02d:%02d", s / 3600, s / 60 % 60, s % 60) : String(format: "%d:%02d", s / 60, s % 60)
}

/// "1.25×", "1×".
func speedName(_ speed: Float) -> String {
    var text = String(format: "%.2f", speed)
    while text.hasSuffix("0") { text.removeLast() }
    if text.hasSuffix(".") { text.removeLast() }
    return text + "×"
}

/// "+2 semitones", "−1 semitone", "+0 semitones".
func pitchName(_ semitones: Float) -> String {
    let value = (semitones * 10).rounded() / 10
    var number = String(format: "%.1f", abs(value))
    if number.hasSuffix(".0") { number.removeLast(2) }
    return (value < 0 ? "−" : "+") + number + (abs(value) == 1 ? " semitone" : " semitones")
}

extension SpeedPitch {
    /// The music plays as recorded.
    var neutral: Bool { abs(speed - 1) < 0.005 && abs(semitones) < 0.05 }
}

/// The moon in the player's foot: lit while the sleep timer runs, with the time left beside it.
struct SleepMoon: View {
    @Environment(Store.self) private var store

    var body: some View {
        let running = store.sleep
        Button { store.sheet = .sleep } label: {
            HStack(spacing: 5) {
                Image(systemName: running == nil ? "moon" : "moon.fill")
                    .font(.needle(.title3).weight(running == nil ? .regular : .semibold))
                if let running {
                    Text(sleepClock(running.remaining)).font(.needle(.footnote).monospacedDigit().weight(.semibold))
                }
            }
            .foregroundStyle(running == nil ? .white.opacity(0.7) : ThemeColors.amber)
            .frame(minWidth: 44, minHeight: 44)
            .padding(.horizontal, running == nil ? 0 : 8)
            // Glass only while the timer runs, in the cover's colour as the player's other lit
            // signs; none while it is off.
            .glassEffect(running == nil ? .identity : .regular.tint((store.coverColor ?? .clear).opacity(0.3)).interactive(), in: .capsule)
            .contentShape(Capsule())
        }
        .buttonStyle(.plain)
        .hoverEffect(.highlight)
        .toolPopover(["sleep"])
        .accessibilityLabel("Sleep timer")
        .accessibilityValue(running.map { "\(sleepClock($0.remaining)) left" } ?? "Off")
    }
}

/// A choice in a tool's sheet: lit while chosen; the lit glass glides from the chip chosen
/// before to this one (`space`).
private struct ToolChip: View {
    @Environment(Store.self) private var store
    let label: String
    let on: Bool
    var enabled = true
    let space: Namespace.ID
    let action: () -> Void

    var body: some View {
        Button { withMotion(.smooth(duration: 0.35), action) } label: {
            Text(label)
                .font(.needle(.subheadline).weight(.medium))
                .foregroundStyle(on ? AnyShapeStyle(store.colors.onAccent) : enabled ? AnyShapeStyle(.primary) : AnyShapeStyle(.tertiary))
                .padding(.horizontal, 14).padding(.vertical, 8)
                .glassEffect(on ? .regular.tint(store.colors.accent).interactive() : .regular.interactive(), in: .capsule)
                .glassEffectID(on ? "chosen" : label, in: space)
                .contentShape(Capsule())
        }
        .buttonStyle(.plain)
        .disabled(!enabled)
        .accessibilityAddTraits(on ? .isSelected : [])
    }
}

extension View {
    /// A half-height tool sheet: the system's glass shows through (no page is painted in it),
    /// with words in the sheet's own light or dark and the accent.
    func toolSheet(_ store: Store) -> some View {
        self
            .scrollContentBackground(.hidden)
            .tint(store.colors.accent)
    }
}

/// The sleep timer: a time or the end of the song or album, with a soft fade; Start, or Turn
/// off while it runs.
struct SleepSheet: View {
    @Environment(Store.self) private var store
    @Environment(\.dismiss) private var dismiss
    @State private var chosen = 1
    @State private var fade = true
    @Namespace private var chips

    /// The chips: their names, and what they start.
    private static let choices: [(String, String, UInt32)] = [
        ("15 min", "minutes", 15), ("30 min", "minutes", 30), ("45 min", "minutes", 45),
        ("1 hour", "minutes", 60), ("End of song", "end_of_song", 0), ("End of album", "end_of_album", 0),
    ]

    var body: some View {
        let colors = store.colors
        let running = store.sleep
        let choice = Self.choices[chosen]
        let isRunning = running.map { $0.kind == choice.1 && (choice.1 != "minutes" || $0.minutes == choice.2) } ?? false
        VStack(alignment: .leading, spacing: 16) {
            VStack(alignment: .leading, spacing: 4) {
                HStack(spacing: 8) {
                    Image(systemName: "moon.fill").foregroundStyle(ThemeColors.amber)
                    Text("Sleep timer").font(.needle(.title2).weight(.bold))
                    Spacer()
                    if let running {
                        Text(sleepClock(running.remaining)).font(.needle(.body).monospacedDigit()).foregroundStyle(ThemeColors.amber)
                    }
                }
                Text("Music stops softly when the time is up.").font(.needle(.subheadline)).foregroundStyle(.secondary)
            }
            GlassEffectContainer(spacing: 4) {
                FlowLayout(spacing: 8) {
                    ForEach(Array(Self.choices.enumerated()), id: \.offset) { i, item in
                        ToolChip(label: item.0, on: i == chosen, space: chips) { chosen = i }
                    }
                }
            }
            Divider()
            Toggle(isOn: Binding(get: { running?.fade ?? fade }, set: { fade = $0; store.setSleepFade($0) })) {
                VStack(alignment: .leading, spacing: 1) {
                    Text("Fade out")
                    Text("Over the last minute").font(.needle(.caption)).foregroundStyle(.secondary)
                }
            }
            .tint(colors.accent)
            Button {
                if isRunning { store.cancelSleep() } else { store.startSleep(choice.1, minutes: choice.2) }
                dismiss()
            } label: {
                Text(isRunning ? "Turn off" : startLabel(choice))
                    .font(.needle(.body).weight(.semibold))
                    .frame(maxWidth: .infinity).padding(.vertical, 8)
            }
            .buttonStyle(.glassProminent)
            .foregroundStyle(colors.onAccent)
            if running != nil && !isRunning {
                Button("Turn off", role: .destructive) { store.cancelSleep(); dismiss() }
                    .frame(maxWidth: .infinity)
            }
        }
        .padding(22)
        .frame(maxHeight: .infinity, alignment: .top)
        .presentationDetents([.medium])
        .toolSheet(store)
        .onAppear {
            fade = store.core.sleepFade()
            if let running, let i = Self.choices.firstIndex(where: { $0.1 == running.kind && ($0.1 != "minutes" || $0.2 == running.minutes) }) {
                chosen = i
            }
        }
    }

    /// "Start · stops at 22:18".
    private func startLabel(_ choice: (String, String, UInt32)) -> String {
        switch choice.1 {
        case "end_of_song": return "Start · after this song"
        case "end_of_album": return "Start · after this album"
        default:
            let at = Date().addingTimeInterval(TimeInterval(choice.2) * 60)
            return "Start · stops at \(at.formatted(date: .omitted, time: .shortened))"
        }
    }
}

/// Speed and pitch: a speed from 0.5× to 2× (with chips), a pitch from −12 to +12 semitones,
/// Keep the pitch, and Only for this song.
struct SpeedSheet: View {
    @Environment(Store.self) private var store
    @Namespace private var chips

    var body: some View {
        let colors = store.colors
        let tempo = store.tempo
        let available = tempo.available
        ScrollView {
            VStack(alignment: .leading, spacing: 14) {
                Text("Speed and pitch").font(.needle(.title2).weight(.bold))
                if !available {
                    Text("Off for this output, so the sound stays untouched.").font(.needle(.subheadline)).foregroundStyle(.secondary)
                }
                HStack {
                    Text("Speed").font(.needle(.body).weight(.semibold))
                    Spacer()
                    Text(speedName(tempo.speed)).font(.needle(.body).monospacedDigit()).foregroundStyle(ThemeColors.amber)
                }
                Slider(value: Binding(get: { Double(tempo.speed) }, set: { v in change { $0.speed = Float((v * 20).rounded() / 20) } }), in: 0.5...2)
                    .tint(colors.accent)
                    .accessibilityLabel("Speed")
                    .accessibilityValue(speedName(tempo.speed))
                GlassEffectContainer(spacing: 4) {
                    FlowLayout(spacing: 8) {
                        ForEach(store.core.speedPresets(), id: \.self) { speed in
                            ToolChip(label: speedName(speed), on: available && abs(tempo.speed - speed) < 0.005, enabled: available, space: chips) {
                                change { $0.speed = speed }
                            }
                        }
                    }
                }
                HStack {
                    Text("Pitch").font(.needle(.body).weight(.semibold))
                    Spacer()
                    Text(pitchName(tempo.semitones)).font(.needle(.body).monospacedDigit()).foregroundStyle(.secondary)
                }
                .padding(.top, 6)
                Slider(value: Binding(get: { Double(tempo.semitones) }, set: { v in change { $0.semitones = Float(v.rounded()) } }), in: -12...12, step: 1)
                    .tint(.secondary)
                    .accessibilityLabel("Pitch")
                    .accessibilityValue(pitchName(tempo.semitones))
                Toggle(isOn: Binding(get: { tempo.keepPitch }, set: { on in change { $0.keepPitch = on } })) {
                    VStack(alignment: .leading, spacing: 1) {
                        Text("Keep the pitch")
                        Text("Faster without the chipmunk voice").font(.needle(.caption)).foregroundStyle(.secondary)
                    }
                }
                Toggle("Only for this song", isOn: Binding(get: { tempo.onlyThisSong }, set: { on in change { $0.onlyThisSong = on } }))
                if available && !tempo.neutral {
                    Button("As recorded") { change { $0.speed = 1; $0.semitones = 0 } }
                        .font(.needle(.body).weight(.semibold))
                        .foregroundStyle(ThemeColors.amber)
                }
            }
            .tint(colors.accent)
            .disabled(!available)
            .padding(22)
        }
        .presentationDetents([.medium, .large])
        .toolSheet(store)
    }

    private func change(_ edit: (inout SpeedPitch) -> Void) {
        var next = store.tempo
        edit(&next)
        store.setTempo(next)
    }
}
