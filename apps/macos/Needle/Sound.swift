import SwiftUI
import UniformTypeIdentifiers

/// The equalizer (ten bands, or parametric bands of any kind), balance, mono, headphone
/// crossfeed, saved presets, and plugins' sound effects. Changes apply while you listen.
struct SoundPane: View {
    private var store: Store { .shared }
    @State private var sound: SoundSettings?
    @State private var bands: [Float] = []
    @State private var presets: [EqPreset] = []
    @State private var mine: [String] = []
    @State private var effects: [Effect] = []
    @State private var naming = false
    @State private var name = ""

    var body: some View {
        Section {
            if let sound {
                equalizer(sound)
            } else {
                ProgressView().frame(maxWidth: .infinity)
            }
        } header: {
            GroupHeader(title: "Equalizer", detail: "Shape the sound. Changes apply while you listen.")
        }
        .task(id: store.libraryVersion) { load() }
        .alert("Name for this sound", isPresented: $naming) {
            TextField("My sound", text: $name)
            Button("Save") {
                let name = name
                Task {
                    if await store.ask({ try $0.savePreset(name: name) }) != nil {
                        store.notify("Saved “\(name)”.")
                        load()
                    }
                }
            }
            Button("Cancel", role: .cancel) {}
        }
        if let sound {
            Section {
                SettingRow(title: "Balance", detail: abs(sound.balance) < 0.01 ? "Centred." : sound.balance < 0 ? "Leaning left." : "Leaning right.") {
                    HStack {
                        Text("L").foregroundStyle(.secondary)
                        Slider(value: Binding(get: { Double(sound.balance) }, set: { v in change { $0.balance = abs(v) < 0.03 ? 0 : Float(v) } }), in: -1...1)
                            .frame(width: 200)
                        Text("R").foregroundStyle(.secondary)
                    }
                }
                SettingRow(title: "Mono", detail: "Plays both channels in both ears. Useful with one earbud, or for hearing loss in one ear.") {
                    RowToggle(isOn: Binding(get: { sound.mono }, set: { v in change { $0.mono = v } })).toggleStyle(.switch).labelsHidden()
                }
                SettingRow(title: "Headphone crossfeed", detail: "Blends a little of each side into the other, the way speakers reach both ears. Makes hard-panned older recordings less tiring on headphones.") {
                    RowToggle(isOn: Binding(get: { sound.crossfeed }, set: { v in change { $0.crossfeed = v } })).toggleStyle(.switch).labelsHidden()
                }
            } header: {
                GroupHeader(title: "Listening tools")
            } footer: {
                Text("ReplayGain, album gain, and loudness measurement are in Settings › Playback.")
                    .font(.caption).foregroundStyle(.secondary)
            }
            effectsSection
        }
    }

    private func load() {
        sound = try? store.core.soundSettings()
        bands = store.core.eqBands()
        presets = store.core.eqPresets()
        mine = store.core.myPresets()
        effects = store.core.effects()
    }

    /// Changes the sound settings and sends them at once.
    private func change(_ edit: (inout SoundSettings) -> Void) {
        guard var next = sound else { return }
        edit(&next)
        sound = next
        store.setSound(next)
    }

