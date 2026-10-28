import SwiftUI

/// Two of the player's tools, each a small pane over the player, as on Windows: the sleep timer
/// (a moon, lit while it runs, with the time left on hover) and speed and pitch. Both are the
/// core's; this is only their look.

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

/// The sleep timer's moon: lit while it runs, with the time left on hover.
struct SleepButton: View {
    private var store: Store { .shared }

    var body: some View {
        let colors = store.colors
        let running = store.sleep
        Button { store.tool = store.tool == "sleep" ? nil : "sleep" } label: {
            Image(systemName: running == nil ? "moon" : "moon.fill")
                .foregroundStyle(running == nil ? colors.ink2 : colors.accent)
                .frame(width: 26, height: 26)
                .background(running == nil ? Color.clear : colors.accent.opacity(0.18), in: RoundedRectangle(cornerRadius: 7, style: .continuous))
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .help(running.map { "Sleep timer · \(sleepClock($0.remaining)) left" } ?? "Sleep timer")
        .accessibilityLabel("Sleep timer")
        .accessibilityValue(running.map { "\(sleepClock($0.remaining)) left" } ?? "Off")
        .popover(isPresented: Binding(get: { store.tool == "sleep" }, set: { if !$0 && store.tool == "sleep" { store.tool = nil } }), arrowEdge: .top) { SleepPane() }
    }
}

/// Speed and pitch: lit when a song plays faster, slower, higher, or lower.
struct SpeedButton: View {
    private var store: Store { .shared }

    var body: some View {
        let colors = store.colors
        let tempo = store.tempo
        let changed = !tempo.neutral && tempo.available
        Button { store.tool = store.tool == "speed" ? nil : "speed" } label: {
            Image(systemName: "gauge.with.dots.needle.67percent")
                .foregroundStyle(changed ? colors.accent : colors.ink2)
                .frame(width: 26, height: 26)
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .help(changed ? "Speed and pitch · \(speedName(tempo.speed))" : "Speed and pitch")
        .accessibilityLabel("Speed and pitch")
        .accessibilityValue(speedName(tempo.speed))
        .popover(isPresented: Binding(get: { store.tool == "speed" }, set: { if !$0 && store.tool == "speed" { store.tool = nil } }), arrowEdge: .top) { SpeedPane() }
    }
}

/// A choice in a tool's pane: lit while chosen.
private struct ToolChip: View {
    private var store: Store { .shared }
    let label: String
    let on: Bool
    var enabled = true
    let action: () -> Void

    var body: some View {
        let colors = store.colors
        Button(action: action) {
            Text(label)
                .font(.system(size: 13))
                .foregroundStyle(on ? colors.accentInk : enabled ? colors.ink : colors.ink3)
                .padding(.horizontal, 12).padding(.vertical, 5)
                .background(on ? colors.accent : colors.raisedHover, in: Capsule())
                .contentShape(Capsule())
        }
        .buttonStyle(.plain)
        .disabled(!enabled)
        .accessibilityAddTraits(on ? .isSelected : [])
    }
}

/// A row of a tool's pane: its name (and a line under it), and its control.
private struct ToolRow<Control: View>: View {
    private var store: Store { .shared }
    let title: String
    var line: String?
    @ViewBuilder let control: Control

    var body: some View {
        HStack(spacing: 12) {
            VStack(alignment: .leading, spacing: 1) {
                Text(title).font(.system(size: 13)).foregroundStyle(store.colors.ink)
                if let line { Text(line).font(.system(size: 11.5)).foregroundStyle(store.colors.ink3) }
            }
            Spacer(minLength: 0)
            control
        }
        .padding(.vertical, 8)
    }
}

/// The sleep timer's pane: the chips (a chip that runs turns it off), Fade out, and Then sleep
/// the Mac.
struct SleepPane: View {
    private var store: Store { .shared }
    @State private var fade = true

    /// The chips: their names, and what they start.
    private static let choices: [(String, String, UInt32)] = [
        ("15 min", "minutes", 15), ("30 min", "minutes", 30), ("45 min", "minutes", 45),
        ("1 hour", "minutes", 60), ("End of song", "end_of_song", 0), ("End of album", "end_of_album", 0),
    ]

