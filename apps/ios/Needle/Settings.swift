import SwiftUI
import UniformTypeIdentifiers

/// Settings, in groups as the iPhone's own: a card for Needle and your library at the top, then
/// listening, the look, your music and where it comes from, and Needle itself.
struct SettingsView: View {
    @Environment(Store.self) private var store
    /// This build has Discord's library.
    @State private var discord = false
    @State private var songs: UInt32?
    /// What's new or About, opened from the card's buttons.
    @State private var opened: Route?

    var body: some View {
        List {
            Section {
                header
            }
            Section("Listening") {
                row("Playback", "Crossfade and even loudness", "play.fill", .blue, .settingsPlayback)
                row("Sound", "Equalizer, balance, and headphones", "slider.vertical.3", .purple, .settingsSound)
                row("Scrobbling", "Last.fm and ListenBrainz", "waveform", .red, .settingsScrobbling)
                if discord {
                    row("Discord", "Show what you play on your profile", "bubble.left.and.text.bubble.right.fill", .indigo, .settingsDiscord)
                }
            }
            Section("Look and feel") {
                row("Appearance", "Themes, colours, the player, and lyrics", "paintpalette.fill", .pink, .settingsAppearance)
            }
            Section("Your music") {
                row("Sources", "This iPhone, your server, and your computer", "globe", .teal, .sources)
                row("Library", "Music folders and online lookups", "music.note.house.fill", .orange, .settingsLibrary)
                row("Music server", "Navidrome and other Subsonic servers", "server.rack", .cyan, .server("subsonic"))
                row("Your computer", "Control Needle there, play its music here", "laptopcomputer", .indigo, .computer)
            }
            Section("Tools") {
                row("Fix my library", "Tags, duplicates, covers, and files", "wrench.and.screwdriver.fill", .green, .fixLibrary)
                row("Import", "Other players and listening history", "square.and.arrow.down.fill", .cyan, .importing)
                row("Sync", "Move history, ratings, and playlists", "arrow.triangle.2.circlepath", .yellow, .sync)
                row("Plugins", "NetEase lyrics, effects, and commands", "puzzlepiece.extension.fill", .teal, .plugins)
            }
        }
        .navigationTitle("Settings")
        .themed()
        .navigationDestination(item: $opened) { route in
            if route == .whatsNew { WhatsNewView() } else { AboutView() }
        }
        .task {
            discord = await store.load { $0.discordAccount() }?.available ?? false
            songs = await store.load { try $0.songCount() }
        }
    }

    /// Needle, its version, and the size of your library, with What's new and About as two
    /// buttons in the card (they were rows of their own at the foot).
    private var header: some View {
        let version = Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? ""
        return VStack(alignment: .leading, spacing: 14) {
            HStack(spacing: 14) {
                Image(systemName: "music.note")
                    .font(.system(size: 28, weight: .bold))
                    .foregroundStyle(.white)
                    .frame(width: 60, height: 60)
                    .background(LinearGradient(colors: [store.colors.accent, store.colors.accent.opacity(0.6)], startPoint: .topLeading, endPoint: .bottomTrailing),
                                in: .rect(cornerRadius: 14))
                    .accessibilityHidden(true)
                VStack(alignment: .leading, spacing: 2) {
                    Text("Needle").font(.title2.bold())
                    Text("Version \(version)").font(.subheadline).foregroundStyle(.secondary)
                    if let songs {
                        Text("\(count(Int(songs), "song")) in your library").font(.caption).foregroundStyle(.secondary)
                    }
                }
                .accessibilityElement(children: .combine)
            }
            // Two buttons in one row, each opening its own page (links in a list row would all
            // open with a tap anywhere on it).
            HStack(spacing: 10) {
                Button { opened = .whatsNew } label: {
                    Label("What's new", systemImage: "sparkles").frame(maxWidth: .infinity)
                }
                Button { opened = .about } label: {
                    Label("About", systemImage: "info.circle").frame(maxWidth: .infinity)
                }
            }
            .buttonStyle(.bordered)
            .buttonBorderShape(.capsule)
            .font(.subheadline.weight(.medium))
        }
        .padding(.vertical, 6)
    }

    private func row(_ title: String, _ detail: String, _ symbol: String, _ tint: Color, _ route: Route) -> some View {
        NavigationLink(value: route) {
            HStack(spacing: 14) {
                Image(systemName: symbol).font(.system(size: 15, weight: .semibold)).foregroundStyle(.white)
                    .frame(width: 30, height: 30)
                    .background(tint.gradient, in: .rect(cornerRadius: 8))
                    .accessibilityHidden(true)
                VStack(alignment: .leading, spacing: 1) {
                    Text(title)
                    Text(detail).font(.caption).foregroundStyle(.secondary).lineLimit(1)
                }
            }
            .padding(.vertical, 1)
        }
    }
}

/// A setting's label, as in the iPhone's own Settings: a small icon, its name, and one short
/// line.
struct SettingLabel: View {
    let title: String
    var detail: String = ""
    let icon: String
    var tint: Color = .gray

    var body: some View {
        HStack(spacing: 12) {
            Image(systemName: icon).font(.system(size: 13, weight: .semibold)).foregroundStyle(.white)
                .frame(width: 28, height: 28)
                .background(tint.gradient, in: .rect(cornerRadius: 7))
                .accessibilityHidden(true)
            VStack(alignment: .leading, spacing: 1) {
                Text(title)
                if !detail.isEmpty { Text(detail).font(.caption).foregroundStyle(.secondary).lineLimit(1) }
            }
        }
    }
}

/// A setting that opens in place to show more, with its current choice on the right.
struct OpenRow<Content: View>: View {
    let label: SettingLabel
    let value: String
    @ViewBuilder let content: Content
    @State private var open = false

    var body: some View {
        DisclosureGroup(isExpanded: $open) {
            content
        } label: {
            HStack {
                label
                Spacer(minLength: 8)
                Text(value).font(.subheadline).foregroundStyle(.secondary).lineLimit(1)
            }
        }
    }
}

/// Loads a record of settings, shows it to change, and saves each change.
@MainActor
private func saved<T: Sendable>(_ value: Binding<T?>, _ store: Store, _ write: @escaping @Sendable (Needle, T) throws -> Void) -> (T) -> Void {
    { new in
        value.wrappedValue = new
        store.inOrder { try write($0, new) }
    }
}