    @ViewBuilder
    private func equalizer(_ s: SoundSettings) -> some View {
        let colors = store.colors
        SettingRow(title: "Equalizer", detail: s.preset.isEmpty ? "" : "Preset: \(s.preset)") {
            RowToggle(isOn: Binding(get: { s.eq }, set: { v in change { $0.eq = v } })).toggleStyle(.switch).labelsHidden()
        }
        VStack(alignment: .leading, spacing: 12) {
            HStack {
                Picker("", selection: Binding(get: { s.mode }, set: { v in change { $0.mode = v; $0.eq = true } })) {
                    Text("Graphic").tag("graphic")
                    Text("Parametric").tag("parametric")
                }
                .pickerStyle(.segmented).labelsHidden().frame(width: 220)
                Spacer()
                Menu("Presets") {
                    ForEach(presets, id: \.name) { p in
                        Button(p.name) { change { $0.eq = true; $0.mode = "graphic"; $0.preamp = p.preamp; $0.bands = p.bands; $0.preset = p.name } }
                    }
                    if !mine.isEmpty {
                        Divider()
                        ForEach(mine, id: \.self) { n in
                            Button(n) { store.run { try $0.usePreset(name: n) }; Task { try? await Task.sleep(for: .milliseconds(150)); load() } }
                        }
                        Menu("Delete a preset") {
                            ForEach(mine, id: \.self) { n in
                                Button(n, role: .destructive) { store.run { try $0.deletePreset(name: n) }; mine.removeAll { $0 == n } }
                            }
                        }
                    }
                    Divider()
                    Button("Save as preset…") { name = "My sound \(mine.count + 1)"; naming = true }
                }
                .fixedSize()
                Button("Reset") { change { $0.preamp = 0; $0.bands = Array(repeating: 0, count: 10); $0.preset = "Flat"; $0.parametric = [] } }
            }
            HStack(alignment: .center, spacing: 14) {
                VStack(spacing: 4) {
                    Text("Preamp").font(.caption).foregroundStyle(.secondary)
                    VerticalSlider(value: Binding(get: { s.preamp }, set: { v in change { $0.preamp = v } }), range: -24...12)
                        .frame(height: 150)
                    Text(String(format: "%+.1f", s.preamp)).font(.caption.monospacedDigit())
                }
                Divider().frame(height: 160)
                if s.mode == "parametric" {
                    parametric(s)
                } else {
                    ForEach(Array(bands.enumerated()), id: \.offset) { i, frequency in
                        VStack(spacing: 4) {
                            Text(String(format: "%+.0f", i < s.bands.count ? s.bands[i] : 0)).font(.caption2.monospacedDigit()).foregroundStyle(.secondary)
                            VerticalSlider(value: Binding(get: { i < s.bands.count ? s.bands[i] : 0 }, set: { v in
                                change { if i < $0.bands.count { $0.bands[i] = v }; $0.eq = true; $0.preset = "Custom" }
                            }), range: -12...12, label: frequency >= 1000 ? "\(Int(frequency / 1000)) kilohertz" : "\(Int(frequency)) hertz")
                            .frame(height: 150)
                            Text(frequency >= 1000 ? "\(Int(frequency / 1000))k" : "\(Int(frequency))").font(.caption2).foregroundStyle(.secondary)
                        }
                        .frame(maxWidth: .infinity)
                    }
                }
            }
            .padding(.vertical, 8)
            .opacity(s.eq ? 1 : 0.55)
            let loudest = (s.mode == "parametric" ? s.parametric.filter(\.on).map(\.gain) : s.bands).max() ?? 0
            if loudest + s.preamp > 0.05 {
                Text(String(format: "Boosted bands can clip. A preamp of %.1f dB keeps the loudest band safe.", -loudest))
                    .font(.caption).foregroundStyle(colors.danger)
            }
            HStack {
                Button("Load EQ file…") { loadFile() }
                    .help("Load a ParametricEQ.txt from AutoEq, or an Equalizer APO configuration")
                Button("Save EQ file…") { saveFile() }
            }
        }
        .padding(.vertical, 6)
    }

    @ViewBuilder
    private func parametric(_ s: SoundSettings) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            ForEach(Array(s.parametric.enumerated()), id: \.offset) { i, band in
                HStack(spacing: 10) {
                    RowToggle(label: "Band \(i + 1)", isOn: Binding(get: { band.on }, set: { v in change { $0.parametric[i].on = v } })).labelsHidden()
                    Picker("", selection: Binding(get: { band.kind }, set: { v in change { $0.parametric[i].kind = v } })) {
                        Text("Peak").tag("peak")
                        Text("Low shelf").tag("lowshelf")
                        Text("High shelf").tag("highshelf")
                        Text("Low pass").tag("lowpass")
                        Text("High pass").tag("highpass")
                        Text("Notch").tag("notch")
                    }
                    .labelsHidden().frame(width: 110)
                    LabeledSlider(label: band.frequency >= 1000 ? String(format: "%.1f kHz", band.frequency / 1000) : "\(Int(band.frequency)) Hz",
                                  value: Binding(get: { log10(Double(band.frequency)) }, set: { v in change { $0.parametric[i].frequency = Float(pow(10, v)) } }),
                                  range: log10(20)...log10(20000))
                    LabeledSlider(label: String(format: "%+.1f dB", band.gain),
                                  value: Binding(get: { Double(band.gain) }, set: { v in change { $0.parametric[i].gain = Float(v) } }), range: -24...24)
                    LabeledSlider(label: String(format: "Q %.2f", band.q),
                                  value: Binding(get: { Double(band.q) }, set: { v in change { $0.parametric[i].q = Float(v) } }), range: 0.1...10)
                    Button { change { $0.parametric.remove(at: i) } } label: { Image(systemName: "minus.circle") }.buttonStyle(.borderless).help("Remove")
                }
            }
            Button { change { $0.parametric.append(EqBand(kind: "peak", frequency: 1000, gain: 0, q: 1, on: true)); $0.eq = true } } label: {
                Label("Add band", systemImage: "plus")
            }
            .disabled(s.parametric.count >= 20)
            Text("Click a band's kind to change it. Peak, shelf, and notch bands follow Q; a higher Q is narrower.").font(.caption).foregroundStyle(.secondary)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    private var effectsSection: some View {
        Section {
            if effects.isEmpty {
                Text("No effects yet. Turn on a plugin that adds effects in Settings › Plugins. Add the example plugins there to try Studio effects and Bitcrusher.")
                    .font(.callout).foregroundStyle(.secondary)
            }
            ForEach(Array(effects.enumerated()), id: \.offset) { _, effect in
                VStack(alignment: .leading, spacing: 6) {
                    HStack {
                        VStack(alignment: .leading, spacing: 1) {
                            Text(effect.name).font(.body.weight(.medium))
                            Text("\(effect.pluginName) · \(effect.description)").font(.caption).foregroundStyle(.secondary).lineLimit(2)
                        }
                        Spacer()
                        RowToggle(label: effect.name, isOn: Binding(get: { effect.on }, set: { on in
                            let (plugin, id) = (effect.plugin, effect.id)
                            store.run { try $0.setEffect(plugin: plugin, effect: id, on: on) }
                            Task { try? await Task.sleep(for: .milliseconds(150)); effects = store.core.effects() }
                        }))
                        .toggleStyle(.switch).labelsHidden()
                    }
                    if let failure = effect.failure {
                        Text("Stopped: \(failure)").font(.caption).foregroundStyle(store.colors.danger)
                    }
                    if effect.on {
                        ForEach(effect.params, id: \.id) { param in
                            HStack {
                                Text(param.name).frame(width: 120, alignment: .leading)
                                Slider(value: Binding(get: { Double(param.value) }, set: { v in
                                    let (plugin, id, pid) = (effect.plugin, effect.id, param.id)
                                    try? store.core.setEffectParam(plugin: plugin, effect: id, param: pid, value: Float(v))
                                    effects = store.core.effects()
                                }), in: Double(param.min)...Double(max(param.max, param.min + 0.001)))
                                Text(param.shown).font(.caption.monospacedDigit()).frame(width: 70, alignment: .trailing)
                            }
                        }
                    }
                }
            }
        } header: {
            GroupHeader(title: "Effects", detail: "Effects come from plugins. They play in this order, after the equalizer.")
        }
    }