    var body: some View {
        let colors = store.colors
        let running = store.sleep
        VStack(alignment: .leading, spacing: 0) {
            HStack(spacing: 8) {
                Image(systemName: "moon.fill").font(.system(size: 16)).foregroundStyle(colors.accent)
                Text("Sleep timer").font(.system(size: 15, weight: .bold)).foregroundStyle(colors.ink)
                Spacer()
                if let running {
                    Text(sleepClock(running.remaining)).font(.system(size: 13, design: .monospaced)).foregroundStyle(colors.accent)
                }
            }
            .padding(.bottom, 10)
            FlowLayout(spacing: 6) {
                ForEach(Self.choices, id: \.0) { label, kind, minutes in
                    let on = running.map { $0.kind == kind && (kind != "minutes" || $0.minutes == minutes) } ?? false
                    ToolChip(label: label, on: on) {
                        if on { store.cancelSleep() } else { store.startSleep(kind, minutes: minutes) }
                    }
                }
            }
            .padding(.bottom, 10)
            Divider()
            ToolRow(title: "Fade out", line: "Over the last minute") {
                Toggle("", isOn: Binding(get: { running?.fade ?? fade }, set: { fade = $0; store.setSleepFade($0) }))
                    .toggleStyle(.switch).labelsHidden().controlSize(.small)
            }
            Divider()
            ToolRow(title: "Then sleep the Mac") {
                Toggle("", isOn: Binding(get: { store.settings.sleepComputer }, set: { store.set(["sleep_computer": $0]) }))
                    .toggleStyle(.switch).labelsHidden().controlSize(.small)
            }
        }
        .padding(16)
        .frame(width: 290)
        .onAppear { fade = store.core.sleepFade() }
    }
}

/// Speed and pitch's pane: speed (with the chips) and pitch, Keep the pitch, and Only for this
/// song; off on exclusive output, where nothing may change the sound.
struct SpeedPane: View {
    private var store: Store { .shared }

    var body: some View {
        let colors = store.colors
        let tempo = store.tempo
        let available = tempo.available
        VStack(alignment: .leading, spacing: 0) {
            Text("Speed and pitch").font(.system(size: 15, weight: .bold)).foregroundStyle(colors.ink).padding(.bottom, 8)
            if !available {
                Text("Off while exclusive output is on, so the sound stays untouched.")
                    .font(.system(size: 12)).foregroundStyle(colors.ink3)
                    .fixedSize(horizontal: false, vertical: true)
                    .padding(.bottom, 6)
            }
            ToolRow(title: "Speed") {
                stepper(speedName(tempo.speed), enabled: available,
                        down: { change { $0.speed = (($0.speed - 0.05) * 20).rounded() / 20 } },
                        up: { change { $0.speed = (($0.speed + 0.05) * 20).rounded() / 20 } })
            }
            FlowLayout(spacing: 6) {
                ForEach(store.core.speedPresets(), id: \.self) { speed in
                    ToolChip(label: speedName(speed), on: available && abs(tempo.speed - speed) < 0.005, enabled: available) {
                        change { $0.speed = speed }
                    }
                }
            }
            .padding(.bottom, 8)
            ToolRow(title: "Pitch") {
                stepper(pitchName(tempo.semitones), enabled: available,
                        down: { change { $0.semitones -= 1 } }, up: { change { $0.semitones += 1 } })
            }
            Divider()
            ToolRow(title: "Keep the pitch", line: "Faster without the chipmunk voice") {
                Toggle("", isOn: Binding(get: { tempo.keepPitch }, set: { on in change { $0.keepPitch = on } }))
                    .toggleStyle(.switch).labelsHidden().controlSize(.small).disabled(!available)
            }
            ToolRow(title: "Only for this song") {
                Toggle("", isOn: Binding(get: { tempo.onlyThisSong }, set: { on in change { $0.onlyThisSong = on } }))
                    .toggleStyle(.switch).labelsHidden().controlSize(.small).disabled(!available)
            }
            if available && !tempo.neutral {
                Button("As recorded") { change { $0.speed = 1; $0.semitones = 0 } }
                    .buttonStyle(.plain)
                    .font(.system(size: 12.5, weight: .medium))
                    .foregroundStyle(colors.accent)
                    .padding(.top, 4)
            }
        }
        .padding(16)
        .frame(width: 290)
    }

    private func change(_ edit: (inout SpeedPitch) -> Void) {
        var next = store.tempo
        edit(&next)
        store.setTempo(next)
    }

    private func stepper(_ value: String, enabled: Bool, down: @escaping () -> Void, up: @escaping () -> Void) -> some View {
        let colors = store.colors
        return HStack(spacing: 4) {
            Button(action: down) { Image(systemName: "minus").frame(width: 22, height: 22).contentShape(Rectangle()) }
                .help("Less").accessibilityLabel("Less")
            Text(value)
                .font(.system(size: 13, weight: .semibold))
                .foregroundStyle(enabled ? colors.accent : colors.ink3)
                .frame(minWidth: 96)
            Button(action: up) { Image(systemName: "plus").frame(width: 22, height: 22).contentShape(Rectangle()) }
                .help("More").accessibilityLabel("More")
        }
        .buttonStyle(.plain)
        .foregroundStyle(colors.ink2)
        .disabled(!enabled)
    }
}