// MARK: - Library

struct LibrarySettingsView: View {
    @Environment(Store.self) private var store
    /// Folders asked to be removed, waiting for a yes.
    @State private var removing: [String] = []
    @State private var folders: [String] = []
    @State private var settings: PlaybackSettings?
    @State private var hasFanart = false
    @State private var fanartKey = ""
    @State private var measured: [UInt32] = []
    @State private var backup: URL?
    @State private var moving = false

    var body: some View {
        Form {
            Section {
                ForEach(folders, id: \.self) { folder in
                    VStack(alignment: .leading, spacing: 2) {
                        Text(Store.realPath(folder) == Store.realPath(store.musicFolder.path) ? "Needle's folder" : (folder as NSString).lastPathComponent)
                        Text(Store.realPath(folder) == Store.realPath(store.musicFolder.path) ? "Files › On My iPhone › Needle" : folder)
                            .font(.caption).foregroundStyle(.secondary).lineLimit(1)
                    }
                    .deleteDisabled(Store.realPath(folder) == Store.realPath(store.musicFolder.path))
                }
                .onDelete { rows in
                    // Asked first: its songs leave the library, with their plays and stars.
                    removing = rows.map { folders[$0] }
                }
                Button("Read the music again") { store.rescan() }.disabled(store.scan.running)
                    .confirmationDialog("Stop reading this folder?", isPresented: Binding(get: { !removing.isEmpty }, set: { if !$0 { removing = [] } }), titleVisibility: .visible) {
                        Button("Stop Reading It", role: .destructive) {
                            let gone = removing
                            removing = []
                            folders.removeAll { gone.contains($0) }
                            Task {
                                for folder in gone { _ = await store.ask { try $0.removeFolder(path: folder) } }
                                store.libraryChanged()
                            }
                        }
                    } message: {
                        Text("Its songs leave the library, with their plays, stars, and places in playlists. The files stay.")
                    }
            } header: {
                Text("Music folders")
            } footer: {
                if store.scan.running { Text("Reading… \(count(Int(store.scan.imported), "song")) so far") }
            }
            if let settings {
                let save = saved($settings, store) { try $0.setPlaybackSettings(value: $1) }
                Section("Artwork and lyrics") {
                    Toggle(isOn: Binding(get: { settings.onlineMedia }, set: { var s = settings; s.onlineMedia = $0; save(s) })) {
                        SettingLabel(title: "Look things up online", detail: "Covers, artist photos, and lyrics.", icon: "globe", tint: .blue)
                    }
                    OpenRow(label: SettingLabel(title: "Artists' logos", detail: "From fanart.tv.", icon: "signature", tint: .indigo),
                            value: hasFanart ? "Your key" : "Needle's key") {
                    if hasFanart {
                        LabeledContent("fanart.tv key", value: "Your own")
                        Button("Remove Your Key", role: .destructive) {
                            Task {
                                if await store.ask({ try $0.setFanartKey(key: "") }) != nil {
                                    hasFanart = await store.load { $0.hasOwnFanartKey() } ?? false
                                }
                            }
                        }
                    } else {
                        SecureField("fanart.tv key", text: $fanartKey).textInputAutocapitalization(.never).autocorrectionDisabled()
                        Button("Save the Key") {
                            let key = fanartKey
                            Task {
                                if await store.ask({ try $0.setFanartKey(key: key) }) != nil { fanartKey = ""; hasFanart = true }
                            }
                        }
                        .disabled(fanartKey.trimmingCharacters(in: .whitespaces).isEmpty)
                    }
                    }
                }
                Section("Radio") {
                    Toggle(isOn: Binding(get: { settings.soundAnalysis }, set: { var s = settings; s.soundAnalysis = $0; save(s) })) {
                        SettingLabel(title: "Measure songs for radio", detail: "Tempo, key, and energy, on this iPhone.", icon: "dot.radiowaves.left.and.right", tint: .orange)
                    }
                    if measured.count == 2 {
                        LabeledContent("Measured", value: "\(measured[0].formatted()) of \(measured[1].formatted())")
                        Button("Measure now") { store.core.measureInBackground(); store.message = "Measuring in the background" }
                            .disabled(measured[0] >= measured[1])
                    }
                    OpenRow(label: SettingLabel(title: "When nothing is up next", detail: "Keep playing songs that match.", icon: "sparkles", tint: .purple),
                            value: settings.autoplayQuery.isEmpty ? "Off" : settings.autoplayQuery) {
                        TextField("For example: rating is at least 4", text: Binding(get: { settings.autoplayQuery }, set: { var s = settings; s.autoplayQuery = $0; save(s) }))
                            .textInputAutocapitalization(.never)
                        Text("Empty: the music stops.").font(.caption).foregroundStyle(.secondary)
                    }
                }
            }
            Section {
                Button {
                    let path = FileManager.default.temporaryDirectory.appendingPathComponent("Needle library.needlebackup").path
                    Task {
                        if await store.ask({ try $0.backupLibrary(path: path) }) != nil {
                            backup = URL(fileURLWithPath: path)
                            moving = true
                        }
                    }
                } label: {
                    SettingLabel(title: "Back up the library", detail: "Plays, stars, playlists, and settings. No music.", icon: "externaldrive.fill", tint: .gray)
                }
                .foregroundStyle(.primary)
            }
        }
        .navigationTitle("Library")
        .themed()
        .fileMover(isPresented: $moving, file: backup) { _ in }
        .task {
            folders = await store.load { try $0.folders() } ?? []
            settings = await store.load { try $0.playbackSettings() }
            hasFanart = await store.load { $0.hasOwnFanartKey() } ?? false
            while !Task.isCancelled {
                measured = await store.load { $0.measured() } ?? []
                try? await Task.sleep(for: .seconds(2))
            }
        }
    }
}

// MARK: - Playback

struct PlaybackSettingsView: View {
    @Environment(Store.self) private var store
    @State private var settings: PlaybackSettings?