    private func loadFile() {
        let panel = NSOpenPanel()
        panel.allowedContentTypes = [.plainText, UTType(filenameExtension: "txt") ?? .plainText]
        guard panel.runModal() == .OK, let path = panel.url?.path else { return }
        Task {
            if let n = await store.ask({ try $0.loadEqFile(path: path) }) {
                store.notify("Loaded \(n) filters.")
                load()
            }
        }
    }

    private func saveFile() {
        let panel = NSSavePanel()
        panel.nameFieldStringValue = "ParametricEQ.txt"
        guard panel.runModal() == .OK, let path = panel.url?.path else { return }
        store.job { try $0.saveEqFile(path: path); return "Saved in the Equalizer APO format." }
    }
}

/// An upright slider, for the equalizer's bands: a track, the part from 0 dB lit, and a knob.
/// (A turned system slider does not fill its height.)
struct VerticalSlider: View {
    private var store: Store { .shared }
    @Binding var value: Float
    let range: ClosedRange<Float>
    /// Its name for VoiceOver ("60 Hz").
    var label = "Preamp"
    private let knob: CGFloat = 14

    var body: some View {
        GeometryReader { geo in
            let height = geo.size.height
            let span = range.upperBound - range.lowerBound
            let place = { (v: Float) -> CGFloat in (height - knob) * (1 - CGFloat((v - range.lowerBound) / span)) + knob / 2 }
            let at = place(min(max(value, range.lowerBound), range.upperBound))
            let zero = place(min(max(0, range.lowerBound), range.upperBound))
            ZStack(alignment: .top) {
                Capsule().fill(.quaternary).frame(width: 4, height: height)
                Rectangle().fill(store.colors.accent).frame(width: 4, height: abs(zero - at)).offset(y: min(zero, at))
                Circle().fill(.white).shadow(color: .black.opacity(0.3), radius: 1.5, y: 1)
                    .frame(width: knob, height: knob).offset(y: at - knob / 2)
            }
            .frame(width: geo.size.width, height: height, alignment: .top)
            .contentShape(Rectangle())
            .gesture(DragGesture(minimumDistance: 0).onChanged { drag in
                let f = 1 - min(max((drag.location.y - knob / 2) / (height - knob), 0), 1)
                value = range.lowerBound + Float(f) * span
            })
        }
        .frame(width: 22)
        .accessibilityElement()
        .accessibilityLabel(label)
        .accessibilityValue(String(format: "%+.1f decibels", value))
        .accessibilityAdjustableAction { direction in
            switch direction {
            case .increment: value = min(range.upperBound, value + 1)
            case .decrement: value = max(range.lowerBound, value - 1)
            @unknown default: break
            }
        }
    }
}

/// A small slider with its value written above it.
struct LabeledSlider: View {
    let label: String
    @Binding var value: Double
    let range: ClosedRange<Double>

    var body: some View {
        VStack(spacing: 0) {
            Text(label).font(.caption2.monospacedDigit()).foregroundStyle(.secondary)
            Slider(value: $value, in: range).controlSize(.mini)
        }
        .frame(minWidth: 90)
    }
}
