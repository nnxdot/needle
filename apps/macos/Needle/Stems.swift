import SwiftUI

/// A song's stems: split it into drums, bass, other, and vocals on this Mac, then play from
/// them with a live mix.
struct StemsPanel: View {
    private var store: Store { .shared }
    let track: Track
    let big: Bool
    @State private var info: StemsInfo?
    @State private var levels: [Float] = [1, 1, 1, 1]

    private static let names = ["drums", "bass", "other", "vocals"]
    /// Ready-made mixes: name, icon, and levels for drums, bass, other, vocals.
    private static let presets: [(String, String, [Float])] = [
        ("Full mix", "square.stack.3d.up", [1, 1, 1, 1]),
        ("Karaoke", "music.mic", [1, 1, 1, 0]),
        ("Vocals only", "person.wave.2", [0, 0, 0, 1]),
        ("No drums", "drum", [0, 1, 1, 1]),
        ("Bass only", "waveform", [0, 1, 0, 0]),
    ]

    var body: some View {
        let colors = store.colors
        let job = store.stemsJob.flatMap { $0.id == track.id ? $0 : nil }
        let busyElsewhere = store.stemsJob != nil && job == nil
        let ready = info?.ready == true
        let active = store.playback?.stems == track.id
        VStack(alignment: .leading, spacing: 10) {
            if big { Text("Stems · \(track.title)").font(.needle(.headline)).foregroundStyle(.white) }
            if !ready && job == nil {
                Text(info?.modelReady == true
                     ? "Split this song into drums, bass, vocals, and everything else, right on this Mac. It takes about half the song's length."
                     : "Split this song into drums, bass, vocals, and everything else, right on this Mac. The first time, Needle downloads the 166 MB separation model.")
                    .font(.needle(.callout)).foregroundStyle(big ? .white.opacity(0.75) : colors.ink2)
                    .fixedSize(horizontal: false, vertical: true)
                Button { split() } label: { Label("Split into stems", systemImage: "square.stack.3d.up") }
                    .glassButton(prominent: true)
                    .controlSize(.small)
                    .disabled(busyElsewhere)
            }
            if let job {
                VStack(alignment: .leading, spacing: 6) {
                    Text(job.stage).font(.needle(.callout))
                    ProgressView(value: Double(min(1, max(0, job.fraction))))
                    Button("Stop") { store.core.stopStems() }.controlSize(.small)
                }
            }
            if ready {
                Toggle(isOn: Binding(get: { active }, set: { on in
                    let id = track.id
                    store.run { try $0.playStems(id: id, on: on) }
                })) {
                    VStack(alignment: .leading, spacing: 1) {
                        Text("Play from stems").font(.needle(.callout).weight(.semibold))
                        Text(store.playback?.current == track.id ? "Changes apply as you listen" : "Starts this song from its stems")
                            .font(.needle(.caption)).foregroundStyle(.secondary)
                    }
                }
                .toggleStyle(.switch)
                ForEach(0..<4, id: \.self) { i in channel(i, active: active) }
                Text("Mixes").font(.needle(.caption)).foregroundStyle(.secondary)
                LazyVGrid(columns: [GridItem(.adaptive(minimum: 96), spacing: 6)], spacing: 6) {
                    ForEach(Self.presets, id: \.0) { name, icon, mix in
                        let on = zip(levels, mix).allSatisfy { abs($0 - $1) < 0.01 }
                        Button { set(mix) } label: {
                            Label(name, systemImage: icon).font(.needle(.caption)).lineLimit(1)
                                .frame(maxWidth: .infinity)
                                .padding(.vertical, 5)
                                .background(on ? colors.accent : colors.raised, in: Capsule())
                                .foregroundStyle(on ? colors.accentInk : colors.ink2)
                        }
                        .buttonStyle(.plain)
                    }
                }
                Button("Delete these stems") {
                    let id = track.id
                    store.job { try $0.deleteStems(id: id); return "Stems deleted. The song itself is untouched." }
                    Task { try? await Task.sleep(for: .milliseconds(300)); refresh() }
                }
                .buttonStyle(.link).font(.needle(.caption))
            }
            if big && !ready {
                Label("Stems use HT-Demucs, running locally. Nothing is uploaded.", systemImage: "info.circle")
                    .font(.needle(.caption)).foregroundStyle(.white.opacity(0.6))
            }
        }
        .task(id: "\(track.id)\(store.stemsJob == nil)") { refresh() }
    }

    private func refresh() {
        let found = store.core.stemsInfo(id: track.id)
        info = found
        if found.levels.count == 4 { levels = found.levels }
    }

    private func split() {
        let id = track.id
        store.run { try $0.splitStems(id: id) }
    }

    private func set(_ mix: [Float]) {
        for (i, level) in mix.enumerated() { store.core.setStemLevel(stem: UInt32(i), level: level) }
        levels = mix
    }

    /// One stem as a compact row: coloured icon, name and level, the slider, Solo and Mute.
    private func channel(_ i: Int, active: Bool) -> some View {
        let (label, icon, hue): (String, String, Double) = switch Self.names[i] {
        case "drums": ("Drums", "drum", 0.01)
        case "bass": ("Bass", "waveform", 0.74)
        case "vocals": ("Vocals", "music.mic", 0.12)
        default: ("Other", "sparkles", 0.47)
        }
        let color = Color(hue: hue, saturation: 0.62, brightness: store.colors.dark ? 0.9 : 0.6)
        let level = levels[i]
        let muted = level <= 0.001
        let soloed = !muted && levels.enumerated().allSatisfy { $0.offset == i || $0.element <= 0.001 }
        return HStack(spacing: 8) {
            Image(systemName: icon).foregroundStyle(color).frame(width: 22, height: 22).background(color.opacity(0.18), in: RoundedRectangle(cornerRadius: 5))
            VStack(alignment: .leading, spacing: 0) {
                Text(label).font(.needle(.caption).weight(.medium))
                Text(muted ? "Muted" : "\(Int(level * 100))%").font(.needle(.caption2)).foregroundStyle(.secondary)
            }
            .frame(width: 52, alignment: .leading)
            Slider(value: Binding(get: { Double(level) }, set: { v in
                levels[i] = Float(v)
                store.core.setStemLevel(stem: UInt32(i), level: Float(v))
            }), in: 0...1.5)
            .controlSize(.mini)
            .tint(color)
            pill("S", on: soloed, color: color, help: "Solo") {
                var mix: [Float] = [0, 0, 0, 0]
                if soloed { mix = [1, 1, 1, 1] } else { mix[i] = 1 }
                set(mix)
            }
            pill("M", on: muted, color: color, help: "Mute") {
                var mix = levels
                mix[i] = muted ? 1 : 0
                set(mix)
            }
        }
        .padding(6)
        .background(color.opacity(active && store.playback?.playing == true && !muted ? 0.1 : 0.05), in: RoundedRectangle(cornerRadius: 8))
        .opacity(active ? 1 : 0.6)
    }

    private func pill(_ text: String, on: Bool, color: Color, help: String, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Text(text).font(.needle(.caption2).weight(.bold)).frame(width: 20, height: 18)
                .background(on ? color : .clear, in: RoundedRectangle(cornerRadius: 4))
                .foregroundStyle(on ? .black : .secondary)
        }
        .buttonStyle(.plain)
        .help(help)
    }
}