    var body: some View {
        Form {
            if let settings {
                let save = saved($settings, store) { try $0.setPlaybackSettings(value: $1) }
                Section("Playing") {
                    VStack(alignment: .leading, spacing: 8) {
                        HStack {
                            SettingLabel(title: "Crossfade", detail: "Songs overlap as one ends.", icon: "arrow.left.arrow.right", tint: .blue)
                            Spacer()
                            Text(settings.crossfade < 0.5 ? "Off" : "\(Int(settings.crossfade.rounded())) s").foregroundStyle(.secondary).monospacedDigit()
                        }
                        Slider(value: Binding(get: { Double(settings.crossfade) }, set: { var s = settings; s.crossfade = Float($0.rounded()); save(s) }), in: 0...12, step: 1)
                            .accessibilityLabel("Crossfade")
                    }
                    VStack(alignment: .leading, spacing: 8) {
                        SettingLabel(title: "Even loudness", detail: "From the songs' ReplayGain tags.", icon: "waveform.path", tint: .teal)
                    Picker("Even loudness", selection: Binding(
                        get: { !settings.replayGain ? 0 : (settings.albumGain ? 2 : 1) },
                        set: { var s = settings; s.replayGain = $0 > 0; s.albumGain = $0 == 2; save(s) })) {
                        Text("Off").tag(0)
                        Text("Song").tag(1)
                        Text("Album").tag(2)
                    }
                    .pickerStyle(.segmented)
                    }
                }
                Section("Sleep timer") {
                    SleepMenu()
                }
            }
        }
        .navigationTitle("Playback")
        .themed()
        .task { settings = await store.load { try $0.playbackSettings() } }
    }
}

/// Pauses the music later.
struct SleepMenu: View {
    @Environment(Store.self) private var store

    var body: some View {
        Menu {
            Button("End of this song") { store.setSleep(minutes: 0) }
            ForEach([15, 30, 45, 60, 90], id: \.self) { m in
                Button("\(m) minutes") { store.setSleep(minutes: m) }
            }
            if store.sleepEnds != nil {
                Divider()
                Button("Turn off", role: .destructive) { store.setSleep(minutes: nil) }
            }
        } label: {
            HStack {
                Label("Sleep timer", systemImage: "moon.zzz")
                Spacer()
                if let ends = store.sleepEnds {
                    if ends == .distantFuture { Text("End of song").foregroundStyle(.secondary) }
                    else { Text(ends, style: .timer).foregroundStyle(.secondary).monospacedDigit() }
                } else {
                    Text("Off").foregroundStyle(.secondary)
                }
            }
        }
    }
}

// MARK: - Sound

struct SoundView: View {
    @Environment(Store.self) private var store
    @State private var sound: SoundSettings?
    @State private var frequencies: [Float] = []
    @State private var presets: [EqPreset] = []
    @State private var mine: [String] = []
    @State private var effects: [Effect] = []
    @State private var naming = false
    @State private var newName = ""
    @State private var loadingFile = false

    var body: some View {
        Form {
            if let sound {
                let save = saved($sound, store) { try $0.setSoundSettings(value: $1) }
                Section {
                    Toggle("Equalizer", isOn: Binding(get: { sound.eq }, set: { var s = sound; s.eq = $0; save(s) }))
                    Picker("Kind", selection: Binding(get: { sound.mode }, set: { var s = sound; s.mode = $0; save(s) })) {
                        Text("Ten bands").tag("graphic")
                        Text("Parametric").tag("parametric")
                    }
                    .pickerStyle(.segmented)
                    Menu {
                        Section("Needle's") {
                            ForEach(presets, id: \.name) { preset in
                                Button(preset.name) {
                                    var s = sound
                                    s.bands = preset.bands; s.preamp = preset.preamp; s.preset = preset.name; s.eq = true; s.mode = "graphic"
                                    save(s)
                                }
                            }
                        }
                        if !mine.isEmpty {
                            Section("Yours") {
                                ForEach(mine, id: \.self) { name in
                                    Button(name) {
                                        Task {
                                            _ = await store.ask { try $0.usePreset(name: name) }
                                            self.sound = await store.load { try $0.soundSettings() }
                                        }
                                    }
                                }
                            }
                        }
                        Divider()
                        Button("Save as preset…", systemImage: "square.and.arrow.down") { newName = ""; naming = true }
                        Button("Headphone correction file…", systemImage: "headphones") { loadingFile = true }
                    } label: {
                        LabeledContent("Preset", value: sound.preset.isEmpty ? "Your own" : sound.preset)
                    }
                } footer: {
                    Text("A headphone correction is an AutoEq or Equalizer APO “ParametricEQ.txt” file.")
                }
                if sound.mode == "graphic" {
                    Section("Bands") {
                        dB("Preamp", value: sound.preamp) { var s = sound; s.preamp = $0; s.preset = ""; save(s) }
                        ForEach(Array(frequencies.enumerated()), id: \.offset) { i, hz in
                            if i < sound.bands.count {
                                dB(hz >= 1000 ? "\((hz / 1000).formatted()) kHz" : "\(Int(hz)) Hz", value: sound.bands[i]) {
                                    var s = sound; s.bands[i] = $0; s.preset = ""; save(s)
                                }
                            }
                        }
                        Button("Flat") { var s = sound; s.bands = s.bands.map { _ in 0 }; s.preamp = 0; s.preset = ""; save(s) }
                    }
                } else {
                    ParametricBands(bands: sound.parametric) { var s = sound; s.parametric = $0; save(s) }
                }
                if !mine.isEmpty {
                    Section("Your presets") {
                        ForEach(mine, id: \.self) { Text($0) }
                            .onDelete { rows in
                                for name in rows.map({ mine[$0] }) { store.run { try $0.deletePreset(name: name) } }
                                mine.remove(atOffsets: rows)
                            }
                    }
                }
                Section("Listening") {
                    VStack(alignment: .leading) {
                        LabeledContent("Balance", value: balance(sound.balance))
                        Slider(value: Binding(get: { Double(sound.balance) }, set: { var s = sound; s.balance = Float(($0 * 20).rounded() / 20); save(s) }), in: -1...1)
                    }
                    Toggle(isOn: Binding(get: { sound.mono }, set: { var s = sound; s.mono = $0; save(s) })) {
                        SettingLabel(title: "Mono", detail: "Both channels in both ears.", icon: "speaker.fill", tint: .gray)
                    }
                    Toggle(isOn: Binding(get: { sound.crossfeed }, set: { var s = sound; s.crossfeed = $0; save(s) })) {
                        SettingLabel(title: "Headphone crossfeed", detail: "Blends the sides a little, as speakers do.", icon: "headphones", tint: .purple)
                    }
                }
            }
            if !effects.isEmpty {
                EffectsSection(effects: $effects)
            }
        }
        .navigationTitle("Sound")
        .themed()
        .alert("Save preset", isPresented: $naming) {
            TextField("Name", text: $newName)
            Button("Save") {
                let name = newName
                Task {
                    _ = await store.ask { try $0.savePreset(name: name) }
                    mine = await store.load { $0.myPresets() } ?? []
                }
            }
            Button("Cancel", role: .cancel) {}
        }
        .fileImporter(isPresented: $loadingFile, allowedContentTypes: [.plainText, .text]) { result in
            guard case .success(let url) = result, let path = store.copyIn(url) else { return }
            Task {
                if let bands = await store.ask({ try $0.loadEqFile(path: path) }) {
                    store.message = "Headphone correction on: \(count(Int(bands), "band"))"
                    sound = await store.load { try $0.soundSettings() }
                }
            }
        }
        .task {
            sound = await store.load { try $0.soundSettings() }
            frequencies = await store.load { $0.eqBands() } ?? []
            presets = await store.load { $0.eqPresets() } ?? []
            mine = await store.load { $0.myPresets() } ?? []
            effects = await store.load { $0.effects() } ?? []
        }
    }

    private func balance(_ value: Float) -> String {
        abs(value) < 0.03 ? "Centre" : value < 0 ? "\(Int(-value * 100))% left" : "\(Int(value * 100))% right"
    }

    private func dB(_ label: String, value: Float, set: @escaping (Float) -> Void) -> some View {
        HStack {
            Text(label).font(.subheadline).frame(width: 70, alignment: .leading)
            Slider(value: Binding(get: { Double(value) }, set: { set(Float(($0 * 2).rounded() / 2)) }), in: -12...12)
            Text(String(format: "%+.1f", value)).font(.caption.monospacedDigit()).foregroundStyle(.secondary).frame(width: 40, alignment: .trailing)
        }
    }
}

/// The parametric equalizer: bands of any kind, frequency, and width.
struct ParametricBands: View {
    let bands: [EqBand]
    let change: ([EqBand]) -> Void

    private static let kinds = [("peak", "Peak"), ("lowshelf", "Low shelf"), ("highshelf", "High shelf"),
                                ("lowpass", "Low pass"), ("highpass", "High pass"), ("notch", "Notch")]

    var body: some View {
        Section("Bands") {
            ForEach(Array(bands.enumerated()), id: \.offset) { i, band in
                VStack(alignment: .leading, spacing: 6) {
                    HStack {
                        Toggle(isOn: Binding(get: { band.on }, set: { var b = bands; b[i].on = $0; change(b) })) {
                            Picker("Kind", selection: Binding(get: { band.kind }, set: { var b = bands; b[i].kind = $0; change(b) })) {
                                ForEach(Self.kinds, id: \.0) { Text($0.1).tag($0.0) }
                            }
                            .labelsHidden()
                        }
                    }
                    slider("Hz", value: log10(Double(band.frequency)), range: log10(20)...log10(20000), shown: band.frequency >= 1000 ? String(format: "%.1f kHz", band.frequency / 1000) : "\(Int(band.frequency)) Hz") {
                        var b = bands; b[i].frequency = Float(pow(10, $0).rounded()); change(b)
                    }
                    if ["peak", "lowshelf", "highshelf"].contains(band.kind) {
                        slider("dB", value: Double(band.gain), range: -15...15, shown: String(format: "%+.1f dB", band.gain)) {
                            var b = bands; b[i].gain = Float(($0 * 2).rounded() / 2); change(b)
                        }
                    }
                    slider("Q", value: Double(band.q), range: 0.1...10, shown: String(format: "%.2f", band.q)) {
                        var b = bands; b[i].q = Float(($0 * 100).rounded() / 100); change(b)
                    }
                }
            }
            .onDelete { rows in var b = bands; b.remove(atOffsets: rows); change(b) }
            Button("Add a band", systemImage: "plus") {
                change(bands + [EqBand(kind: "peak", frequency: 1000, gain: 0, q: 1, on: true)])
            }
        }
    }

    private func slider(_ unit: String, value: Double, range: ClosedRange<Double>, shown: String, set: @escaping (Double) -> Void) -> some View {
        HStack {
            Slider(value: Binding(get: { value }, set: set), in: range)
            Text(shown).font(.caption.monospacedDigit()).foregroundStyle(.secondary).frame(width: 70, alignment: .trailing)
        }
    }
}

/// Sound effects from plugins, each with its own settings.
struct EffectsSection: View {
    @Environment(Store.self) private var store
    @Binding var effects: [Effect]

    var body: some View {
        Section("Effects from plugins") {
            ForEach(Array(effects.enumerated()), id: \.element.id) { i, effect in
                VStack(alignment: .leading, spacing: 6) {
                    Toggle(isOn: Binding(get: { effect.on }, set: { on in
                        effects[i].on = on
                        store.run { try $0.setEffect(plugin: effect.plugin, effect: effect.id, on: on) }
                    })) {
                        Text(effect.name)
                        Text(effect.pluginName + (effect.description.isEmpty ? "" : " · " + effect.description))
                    }
                    if let failure = effect.failure { Text(failure).font(.caption).foregroundStyle(.red) }
                    if effect.on {
                        ForEach(Array(effect.params.enumerated()), id: \.element.id) { j, param in
                            HStack {
                                Text(param.name).font(.subheadline)
                                Slider(value: Binding(get: { Double(param.value) }, set: { value in
                                    let v = param.step > 0 ? (Float(value) / param.step).rounded() * param.step : Float(value)
                                    effects[i].params[j].value = v
                                    store.run { try $0.setEffectParam(plugin: effect.plugin, effect: effect.id, param: param.id, value: v) }
                                }), in: Double(param.min)...Double(max(param.max, param.min + 0.001)))
                                Text(param.shown).font(.caption.monospacedDigit()).foregroundStyle(.secondary)
                            }
                        }
                    }
                }
            }
        }
    }
}

extension Effect: Identifiable {}
extension EffectParam: Identifiable {}

// MARK: - Appearance (the page is in AppearanceCards.swift)

// MARK: - Scrobbling

struct ScrobblingView: View {
    @Environment(Store.self) private var store
    @Environment(\.openURL) private var openURL
    @State private var accounts: Accounts?
    @State private var settings: PlaybackSettings?
    @State private var waitingForBrowser = false
    @State private var apiKey = ""
    @State private var secret = ""
    @State private var token = ""
    /// Sign-in forms, shown only after Connect, until Cancel or a sign-in.
    @State private var connecting: Set<String> = []

    var body: some View {
        Form {
            if let a = accounts {
                Section {
                    // Needle's own key built in: nothing to fill in, so Connect opens Last.fm's
                    // sign-in page at once.
                    service("Last.fm", user: a.lastfmUser, icon: "music.note", tint: .red, id: "lastfm", busy: waitingForBrowser,
                            direct: a.lastfmKeyBuiltIn ? { beginLastfm(key: "", secret: "") } : nil)
                    if a.lastfmUser != nil {
                        Toggle("Send listens", isOn: Binding(get: { a.lastfmOn }, set: { on in
                            accounts?.lastfmOn = on; Task { _ = await store.ask { try $0.setScrobbling(lastfm: on, listenbrainz: nil) }; await reload() }
                        }))
                        Button("Sign out", role: .destructive) { Task { _ = await store.ask { try $0.signOut(service: "lastfm") }; await reload() } }
                    } else if waitingForBrowser {
                        Text("Allow Needle on the Last.fm page, then come back.")
                        Button("I allowed Needle") {
                            Task {
                                if await store.ask({ try $0.lastfmComplete() }) != nil { waitingForBrowser = false; connecting.remove("lastfm") }
                                await reload()
                            }
                        }
                        Button("Cancel", role: .cancel) { waitingForBrowser = false; connecting.remove("lastfm") }
                    } else if connecting.contains("lastfm") {
                        if !a.lastfmKeyBuiltIn {
                            TextField("API key", text: $apiKey).textInputAutocapitalization(.never).autocorrectionDisabled()
                            SecureField("Shared secret", text: $secret)
                        }
                        Button("Sign in to Last.fm") { beginLastfm(key: apiKey, secret: secret) }
                        .disabled(!a.lastfmKeyBuiltIn && (apiKey.isEmpty || secret.isEmpty))
                        if !a.lastfmKeyBuiltIn {
                            Text("Make a free key at last.fm/api/account/create.").font(.caption).foregroundStyle(.secondary)
                        }
                    }
                }
                Section {
                    service("ListenBrainz", user: a.listenbrainzUser, icon: "brain.head.profile", tint: .orange, id: "listenbrainz", busy: false)
                    if a.listenbrainzUser != nil {
                        Toggle("Send listens", isOn: Binding(get: { a.listenbrainzOn }, set: { on in
                            accounts?.listenbrainzOn = on; Task { _ = await store.ask { try $0.setScrobbling(lastfm: nil, listenbrainz: on) }; await reload() }
                        }))
                        Button("Sign out", role: .destructive) { Task { _ = await store.ask { try $0.signOut(service: "listenbrainz") }; await reload() } }
                    } else if connecting.contains("listenbrainz") {
                        SecureField("User token, from listenbrainz.org/settings", text: $token)
                        Button("Sign in") {
                            let t = token
                            Task {
                                if await store.ask({ try $0.listenbrainzSignIn(token: t) }) != nil { connecting.remove("listenbrainz") }
                                await reload()
                            }
                        }
                        .disabled(token.isEmpty)
                    }
                }
                Section {
                    if a.waiting > 0 { LabeledContent("Waiting to be sent", value: a.waiting.formatted()) }
                    if let problem = a.problem { Text(problem).foregroundStyle(.red) }
                    if let settings {
                        let save = saved($settings, store) { try $0.setPlaybackSettings(value: $1) }
                        Toggle(isOn: Binding(get: { settings.scrobbleCorrections }, set: { var s = settings; s.scrobbleCorrections = $0; save(s) })) {
                            SettingLabel(title: "Fix artist names", detail: "Apple Music's spelling, such as KiiiKiii.", icon: "pencil", tint: .gray)
                        }
                    }
                } footer: {
                    Text("Your passwords and keys stay in the iPhone Keychain.")
                }
            }
        }
        .navigationTitle("Scrobbling")
        .themed()
        .task {
            #if DEBUG
            // For testing from a shell: -open-connect lastfm|listenbrainz opens that form.
            if let id = UserDefaults.standard.string(forKey: "open-connect") { connecting.insert(id) }
            #endif
            await reload()
            settings = await store.load { try $0.playbackSettings() }
        }
    }

    private func reload() async {
        accounts = await store.load { $0.accounts() }
    }

    /// Opens Last.fm's page to allow Needle, then waits for the listener to come back.
    private func beginLastfm(key: String, secret: String) {
        Task {
            if let page = await store.ask({ try $0.lastfmBegin(apiKey: key, secret: secret) }), let url = URL(string: page) {
                waitingForBrowser = true
                openURL(url)
            }
        }
    }

    /// A service's row: its name and whether it is connected, with Connect (or Cancel) while not.
    private func service(_ name: String, user: String?, icon: String, tint: Color, id: String, busy: Bool, direct: (() -> Void)? = nil) -> some View {
        HStack {
            SettingLabel(title: name, detail: user.map { $0.isEmpty ? "Signed in" : "Signed in as \($0)" } ?? "Not connected", icon: icon, tint: tint)
            Spacer()
            if user == nil && !busy {
                if connecting.contains(id) {
                    Button("Cancel") { connecting.remove(id) }.buttonStyle(.borderless)
                } else {
                    Button("Connect") { if let direct { direct() } else { connecting.insert(id) } }
                        .buttonStyle(.borderedProminent).buttonBorderShape(.capsule).controlSize(.small)
                }
            }
        }
    }
}

// MARK: - Discord

/// Discord's "Listening to" status, as on Android, once the Discord account is linked.
struct DiscordView: View {
    @Environment(Store.self) private var store
    @State private var settings: DiscordSettings?
    @State private var account: DiscordAccount?

    var body: some View {
        Form {
            if let account {
                Section {
                    if account.linked {
                        LabeledContent("Discord account", value: "Linked")
                        Button("Unlink", role: .destructive) {
                            store.core.discordUnlink()
                            Task { await reload() }
                        }
                    } else if account.linking {
                        HStack(spacing: 10) {
                            ProgressView()
                            Text("Allow Needle on Discord's page.")
                        }
                        Button("Cancel", role: .cancel) {
                            store.core.discordCancelLink()
                        }
                    } else {
                        Button("Link Your Discord Account") {
                            store.core.discordLink()
                            Task { await follow() }
                        }
                    }
                    if let problem = account.problem { Text(problem).foregroundStyle(.red) }
                } footer: {
                    Text("Discord asks you to allow Needle once.")
                }
            }
            if let s = settings {
                let save = saved($settings, store) { try $0.setDiscordSettings(value: $1) }
                Section {
                    Toggle(isOn: Binding(get: { s.on }, set: { var n = s; n.on = $0; save(n) })) {
                        SettingLabel(title: "Show what you're playing", detail: "On your Discord profile.", icon: "bubble.left.and.text.bubble.right.fill", tint: .indigo)
                    }
                }
                if s.on {
                    Section("The card") {
                        choice("Listening to …", [("song", "Song"), ("artist", "Artist"), ("needle", "Needle")], s.title) { var n = s; n.title = $0; save(n) }
                        choice("First line", [("artist", "Artist"), ("song", "Song"), ("album", "Album")], s.top) { var n = s; n.top = $0; save(n) }
                        choice("Second line", [("song", "Song"), ("artist", "Artist"), ("album", "Album"), ("none", "None")], s.middle) { var n = s; n.middle = $0; save(n) }
                        choice("Third line", [("album", "Album"), ("song", "Song"), ("artist", "Artist"), ("none", "None")], s.bottom) { var n = s; n.bottom = $0; save(n) }
                    }
                    Section {
                        Toggle(isOn: Binding(get: { s.covers }, set: { var n = s; n.covers = $0; save(n) })) {
                            SettingLabel(title: "Look up covers", detail: "From Apple's iTunes catalog, by name.", icon: "photo.fill", tint: .orange)
                        }
                        Toggle(isOn: Binding(get: { s.logo }, set: { var n = s; n.logo = $0; save(n) })) {
                            SettingLabel(title: "Show the Needle logo", detail: "A small badge on the cover.", icon: "seal.fill", tint: .pink)
                        }
                    } header: {
                        Text("Picture")
                    }
                    Section {
                        Toggle(isOn: Binding(get: { s.paused }, set: { var n = s; n.paused = $0; save(n) })) {
                            SettingLabel(title: "Show while paused", detail: "The song stays, marked Paused.", icon: "pause.fill", tint: .gray)
                        }
                        Picker("Clear after", selection: Binding(get: { s.idleMinutes }, set: { var n = s; n.idleMinutes = $0; save(n) })) {
                            Text("5 min").tag(UInt32(5))
                            Text("10 min").tag(UInt32(10))
                            Text("30 min").tag(UInt32(30))
                            Text("1 h").tag(UInt32(60))
                            Text("Never").tag(UInt32(0))
                        }
                    } header: {
                        Text("When nothing plays")
                    }
                }
            }
        }
        .navigationTitle("Discord")
        .themed()
        .task {
            settings = await store.load { try $0.discordSettings() }
            await follow()
        }
    }

    private func choice(_ title: String, _ options: [(String, String)], _ current: String, _ change: @escaping (String) -> Void) -> some View {
        Picker(title, selection: Binding(get: { current }, set: change)) {
            ForEach(options, id: \.0) { Text($0.1).tag($0.0) }
        }
    }

    private func reload() async {
        account = await store.load { $0.discordAccount() }
    }

    /// Reads the link again while Discord's sign-in page is open.
    private func follow() async {
        await reload()
        while account?.linking == true, !Task.isCancelled {
            try? await Task.sleep(for: .milliseconds(500))
            await reload()
        }
    }
}

// MARK: - Plugins

struct PluginsView: View {
    @Environment(Store.self) private var store
    @State private var plugins: [PluginItem]?

    var body: some View {
        List {
            Section {
                Button("Add Needle's plugins", systemImage: "plus") {
                    Task {
                        let added = await store.ask { try $0.installBundledPlugins() } ?? 0
                        store.message = added > 0 ? "Added \(count(Int(added), "plugin"))" : "Needle's plugins are all here"
                        await reload()
                    }
                }
                Button("Load plugins again", systemImage: "arrow.clockwise") {
                    store.core.reloadPlugins()
                    Task { await reload() }
                }
            } footer: {
                Text("Plugins add lyrics from NetEase, sound effects, and commands. They run inside Needle, with only the permissions they list. Music servers are built into Needle: see Sources.")
            }
            Section {
                ForEach(plugins ?? [], id: \.id) { plugin in
                    NavigationLink(value: Route.plugin(plugin.id)) {
                        VStack(alignment: .leading, spacing: 2) {
                            HStack(spacing: 6) {
                                Text(plugin.name)
                                if plugin.official { Image(systemName: "checkmark.seal.fill").foregroundStyle(.tint).font(.caption) }
                            }
                            Text(plugin.enabled ? plugin.description : "Off").font(.caption).foregroundStyle(.secondary).lineLimit(2)
                            if let error = plugin.error { Text(error).font(.caption).foregroundStyle(.red) }
                        }
                    }
                }
            }
        }
        .navigationTitle("Plugins")
        .themed()
        .task(id: store.libraryVersion) { await reload() }
    }

    private func reload() async {
        // Parts built into Needle (its music server) are not plugins here.
        plugins = await store.load { $0.plugins().filter { !$0.builtIn } }
    }
}

struct PluginView: View {
    @Environment(Store.self) private var store
    let id: String
    @State private var plugin: PluginItem?

    var body: some View {
        Form {
            if let plugin {
                Section {
                    Toggle("On", isOn: Binding(get: { plugin.enabled }, set: { on in
                        store.core.setPluginEnabled(id: id, on: on)
                        Task { await reload() }
                    }))
                    if !plugin.description.isEmpty { Text(plugin.description).foregroundStyle(.secondary) }
                    if let error = plugin.error { Text(error).foregroundStyle(.red) }
                } footer: {
                    Text("\(plugin.version)\(plugin.author.isEmpty ? "" : " · " + plugin.author)\(plugin.official ? " · Comes with Needle" : "")\(plugin.verified ? " · Checked" : "")")
                }
                if plugin.source != nil {
                    Section {
                        NavigationLink("Music server") { ServerView(plugin: id) }
                    }
                }
                let commands = plugin.commands.filter { !$0.forSongs }
                if !commands.isEmpty {
                    Section("Commands") {
                        ForEach(commands, id: \.id) { command in
                            Button(command.title) { store.core.runPluginCommand(plugin: id, command: command.id, songs: []) }
                        }
                    }
                }
                if !plugin.effects.isEmpty {
                    Section {
                        NavigationLink("Its sound effects are in Sound", value: Route.settingsSound)
                    }
                }
                if plugin.findsLyrics {
                    Section { Text("Finds lyrics for songs that have none.").foregroundStyle(.secondary) }
                }
                if !plugin.permissions.isEmpty {
                    Section("It may") {
                        ForEach(plugin.permissions, id: \.self) { Text("• " + $0) }
                    }
                }
            }
        }
        .navigationTitle(plugin?.name ?? "Plugin")
        .themed()
        .task { await reload() }
    }

    private func reload() async {
        let id = id
        plugin = await store.load { $0.plugins().first { $0.id == id } } ?? nil
    }
}

// MARK: - Sync

/// A sync file: history, ratings, and playlists in one file locked with a passphrase, to move
/// between Needle on the phone and on a computer. No music and no passwords go in it.
struct SyncView: View {
    @Environment(Store.self) private var store
    @State private var passphrase = ""
    @State private var exported: URL?
    @State private var moving = false
    @State private var choosing = false
    @State private var chosen: String?
    @State private var asking = false
    @State private var report: SyncReport?

    var body: some View {
        Form {
            Section {
                SecureField("Passphrase", text: $passphrase)
                Button("Make a sync file") {
                    let (path, words) = (FileManager.default.temporaryDirectory.appendingPathComponent("Needle.needlesync").path, passphrase)
                    Task {
                        if await store.ask({ try $0.exportSync(path: path, passphrase: words) }) != nil {
                            exported = URL(fileURLWithPath: path)
                            moving = true
                        }
                    }
                }
                .disabled(passphrase.count < 4)
            } header: {
                Text("From this iPhone")
            } footer: {
                Text("History, ratings, and playlists, locked with the passphrase. No music and no passwords go in it.")
            }
            Section("Into this iPhone") {
                Button("Open a sync file…") { choosing = true }
            }
            if let report {
                Section("Brought in") {
                    LabeledContent("Songs matched", value: report.songsMatched.formatted())
                    LabeledContent("Listens added", value: report.listensAdded.formatted())
                    LabeledContent("Playlists added", value: report.playlistsAdded.formatted())
                    if report.songsNotFound > 0 { LabeledContent("Songs not on this iPhone", value: report.songsNotFound.formatted()) }
                }
            }
        }
        .navigationTitle("Sync")
        .themed()
        .fileMover(isPresented: $moving, file: exported) { _ in }
        .fileImporter(isPresented: $choosing, allowedContentTypes: [.data]) { result in
            guard case .success(let url) = result else { return }
            chosen = store.copyIn(url)
            passphrase = ""
            asking = chosen != nil
        }
        .alert("Passphrase", isPresented: $asking) {
            SecureField("Passphrase", text: $passphrase)
            Button("Open") {
                guard let path = chosen else { return }
                let words = passphrase
                Task {
                    report = await store.ask { try $0.importSync(path: path, passphrase: words) }
                    if report != nil { store.libraryChanged() }
                }
            }
            Button("Cancel", role: .cancel) {}
        } message: {
            Text("The one the sync file was made with.")
        }
    }
}

// MARK: - What's new and About

/// The iPhone app's release notes (whats-new.md, in the app).
struct WhatsNewView: View {
    private struct Change: Identifiable {
        let id: String
        let title: String
        let text: String
        /// The note's picture tag ([player], [lyrics], …), "!" for a release's main change, or "".
        let tag: String
    }

    private struct Release: Identifiable {
        let version: String
        var tagline: String?
        var changes: [Change] = []
        var id: String { version }
    }

    /// Notes opened to their full text (a tap on a change).
    @State private var opened: Set<String> = []
    /// Older releases opened from their folded row.
    @State private var unfolded: Set<String> = []

    private var releases: [Release] {
        guard let url = Bundle.main.url(forResource: "whats-new", withExtension: "md"),
              let text = try? String(contentsOf: url, encoding: .utf8) else { return [] }
        var all: [Release] = []
        for line in text.components(separatedBy: .newlines).map({ $0.trimmingCharacters(in: .whitespaces) }) where !line.isEmpty {
            if line.hasPrefix("# ") {
                all.append(Release(version: String(line.dropFirst(2))))
            } else if !all.isEmpty {
                var body = line
                for prefix in ["! ", "- "] where body.hasPrefix(prefix) { body = String(body.dropFirst(prefix.count)) }
                var tag = line.hasPrefix("! ") ? "!" : ""
                if body.hasPrefix("["), let end = body.firstIndex(of: "]") {
                    tag = String(body[body.index(after: body.startIndex)..<end])
                    body = String(body[body.index(after: end)...]).trimmingCharacters(in: .whitespaces)
                }
                if line.hasPrefix("- ") || line.hasPrefix("! ") {
                    let id = "\(all[all.count - 1].version)-\(all[all.count - 1].changes.count)"
                    if body.hasPrefix("**"), let close = body.range(of: "**", range: body.index(body.startIndex, offsetBy: 2)..<body.endIndex) {
                        let title = String(body[body.index(body.startIndex, offsetBy: 2)..<close.lowerBound]).trimmingCharacters(in: CharacterSet(charactersIn: "."))
                        all[all.count - 1].changes.append(Change(id: id, title: title, text: String(body[close.upperBound...]).trimmingCharacters(in: .whitespaces), tag: tag))
                    } else {
                        all[all.count - 1].changes.append(Change(id: id, title: "", text: body, tag: tag))
                    }
                } else if all[all.count - 1].tagline == nil {
                    all[all.count - 1].tagline = body
                }
            }
        }
        return all
    }

    var body: some View {
        let releases = releases
        List {
            ForEach(Array(releases.enumerated()), id: \.element.id) { index, release in
                if index == 0 || unfolded.contains(release.version) {
                    Section {
                        if let tagline = release.tagline { Text(tagline).font(.subheadline) }
                        ForEach(release.changes) { change in row(change) }
                        if index > 0 {
                            Button("Fold \(release.version)") { withAnimation { _ = unfolded.remove(release.version) } }
                                .font(.subheadline)
                        }
                    } header: {
                        Text(release.version)
                    }
                } else {
                    Section {
                        Button {
                            withAnimation { _ = unfolded.insert(release.version) }
                        } label: {
                            HStack(spacing: 12) {
                                VStack(alignment: .leading, spacing: 2) {
                                    Text("\(release.version) · \(count(release.changes.count, "change"))").font(.body.weight(.semibold))
                                    if let tagline = release.tagline {
                                        Text(tagline).font(.subheadline).foregroundStyle(.secondary).lineLimit(1)
                                    }
                                }
                                Spacer()
                                Image(systemName: "chevron.down").font(.footnote.weight(.bold)).foregroundStyle(.tertiary)
                            }
                        }
                        .foregroundStyle(.primary)
                        .accessibilityHint("Shows the changes in \(release.version)")
                    }
                }
            }
        }
        .navigationTitle("What's new")
        .themed()
    }

    /// A change: its icon, its title in bold, and one short line; a tap shows all of it.
    private func row(_ change: Change) -> some View {
        let open = opened.contains(change.id)
        let (symbol, tint) = Self.icon(change)
        return Button {
            withAnimation(.snappy(duration: 0.2)) { if open { opened.remove(change.id) } else { opened.insert(change.id) } }
        } label: {
            HStack(alignment: .top, spacing: 12) {
                Image(systemName: symbol).font(.system(size: 13, weight: .semibold)).foregroundStyle(.white)
                    .frame(width: 28, height: 28)
                    .background(tint.gradient, in: .rect(cornerRadius: 7))
                    .accessibilityHidden(true)
                VStack(alignment: .leading, spacing: 2) {
                    if change.title.isEmpty {
                        // A small change: its words in full weight, two lines until opened.
                        Text(change.text).font(.subheadline).lineLimit(open ? nil : 2)
                    } else {
                        Text(change.title).font(.subheadline.weight(.semibold))
                        Text(change.text).font(.subheadline).foregroundStyle(.secondary).lineLimit(open ? nil : 1)
                    }
                    if !open && Self.long(change) {
                        Text("More").font(.caption.weight(.semibold)).foregroundStyle(.tint)
                    }
                }
                Spacer(minLength: 0)
            }
            .padding(.vertical, 2)
            .contentShape(.rect)
        }
        .foregroundStyle(.primary)
        .accessibilityElement(children: .combine)
    }

    /// Whether a change has more to show than its folded lines.
    private static func long(_ change: Change) -> Bool {
        change.title.isEmpty ? change.text.count > 90 : change.text.count > 45
    }

    /// The icon for a change, from its tag; a star for a release's main change, a wrench for a fix.
    private static func icon(_ change: Change) -> (String, Color) {
        switch change.tag {
        case "!": ("star.fill", .pink)
        case "player": ("play.fill", .blue)
        case "lyrics": ("quote.bubble.fill", .orange)
        case "discord": ("bubble.left.and.text.bubble.right.fill", .indigo)
        case "widget": ("square.grid.2x2.fill", .teal)
        case "computer": ("laptopcomputer", .indigo)
        case "sound": ("slider.vertical.3", .purple)
        case "playlist": ("music.note.list", .green)
        case "albums": ("square.stack.fill", .orange)
        case "palette", "theme": ("paintbrush.fill", .pink)
        case "server": ("server.rack", .cyan)
        case "radio": ("dot.radiowaves.left.and.right", .red)
        case "signal": ("antenna.radiowaves.left.and.right", .green)
        case "account": ("person.crop.circle.fill", .blue)
        case "": change.title.isEmpty ? ("wrench.and.screwdriver.fill", .gray) : ("sparkles", .yellow)
        default: ("sparkles", .yellow)
        }
    }
}

struct AboutView: View {
    @Environment(Store.self) private var store
    @Environment(\.openURL) private var openURL
    @State private var crashReports = false
    @State private var settings: PlaybackSettings?

    var body: some View {
        Form {
            Section {
                LabeledContent("Version", value: store.core.version())
                Button("Check for a newer Needle") {
                    Task {
                        guard let found = await store.ask({ try $0.checkUpdate() }) else { return }
                        if let found, let url = URL(string: found.page) {
                            store.message = "Needle \(found.version) is out"
                            openURL(url)
                        } else {
                            store.message = "This is the newest Needle"
                        }
                    }
                }
            }
            Section {
                Toggle(isOn: Binding(get: { crashReports }, set: { on in
                    crashReports = on
                    store.run { try $0.setCrashReports(on: on) }
                })) {
                    SettingLabel(title: "Send crash reports", detail: "What went wrong. No music, names, or files.", icon: "exclamationmark.triangle.fill", tint: .orange)
                }
            }
            Section("Privacy") {
                Text("Needle has no account and no tracking. Your library stays on this iPhone. It talks to the internet only for what you turn on: online lookups, music servers, scrobbling, plugins, and update checks.")
                    .font(.subheadline).foregroundStyle(.secondary)
            }
            Section {
                Link("needle.nnx.fyi", destination: URL(string: "https://needle.nnx.fyi")!)
            }
        }
        .navigationTitle("About")
        .themed()
        .task {
            crashReports = await store.load { $0.crashReports() } ?? false
            settings = await store.load { try $0.playbackSettings() }
        }
    }
}
